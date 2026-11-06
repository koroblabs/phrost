//! The walk itself: the memo in front of it, and what a path's root holds.
//!
//! Every value in a body is a root plus the steps taken off it. This owns the first half -- the
//! match that turns each way a value can start into a type -- because that match is where a new
//! kind of root lands, and it must not be the same file as the walk over steps that a new kind of
//! *step* lands in. What happens after the root is [`super::steps`].

use phrost_body::PathRoot;
use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, Name, TextSize};

use crate::SentinelReach;
use crate::{resolve::Held, view::IndexView};

/// How many entries of a list literal are read before the answer is the bare `array`.
///
/// A framework's render array is a literal with hundreds of entries, and every one of them would be
/// a full path resolution on the path a keystroke takes. Eight is past every list anybody writes by
/// hand and far short of a configuration array.
const LITERAL_ENTRIES: usize = 8;

impl IndexView<'_> {
    /// The type a value path ends at: its root, then every step applied in turn.
    ///
    /// Held as a type rather than a class, because a step can ask for an element: `array<Repo>` is
    /// not a class and a walker that only tracked classes would have nothing left to iterate.
    pub(crate) fn resolve_path(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
    ) -> Option<Held> {
        // Keyed by the budget as well. A recursive call has spent some of it and can therefore reach
        // a different answer for the same path, so a budget-limited answer must never stand in for
        // the real one -- which is why this was once kept at full budget only. Keeping the budget in
        // the key keeps that promise and caches the rest: every chain below the top ran uncached,
        // so a value reached through a chain of locals was re-resolved from scratch at every read,
        // and WordPress's `class-wp-query.php` spent most of half a second doing it.
        let key = (offset, budget);
        let known = self.resolved.borrow().get(&key).and_then(|answers| {
            answers
                .iter()
                .find(|(written, _)| written == path)
                .map(|(_, answer)| answer.clone())
        });
        if let Some((held, touched)) = known {
            // A remembered answer a contract changed is still one the caller's answer rests on.
            if touched {
                self.contract_applied.set(true);
            }
            return held.map(|held| self.written_nullsafe(hir, path, held));
        }

        // Borrowed and released around the call, which re-enters this method: holding it across
        // would be a panic the first time a path resolved through another path. The flag is taken
        // for this path alone and handed back to whatever encloses it.
        let enclosing = self.contract_applied.replace(false);
        let held = self.resolve_path_uncached(offset, hir, path, budget);
        let touched = self.contract_applied.get();
        self.contract_applied.set(enclosing || touched);
        self.resolved
            .borrow_mut()
            .entry(key)
            .or_default()
            .push((path.clone(), (held.clone(), touched)));
        held.map(|held| self.written_nullsafe(hir, path, held))
    }

    /// The null a `?->` puts on the whole expression.
    ///
    /// Applied here and stored nowhere, which is what the memo needs: the cache is keyed by the
    /// *slot*, and `$x?->y()` and `$x->y()` are one slot written two ways -- so the null belongs to
    /// the reading rather than to the value. `$a?->b()->c()` is still not at risk of calling on
    /// null, because PHP short-circuits the rest of the chain; what is at risk is whoever receives
    /// the expression, and that is exactly who holds this path.
    fn written_nullsafe(&self, hir: &FileHir, path: &phrost_body::ValuePath, held: Held) -> Held {
        if !path.may_be_absent {
            return held;
        }
        // `mixed` already admits null, and saying it twice makes a reader choose which half to
        // believe. Through `kind_of_held` rather than by matching the arm, because a builtin
        // arrives in three spellings and `Held::nullable`'s own fold knows only one of them -- a
        // `?mixed` reached a real report before this asked.
        match self.kind_of_held(hir, &held) {
            Some(phrost_model::BuiltinTy::Mixed) => held,
            _ => Self::nullable(held),
        }
    }

    fn resolve_path_uncached(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
    ) -> Option<Held> {
        // What the body settled about this value, which the declaration alone cannot say. Which
        // paths have a slot a guard can be written about is [`ValuePath::can_be_guarded`]'s
        // question; a bare local is not one, because its writes and narrowings are already ranked
        // against each other -- see `slot_value`.
        let settled = (path.can_be_guarded() && budget > 0)
            .then(|| {
                phrost_body::slot_value(
                    hir,
                    offset,
                    path,
                    &|hir, expr| self.call_never_returns(offset, hir, expr),
                    &self.asserting(),
                    &self.assigned_properties(hir),
                )
            })
            .flatten();

        // A guard replaces the declaration outright, for the reason it does for a local: the
        // declaration says what the value may be and the guard says what it is here.
        if let Some(settled) = &settled
            && settled.written_at.is_none()
            && let Some(held) = self.resolve_path(offset, hir, &settled.value, budget - 1)
        {
            return Some(held);
        }

        let held = match &path.root {
            PathRoot::Written(id) => Held::Written(*id),
            // The question `phrost_body` left open: a method parameter with no type of its own
            // holds whatever the declaration it overrides says about the same position.
            PathRoot::InheritedParameter { position } => {
                self.inherited_parameter(offset, *position)?
            }
            PathRoot::VariadicValues(id) => Held::ListOf(Box::new(Held::Written(*id))),
            PathRoot::Builtin(builtin) => Held::Builtin(*builtin),
            PathRoot::FunctionReturn {
                name,
                anchor,
                arguments,
                every_argument_written: _,
                call,
            } => {
                let location = self.resolve_name(
                    offset,
                    name.as_str(),
                    *anchor,
                    phrost_hir::ImportKind::Function,
                )?;
                // A conditional return type first: it is a claim about this call, and what the
                // declaration says on its own is the union of both branches.
                let at = crate::symbols::CallFacts {
                    offset,
                    hir,
                    literals: arguments,
                    call: *call,
                    budget,
                    // A root, so there is no receiver to have been written with arguments.
                    receiver: &[],
                    // The call is an expression here, so its own arguments are read.
                    written: None,
                };
                // Each answer is the arm's value rather than a `return`, which would skip the steps
                // written after the call: `app('redirect')->back()` read as the `Redirector`.
                if let Some(named) =
                    // A framework's container helper, whose argument is a key no declaration of it
                    // can read: `app('router')` is a `Router` because of what Laravel registered.
                    self.returned_from_function_argument(location.fqn.as_str(), arguments)
                {
                    named
                } else if let Some(narrowed) =
                    // A literal the caller wrote out, where the declaration's failure branch cannot
                    // be taken for it -- `strtotime('-25 days')`, and `strtotime("-$i months")` for
                    // an integer `$i`. Asked after the docblock's own conditional, which is the
                    // more specific claim where there is one.
                    self.narrowed_by_what_the_call_wrote(location.fqn.as_str(), &at)
                {
                    Held::Builtin(narrowed)
                } else {
                    let returned =
                        match self.returned_by_function_contract(location.fqn.as_str(), &at) {
                            Some(settled) => settled,
                            None => Held::Stored {
                                ty: self.function_return(location.fqn.as_str())?,
                                // A free function has no enclosing class, so `self` and `static` cannot
                                // appear in what it returns.
                                scope: None,
                            },
                        };
                    // And PHP's own rule about what a filtering call leaves behind, which no
                    // declaration of it can state. See `filtered_elements`.
                    self.without_filtered_elements(
                        offset,
                        hir,
                        location.fqn.as_str(),
                        *call,
                        returned,
                    )
                }
            }
            // A `::` receiver, which may be written as an instance rather than as a name -- see
            // `class_of_scope`.
            PathRoot::Class(target) => match self.class_of_scope(offset, hir, *target) {
                Some(class) => Held::Class(class),
                // `Application::APPLICATION_TYPE[$type]::PARAGRAPH`: a table of classes read by key
                // names whichever class the key picks, which is each of them -- the reading
                // `new` gives the same value below, and never the ancestor they share.
                None => {
                    let classes = self.classes_a_value_names(hir, *target);
                    if classes.len() < 2 {
                        return None;
                    }
                    Self::one_of(classes.into_iter().map(Held::Class).collect())
                }
            },
            // A string, which is what it is at runtime and what every reader but one wants. The
            // exception reads the target off the root instead -- see `class_a_path_names`.
            //
            // Which string is known, though: a class's name is never empty and never `'0'`, and a
            // quoted literal says by its text -- see `quoted_string_holds`.
            PathRoot::ClassName(_) => Held::Builtin(BuiltinTy::NonFalsyString),
            PathRoot::StringLiteral(literal) => Held::Builtin(match hir.expr(*literal) {
                phrost_hir::body::ExprData::Literal { range, .. } => {
                    self.literal_string_holds(offset, *range)
                }
                _ => BuiltinTy::String,
            }),
            PathRoot::Instantiated { class, call } => {
                match self.class_of_target(offset, hir, *class) {
                    // A value naming one of several classes -- a factory's table read by key --
                    // makes an instance of whichever it names, which is each of them. The arm's
                    // value and not a `return`, which skipped every step after the `new`.
                    None => {
                        let classes = self.classes_a_value_names(hir, *class);
                        if classes.len() < 2 {
                            return None;
                        }
                        Self::one_of(classes.into_iter().map(Held::Class).collect())
                    }
                    // What the constructor was given, for a class whose templates its parameters
                    // bind. Only the `new` says what *this* instance holds -- the declaration says
                    // what every instance of the class could.
                    Some(named) => self.instance_of(
                        named,
                        &crate::symbols::CallFacts {
                            offset,
                            hir,
                            literals: &[],
                            call: *call,
                            budget,
                            // A root, so there is no receiver to have been written with arguments.
                            receiver: &[],
                            // The call is an expression here, so its own arguments are read.
                            written: None,
                        },
                    )?,
                }
            }
            // What the guard proved, beside what the declaration already said. Beside, because the
            // two are not alternatives: `assert($mock instanceof Payment)` on a value the signature
            // typed as `MockInterface` leaves something that answers to both, and replacing the
            // interface would take away every method the mock framework put there.
            PathRoot::Narrowed { of, at, class } => {
                let named = self.class_of_target(offset, hir, *class)?;
                match self.resolve_path(*at, hir, of, budget.checked_sub(1)?) {
                    Some(held) => self.also_of_class(offset, hir, held, &named),
                    // Nothing said what it was before, so the guard is the whole answer.
                    None => Held::Class(named),
                }
            }
            // A closure literal. What it gives back is read here rather than at the invocation,
            // because the value travels: `$make = fn (): Query => ...` is a variable holding a
            // factory, and the call that uses it is written somewhere else entirely.
            //
            // Still a callable when the return does not resolve -- `fn (): Unknown => ...` is a
            // callable whose result nothing can name, which is a smaller claim than the nothing
            // this said before and a true one.
            // A list literal, whose element type is the union of what its entries hold. Resolved
            // here rather than at the body layer for the reason a closure is: each entry is itself a
            // path, and following one needs the index.
            //
            // Every entry has to answer, and that is the load-bearing half. A union built from the
            // ones that did would be *narrower than the truth* -- `[1, $unknown]` would read as
            // `int[]` and report the next call that takes a `string[]` -- and a type narrower than
            // the truth is worse than the bare `array` this used to give. So one silent entry gives
            // the bare array back.
            PathRoot::ArrayLiteral(expr) => {
                // The record reading comes first, because the two questions are asked of the same
                // node and only one of them can be true: a literal keyed throughout is a record and
                // a literal keyed nowhere is a list. Asked the other way round a record would fall
                // to the element rule, whose all-or-nothing test its values have no reason to pass.
                match self.record_literal_fields(offset, hir, *expr, budget) {
                    Some(fields) => Held::Record(fields),
                    None => match self.list_literal_element(offset, hir, *expr, budget) {
                        Some(element) => Held::ListOf(Box::new(element)),
                        None => Held::Builtin(BuiltinTy::Array),
                    },
                }
            }
            PathRoot::Closure(expr) => {
                let at = crate::symbols::CallFacts {
                    offset,
                    hir,
                    literals: &[],
                    call: phrost_body::CallSource::unread(),
                    budget,
                    // A root, so there is no receiver to have been written with arguments.
                    receiver: &[],
                    // The call is an expression here, so its own arguments are read.
                    written: None,
                };
                match self.callback_held(&at, *expr) {
                    Some(returns) => Held::Callable(Box::new(crate::resolve::Callable {
                        returns,
                        literal: Some(*expr),
                    })),
                    None => Held::Builtin(BuiltinTy::Callable),
                }
            }
            // A first-class callable: `strlen(...)`, `$repo->find(...)`. The inner path is the call
            // that was never made, so resolving it gives what invoking this value hands back -- and
            // the wrapper is what keeps the two apart. Without it `$fn = strlen(...)` would hold an
            // `int`, and `$fn('a')` would be a call on one.
            //
            // Still a callable when the callee's return does not resolve, for the reason a closure
            // literal is: "a callable whose result nothing can name" is a smaller claim than
            // nothing at all, and a true one.
            PathRoot::Callable(of) => {
                match self.resolve_path(offset, hir, of, budget.checked_sub(1)?) {
                    Some(returns) => Held::Callable(Box::new(crate::resolve::Callable {
                        returns,
                        // No parameter list to point at: the callee's is declared wherever the
                        // callee is, which is usually not this file.
                        literal: None,
                    })),
                    None => Held::Builtin(BuiltinTy::Callable),
                }
            }
            // A local with steps after it is a value of its own first, and that value is what a
            // call made on it or handed it changed: `$users->transform(...)` makes `$users` a
            // collection of something else, and so changes what `$users->first()` returns. The
            // contract question `held_after_steps` asks at the end is about the *whole* path, which
            // no call before this one was handed -- so without this the bare local and the chain off
            // it answered two different things, and a receiver resolved for goto disagreed with the
            // value the same expression assigned. Through the memo, so it is the answer the bare
            // read at this position already paid for.
            //
            // A local only. `$this` is the root of most paths in a class and almost never what a
            // contract is about: asking for it changed no finding on eight corpora and no answer in
            // Symfony's coverage sweep, while costing every `$this->...` read a second lookup.
            PathRoot::Local(_) if !path.steps.is_empty() => {
                self.resolve_path(offset, hir, &root_alone(path), budget)?
            }
            PathRoot::Local(name) => self.resolve_local_held(offset, name.as_str(), budget)?,
            // What the declaration's value stated, which the index keeps because the value is text
            // in a file a closed buffer does not have.
            PathRoot::Constant(name) => {
                let written = hir.name_ref(*name);
                let declared = self.resolve_name(
                    offset,
                    written.path.as_str(),
                    written.anchor,
                    phrost_hir::ImportKind::Constant,
                )?;
                self.constant_held(offset, hir, declared.fqn.as_str())?
            }
            // An instance of a class is left as it is; every other kind of value PHP turns into a
            // `stdClass` holding it -- an array's keys as properties, a scalar as `scalar`, a null
            // as nothing at all. What could still be either says only `object`.
            PathRoot::ObjectCast(value) => {
                let budget = budget.checked_sub(1)?;
                let cast = self.resolve_path(offset, hir, value, budget);
                let object = Held::Builtin(BuiltinTy::Object);
                match cast {
                    Some(held) => match self.kind_of_held(hir, &held) {
                        // A `?Foo` could be the null that becomes a `stdClass`, so only an object
                        // nothing can take away is kept as itself.
                        Some(BuiltinTy::Object)
                            if self.null_reach_of(hir, &held)
                                == crate::SentinelReach::Impossible =>
                        {
                            held
                        }
                        Some(
                            BuiltinTy::Array
                            | BuiltinTy::Int
                            | BuiltinTy::Float
                            | BuiltinTy::String
                            | BuiltinTy::Bool
                            | BuiltinTy::True
                            | BuiltinTy::False
                            | BuiltinTy::Null,
                        ) => self
                            .class_location("stdClass")
                            .map_or(object, |location| Held::Class(location.fqn)),
                        _ => object,
                    },
                    None => object,
                }
            }
            // An operator PHP settles by what it was given. Both operands have to be known for the
            // answer to be one -- `$count * $rate` where either is a mixed says nothing -- which is
            // why this asks for both and gives up rather than assuming a number.
            PathRoot::Arithmetic {
                operator,
                left,
                right,
            } => {
                let budget = budget.checked_sub(1)?;
                let left = self.resolve_path(offset, hir, left, budget)?;
                let right = self.resolve_path(offset, hir, right, budget)?;
                self.arithmetic_result(offset, hir, *operator, &left, &right)?
            }
            // A string built from pieces holds what the piece holding the most does. A piece that
            // does not resolve adds nothing, which is the same as holding nothing -- the syntax's own
            // reading stands for it.
            PathRoot::Concatenation { holds, pieces } => {
                let budget = budget.checked_sub(1)?;
                let held = pieces.iter().fold(*holds, |held, piece| {
                    match held == BuiltinTy::NonFalsyString {
                        true => held,
                        false => self
                            .resolve_path(offset, hir, piece, budget)
                            .map_or(held, |piece| {
                                held.concatenated(self.piece_held(hir, &piece))
                            }),
                    }
                });
                Held::Builtin(held)
            }
            // A number stepped by one is the same kind of number, and no other scalar steps the way
            // it adds -- so a number is the only scalar answered. Through `scalar_named`, because a
            // builtin arrives in three spellings and only one of them is `Held::Builtin`. The one
            // object that does step that way is of a class overloading the operators: see
            // `stepped_object`.
            PathRoot::Stepped(target) => {
                let budget = budget.checked_sub(1)?;
                let target = self.resolve_path(offset, hir, target, budget)?;
                match self.scalar_named(hir, &target) {
                    Some(number @ (BuiltinTy::Int | BuiltinTy::Float)) => Held::Builtin(number),
                    Some(_) => return None,
                    None => self.stepped_object(offset, hir, &target)?,
                }
            }
            // One value out of several, which is a union -- so only what every operand agrees on
            // can be reached through it. Every one has to resolve: half a union is a claim the code
            // did not make, and offering one operand's members would offer a call that is a fatal
            // error whenever a different branch ran.
            //
            // Each operand at its own position, which is what makes the union the code's rather
            // than one wider than it: `is_array($x) ? $x[1] : $x` selects each side by a condition,
            // and read at the join both sides are the value the condition was written to split.
            PathRoot::EitherOf { operands, unless } => {
                let budget = budget.checked_sub(1)?;
                let mut resolved = Vec::with_capacity(operands.len());
                for operand in operands {
                    if self.null_that_cannot_hold(hir, &operand.value, budget) {
                        continue;
                    }
                    resolved.push(self.resolve_path(operand.at, hir, &operand.value, budget)?);
                }
                self.union_of(offset, hir, resolved, *unless)?
            }
            // A name written in a branch the position is not in. The same union, with the one
            // difference `EitherOf` cannot state: each write is read where it was written, and the
            // value before them may not exist at all -- a name written in one branch and nowhere
            // else holds what that branch wrote, and there is no second operand to join it with.
            // A container a loop wrote into through a reference: what it was at the loop's header,
            // widened by what was written -- see `rewritten_elements`.
            PathRoot::RewrittenElements { before, at, writes } => {
                let budget = budget.checked_sub(1)?;
                let held = self.resolve_path(*at, hir, before, budget)?;
                self.rewritten_elements(offset, hir, held, *at, writes.as_deref(), budget)
            }
            PathRoot::MaybeWritten {
                writes,
                read,
                before,
                otherwise,
            } => {
                let budget = budget.checked_sub(1)?;
                let mut resolved = Vec::with_capacity(writes.len() + 1);
                for write in writes {
                    resolved.push(self.resolve_path(write.at, hir, &write.value, budget)?);
                }
                // Last, so that a write is the operand a reduction keeps: what the branch put there
                // is the more specific claim, and the one a reader is asking about. Left out
                // entirely where there is no path that skipped the writes -- see
                // [`Otherwise::Impossible`], which is the claim that the construct's every arm wrote.
                if !matches!(otherwise, phrost_body::Otherwise::Impossible)
                    && let Some(held) = self
                        .resolve_path(*before, hir, read, budget)
                        .and_then(|held| self.otherwise(offset, hir, held, otherwise, budget))
                {
                    resolved.push(held);
                }
                self.union_of(offset, hir, resolved, phrost_body::Unless::Nothing)?
            }
            // A guard that called a boolean method. The receiver is resolved at the guard, where
            // this narrowing does not reach, and the class comes off that method's own docblock.
            PathRoot::Asserted {
                receiver,
                method,
                when_true,
                at,
            } => {
                let held = self.resolve_path(*at, hir, receiver, budget)?;
                match self.class_of_held(*at, hir, &held).and_then(|class| {
                    self.member_assertion(class.as_str(), method.as_str(), *when_true)
                }) {
                    // The asserted class is written in the declaring file, so `self` inside it means
                    // that class rather than the one it was reached through.
                    Some(ty) => Held::Stored { ty, scope: None },
                    // The method carries no tag for this branch -- an `@assert-if-true` says nothing
                    // about the else. The value is what it already was; a guard that proves nothing
                    // must not take away what the declaration said.
                    None => held,
                }
            }
            // The mirror of `PathRoot::Asserted`: the call is on something else, and the narrowed
            // value is one of its arguments rather than its receiver. `of` is resolved at the
            // guard for the same reason every other guard root here is -- so the callee's own
            // declaration, not `of`'s, is what the tag is read off.
            PathRoot::AssertedArgument {
                of,
                callee,
                argument,
                name,
                when_true,
                at,
            } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                let call = phrost_body::PassedTo {
                    at: *at,
                    callee: (**callee).clone(),
                    passed_as: phrost_body::PassedAs::Argument(*argument),
                    name: name.clone(),
                    class_arguments: Vec::new(),
                };
                self.asserted_argument_held(offset, hir, held, &call, *when_true, budget)
            }
            // A null guard says what the value is *not*. What it is was settled before the guard,
            // so that is where it is asked -- and a guard does not reach its own position, which is
            // what makes asking there terminate instead of coming back here. The budget goes down
            // as well, because an outer guard found at that earlier position lands here again.
            PathRoot::NotNull { of, at, refused } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                self.without_sentinels(*at, hir, held, *refused)?
            }
            // The same, for a guard that ruled out `''` and left a null where it was -- and, for a
            // needle the value was found to start with, end with or contain, whatever its text holds.
            PathRoot::NotEmptyString { of, at, like } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                let holding = match like.map(|like| hir.expr(like)) {
                    None => BuiltinTy::NonEmptyString,
                    Some(phrost_hir::body::ExprData::Literal { range, .. }) => {
                        self.literal_string_holds(*at, *range)
                    }
                    // The haystack of a loose `in_array`: what its weakest string holds.
                    Some(phrost_hir::body::ExprData::ArrayLiteral { elements, .. }) => {
                        self.loosely_found_among(hir, hir.expr_list(*elements))
                    }
                    Some(_) => BuiltinTy::String,
                };
                match holding {
                    BuiltinTy::String => held,
                    holding => self.with_strings_holding(*at, hir, held, holding),
                }
            }
            // The key a container was found to hold is of the kind its keys are -- and only a
            // string key says anything, because PHP stores a numeric string as the integer it
            // spells: `'5'` finds an `array<int, V>`'s key 5, so an int-keyed container leaves a
            // string key a string. An int never finds a string-keyed one's, and neither does a
            // numeric string. A filter over what the key was declared as, never a claim beside it.
            // The classes whose property cannot be of the kind the guard said are not the one the
            // value is. Only a union says anything: one class is what the declaration named, and a
            // guard about its property is about the property.
            //
            // By the property's *native* type, which PHP enforces on every write. A docblock's is a
            // claim, and WordPress's `WP_Post::$post_author` is documented `string` and arrives as
            // an `int` from a REST controller -- which is the very guard that reads it.
            PathRoot::MemberKind {
                of,
                at,
                property,
                kind,
                holds,
            } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                self.narrowed_union(offset, hir, held, &|operand| {
                    let Some(class) = &operand.class else {
                        return Some(operand.held);
                    };
                    let declared = self
                        .members(class.as_str())
                        .find(phrost_sema::MemberKind::Property, property.as_str())
                        .and_then(|member| member.row.ty)
                        .and_then(|ty| self.kind_of_held(hir, &Held::Stored { ty, scope: None }));
                    let answers = declared.map_or(crate::resolve::Answers::Maybe, |declared| {
                        Self::kind_answers(declared, *kind)
                    });
                    let kept = match answers {
                        crate::resolve::Answers::Maybe => true,
                        crate::resolve::Answers::Yes => *holds,
                        crate::resolve::Answers::No => !*holds,
                    };
                    kept.then_some(operand.held)
                })
            }
            PathRoot::KeyOf { of, at, container } => {
                let budget = budget.checked_sub(1)?;
                let held = self.resolve_path(*at, hir, of, budget)?;
                // Only a union with something a string key rules out is narrowed at all, so the
                // container -- a resolution of its own, asked under every `isset($map[$key])` --
                // is read only for one. A string key, or a key nothing typed, is answered as is.
                let narrows = self
                    .union_operands(offset, hir, &held)
                    .is_some_and(|operands| {
                        operands.iter().any(|operand| {
                            matches!(
                                operand.kind,
                                Some(
                                    BuiltinTy::Int
                                        | BuiltinTy::Float
                                        | BuiltinTy::Bool
                                        | BuiltinTy::True
                                        | BuiltinTy::False
                                )
                            )
                        })
                    });
                // The arm's value and not a `return`, which would skip the steps after the root.
                let keyed_by_strings = narrows
                    && self
                        .resolve_path(*at, hir, container, budget)
                        .and_then(|container| self.declared_key_kind(hir, &container))
                        == Some(crate::key_kinds::KeyKind::String);
                match keyed_by_strings {
                    true => self.of_kind(offset, hir, held, BuiltinTy::String, true, false),
                    false => held,
                }
            }
            // A needle a strict `in_array` found. Both sides are read before the guard, where it
            // does not reach, and met -- see `found_among`.
            PathRoot::ElementOf { of, at, container } => {
                let budget = budget.checked_sub(1)?;
                let needle = self.resolve_path(*at, hir, of, budget);
                // Every slot at once for a record, which a list literal written out is: the needle
                // is identical to one of its values, and `element_of_held` refuses a record because
                // a keyed read must not be answered from the other keys.
                let element =
                    self.resolve_path(*at, hir, container, budget)
                        .and_then(|container| {
                            self.element_of_held(*at, hir, &container)
                                .or_else(|| self.shape_values(*at, hir, &container))
                        });
                self.found_among(offset, hir, needle, element)?
            }
            // A value an identity found not to be an array literal: read before the guard and
            // compared slot by slot -- see `not_identical_to`.
            PathRoot::NotIdenticalTo { of, at, literal } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                self.not_identical_to(offset, hir, held, *literal)
            }
            // A strict identity with a constant that held: the subject is none of the sentinels the
            // constant cannot be. A constant nothing here can read leaves the subject as it was,
            // which is the answer for `const NONE = null` too.
            PathRoot::SentinelsRefusedBy { of, at, other } => {
                let budget = budget.checked_sub(1)?;
                let held = self.resolve_path(*at, hir, of, budget)?;
                let refused = self
                    .resolve_path(*at, hir, other, budget)
                    .and_then(|other| {
                        match (
                            self.proves_absent(hir, &other, BuiltinTy::Null),
                            self.proves_absent(hir, &other, BuiltinTy::False),
                        ) {
                            (true, true) => Some(phrost_body::Refused::TheSentinels),
                            (true, false) => Some(phrost_body::Refused::NullOnly),
                            (false, _) => None,
                        }
                    });
                match refused {
                    Some(refused) => self.without_sentinels(*at, hir, held, refused)?,
                    None => held,
                }
            }
            PathRoot::NoneOfKinds {
                of,
                at,
                refused,
                kinds,
            } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                let held = self.without_sentinels(*at, hir, held, *refused)?;
                self.none_of_kinds(offset, hir, held, kinds)
            }
            // A call that may have written through a by-reference parameter. `None` when it did
            // not, which leaves the name with no type -- and is why this is reached only where
            // nothing else bound it.
            // Every call that passed the variable, newest first, and the first whose parameter
            // really is by-reference wins. One recorded call was the wrong one of two candidates
            // whenever an ordinary call was written in between -- `fills($found); reads($found);`
            // left `$found` with no type at all, because `reads` was the call recorded and takes
            // its argument by value. The same walk `Self::contract_narrowed` does on its own route,
            // asked here through the same two functions rather than a third copy of the question.
            PathRoot::ByReference { subject, .. } => {
                let budget = budget.checked_sub(1)?;
                let mut written = None;
                // From the asking position and not from the call the root recorded: a statement the
                // offset sits *inside* is not one of its preceding statements, so asking just past
                // the recorded call's own start excludes that call -- and the answer was then empty
                // exactly on the line after the out-parameter, which is where it matters most.
                // The wide walk, because this root exists precisely for a name nothing else
                // bound -- `if (preg_match($p, $s, $m))` writes `$m` and no assignment does. The
                // narrow one is for `contract_narrowed`, which is asked about every vague value and
                // where the same width cost +30 % of `phrost check`.
                // Reversed, because the walk hands them back in source order and the *last* write
                // is the one still standing: `preg_match($p, $s, $m); preg_match_all($q, $s, $m);`
                // leaves `$m` in the second one's shape, and taking the first by-reference call
                // found meant the older tag won every time two of them wrote the same name.
                for call in phrost_body::passed_to_any_call(hir, offset, subject)
                    .into_iter()
                    .rev()
                {
                    let Some(declared) = self.declaration_called(offset, hir, &call, budget) else {
                        continue;
                    };
                    if let Some(found) =
                        self.written_through_reference(offset, hir, &declared, &call, None)
                    {
                        written = Some(found);
                        break;
                    }
                }
                // No call wrote it, so passing it wrote nothing: the name is still one nothing in
                // the file binds, and whoever binds those -- the engine, a template's renderers --
                // answers. A template's `<x-card :post="$post">` passes `$post` and keeps it.
                match (written, &subject.root) {
                    (Some(written), _) => written,
                    (None, PathRoot::Local(name)) if subject.steps.is_empty() => {
                        self.unbound_held(name.as_str(), budget)?
                    }
                    (None, _) => return None,
                }
            }
            // A callback's parameter, from whichever source at the call site says what it
            // receives. Ranked and not joined -- both are claims about one value, so the first
            // that answers is the answer.
            //
            // The declared signature first, because it is a statement rather than a guess: a
            // `@param callable(Application): void` says what the callback is handed, and it is the
            // only thing that can where the sibling is a class name. Then the container, whose
            // element type is right for every function that applies a callback to what it was
            // given beside it -- and which is why the ranking has to happen here rather than where
            // the root is built: the body layer cannot tell whether the callee's declaration will
            // resolve, so building the two as separate roots would always pick the first and take
            // the container reading out of every `array_map` in the project.
            PathRoot::CallbackParameter {
                parameter,
                callee,
                argument,
                argument_name,
                siblings,
                at,
                declared: written,
            } => {
                let budget = budget.checked_sub(1)?;
                let native = *written;
                let written = written.map(Held::Written);
                let call = phrost_body::PassedTo {
                    at: *at,
                    callee: (**callee).clone(),
                    passed_as: phrost_body::PassedAs::Argument(*argument),
                    name: argument_name.clone(),
                    class_arguments: Vec::new(),
                };
                let found = self.declaration_called(offset, hir, &call, budget);
                let declared = found
                    .as_ref()
                    .and_then(|found| self.callback_parameter(found, &call, *parameter));
                let inferred = match declared {
                    Some(held) => held,
                    // The signature names a template no container holds -- an accumulator -- and
                    // what the container reading would answer is an element, which it is not.
                    None if found.as_ref().is_some_and(|found| {
                        self.callback_parameter_is_unheld_template(found, &call, *parameter)
                    }) =>
                    {
                        return written;
                    }
                    // The containers passed beside the callback, in written order: `array_map`
                    // takes the callback first and `array_filter` second, and nothing here reads a
                    // signature to know which.
                    //
                    // A callback written *first* is handed one element of each container after it,
                    // so its parameters and those containers pair up in order --
                    // `array_map($cb, $timestamps, $payments)`. A callback written after a
                    // container is being told about that one container, and only its first
                    // parameter can be read: a second is a key, an element again or a flag's
                    // business, and telling those apart needs the declaration this branch is the
                    // fallback for.
                    None => {
                        // Only a free function, which is what this pairing describes: `array_map`,
                        // `array_filter`, `usort` and their neighbours take a callback beside the
                        // containers it is handed elements of, and the standard library is where
                        // that shape lives. A *method* taking a callback beside an array is not
                        // that shape at all -- `$validator->sometimes('x', ['Foo', 'Bar'], fn ($i)
                        // => ...)` passes rules, not elements, and reading them as the callback's
                        // parameter gave `$i` a `string` and reported the property access on it.
                        // Whatever the parameter declared, when the container says nothing: a
                        // bare `array` is less than the element and more than no answer at all.
                        let from_a_container = || {
                            if !matches!(**callee, phrost_body::Callee::Function(_)) {
                                return None;
                            }
                            let containers = siblings
                                .iter()
                                .filter_map(|sibling| self.resolve_path(*at, hir, sibling, budget))
                                .filter(|held| self.holds_elements(*at, hir, held))
                                .collect::<Vec<_>>();
                            let container = match argument {
                                0 => containers.get(*parameter)?,
                                _ if *parameter == 0 => containers.first()?,
                                _ => return None,
                            };
                            self.element_of_held(*at, hir, container)
                        };
                        from_a_container().or(written)?
                    }
                };
                // The closure's own declaration is a guarantee: PHP checks it before the body runs,
                // so whatever was inferred may refine it -- `array` into `array<int, Row>` -- and
                // can never contradict it. `Type\converted($from, $into, static fn (array $data):
                // Person => ...)` read `$data` as the element of `$into`, the wrong one of two
                // containers, and every subscript on it was reported as an object used as an array.
                match native {
                    Some(native)
                        if self.fit_held_against_written(*at, hir, &inferred, native)
                            == phrost_ty::compat::Fit::Rejected =>
                    {
                        Held::Written(native)
                    }
                    _ => inferred,
                }
            }
            // A guard named which of the alternatives are left. Resolved before the guard, where
            // the declaration still lists them all, and then filtered to the ones it named.
            //
            // `None` from that resolution is not a reason to give up, which is what `PathRoot::
            // Narrowed` above already gets right: a parameter written `$v` with no hint at all
            // resolves to nothing before the guard, and the guard is then the whole answer rather
            // than no answer. A `?` here meant `if ($v instanceof Baz || $v instanceof Boo)` proved
            // nothing for exactly the parameter that needed it most.
            PathRoot::OneOfClasses { of, at, classes } => {
                let named = classes
                    .iter()
                    .filter_map(|class| self.class_of_target(*at, hir, *class))
                    .collect::<Vec<_>>();
                match self.resolve_path(*at, hir, of, budget.checked_sub(1)?) {
                    Some(held) if named.is_empty() => held,
                    Some(held) => self.of_one_of_classes(offset, hir, held, &named),
                    None if named.is_empty() => return None,
                    None => Self::one_of_classes(&named),
                }
            }
            // An `instanceof` guard ruled a class out. Resolved before the guard, where the
            // declaration still lists every alternative, and then that one is dropped.
            PathRoot::NotOfClass {
                of,
                at,
                class,
                exactly,
            } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                match self.class_of_target(*at, hir, *class) {
                    // Ruling out the class itself rules out everything that is one only where
                    // nothing extends it.
                    Some(named) if *exactly && !self.nothing_extends(named.as_str()) => held,
                    Some(named) => self.not_of_class(offset, hir, held, named.as_str()),
                    // The class the guard named does not resolve, so what it ruled out is unknown
                    // and the value is whatever it already was.
                    None => held,
                }
            }
            // A type predicate said which sort of value it is. What that leaves is decided by what
            // the declaration allowed, so the value is resolved before the guard and its union
            // operands are kept or dropped by kind.
            PathRoot::OfKind {
                of,
                at,
                kind,
                holds,
                settled,
            } => {
                let held = self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?;
                self.of_kind(offset, hir, held, *kind, *holds, *settled)
            }
            // `=== null` named null outright, so there is nothing to look up: whatever the
            // declaration said, here the value is null and only null.
            PathRoot::Null { strictly: true, .. } => Held::Builtin(phrost_model::BuiltinTy::Null),
            PathRoot::Null {
                of,
                at,
                strictly: false,
            } => self.falsy_part(
                offset,
                hir,
                &self.resolve_path(*at, hir, of, budget.checked_sub(1)?)?,
                reads_no_call(of),
            )?,
            PathRoot::This => self.this_held(offset)?,
            PathRoot::SelfClass => Held::Class(self.enclosing_class(offset)?),
            PathRoot::Parent => Held::Class(self.parent_of_enclosing_class(offset)?),
        };

        let declared = self.held_after_steps(offset, hir, path, budget, held);

        // An assignment *refines* what the slot declares and can never widen it. PHP checks a write
        // against the property's declared type, so at the read the declaration is a guarantee and
        // the write is only worth having where it is the more specific of the two:
        // `$this->node = $storage->load($id)` puts an `?EntityInterface` into a `?NodeInterface`
        // property, and reading the write as the answer took `get()` away from five real call
        // sites. Same rule as a docblock refining a return, and the same reason.
        if let Some(settled) = &settled
            && let Some(at) = settled.written_at
            // Where it was written, not where it was asked for: `$this->next = $this->next->after()`
            // reads its own subject, and reading it here would find this same assignment again.
            && let Some(written) = self.resolve_path(at, hir, &settled.value, budget - 1)
            && declared.as_ref().is_none_or(|declared| {
                self.fit_held_against_held(offset, hir, &written, declared)
                    == phrost_ty::compat::Fit::Accepted
            })
        {
            return Some(written);
        }

        // The write said nothing the declaration does not already allow, so the declaration stands
        // -- and it still takes its null off, because PHP checks a write against the declared type
        // and what got through is both. `$this->rows = JsonUtility::decode($text) ?? [];` into a
        // `?array<int, string>` property is the shape: `decode` is declared `mixed`, which is
        // broader than the declaration and so cannot replace it, while the `??` says the one thing
        // about it that matters here.
        match declared {
            Some(declared)
                if settled.as_ref().is_some_and(|settled| {
                    settled.written_at.is_some()
                        && self.cannot_be_null(offset, hir, &settled.value, budget)
                }) =>
            {
                self.without_null(declared)
            }
            declared => declared,
        }
    }

    /// Whether the value this path names cannot be null, whatever the type it resolves to says.
    ///
    /// The one question a `mixed` cannot answer about itself. [`Self::union_of`] folds a `mixed`
    /// operand into the whole answer deliberately -- PHP has no spelling for `?mixed`, and
    /// `$row[$key] ?? NULL` is written constantly -- so `decode($text) ?? []` resolves to `mixed`
    /// and the operator's own guarantee goes with it. Read off the *path* instead, where both
    /// constructs that make the guarantee still say so:
    ///
    /// - `A ?? B` chooses `B` exactly when `A` was null, so the value is null only if `B` is;
    /// - the join over a slot's writes is null only where one of its operands is, and its operands
    ///   are every path control could have taken -- the writes, and the one that skipped them.
    ///
    /// Anything else is asked of its own type, which is the answer everywhere the vocabulary can
    /// hold it. A path with steps is asked that way too: what a step off a non-null value reaches
    /// is the step's business.
    fn cannot_be_null(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
    ) -> bool {
        let Some(budget) = budget.checked_sub(1) else {
            return false;
        };
        if path.steps.is_empty() {
            match &path.root {
                // `??` and `?:` alike: one tests for null and the other for anything falsy, and a
                // null is falsy, so both choose the operand after it exactly when the one before
                // was null.
                PathRoot::EitherOf {
                    operands,
                    unless: phrost_body::Unless::Null | phrost_body::Unless::Falsy,
                } => {
                    if let Some(last) = operands.last() {
                        return self.cannot_be_null(offset, hir, &last.value, budget);
                    }
                }
                PathRoot::MaybeWritten {
                    writes,
                    read,
                    before,
                    otherwise,
                } => {
                    let every_write = writes
                        .iter()
                        .all(|write| self.cannot_be_null(offset, hir, &write.value, budget));
                    let skipped = matches!(otherwise, phrost_body::Otherwise::Impossible)
                        || self
                            .resolve_path(*before, hir, read, budget)
                            .and_then(|held| self.otherwise(offset, hir, held, otherwise, budget))
                            .is_some_and(|held| self.states_non_null(hir, &held));
                    return every_write && skipped;
                }
                _ => {}
            }
        }

        self.resolve_path(offset, hir, path, budget)
            .is_some_and(|held| self.states_non_null(hir, &held))
    }

    /// Whether the value this path names may be a `NULL` written in it, whatever else it may be.
    ///
    /// The dual of [`Self::cannot_be_null`], for the value whose type nothing resolves:
    /// `$arf = $this->data['arf'] ?? NULL; return $arf;` out of a `: string` is a `TypeError` on
    /// the run the key is missing, and it was silent because the element of a bare `array` is not
    /// known and the union with it is therefore not built. The `NULL` is not a guess about that
    /// element -- the author wrote it -- so the path says what the type cannot.
    ///
    /// Through the constructs that pass a written value on unchanged: `??` and `?:` and a ternary,
    /// the join over a name's writes, and a local's own binding. Anything else -- a guard's
    /// narrowing root among them, which is what `if (!$arf) { throw ...; }` leaves -- is `false`,
    /// which is the side that reports nothing.
    pub(crate) fn writes_a_null(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
    ) -> bool {
        let Some(budget) = budget.checked_sub(1) else {
            return false;
        };
        if path.names_null() {
            return true;
        }
        if !path.steps.is_empty() {
            return false;
        }
        match &path.root {
            // `A ?? B` and `A ?: B` replace a null `A` with `B`, so only the last operand's null
            // gets out -- `$a ?? $b` where `$a` fell back to `NULL` is `$b` exactly then. A
            // ternary hands either branch on as it is.
            PathRoot::EitherOf {
                operands,
                unless: phrost_body::Unless::Null | phrost_body::Unless::Falsy,
            } => operands
                .last()
                .is_some_and(|last| self.writes_a_null(last.at, hir, &last.value, budget)),
            PathRoot::EitherOf { operands, .. } => operands
                .iter()
                .any(|operand| self.writes_a_null(operand.at, hir, &operand.value, budget)),
            PathRoot::MaybeWritten {
                writes,
                read,
                before,
                otherwise,
            } => {
                // The path that skipped every write keeps what was there before -- but only a plain
                // `$x = NULL` is read through it. A fallback written in a condition may have been
                // tested by that very condition (`elseif (null === $ip = $cache[$h] ?? null)`), and
                // the arm that skipped the writes is then the one where it was not null; the guard
                // is invisible to a walk of the path, so the path is not asked.
                writes
                    .iter()
                    .filter(|write| !phrost_body::written_in_a_condition(hir, write.at))
                    .any(|write| self.writes_a_null(write.at, hir, &write.value, budget))
                    || (matches!(otherwise, phrost_body::Otherwise::Unproved)
                        && self.holds_a_written_null(*before, hir, read))
            }
            PathRoot::Local(name) => phrost_body::local_binding(
                hir,
                offset,
                name.as_str(),
                &|hir, expr| self.call_never_returns(offset, hir, expr),
                &self.asserting(),
            )
            .is_some_and(|bound| {
                let at = bound.written_at.unwrap_or(offset);
                self.writes_a_null(at, hir, &bound.value, budget)
            }),
            _ => false,
        }
    }

    /// Whether a local was last given the literal `NULL` itself -- `$best = NULL;` -- before `at`.
    fn holds_a_written_null(
        &self,
        at: TextSize,
        hir: &FileHir,
        read: &phrost_body::ValuePath,
    ) -> bool {
        let (PathRoot::Local(name), []) = (&read.root, read.steps.as_slice()) else {
            return read.names_null();
        };
        phrost_body::local_binding(
            hir,
            at,
            name.as_str(),
            &|hir, expr| self.call_never_returns(at, hir, expr),
            &self.asserting(),
        )
        .is_some_and(|bound| bound.value.names_null())
    }

    /// Whether a type says the value is not null, rather than merely failing to say it is.
    ///
    /// `mixed` is the one type that answers `Impossible` to the null question without meaning it,
    /// and the answer is deliberate: it is what keeps a `mixed` silent everywhere it is passed on,
    /// because PHP has no `?mixed` to distinguish. Read as a *claim* it turns that silence into an
    /// assertion about another type -- `$this->shown = apply_filters($hook, $show);` into a
    /// `?bool` property would have had its null taken off by a call declared to return anything at
    /// all. So the one type that cannot make the claim is named here.
    fn states_non_null(&self, hir: &FileHir, held: &Held) -> bool {
        self.kind_of_held(hir, held) != Some(phrost_model::BuiltinTy::Mixed)
            && self.null_reach_of(hir, held) == SentinelReach::Impossible
    }

    /// What a name held on the path that skipped every branch that wrote it.
    ///
    /// The guard that let control into those branches said what the value was *not*, so the path
    /// that skipped them is the path where it was: `if (!$x instanceof T) { $x = makeT(); }` leaves
    /// a `T` behind it whatever the declaration said, and `if (!$x) { $x = default(); }` leaves
    /// something that is not null. `None` where the claim and the value contradict each other --
    /// nothing but null, past a guard that ruled null out -- which is the whole answer for
    /// `$x = NULL; if (!$x) { $x = new Foo(); }`: the write is the only operand left.
    /// What a record literal names, when every one of its entries names a slot.
    ///
    /// `['id' => 1, 'when' => new DateTimeImmutable()]` is an `array{id: int, when: DateTimeImmutable}`
    /// and was a bare `array` until this existed -- which is the reason a value read back out of one
    /// had no type, and through that the reason a call on such a value was invisible to every check
    /// that asks what a receiver is.
    ///
    /// All or nothing on the *keys* and never on the values, which is the opposite of
    /// [`Self::list_literal_element`] and follows from what each claims. A list's element type is a
    /// union, so one silent entry makes it narrower than the truth; a record's field is a slot, so a
    /// silent entry is a key that is certainly there holding something unstated. `mixed` says exactly
    /// that, and it is the word `returned_shape` already writes into a docblock for the same entry.
    ///
    /// One element that is not a literal key ends it: a bare entry means the array is a list, a
    /// computed key names a slot this layer cannot identify, and a spread brings keys from somewhere
    /// else. Each is a reason the literal says less than a record would claim.
    fn record_literal_fields(
        &self,
        offset: TextSize,
        hir: &FileHir,
        expr: phrost_hir::ExprId,
        budget: u8,
    ) -> Option<Box<[(phrost_model::Name, Held)]>> {
        let phrost_hir::body::ExprData::ArrayLiteral { elements, .. } = hir.expr(expr) else {
            return None;
        };
        let items = hir.expr_list(*elements);
        if items.is_empty() || items.len() > LITERAL_ENTRIES {
            return None;
        }

        let mut fields: Vec<(phrost_model::Name, Held)> = Vec::with_capacity(items.len());
        // PHP's own counter. A bare entry takes the next index, and an integer key written out
        // moves the counter past it -- which is the whole of how `[1, 5 => 2, 3]` is keyed.
        let mut next = 0_usize;
        for &item in items {
            let (name, value) = match hir.expr(item) {
                phrost_hir::body::ExprData::ArrayElement {
                    kind: phrost_hir::body::ArrayElementKind::LiteralKey { name, .. },
                    value,
                    ..
                } => {
                    if let Ok(written) = name.as_str().parse::<usize>() {
                        next = written.saturating_add(1);
                    }
                    (name.clone(), *value)
                }
                // A spread contributes another array's entries in place. When what it spreads is
                // itself a record its arity is known, so the slots can be spliced in -- PHP
                // renumbers the integer keys from where the counter stands and keeps the string
                // ones, which is exactly what this does. `[$flag, ...$pair]` is how a caller adds
                // one field to a tuple a callee built, and reading it as a bare element type put
                // the `bool` among the operands of every slot.
                phrost_hir::body::ExprData::ArrayElement {
                    kind: phrost_hir::body::ArrayElementKind::Spread,
                    value,
                    ..
                } => {
                    let path = phrost_body::value_path_of(hir, *value)?;
                    let spread = self.resolve_path(offset, hir, &path, budget.checked_sub(1)?)?;
                    let Held::Record(inner) = spread else {
                        return None;
                    };
                    for (key, held) in inner {
                        let key = match key.as_str().parse::<usize>() {
                            Ok(_) => {
                                let renumbered = phrost_model::slot_name(next)?;
                                next += 1;
                                renumbered
                            }
                            Err(_) => key,
                        };
                        Self::put_in_slot(&mut fields, key, held);
                        if fields.len() > LITERAL_ENTRIES {
                            return None;
                        }
                    }
                    continue;
                }
                // A computed key names a slot this layer cannot identify, which makes every index
                // after it unknown -- so the literal keeps the bare element type it had.
                phrost_hir::body::ExprData::ArrayElement { .. } => return None,
                // A bare entry, which is what a list literal is made of. It is not wrapped at all --
                // see `ArrayElementKind`, which exists so that it is not -- so its slot is the
                // position, and the element *is* its own expression.
                _ => {
                    let name = phrost_model::slot_name(next)?;
                    next += 1;
                    (name, item)
                }
            };
            let held = phrost_body::value_path_of(hir, value)
                .and_then(|path| self.resolve_path(offset, hir, &path, budget.checked_sub(1)?))
                .unwrap_or(Held::Builtin(BuiltinTy::Mixed));
            Self::put_in_slot(&mut fields, name, held);
        }
        Some(fields.into_boxed_slice())
    }

    /// What every slot of a record holds, when they all hold the same thing.
    ///
    /// `None` for anything else, including a record whose slots disagree: flattening those into a
    /// union is the approximation [`Self::list_literal_element`] refuses for its own entries, and a
    /// spread is where it would otherwise come back.
    fn slots_that_agree(&self, offset: TextSize, hir: &FileHir, held: &Held) -> Option<Held> {
        let Held::Record(fields) = held else {
            return None;
        };
        let (_, first) = fields.first()?;
        fields
            .iter()
            .all(|(_, value)| value == first)
            .then(|| self.shape_values(offset, hir, held))
            .flatten()
    }

    /// Puts a value in a slot, replacing whatever was there.
    ///
    /// PHP keeps the last of a repeated key and so does this. A record holding one name twice would
    /// answer whichever the lookup reached first, which is not what runs.
    fn put_in_slot(
        fields: &mut Vec<(phrost_model::Name, Held)>,
        name: phrost_model::Name,
        held: Held,
    ) {
        match fields.iter_mut().find(|(named, _)| *named == name) {
            Some(slot) => slot.1 = held,
            None => fields.push((name, held)),
        }
    }

    /// What every entry of a list literal holds, when every one of them says.
    ///
    /// `[new Row(), new Row()]` is a `Row[]`, and `[1, 'x']` is an `int|string`. Bounded at
    /// [`LITERAL_ENTRIES`], because a framework's render array is a literal with hundreds of entries
    /// and this runs on the path a keystroke takes -- past the bound the answer is the bare `array`
    /// it always was.
    ///
    /// `None` the moment an entry answers nothing, for the reason the caller states: a union of the
    /// entries that did answer is narrower than the truth, and a type narrower than the truth reports
    /// correct code.
    fn list_literal_element(
        &self,
        offset: TextSize,
        hir: &FileHir,
        expr: phrost_hir::ExprId,
        budget: u8,
    ) -> Option<Held> {
        let phrost_hir::body::ExprData::ArrayLiteral { elements, .. } = hir.expr(expr) else {
            return None;
        };
        let items = hir.expr_list(*elements);
        if items.is_empty() || items.len() > LITERAL_ENTRIES {
            return None;
        }

        let mut held = Vec::with_capacity(items.len());
        for &item in items {
            // A `...$rest` contributes the *elements* of what it spreads rather than itself, so
            // `[...$rows]` on a `Row[]` is a `Row[]` and not an array of arrays. The one entry shape
            // that is not simply a value.
            let spread = match hir.expr(item) {
                phrost_hir::body::ExprData::ArrayElement {
                    kind: phrost_hir::body::ArrayElementKind::Spread,
                    value,
                    ..
                } => Some(*value),
                _ => None,
            };
            let path = phrost_body::value_path_of(hir, spread.unwrap_or(item))?;
            let resolved = self.resolve_path(offset, hir, &path, budget.checked_sub(1)?)?;
            let entry = match spread {
                // A record has no uniform element on purpose -- a keyed read that misses one slot
                // must stay a miss -- so what a spread of one contributes is asked the other way,
                // as every slot at once.
                //
                // Only where the slots **agree**, which is this rule's own refusal arriving one
                // level in. `[...$pair]` on an `array{0: Row, 1: Row}` is a `Row[]` and loses
                // nothing; on an `array{0: string, 1: array}` the union is what every entry of the
                // outer literal would have to be, and the positions that said otherwise are gone.
                // Laravel spreads exactly such a pair and destructures the result, and reading the
                // union put a `bool` among the operands of a slot holding an event name.
                Some(_) => match self.slots_that_agree(offset, hir, &resolved) {
                    Some(values) => values,
                    None => self.element_of_held(offset, hir, &resolved)?,
                },
                None => resolved,
            };
            // A `mixed` entry is the shape of not answering, so the literal says nothing about what
            // it holds -- `[1, $unknown]` reading as `mixed[]` costs a reader the doubt and says
            // nothing the bare `array` did not.
            //
            // Through `kind_of_held` and not by matching the arm, because a builtin arrives in three
            // spellings: a `mixed` parameter's type is a written reference into this file's arena or
            // an interned id, and only a cast produces the arm.
            if self.kind_of_held(hir, &entry) == Some(BuiltinTy::Mixed) {
                return None;
            }
            held.push(entry);
        }
        // The same reduction a join makes, and for the same reason: `[...$rows, new Row()]` reaches
        // here as an interned `Row` beside a built one, which would render `Row|Row`. Two helpers
        // already know how to fold an operand spelled twice -- and reaching for them rather than
        // comparing the arms is the difference between one rule and two that drift.
        Some(match held.len() {
            0 => return None,
            1 => held.remove(0),
            _ if self.every_operand_names_one_class(offset, hir, &held) => held.remove(0),
            _ => self.without_repeated_scalars(hir, Self::one_of(held)),
        })
    }

    fn otherwise(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        otherwise: &phrost_body::Otherwise,
        budget: u8,
    ) -> Option<Held> {
        match otherwise {
            phrost_body::Otherwise::Unproved => Some(held),
            // The caller does not ask about a path that does not exist, and answering anything here
            // would put the value from before the construct back among the operands.
            phrost_body::Otherwise::Impossible => None,
            // The same rule on the skipped path, through the same call `PathRoot::NotNull` makes for
            // the same reason: `array|false` from the standard library with an `if (!$row) { $row =
            // []; }` above it is an `array` after it.
            phrost_body::Otherwise::NotNull { refused } => {
                self.without_sentinels(offset, hir, held, *refused)
            }
            phrost_body::Otherwise::NotEmptyString => {
                Some(self.with_strings_holding(offset, hir, held, BuiltinTy::NonEmptyString))
            }
            phrost_body::Otherwise::NoneOfKinds { refused, kinds } => {
                let held = self.without_sentinels(offset, hir, held, *refused)?;
                Some(self.none_of_kinds(offset, hir, held, kinds))
            }
            // The guard let control into the write because the value was of this kind, so on the
            // path that skipped it the value is everything else the declaration listed.
            phrost_body::Otherwise::NotOfKind(kind) => {
                Some(self.of_kind(offset, hir, held, *kind, false, true))
            }
            // The path around a write guarded by `$x !== null` is where the value was null -- and
            // where it could not have been, there is no such path. Asked before the general kind,
            // which answers a contradiction with the kind: right for a guard read where the value is
            // used, and wrong here, where it would join a null to a `string` the signature already
            // promised. `if ($s !== null) { $s = trim($s); }` on a `string $s` is defensive code,
            // not a path.
            phrost_body::Otherwise::OfKind {
                kind: BuiltinTy::Null,
                ..
            } => match self.null_reach_of(hir, &held) {
                SentinelReach::Impossible => None,
                SentinelReach::Possible | SentinelReach::Certain => {
                    Some(Held::Builtin(BuiltinTy::Null))
                }
            },
            // And the mirror: the branch ran where the value was *not* of this kind, so around it
            // the value is of it. Sound because a type predicate has no third answer.
            phrost_body::Otherwise::OfKind { kind, settled } => {
                Some(self.of_kind(offset, hir, held, *kind, true, *settled))
            }
            phrost_body::Otherwise::NotOfClass(class) => {
                Some(match self.class_of_target(offset, hir, *class) {
                    Some(named) => self.not_of_class(offset, hir, held, named.as_str()),
                    // The class the guard named does not resolve, so what it ruled out is unknown.
                    None => held,
                })
            }
            phrost_body::Otherwise::OfClass(class) => {
                Some(match self.class_of_target(offset, hir, *class) {
                    Some(named) => self.also_of_class(offset, hir, held, &named),
                    // The class the guard named does not resolve, so what it proved is unknown and
                    // the value is whatever it already was.
                    None => held,
                })
            }
            // A claim written on the callee's docblock rather than in this vocabulary, so it
            // arrives as the path a reader standing in the branch that was not taken would have
            // resolved. What it points at is the same value `held` was read from, with the tag on
            // top -- so it replaces rather than narrows, and falls back to `held` where the callee
            // carries no tag after all.
            phrost_body::Otherwise::Asserted(path) => Some(match budget.checked_sub(1) {
                Some(budget) => self.resolve_path(offset, hir, path, budget).unwrap_or(held),
                None => held,
            }),
        }
    }

    /// A value with the empty array taken out of it, or `None` when that is all it was.
    fn without_empty_array(held: Held) -> Option<Held> {
        let empty = |held: &Held| matches!(held, Held::Record(fields) if fields.is_empty());
        match held {
            held if empty(&held) => None,
            Held::OneOf(operands) => {
                let kept = operands
                    .into_vec()
                    .into_iter()
                    .filter(|held| !empty(held))
                    .collect::<Vec<_>>();
                (!kept.is_empty()).then(|| Self::one_of(kept))
            }
            held => Some(held),
        }
    }

    /// Whether an alternative is a guard's `is_null` on a value that cannot be null.
    ///
    /// Alone, such a guard is the answer -- it is about this position, the declaration about every
    /// one. Beside others it is an alternative that never held: `is_null($name) ||
    /// isset($config[$name])` on a string is the second one, and keeping the first put a null in a
    /// union of what the value may be.
    fn null_that_cannot_hold(
        &self,
        hir: &FileHir,
        operand: &phrost_body::ValuePath,
        budget: u8,
    ) -> bool {
        let PathRoot::Null {
            of,
            at,
            strictly: true,
        } = &operand.root
        else {
            return false;
        };
        operand.steps.is_empty()
            && self.resolve_path(*at, hir, of, budget).is_some_and(|held| {
                self.null_reach_of(hir, &held) == crate::SentinelReach::Impossible
            })
    }

    /// One value out of several, reduced to what can honestly be reached through it.
    ///
    /// A union, so only what every operand agrees on is there. Shared by the two roots that produce
    /// one -- an operator that chose between values written side by side, and a name several
    /// branches may have written -- because the reductions are about the *types*, not about which
    /// shape wrote them: a `mixed` swallows the rest either way, and two operands naming one class
    /// are one class either way.
    ///
    /// `unless` says what ruled an operand out for the next one to have been chosen -- see
    /// [`phrost_body::Unless`], which is where the difference between `??` and `?:` lives.
    pub(crate) fn union_of(
        &self,
        offset: TextSize,
        hir: &FileHir,
        mut resolved: Vec<Held>,
        unless: phrost_body::Unless,
    ) -> Option<Held> {
        // `mixed` is already every type, null included, and PHP has no spelling for `?mixed`.
        // `$data[$key] ?? NULL` on an `array<string, mixed>` is written constantly, and a union
        // built out of it would make every argument the value is passed on to read as possibly
        // null.
        if let Some(index) = resolved
            .iter()
            .position(|held| self.kind_of_held(hir, held) == Some(BuiltinTy::Mixed))
        {
            return Some(resolved.swap_remove(index));
        }

        // What chose the next operand cannot have survived in the one it skipped -- so keeping it
        // there would make the one thing the operator was written to rule out read as still
        // possible. `match` rules out nothing, and an arm that resolves to null stays exactly
        // that: a `match` arm is not a fallback for the ones after it.
        let last = resolved.len().checked_sub(1)?;
        let mut narrowed = Vec::with_capacity(resolved.len());
        for (index, held) in resolved.into_iter().enumerate() {
            if unless == phrost_body::Unless::Nothing || index == last {
                narrowed.push(held);
                continue;
            }
            // Believed to be nothing but null, and not the operand that is always kept -- so it
            // drops out rather than joining the union as a type nobody could observe.
            let Some(held) = self.without_null(held) else {
                continue;
            };
            narrowed.push(match unless {
                // `?:` tested truthiness, so the `false` went with the null -- and it is the
                // half that matters in practice, because `false` is how the standard library
                // reports "nothing": `tempnam(...) ?: NULL` is a `?string`, never a
                // `string|false|null`. Through the same call `$x !== FALSE` narrows by. And an
                // empty array, which is never truthy either.
                phrost_body::Unless::Falsy => {
                    let Some(held) = Self::without_empty_array(self.of_kind(
                        offset,
                        hir,
                        held,
                        phrost_model::BuiltinTy::False,
                        false,
                        true,
                    )) else {
                        continue;
                    };
                    // And a string it hands on is one that was not falsy: `$name ?: 'Guest'`.
                    self.with_strings_holding(
                        offset,
                        hir,
                        held,
                        phrost_model::BuiltinTy::NonFalsyString,
                    )
                }
                phrost_body::Unless::Null | phrost_body::Unless::Nothing => held,
            });
        }
        // `$rows = $thing->rows() ?? [];` unions what the call declares with what the literal is,
        // and an empty literal resolves to a bare `array` -- so the element type was stated on one
        // side and absent on the other, and iterating the union yielded nothing at all. 65
        // occurrences of `) ?? []` in one real project, every `foreach` over the result untyped.
        //
        // The bare side is not an alternative here: an empty array holds nothing, so it contributes
        // no element and the other operand is the whole of what can come out. Only against an
        // operand that *states* its element -- a bare `array` beside a `string` is a real
        // alternative and stays.
        let states_an_element = |held: &Held| self.element_of_held(offset, hir, held).is_some();
        if narrowed.len() > 1 && narrowed.iter().any(states_an_element) {
            narrowed.retain(|held| {
                !matches!(self.kind_of_held(hir, held), Some(BuiltinTy::Array))
                    || states_an_element(held)
            });
        }

        Some(match narrowed.len() {
            1 => narrowed.remove(0),
            // Every operand naming the same one class is not a choice the code made:
            // `$row = load() ?? new Row()` holds a `Row`, and calling it `Row|Row` would render a
            // union nobody wrote.
            //
            // The operand kept is a *nullable* one where the operands disagree about the null,
            // because naming one class is a claim about the class and not about whether the value
            // is there. `$row = $this->find(); if ($c) { $row = new Row(); }` joins a `?Row` with a
            // `Row` and holds a `?Row` -- reducing to the write dropped the null on the path the
            // branch did not take, which is the whole of what the join was asked. `??` and `?:`
            // are unaffected: their left operand had its null taken off by `unless` above, so both
            // operands are already non-null by the time this runs.
            _ if self.every_operand_names_one_class(offset, hir, &narrowed) => {
                let keep = narrowed
                    .iter()
                    .position(|held| self.takes_a_null_off(held))
                    .unwrap_or(0);
                narrowed.remove(keep)
            }
            _ => self.without_repeated_scalars(hir, Self::one_of(narrowed)),
        })
    }

    /// Drops an operand that is a scalar another operand already named.
    ///
    /// The scalar half of the rule above it, and a join is where both are needed: reading each arm
    /// where it was written makes the arms *agree* far more often than they used to.
    /// `is_array($v) ? $v[1] : $v` on a `string|array<int, string>` leaves a `string` on either
    /// side, and `string|string` is a union nobody wrote -- it renders in a diagnostic as though
    /// the code offered a choice, and every reader that takes a union apart pays for an operand
    /// that adds nothing.
    ///
    /// After the join is built rather than over the arms, because an arm can be a union in its own
    /// right and its operands only become comparable once they are operands. `$alter` written in
    /// three `switch` arms -- two string literals and a `str_replace`, which declares
    /// `string|array` -- reads as `string|string|array` for exactly as long as the third arm is
    /// still one value.
    ///
    /// Scalars only, because they are the operands nothing distinguishes once the name matches.
    /// Two classes of one name can still differ in what they were applied to, and two containers
    /// of one kind differ in what they hold -- so those are left to the class rule, which asks the
    /// catalog rather than the spelling.
    fn without_repeated_scalars(&self, hir: &FileHir, held: Held) -> Held {
        match held {
            // The null a join folded out sits outside the operands, so the union to look in is one
            // level down.
            Held::Nullable(inner) => Self::nullable(self.without_repeated_scalars(hir, *inner)),
            Held::OneOf(operands) => {
                let mut seen: Vec<BuiltinTy> = Vec::new();
                // An empty array beside another array is one of its values: `array{}|array` is the
                // `array` a falsy guard on one branch and nothing on the other leave.
                // The kinds are asked only of a union that holds an empty array at all, which is
                // nearly none of them: this runs for every union a join or an operator builds.
                let empty = |held: &Held| matches!(held, Held::Record(fields) if fields.is_empty());
                let empty_beside_an_array = operands.iter().any(empty)
                    && operands.iter().any(|held| {
                        !empty(held) && self.kind_of_held(hir, held) == Some(BuiltinTy::Array)
                    });
                // Two strings are one, holding what both are known to: `''` written in one arm and
                // `'name'` in another join to a `string`, whichever spelling each arrived in. Where
                // the first of them stood, so a union still reads in the order it was written.
                let strings = operands
                    .iter()
                    .filter_map(|held| self.scalar_named(hir, held))
                    .filter(|scalar| scalar.is_string())
                    .collect::<Vec<_>>();
                let string = match strings.len() > 1 {
                    true => strings
                        .iter()
                        .copied()
                        .reduce(|seen, one| seen.string_join(one).unwrap_or(seen)),
                    false => None,
                };
                let kept = operands
                    .into_vec()
                    .into_iter()
                    .filter(|held| {
                        !(empty_beside_an_array
                            && matches!(held, Held::Record(fields) if fields.is_empty()))
                    })
                    .filter_map(|held| match (self.scalar_named(hir, &held), string) {
                        (Some(scalar), Some(joined)) if scalar.is_string() => {
                            match seen.contains(&joined) {
                                true => None,
                                false => {
                                    seen.push(joined);
                                    Some(match scalar == joined {
                                        true => held,
                                        false => Held::Builtin(joined),
                                    })
                                }
                            }
                        }
                        (Some(scalar), _) if seen.contains(&scalar) => None,
                        (Some(scalar), _) => {
                            seen.push(scalar);
                            Some(held)
                        }
                        (None, _) => Some(held),
                    })
                    .collect();
                Self::one_of(kept)
            }
            held => held,
        }
    }

    /// The builtin a held type is *exactly*, and `None` for anything with more to it.
    ///
    /// Not [`kind_of_held`](IndexView::kind_of_held), which answers what a type predicate would
    /// see: there a class is an `object` and a `list<Foo>` is an `array`, which is right for
    /// `is_object` and wrong here -- a `list<Foo>` and a `list<Bar>` would fold into one. Only the
    /// three spellings of a bare builtin answer, because they are the same type written three
    /// ways: a cast states one outright, a declaration in this file writes one into its arena, and
    /// the index interns one.
    pub(crate) fn scalar_named(&self, hir: &FileHir, held: &Held) -> Option<BuiltinTy> {
        match held {
            Held::Builtin(builtin) => Some(*builtin),
            Held::Written(id) => match hir.type_ref(*id) {
                phrost_hir::TypeRefData::Builtin(builtin) => Some(*builtin),
                _ => None,
            },
            Held::Stored { ty, .. } => match self.skeleton.types().get(*ty) {
                phrost_ty::TypeRef::Builtin(builtin) => Some(*builtin),
                _ => None,
            },
            // A fourth spelling of `array`, and the note above about three was written before it
            // existed. `['a' => 1] + ['b' => 2]` is two records and an array, and without this arm
            // the operator answered nothing where it used to answer `array` -- which is the whole
            // hazard of adding a spelling: the helper that knows them all has to learn it too.
            Held::Record(_) => Some(BuiltinTy::Array),
            _ => None,
        }
    }

    /// Whether every one of several held types names the same single class.
    ///
    /// The N-ary form of [`name_one_class`](Self::name_one_class), asked where a whole union's
    /// operands have to be told apart from a single choice the code made: two arms of a `match`
    /// naming one class are not a choice between two things, and three arms doing the same are
    /// not a choice between three.
    fn every_operand_names_one_class(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &[Held],
    ) -> bool {
        let Some((first, rest)) = held.split_first() else {
            return false;
        };
        rest.iter()
            .all(|other| self.name_one_class(offset, hir, first, other))
    }
}

impl IndexView<'_> {
    /// What the declaration this method overrides says about the parameter at `position`.
    ///
    /// The answer to [`PathRoot::InheritedParameter`]. An override with no type of its own and
    /// `{@inheritdoc}` above it is saying the parent's word is its own, and on a framework the
    /// parent is where every type is written: `preRender(&$values)` against
    /// `FieldPluginBase::preRender(ResultRow[] $values)` is 188 of the reference project's 202
    /// untyped parameters in one shape.
    ///
    /// By position and not by name, because that is how PHP matches a call to a signature and how
    /// an override is checked -- a parent free to name its parameter differently still speaks about
    /// the same slot.
    ///
    /// Strictly a lower bound, since PHP is contravariant on inputs and an override may accept
    /// more. What keeps that safe is where it is read: this answers what a *receiver* holds, which
    /// is the question an editor asks, and a declaration that widens writes a type rather than
    /// leaving the slot empty.
    fn inherited_parameter(&self, offset: TextSize, position: u16) -> Option<Held> {
        let class = self.enclosing_class(offset)?;
        let method = self.enclosing_method(offset)?;
        // Climbing rather than one step, because the nearest declaration above usually has no type
        // either: Drupal's `FieldPluginBase::preRender(&$values)` carries `{@inheritdoc}` and the
        // `@param \Drupal\views\ResultRow[]` is on `FieldHandlerInterface`, two levels up. A
        // parent that says nothing is not an answer, it is the same question again.
        let mut above = class.clone();
        let mut owner = None;
        let mut ty = None;
        for _ in 0..crate::symbols::INHERIT_DEPTH {
            let Some((declarer, parent)) = self.overridden(above.as_str(), method.as_str()) else {
                break;
            };
            let found = parent
                .params
                .get(usize::from(position))
                .and_then(|row| row.effective_ty(self.skeleton.types()));
            above = declarer.clone();
            if found.is_some() {
                owner = Some(declarer);
                ty = found;
                break;
            }
        }
        let (owner, ty) = (owner?, ty?);
        // `self` in the parent's `@param` means the class that wrote it, and `static` the one the
        // call was made on -- which here is the class doing the overriding.
        let scope = crate::resolve::RelativeScope {
            declaring: owner.clone(),
            called: class.clone(),
        };
        // And a template in it is the declarer's, filled in by whatever lies between the two.
        match self.declared_through(class.as_str(), owner.as_str(), ty, &[], &scope) {
            crate::symbols::Declared::Held(held) => Some(held),
            crate::symbols::Declared::Type { ty, nullable } => {
                let held = Held::Stored {
                    ty,
                    scope: Some(scope),
                };
                Some(match nullable {
                    true => Self::nullable(held),
                    false => held,
                })
            }
            crate::symbols::Declared::Unbound => None,
        }
    }

    /// The name of the method `offset` sits in, if it sits in one.
    ///
    /// The innermost by range, so a position inside a closure written in a method still answers
    /// with the method -- which is what every caller here wants, since a closure has no declaration
    /// of its own to override.
    fn enclosing_method(&self, offset: TextSize) -> Option<Name> {
        let hir = self.current()?.hir();
        hir.classes()
            .flat_map(|(_, class)| hir.member_list(class.members))
            .map(|&id| hir.member(id))
            .filter(|member| member.full_range.contains_inclusive(offset))
            .min_by_key(|member| member.full_range.len())
            .map(|member| member.name.clone())
    }
}

/// Whether a path reaches its value through no call: a local, `$this`, and the properties and
/// elements read off them.
///
/// What a falsy guard on such a value proves is about *that* value. One on a call is read as a slot
/// -- `if ($this->find()) { $this->find()->run(); }` -- which is right for a getter and stretches to
/// a second call the step cannot tell apart, `get(SOME_CONSTANT)` beside `get(OTHER)`. Saying the
/// first was empty is then a claim about the second, so only a value read without a call is the
/// empty array its guard proves.
fn reads_no_call(path: &phrost_body::ValuePath) -> bool {
    let mut path = path;
    while let Some((of, _)) = path.root.narrows() {
        if !path.steps.is_empty() {
            return false;
        }
        path = of;
    }
    matches!(path.root, PathRoot::Local(_) | PathRoot::This)
        && path.steps.iter().all(|step| {
            matches!(
                step,
                phrost_body::PathStep::Property(_) | phrost_body::PathStep::Element(_)
            )
        })
}

/// The path's root with no steps taken off it.
fn root_alone(path: &phrost_body::ValuePath) -> phrost_body::ValuePath {
    phrost_body::ValuePath {
        root: path.root.clone(),
        steps: Vec::new(),
        may_be_absent: false,
    }
}
