//! What a call did to the value it was passed.
//!
//! Three claims, all read off the callee's declaration rather than off the value: a by-reference
//! parameter may have written a type of its own into the caller's variable, an `@assert` tag may
//! say what the value is once the call has returned, and a `@phpstan-self-out` says what the value
//! a method was called *on* holds afterwards. They share this file because they share the
//! resolution that finds the declaration -- doing it twice for one call statement is what made it
//! worth doing once.

use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, Name, TextSize};
use phrost_sema::MemberKind;

use crate::{resolve::Held, symbols::Declared, view::IndexView};

impl IndexView<'_> {
    /// The type a by-reference parameter may have written into the caller's variable.
    ///
    /// Three claims, ranked, and the ranking is the whole of the rule. A `@param-out` tag is
    /// somebody stating what comes back out and **replaces** what the caller knew. A `mixed`
    /// parameter is the declaration saying nothing about what it accepts, so nothing bounds what was
    /// written, and it **voids** what the caller knew. Anything else is a bound on what the callee
    /// *accepts*, which is not a claim about what it left -- so the caller's own type **stands**
    /// unless the parameter can be shown to contradict it.
    ///
    /// # Why a signature is not a claim about what came back
    ///
    /// PHP's parameters are contravariant: `sort(array &$array)` says it takes an array and nothing
    /// more. A caller holding an `int[]` satisfies that bound, and reading the declaration as "an
    /// array is what is there now" throws the element type away -- 36 sites on one real checkout
    /// (`sort` 11, `usort` 11, `array_splice` 6, `uasort` 4, `rsort` 2, `array_walk` 1,
    /// `array_push` 1), every one of them a place where precision existed and stopped existing. Not
    /// a false positive, since wider than the truth reports nothing; it is the same loss of an
    /// element type that a `T[]` leaking into a render was, arriving through inference instead.
    ///
    /// So the parameter is asked only whether it *contradicts* what the caller knew, and `Rejected`
    /// rather than `Accepted` is the test. `sort` writes `@param array<T>` beside its `array` hint
    /// and a bare container is a hint that says nothing, so `effective_ty` is `array<T>` with `T`
    /// bound by nothing -- an `int[]` against that is [`Fit::Unknown`], and demanding `Accepted`
    /// would have kept the bug on exactly the function that exposed it. `$rows = "text"` before
    /// `fills(array &$out)` is the other direction and is `Fit::Rejected`, so there the declaration
    /// is still the answer.
    ///
    /// What this gives up is a callee that replaces the value with a different type its own
    /// declaration still admits. That is what `@param-out` is for, and a callee writing neither the
    /// tag nor a type has said nothing for a reader to be right about.
    ///
    /// # Why `mixed` is the opposite and a union is neither
    ///
    /// `mixed &$level` says **anything of any type may come back out**, which is not a bound at all
    /// -- so keeping the caller's type there leaves a claim the call has already broken, and it
    /// breaks in the direction that *reports*. On one real project `assertSeverityLevel(mixed
    /// &$level)` narrows its argument internally, the caller's `Level|int` was stale by the time it
    /// reached a `Level` property, and the assignment was reported. PHPStan says nothing there.
    /// Widening to `mixed` can only ever remove a finding, because every check treats it as no
    /// claim.
    ///
    /// A union is refused outright rather than ranked, because it is the shape where believing the
    /// declaration is most expensive and least informative. `array_walk(object|array &$array)` does
    /// not turn an array into an object, and neither do `reset`, `end`, `next`, `prev` or `current`.
    /// Measured over the bundled stubs before choosing: 108 by-reference parameters name one type
    /// and 11 name a union -- of which eight are that container shape, one is a typo in `exec`
    /// (`int|int`), and three are docblocks on a Windows-only extension. Not one informative union.
    ///
    /// `prior` is what the caller already held, and `None` is honest rather than missing: the
    /// [`PathRoot::ByReference`](phrost_body::PathRoot) root exists precisely for a name nothing
    /// else bound, so there is no earlier claim to keep and resolving one would be work with no
    /// answer at the end of it.
    pub(crate) fn written_through_reference(
        &self,
        offset: TextSize,
        hir: &FileHir,
        declared: &CalledDeclaration,
        call: &phrost_body::PassedTo,
        prior: Option<&Held>,
    ) -> Option<Held> {
        let param = declared.params.get(declared.parameter(call)?)?;
        if !param.by_reference {
            return None;
        }
        // `@param-out` first, and it wins outright rather than refining: it is the one claim written
        // about this exact moment. `preg_match`'s third parameter takes `array` and leaves
        // `array<int, string>`; `str_replace`'s `$count` takes nothing at all and leaves an `int`.
        // The bundled stubs write the tag 145 times, and until it was read here the 129 by-reference
        // parameters with no declared type of their own answered nothing -- which is the whole of why
        // `preg_match($p, $s, $m); $m[1]` was unresolvable.
        //
        // A union is kept here where it is refused below, and the difference is who wrote it: a
        // union in the *signature* is an upper bound on what the callee accepts, while a union in
        // `@param-out` is somebody stating what comes back. `?array` is the commonest shape of it in
        // the stubs and refusing it would leave exactly the tags this exists to read.
        if let Some(out) = param.out_ty {
            return Some(Held::Stored {
                ty: out,
                scope: None,
            });
        }
        let ty = param.effective_ty(self.skeleton.types())?;
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Union(_) => return None,
            // Written where the callee is declared, so a relative type in it means that class --
            // and `mixed` is neither relative nor a bound, so it needs no scope and admits no
            // earlier claim.
            phrost_ty::TypeRef::Builtin(BuiltinTy::Mixed) => {
                return Some(Held::Stored { ty, scope: None });
            }
            _ => {}
        }
        // A `mixed` on the caller's side is the same absence read from the other end: it is not a
        // claim to keep, so the declaration is still the more specific of the two. A sentinel is
        // the same absence again, written by the caller rather than by either declaration: `$out =
        // null;` above `fills(?string &$out)` is the variable being *declared*, and the call is
        // what puts a value in it. Keeping the null there says the call left one, which is a
        // stronger claim than anything either side wrote -- and it is a claim that *reports*, as an
        // `error` rather than a warning, on the ordinary shape of a populator.
        //
        // What the declaration says instead is the honest weaker answer: `?string` keeps the null
        // the caller may still be holding, so a use of it downstream is a warning about a branch
        // rather than an error about a certainty.
        //
        // Through `kind_of_held` because a builtin arrives in three spellings and only one of them
        // is `Held::Builtin`.
        if let Some(prior) = prior
            && !matches!(
                self.kind_of_held(hir, prior),
                Some(BuiltinTy::Mixed | BuiltinTy::Null | BuiltinTy::False)
            )
            && !matches!(
                self.fit_held(offset, hir, prior, ty),
                phrost_ty::compat::Fit::Rejected
            )
        {
            return Some(prior.clone());
        }
        // Written where the callee is declared, so a relative type in it means that class.
        Some(Held::Stored { ty, scope: None })
    }

    /// What the callee declared its callback parameter at `parameter` receives.
    ///
    /// Read off [`ParamRow::callback_params_ty`](phrost_sema::ParamRow::callback_params_ty), whose
    /// arguments are the callback's parameters in written order -- so this is an index into them
    /// and nothing more. `None` where the callee documented no signature, where the position is
    /// past the ones it named, or where it named that position and said nothing about it.
    ///
    /// `mixed` is that last case, and it is refused rather than returned: the placeholder exists so
    /// the list stays dense enough to index, and handing a `mixed` back would replace whatever the
    /// container reading beside this one could have found with a claim that says nothing. The two
    /// are ranked, and a source that answers `mixed` has not answered.
    pub(crate) fn callback_parameter(
        &self,
        declared: &CalledDeclaration,
        call: &phrost_body::PassedTo,
        parameter: usize,
    ) -> Option<Held> {
        let param = declared.params.get(declared.parameter(call)?)?;
        let Some(declared_params) = param.callback_params_ty else {
            // Nothing written about this callback, which is the case a framework contract exists
            // for -- see `Self::contracted_callback_parameter` for why it is ranked second.
            return self.contracted_callback_parameter(declared, call, parameter);
        };
        let phrost_ty::TypeRef::Generic { args, .. } = self.skeleton.types().get(declared_params)
        else {
            return None;
        };
        let found = *self.skeleton.types().operands(*args).get(parameter)?;
        if matches!(
            self.skeleton.types().get(found),
            phrost_ty::TypeRef::Builtin(BuiltinTy::Mixed)
        ) {
            return None;
        }
        let scope = declared
            .class
            .clone()
            .map(|class| crate::resolve::RelativeScope {
                declaring: class.clone(),
                called: class,
            });
        // The receiver's arguments put in, through every `@extends` and `@use` between the class the
        // call was made on and the one that wrote the signature -- the ladder a return goes through,
        // so the two halves of one declaration agree. Eloquent's builder is the shape:
        // `chunkById(callable(Collection<int, TValue>, int))` is written on the `BuildsQueries` trait,
        // which the builder uses as `BuildsQueries<TModel>`, so on a `Builder<Book>` the callback
        // is handed a `Collection<int, Book>` -- and read without the ladder it was handed a
        // collection of a class called `TValue`.
        //
        // A template nothing supplied stays as written, for the guards below and the container
        // reading beside this one to answer, as they did before.
        if let (Some(class), Some(owner), Some(scope)) = (&declared.class, &declared.owner, &scope)
        {
            match self.declared_through(
                class.as_str(),
                owner.as_str(),
                found,
                &declared.arguments,
                scope,
            ) {
                Declared::Held(held) => return Some(held),
                Declared::Type { ty, nullable } if ty != found => {
                    let held = Held::Stored {
                        ty,
                        scope: Some(scope.clone()),
                    };
                    return Some(match nullable {
                        true => Self::nullable(held),
                        false => held,
                    });
                }
                Declared::Type { .. } | Declared::Unbound => {}
            }
        }
        // A name the project does not declare says nothing, and a `@template` parameter is exactly
        // that: `Collection::map`'s `@param callable(TValue, TKey): T` names types bound by the
        // receiver, and nothing here binds them -- so the name is a class only once a substitution
        // has happened. Handing it back is wrong in *both* directions, which is what makes it worth
        // a check rather than a note: `compat` rejects every scalar against it for not being a
        // class, and accepts every class because nothing relates to an unresolvable name. Measured
        // without it, on Laravel: 24 `incompatible-argument` false positives reading "and this is
        // TValue", and -- invisible in that count -- the container reading beside this one silently
        // taken out of every `array_map`, because an answer of `TValue` is still an answer and won
        // the ranking.
        //
        // The same guard `Self::asserted_held` below applies to an `@assert` tag, for the same
        // reason and against the same shape. This is the complete one: lowering refuses a template
        // its *own* docblock declares and cannot see one the enclosing class declared, so a rule
        // that lived only there would catch a method's templates and none of a class's.
        if let Some(named) = self.class_of_stored(found, None) {
            self.class_location(named.as_str())?;
        }
        // And a union holding one, which is how a method writes an accumulator: Laravel's
        // `reduce` is `callable(TReduceInitial|TReduceReturnType, TValue, TKey)`, where
        // `array_reduce` writes a bare `I`. The value may be the template, so the union says as
        // little as the name does.
        if self.a_union_names_nothing_declared(found) {
            return None;
        }
        // Written where the callee is declared, and a relative type in it is about the object the
        // call was made on: Eloquent's `where` is `@param (\Closure(static): mixed)`, and the
        // closure is handed the builder it was called on.
        Some(Held::Stored { ty: found, scope })
    }

    /// Whether the callee's own signature for this callback names, at `parameter`, a template no
    /// container parameter holds.
    ///
    /// `array_reduce` declares `@param callable(I, V): I $callback` beside `array<K, V> $array`:
    /// `V` is what the array holds, so the callback's second parameter is an element of it, and `I`
    /// is the accumulator, bound by `$initial`. When the signature cannot settle `I` the container
    /// reading was asked instead, and it hands the *first* parameter an element: Symfony's
    /// `array_reduce($names, static fn (array $methods, string $m) => array_merge($methods, ...),
    /// [])` read `$methods` as the string it reduces over. A template that is no container's
    /// element is a claim the container reading has no part in.
    pub(crate) fn callback_parameter_is_unheld_template(
        &self,
        declared: &CalledDeclaration,
        call: &phrost_body::PassedTo,
        parameter: usize,
    ) -> bool {
        let types = self.skeleton.types();
        let Some(param) = declared
            .parameter(call)
            .and_then(|position| declared.params.get(position))
        else {
            return false;
        };
        let Some(signature) = param.callback_params_ty else {
            return false;
        };
        let phrost_ty::TypeRef::Generic { args, .. } = types.get(signature) else {
            return false;
        };
        let Some(&found) = types.operands(*args).get(parameter) else {
            return false;
        };
        let phrost_ty::TypeRef::Named(template) = types.get(found) else {
            return false;
        };
        if self.class_location(template.as_str()).is_some() {
            return false;
        }
        let held_by_a_container = declared.params.iter().any(|other| {
            other
                .effective_ty(types)
                .and_then(|ty| self.element_of_stored(ty))
                .is_some_and(|element| {
                    matches!(types.get(element), phrost_ty::TypeRef::Named(named) if named == template)
                })
        });
        !held_by_a_container
    }

    /// The same, from what a framework says its own source hands the callback.
    ///
    /// Ranked below the declaration's own `@param callable(..)`, and reached only where there is
    /// none: a method that documents its callback is describing that exact version of itself, where
    /// this is a statement made from outside. `Container::singleton` documents
    /// `\Closure|string|null` and calls the closure with the container, so there is nothing to
    /// rank against -- which is the shape every entry has.
    ///
    /// # Why this cannot contradict a declaration
    ///
    /// Not by a check here, but by where the question is asked from: the root that reaches this
    /// exists only for a callback parameter whose own declaration is absent or a bare container --
    /// see `PathRoot::CallbackParameter`. So the same discipline
    /// [`Self::declares_only_an_object`](crate::view::IndexView) applies to a contributed return is
    /// already satisfied upstream, and a closure written `function (Blueprint $table)` never
    /// reaches this at all.
    ///
    /// The named class has to be one the project declares, which is what makes an entry naming
    /// `Illuminate\Foundation\Application` inert in a project holding `illuminate/container`
    /// without the foundation -- there the callback really does receive a bare container, and
    /// saying nothing is the right answer rather than a missed one.
    fn contracted_callback_parameter(
        &self,
        declared: &CalledDeclaration,
        call: &phrost_body::PassedTo,
        parameter: usize,
    ) -> Option<Held> {
        let method = match &call.callee {
            phrost_body::Callee::Member { name, .. } => name.as_str(),
            phrost_body::Callee::Constructed(_) => "__construct",
            // No contract of this kind names a free function; asking would be a scan for nothing.
            phrost_body::Callee::Function(_) => return None,
        };
        let rows = self.skeleton.returns().invoked_callbacks(method);
        if rows.is_empty() {
            return None;
        }
        let class = declared.class.as_ref()?;
        // Which *parameter* the callback filled, not which position it was written at: PHP lets a
        // caller name its arguments, and `singleton(concrete: fn ($app) => ..., abstract: 'x')`
        // writes the callback first while filling the second parameter. The same mapping the slot
        // path above goes through, so the two sources agree about which callback is being asked
        // about.
        let filled = declared.parameter(call)?;
        // The hierarchy last, for the reason `Self::chained_kind` gives: it is a climb, and the
        // name test has already settled almost every call.
        let row = rows.iter().find(|row| {
            row.argument == filled
                && (class.as_str().eq_ignore_ascii_case(&row.class)
                    || self.descends_from(
                        class.as_str(),
                        &row.class,
                        crate::resolve::HIERARCHY_DEPTH,
                    ))
        })?;
        let named = row.parameters.get(parameter)?.as_deref()?;
        let interned = self.skeleton.interner().lookup_name(named)?;
        self.class_location(interned.as_str())?;
        Some(Held::Class(interned))
    }

    /// The declaration a call statement reaches, resolved once.
    ///
    /// Once, because two questions are asked of it -- whether the parameter is by-reference and what
    /// an `@assert` tag claims -- and resolving a method's receiver is a path resolution. Asking
    /// twice doubled it for every call statement in the file.
    pub(crate) fn declaration_called(
        &self,
        offset: TextSize,
        hir: &FileHir,
        call: &phrost_body::PassedTo,
        budget: u8,
    ) -> Option<CalledDeclaration> {
        match &call.callee {
            // Remembered per call and budget. The receiver is resolved below full budget, which the
            // path cache does not keep, so every value passed to this call re-resolved it -- and a
            // function writing `$scripts->add(..., $ver)` two hundred times resolved `$scripts` at
            // each call once for every later read of `$ver`. The budget is part of the key, so an
            // answer is only ever reused for exactly the question that produced it.
            phrost_body::Callee::Member { receiver, name } => {
                // Where it starts is not enough: every link of `(new C)->map(...)->when(...)` starts
                // at the same offset, so `map` was handed `when`'s declaration -- and a callback
                // to `map` read `when`'s `callable($this)` as its own parameter. How many steps the
                // receiver has is what tells the links of one chain apart.
                let key = (call.at, receiver.steps.len(), budget);
                if let Some(known) = self.declarations_called.borrow().get(&key) {
                    return known.clone();
                }
                let found = (|| {
                    let held = self.resolve_path(call.at, hir, receiver, budget.checked_sub(1)?)?;
                    // The class declaring the method on this value. A reduction to one class saw
                    // nothing in an intersection, so `$this->chunk($n, function ($items) { ... })`
                    // inside a trait that is `@mixin Builder` read no signature for `$items` once
                    // `$this` was both -- while the call itself resolved, off `owner_of_member`.
                    let class = self.declaring_class(
                        call.at,
                        hir,
                        &held,
                        MemberKind::Method,
                        name.as_str(),
                    )?;
                    self.method_declared(
                        class.as_str(),
                        name.as_str(),
                        self.arguments_of_held(hir, &held),
                    )
                })();
                self.declarations_called
                    .borrow_mut()
                    .insert(key, found.clone());
                found
            }
            phrost_body::Callee::Function(reference) => {
                let fqn = self.called_function(offset, hir, *reference)?;
                Some(CalledDeclaration {
                    params: self.function_params(fqn.as_str())?,
                    contract: self.function_contract(fqn.as_str()).cloned(),
                    // A free function has no class, and no contract of this kind names one.
                    class: None,
                    owner: None,
                    arguments: Vec::new(),
                })
            }
            phrost_body::Callee::Constructed(target) => {
                let class = self.class_of_target(call.at, hir, *target)?;
                self.method_declared(class.as_str(), "__construct", Vec::new())
            }
        }
    }

    /// The method `class_fqn` answers `method` with, called on a value carrying `arguments`.
    ///
    /// A call a framework forwards to a fresh instance of another class is that class's method on
    /// that instance -- `User::chunk(...)` is `Builder<User>::chunk(...)`, see `ImpliedMixin` -- so
    /// what its callback is handed is the builder's to say, the way its return already is. One hop:
    /// a member the mixin forwards in turn keeps its own row.
    fn method_declared(
        &self,
        class_fqn: &str,
        method: &str,
        arguments: Vec<Held>,
    ) -> Option<CalledDeclaration> {
        let forwarded = self
            .members(class_fqn)
            .find(MemberKind::Method, method)?
            .forwarded;
        if forwarded
            && let Some((mixin, supplied)) =
                self.implied_forward(class_fqn, None, MemberKind::Method, method)
            && let Some(declared) = self.method_row_on(mixin.as_str(), method, supplied)
        {
            return Some(declared);
        }
        self.method_row_on(class_fqn, method, arguments)
    }

    fn method_row_on(
        &self,
        class_fqn: &str,
        method: &str,
        arguments: Vec<Held>,
    ) -> Option<CalledDeclaration> {
        let table = self.members(class_fqn);
        let member = table.find(MemberKind::Method, method)?;
        let class = self.skeleton.interner().lookup_name(class_fqn);
        // Whose `@template`s the signature names. A member still forwarded here is one nothing
        // above settled, and its row says nothing about whose they are.
        let owner = match member.forwarded {
            true => None,
            false => member
                .provenance
                .declarer()
                .cloned()
                .or_else(|| class.clone()),
        };
        Some(CalledDeclaration {
            params: member.row.params.clone(),
            contract: member.row.contract.as_deref().cloned(),
            class,
            owner,
            arguments,
        })
    }

    /// The function a written name resolves to, against the imports where it was written.
    fn called_function(
        &self,
        offset: TextSize,
        hir: &FileHir,
        reference: phrost_hir::NameRefId,
    ) -> Option<Name> {
        let written = hir.name_ref(reference);
        Some(
            self.resolve_name(
                offset,
                written.path.as_str(),
                written.anchor,
                phrost_hir::ImportKind::Function,
            )?
            .fqn,
        )
    }

    /// What the value a method was called on holds once the call returns, when the method's
    /// `@phpstan-self-out` says.
    ///
    /// A replacement rather than a refinement, and the same reading `@param-out` gets in
    /// [`Self::written_through_reference`]: it is a claim written about this exact moment, so it
    /// wins over whatever the value held. What it held still matters, because it is what the
    /// receiver was written with -- `transform()` keeps the keys of the `Collection<int, User>` it
    /// was called on -- and that is resolved where the call was made, as every argument is.
    pub(crate) fn left_in_receiver_by(
        &self,
        hir: &FileHir,
        declared: &CalledDeclaration,
        call: &phrost_body::PassedTo,
        budget: u8,
    ) -> Option<Held> {
        let phrost_body::PassedAs::Receiver(source) = call.passed_as else {
            return None;
        };
        let contract = declared.contract.as_ref()?;
        // A member nothing settled the owner of has no `self` to read the type against.
        let called = declared.class.clone()?;
        let declaring = declared.owner.clone()?;
        let literals = match hir.expr(source.expr()?) {
            phrost_hir::body::ExprData::MethodCall {
                literal_arguments, ..
            } => literal_arguments,
            _ => return None,
        };
        let at = crate::symbols::CallFacts {
            offset: call.at,
            hir,
            literals,
            call: source,
            budget,
            receiver: &declared.arguments,
            // The call is an expression here, so its own arguments are read.
            written: None,
        };
        self.left_in_receiver(contract, &declared.params, called, declaring, &at)
    }

    /// What a value is once an `@assert` tag has been applied to it.
    ///
    /// Three claims in one shape. `!null` takes the null off what the declaration said, which is
    /// what `assertNotNull` promises and the only form this used to read. `!T` drops that operand,
    /// exactly as an `instanceof` guard's else does. And a plain `T` replaces the value outright --
    /// the tag is a statement about what it is, made by whoever wrote the method.
    ///
    /// `None` when the claim is one this cannot express, which leaves the value as it was.
    pub(crate) fn asserted_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        check: &phrost_body::PassedTo,
        asserted: &crate::symbols::ArgumentAssertion,
    ) -> Option<Held> {
        // A tag that only says *where* the class is written: the call carries it as `Foo::class`.
        if let Some(position) = asserted.from_class_string {
            let target = (*check.class_arguments.get(usize::from(position))?)?;
            let named = self.class_of_target(check.at, hir, target)?;
            return match asserted.negated {
                false => Some(Held::Class(named)),
                true => Some(self.not_of_class(offset, hir, held, named.as_str())),
            };
        }

        let (ty, negated) = (asserted.ty, asserted.negated);

        // A *kind* rather than a class: `@phpstan-assert array $actual` is what PHPUnit writes on
        // `assertIsArray`, and `assertIsString`, `assertIsInt` and the rest are the same tag with
        // another word. Read the way `is_array($x)` is read: the alternatives of that kind survive
        // and the others -- null first among them -- do not, so a `?array` becomes the `array` the
        // declaration described rather than a bare one. Where nothing the value held is of the
        // kind, the tag is the whole answer, which is the rule a class assertion below follows too.
        // This used to fall through to the class lookup, find no class, and leave the value as it
        // was -- so `assertIsArray($group)` narrowed nothing and the next line reported a null.
        if let Some(kind) = self
            .asserted_kind(ty)
            .filter(|kind| is_a_kind_a_value_is_tested_for(*kind))
        {
            // A tag naming a kind names it, so the guard settles what the value is.
            let narrowed = self.of_kind(offset, hir, held, kind, !negated, true);
            if negated {
                return Some(narrowed);
            }
            // A tag *names* the type, so where the value has nothing of that kind the tag is the
            // whole answer -- including for a `mixed`, which `of_kind` deliberately leaves alone
            // because a guard's claim about a value nothing described reaches it through a root
            // that may have built it backwards. A bare kind is the same answer with less in it:
            // `@phpstan-assert array<string, int>` says more than `array`.
            //
            // Unless what the tag says it holds is a template: PHPUnit's
            // `assertContainsOnlyInstancesOf` is `@phpstan-assert iterable<T> $haystack`, and until
            // something substitutes the class its argument named, `T` is a class nobody declares --
            // every element read off the value was one. The kind still holds, so what the value
            // already had of it stands, and a value with nothing of it is the bare kind.
            let names_a_template = self.arguments_name_nothing_declared(ty);
            let written = match names_a_template {
                true => Held::Builtin(kind),
                false => Held::Stored { ty, scope: None },
            };
            let answered = match narrowed {
                Held::Builtin(answered) if answered == kind => written,
                narrowed if self.kind_of_held(hir, &narrowed) == Some(kind) => narrowed,
                narrowed
                    if names_a_template
                        && !matches!(
                            self.kind_of_held(hir, &narrowed),
                            None | Some(BuiltinTy::Mixed)
                        ) =>
                {
                    narrowed
                }
                _ => written,
            };
            // A tag naming a string that holds something -- `@phpstan-assert non-empty-string $s`
            // -- says more than the kind: the strings the value kept hold it too.
            return Some(
                match self.string_held(hir, &Held::Stored { ty, scope: None }) {
                    Some(holding) if holding != BuiltinTy::String => {
                        self.with_strings_holding(offset, hir, answered, holding)
                    }
                    _ => answered,
                },
            );
        }

        if !negated {
            // A tag naming something the project does not declare says nothing. `@phpstan-assert
            // =ExpectedType $actual` names a `@template` parameter, which is a class name only
            // where the argument that bound it has been substituted -- and until that exists,
            // replacing the value with it would take away the type the declaration did give.
            let named = self.class_of_stored(ty, None)?;
            self.class_location(named.as_str())?;
            // The type is written where the method is declared, so `self` inside it means that
            // class rather than the one the call was reached through.
            return Some(Held::Stored { ty, scope: None });
        }
        if matches!(
            self.skeleton.types().get(ty),
            phrost_ty::TypeRef::Builtin(BuiltinTy::Null)
        ) {
            return self.without_null(held);
        }
        let class = self.class_of_stored(ty, None)?;
        Some(self.not_of_class(offset, hir, held, class.as_str()))
    }

    /// What a value is once a guard's `@assert-if-true`/`@assert-if-false` has been checked
    /// against the polarity of the branch the guard is in.
    ///
    /// Beside what it already held, like every other guard: a callee this cannot resolve, a call
    /// with no tag on this parameter, or a tag written for the opposite branch all leave the value
    /// exactly as it was -- only a tag that matches the branch replaces or refines it. `when_true`
    /// here is the *branch's* polarity, carried in from `PathRoot::AssertedArgument`; the tag's own
    /// polarity comes from `asserts_about` and the two are compared only here, once, rather than
    /// conflated where the guard was read.
    pub(crate) fn asserted_argument_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        call: &phrost_body::PassedTo,
        when_true: bool,
        budget: u8,
    ) -> Held {
        let Some(declared) = self.declaration_called(offset, hir, call, budget) else {
            return held;
        };
        let Some(asserted) = declared.asserts_about(call) else {
            return held;
        };
        // A tag with neither suffix is unconditional and is read at the call site itself, by
        // `passed_to_calls` in `steps.rs` -- a suffixed tag applies to exactly one branch, and the
        // other says nothing, the same rule `PathRoot::Asserted` already follows for the receiver
        // form.
        if asserted.when_true != Some(when_true) {
            return held;
        }
        self.asserted_held(offset, hir, held.clone(), call, &asserted)
            .unwrap_or(held)
    }
}

/// The declaration a call statement reaches: its parameters, and what it promises beyond them.
#[derive(Clone, Debug)]
pub(crate) struct CalledDeclaration {
    params: Box<[phrost_sema::ParamRow]>,
    contract: Option<phrost_sema::Contract>,
    /// The class the method was found on, fully qualified. `None` for a free function.
    ///
    /// Resolved here rather than again at the reader, because getting it means resolving the
    /// receiver's path -- the work this type exists to do exactly once. See
    /// [`IndexView::declaration_called`].
    class: Option<Name>,
    /// Whose `@template`s the declaration names: the trait or the ancestor that wrote it, or the
    /// class itself. `None` for a free function, and for a member another class answers for.
    owner: Option<Name>,
    /// The type arguments the receiver carries -- the `Book` of a `Builder<Book>` -- which a
    /// template in a callback's signature is filled from.
    arguments: Vec<Held>,
}

impl CalledDeclaration {
    /// Which parameter an argument filled: its own position, or the one its name says.
    ///
    /// PHP lets a caller write the arguments in any order it likes as long as it names them, and a
    /// contract is about a *parameter* -- so an index alone answers only half the calls.
    fn parameter(&self, call: &phrost_body::PassedTo) -> Option<usize> {
        let phrost_body::PassedAs::Argument(argument) = call.passed_as else {
            // The receiver fills no parameter.
            return None;
        };
        match &call.name {
            None => Some(argument),
            Some(written) => self.params.iter().position(|param| param.name == *written),
        }
    }

    /// What `$this` is inside the closure passed as `call`'s argument, when the callee says --
    /// `@param-closure-this` -- and the class the call was made on, which a `$this` or `static`
    /// there means.
    pub(crate) fn closure_this(
        &self,
        call: &phrost_body::PassedTo,
    ) -> Option<(phrost_ty::TypeId, Option<&Name>)> {
        let param = self.params.get(self.parameter(call)?)?;
        Some((param.closure_this_ty?, self.class.as_ref()))
    }

    /// Whether this declaration can say anything about the value `call` handed it.
    ///
    /// The three things a statement call is read for, each by the least its reader needs: a
    /// receiver is replaced only by a contract ([`IndexView::left_in_receiver_by`]), an argument is
    /// written only through a by-reference parameter ([`IndexView::written_through_reference`]),
    /// and is claimed only by an `@assert` tag naming that parameter ([`Self::asserts_about`]). A
    /// superset of what each goes on to accept, so a call this refuses is one all three would have
    /// answered nothing for -- and nearly every call a body writes is one of those.
    pub(crate) fn speaks_of(&self, call: &phrost_body::PassedTo) -> bool {
        match call.passed_as {
            phrost_body::PassedAs::Receiver(_) => self.contract.is_some(),
            phrost_body::PassedAs::Argument(_) => {
                self.parameter(call)
                    .and_then(|at| self.params.get(at))
                    .is_some_and(|param| param.by_reference)
                    || self.asserts_about(call).is_some()
            }
        }
    }

    /// What an `@assert` tag claims about the value the call passed.
    pub(crate) fn asserts_about(
        &self,
        call: &phrost_body::PassedTo,
    ) -> Option<crate::symbols::ArgumentAssertion> {
        let phrost_sema::Contract::Argument {
            parameter,
            ty,
            negated,
            when_true,
            from_class_string,
        } = self.contract.as_ref()?
        else {
            return None;
        };
        // Matched by parameter name against the position, because that is what the tag writes and
        // what the call site has is an index.
        (self.params.get(self.parameter(call)?)?.name == *parameter).then_some(
            crate::symbols::ArgumentAssertion {
                ty: *ty,
                negated: *negated,
                when_true: *when_true,
                from_class_string: *from_class_string,
            },
        )
    }
}

impl IndexView<'_> {
    /// The kind a tag's type names when it names a kind and not a class.
    ///
    /// Not [`Self::stored_kind`], which classifies a *class* as `object` -- read through that,
    /// `@phpstan-assert Foobar $value` would be an object assertion and hand back the declared
    /// `object` instead of the `Foobar`. Only what is spelled as a kind counts: a builtin, a generic
    /// over one (`array<string, mixed>`), a shape, and PHPDoc's own words for an array.
    fn asserted_kind(&self, ty: phrost_ty::TypeId) -> Option<BuiltinTy> {
        let types = self.skeleton.types();
        match types.get(ty) {
            // The kind a refinement is of: `non-empty-string` is tested for as a string, and what
            // it holds is the asserting reader's to add.
            phrost_ty::TypeRef::Builtin(kind) => Some(kind.unrefined()),
            phrost_ty::TypeRef::Generic { base, .. } => match types.get(*base) {
                phrost_ty::TypeRef::Builtin(kind) => Some(kind.unrefined()),
                _ => self
                    .stored_kind(*base)
                    .filter(|kind| *kind == BuiltinTy::Array),
            },
            phrost_ty::TypeRef::Shape { .. } => Some(BuiltinTy::Array),
            phrost_ty::TypeRef::Named(_) => self
                .stored_kind(ty)
                .filter(|kind| *kind == BuiltinTy::Array),
            _ => None,
        }
    }

    /// Whether a container type's arguments name a class nobody declares -- a template the tag
    /// writes and nothing here has substituted.
    fn arguments_name_nothing_declared(&self, ty: phrost_ty::TypeId) -> bool {
        let types = self.skeleton.types();
        let phrost_ty::TypeRef::Generic { args, .. } = types.get(ty) else {
            return false;
        };
        types.operands(*args).iter().any(|&argument| {
            !self.names_something_real(argument) || self.a_union_names_nothing_declared(argument)
        })
    }
}

/// The kinds an `is_*` test, and so an `@assert`, can tell a value to be.
///
/// Not `mixed`, which is the absence of a kind; not `null`, which the negated form reads on its own
/// and the positive form has no use for; not `void` or `never`, which no value is; and not `true`
/// or `false`, which are values of a kind rather than kinds. `object` is one: `@assert object $x`
/// leaves every class alternative and drops the scalars.
fn is_a_kind_a_value_is_tested_for(kind: BuiltinTy) -> bool {
    matches!(
        kind,
        BuiltinTy::Array
            | BuiltinTy::String
            | BuiltinTy::Int
            | BuiltinTy::Float
            | BuiltinTy::Bool
            | BuiltinTy::Callable
            | BuiltinTy::Iterable
            | BuiltinTy::Object
    )
}
