//! Everything a path does after its root: each step applied in turn, and what refines the result.
//!
//! Separate from [`super::path`] because the two grow for different reasons. A new kind of root is
//! a new arm in the match there; a new kind of step -- and the rule for what reaching through it
//! holds -- is here, along with the two claims that are applied once the last step has been taken.

use phrost_body::{PathRoot, PathStep};
use phrost_hir::FileHir;
use phrost_model::{Name, TextSize};
use phrost_sema::MemberKind;

use crate::{references::SentinelReach, resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What a member reached through `held` holds, on whichever class the value has it from.
    ///
    /// The single class first, which is every ordinary value and costs exactly what it did before.
    /// An intersection reduces to no class at all -- `Application&MockObject` is both at once -- so
    /// its operands are tried in the order written, and the first that declares the member answers.
    /// That is the semantics of `&`: the member exists if any operand has it.
    pub(crate) fn member_through(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        kind: MemberKind,
        name: &str,
        called_on: Option<Name>,
    ) -> Option<Held> {
        // A property an object shape in the value names, which no class's table holds: the
        // `->pivot` of a model `BelongsToMany` loaded is `object{pivot: TPivotModel}` beside it.
        if kind == MemberKind::Property
            && let Some(property) = self.object_shape_property(hir, held, name)
        {
            return Some(property);
        }
        // What the value itself was written with, which is the most specific thing anyone knows
        // about its templates -- more specific than its class's `@extends` and than a bound.
        let arguments = self.arguments_of_held(hir, held);
        // Through `owner_of_member`, which is where "which class of this value owns this name"
        // already lives. This had two of that ladder's four rungs -- a single class and an
        // intersection -- and so was blind to a union whose operands share no ancestor, which is
        // what `@var Thing|MockObject` is: the member *resolved* for goto, because that answer came
        // off the full ladder, and the chain died at the next hop because this one reduced the
        // union to a common ancestor, found none, and gave up. Every PHPUnit mock chain in every
        // project -- `$mock->expects($this->any())->method('x')->willReturn($y)` -- stopped after
        // the first call, and it was 11 of the 17 sites a comparison against intelephense found.
        let class = self.owner_of_member(offset, hir, held, kind, name)?;
        // A member PHP would not let this scope touch is not what runs: the class's `__get`/`__call`
        // does, and what that hands back is not what the declaration says. See `MemberReach`.
        if self.reaches_through_magic(offset, class.as_str(), kind, name) {
            return match kind {
                MemberKind::Method => self.marked_method_held(class.as_str(), name),
                _ => None,
            };
        }
        self.member_held_on(class.as_str(), kind, name, called_on, &arguments)
    }

    /// The branch of a method's declared union that an earlier call on the same receiver settles.
    ///
    /// A query builder's shape, and the reason it needs the chain rather than the signature:
    /// `execute()` on a Drupal entity query declares `int|array` because `count()` decides which,
    /// and Laravel's builder does the same with different names. The union is a true statement
    /// about the method and a useless one about a call, so a checker believing it reports the
    /// branch nobody took.
    ///
    /// Only the receiver's own chain is read, so `->count()->execute()` is settled and
    /// `$q->count(); $q->execute();` is not -- the first is how both frameworks document it, and
    /// the second falls back to the branch a plain call gets, which is the commoner answer anyway.
    fn settled_by_chain(
        &self,
        offset: TextSize,
        hir: &FileHir,
        receiver: &Held,
        earlier: &[PathStep],
        method: &str,
    ) -> Option<phrost_model::BuiltinTy> {
        let contracts = self.skeleton.returns().chained_branches(method);
        if contracts.is_empty() {
            return None;
        }
        // The hierarchy is asked last, for the reason the catalog bindings give: it is a climb, and
        // the name test above has already settled almost every call.
        let class = self.class_of_held(offset, hir, receiver)?;
        let contract = contracts.iter().find(|contract| {
            class.as_str().eq_ignore_ascii_case(&contract.class)
                || self.descends_from(
                    class.as_str(),
                    &contract.class,
                    crate::resolve::HIERARCHY_DEPTH,
                )
        })?;

        let switched = earlier.iter().any(|step| {
            matches!(step, PathStep::MethodReturn { name, .. }
                if name.as_str().eq_ignore_ascii_case(&contract.after))
        });
        Some(match switched {
            true => contract.then,
            false => contract.otherwise,
        })
    }

    /// Which operand of a collection-or-element union an earlier call settles: the many class, and
    /// whether the chain counted. `None` where no contract speaks, where the chain does not start
    /// the factory itself, or where a count was handed over as a value -- `factory($n)` may be
    /// either, and the union is then the honest answer.
    fn operand_settled_by_chain(
        &self,
        offset: TextSize,
        hir: &FileHir,
        receiver: &Held,
        earlier: &[PathStep],
        method: &str,
    ) -> Option<(Box<str>, bool)> {
        let contracts = self.skeleton.returns().chained_operands(method);
        if contracts.is_empty() {
            return None;
        }
        let class = self.class_of_held(offset, hir, receiver)?;
        let contract = contracts.iter().find(|contract| {
            class.as_str().eq_ignore_ascii_case(&contract.class)
                || self.descends_from(
                    class.as_str(),
                    &contract.class,
                    crate::resolve::HIERARCHY_DEPTH,
                )
        })?;
        // Only a chain that starts the factory itself shows every call that could have counted it.
        // One reached through a variable or `$this` carries a count set somewhere else --
        // `$factory = User::factory()->count(3); $factory->create();` -- so it keeps the union.
        let mut started = false;
        let mut counted = false;
        for step in earlier {
            let PathStep::MethodReturn { name, call, .. } = step else {
                continue;
            };
            let named = |names: &[Box<str>]| {
                names
                    .iter()
                    .any(|wanted| name.as_str().eq_ignore_ascii_case(wanted))
            };
            if named(&contract.after) {
                counted = true;
            } else if named(&contract.counted_by) {
                started = true;
                match Self::count_written(hir, call.expr()?)? {
                    Some(true) => counted = true,
                    Some(false) => {}
                    None => return None,
                }
            }
        }
        started.then(|| (contract.many.clone(), counted))
    }

    /// Whether a call was handed a count: `Some(true)` for an integer literal, `Some(false)` for
    /// none or a literal `null`, and `None` for a value nothing here can read.
    fn count_written(hir: &FileHir, call: phrost_hir::ExprId) -> Option<Option<bool>> {
        use phrost_hir::body::{ExprData, LiteralKind};
        let args = match hir.expr(call) {
            ExprData::StaticCall { args, .. } | ExprData::MethodCall { args, .. } => *args,
            _ => return None,
        };
        let Some(&first) = hir.expr_list(args).first() else {
            return Some(Some(false));
        };
        let value = match hir.expr(first) {
            ExprData::Argument { value, .. } => *value,
            _ => first,
        };
        Some(match hir.expr(value) {
            ExprData::Literal {
                kind: LiteralKind::Int,
                ..
            } => Some(true),
            ExprData::Literal {
                kind: LiteralKind::Null,
                ..
            } => Some(false),
            _ => None,
        })
    }

    /// What was written into the slot a path's prefix ends at, or into the whole path.
    ///
    /// Two answers, because a write describes exactly one path and no prefix of itself. A container
    /// that is itself a slot needs the prefix -- `$sums[$id] = $this->shape()` describes `$sums[$id]`
    /// -- and a slot several levels down needs the whole thing, because
    /// `$return['data'][$i]['earnings'] = $price` says nothing at all about `$return['data']`.
    ///
    /// The prefix first: it is the one that composes with the steps still to walk.
    fn slot_written(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        index: usize,
        budget: u8,
    ) -> Option<Written> {
        let prefix = phrost_body::ValuePath {
            root: path.root.clone(),
            steps: path.steps[..index].to_vec(),
            // A prefix is inside the chain, and PHP short-circuits the rest of one rather than
            // reaching through a null -- so what the whole expression may be says nothing here.
            may_be_absent: false,
        };
        // Resolved where the write is, not where the slot is read: the value means what it meant
        // there, and a guard that held at the write may not hold at the read -- see `ElementWrite`.
        if let Some(written) =
            phrost_body::element_written(hir, offset, &prefix, named_key(&path.steps[index]))
            && let Some(slot) =
                self.resolve_path(written.at, hir, &written.value, budget.checked_sub(1)?)
        {
            return Some(Written::Slot(slot));
        }

        // Already the last step, so the whole path *is* the prefix and asking again asks the same
        // question.
        if index + 1 == path.steps.len() {
            return None;
        }
        // And only where that last step reads an element: what was written into a slot of the
        // container is what an element of it holds, and not its key, nor a member of it. Asked of a
        // `foreach` key, it answered with whichever value was last written in -- so after
        // `$info['audio']['bitrate_distribution'] = $dist`, every key of `$info['audio']` was an array.
        let (last, before) = path.steps.split_last()?;
        if !matches!(last, phrost_body::PathStep::Element(_)) {
            return None;
        }
        let whole = phrost_body::ValuePath {
            root: path.root.clone(),
            steps: before.to_vec(),
            may_be_absent: false,
        };
        let written = phrost_body::element_written(hir, offset, &whole, named_key(last))?;
        self.resolve_path(written.at, hir, &written.value, budget.checked_sub(1)?)
            .map(Written::Whole)
    }

    /// The type a path ends at once every step has been applied to what its root held.
    ///
    /// Split from [`resolve_path`](IndexView::resolve_path) rather than inlined there: the root is
    /// one question and the steps are another, and the `@assert` refinement at the end belongs to
    /// this one because a step is what makes a value vague enough for it to be asked.
    pub(crate) fn held_after_steps(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
        mut held: Held,
    ) -> Option<Held> {
        // What `static` means for a member reached off `parent::`, which is not the parent.
        //
        // `parent::`, `self::` and `static::` are PHP's *forwarding* calls: they leave late static
        // binding alone, so `parent::create(): static` called from `Child` returns a `Child`. Only an
        // explicit `Foo::create()` resets it. `self::` and `static::` need nothing here because the
        // class they look up on is already the enclosing one; `parent::` is the single spelling where
        // the two differ.
        //
        // It applies to the first step and no further: the forwarding is a property of the call
        // written on `parent::`, not of whatever that call returned.
        //
        // Getting this wrong reports every property a Drupal factory sets --
        // `$instance = parent::create($container); $instance->thing = ...` is in every form, plugin
        // and controller there is -- as undefined on the base class.
        let mut forwarding = match &path.root {
            PathRoot::Parent => self.enclosing_class(offset),
            _ => None,
        };

        for (index, step) in path.steps.iter().enumerate() {
            // Asked while `held` is still the *receiver*, which is the only moment both it and the
            // steps before this one are in hand. Gated on the method name first -- a scan of the
            // handful of short strings a framework contributes -- so every other call pays that and
            // nothing more.
            let chained = match step {
                PathStep::MethodReturn { name, .. } => {
                    self.settled_by_chain(offset, hir, &held, &path.steps[..index], name.as_str())
                }
                _ => None,
            };
            let picked = match step {
                PathStep::MethodReturn { name, .. } => self.operand_settled_by_chain(
                    offset,
                    hir,
                    &held,
                    &path.steps[..index],
                    name.as_str(),
                ),
                _ => None,
            };

            held = match step {
                PathStep::Property(name) => {
                    // A tabled read first, because it answers a question the declaration cannot:
                    // `->value` on a Drupal field list is whatever the field's own kind holds, and
                    // the interface declares nothing at all. Gated on any framework having said so,
                    // which is an emptiness test -- the lookback below it is a walk.
                    let tabled = self
                        .has_tabled_reads()
                        .then(|| self.class_of_held(offset, hir, &held))
                        .flatten()
                        .and_then(|class| {
                            let produced_by = self.producing_call(offset, hir, path, index);
                            self.tabled_property(
                                class.as_str(),
                                name.as_str(),
                                produced_by.as_ref().map(|(method, key)| (method, key)),
                            )
                        });
                    match tabled {
                        Some(tabled) => {
                            forwarding = None;
                            tabled
                        }
                        None => self.member_through(
                            offset,
                            hir,
                            &held,
                            MemberKind::Property,
                            name.as_str(),
                            forwarding.take(),
                        )?,
                    }
                }
                PathStep::MethodReturn {
                    name,
                    arguments,
                    call,
                } => 'method: {
                    // Which class of this value owns this name, through the one place that
                    // answers it. This had two rungs of that ladder -- a single class, and an
                    // intersection's first declaring operand -- and so was blind to a union whose
                    // operands share no ancestor, which is exactly what `@var Thing|MockObject` is.
                    // The member itself still resolved, because goto asks the full ladder; the
                    // *chain* died here, so every mock chain stopped after one call.
                    let class = self.owner_of_member(
                        offset,
                        hir,
                        &held,
                        MemberKind::Method,
                        name.as_str(),
                    )?;
                    // The same exemption the property arm makes, for the same reason: Laravel's
                    // `Router::prefix()` is protected and its `__call` forwards to a registrar, so
                    // reading the declaration answered `string` and reported the next call in the
                    // chain as one made on it.
                    if self.reaches_through_magic(
                        offset,
                        class.as_str(),
                        MemberKind::Method,
                        name.as_str(),
                    ) {
                        // Unless a framework says what the magic method makes of a marked one --
                        // the step's value, so the chain after it goes on.
                        forwarding = None;
                        break 'method self.marked_method_held(class.as_str(), name.as_str())?;
                    }
                    let called_on = forwarding.take();
                    // What the receiver itself was written with: `Collection<Item>` says what this
                    // one holds, and its class's `@extends` says only what every one of them does.
                    // Read once and handed to both readers -- the contract needs it for a `@return`
                    // operand the class supplies, and the declaration reader for the same reason.
                    let supplied = self.arguments_of_held(hir, &held);
                    let facts = crate::symbols::CallFacts {
                        offset,
                        hir,
                        literals: arguments,
                        call: *call,
                        budget,
                        receiver: &supplied,
                        // The call is an expression here, so its own arguments are read.
                        written: None,
                    };
                    let bound = self
                        .returned_from_argument(
                            class.as_str(),
                            name.as_str(),
                            // A catalog contract is about the first argument, so a literal
                            // anywhere else is not the one it means.
                            arguments
                                .iter()
                                .find(|argument| argument.position == 0)
                                .map(|argument| argument.value.as_str()),
                        )
                        .or_else(|| self.returned_by_key(class.as_str(), name.as_str()))
                        .or_else(|| self.returned_named_after(class.as_str(), name.as_str()))
                        // A union the call's own shape settles, which is the one contract here that
                        // needs no argument to have been written -- and cannot race the two that do,
                        // because it fires only when none was.
                        .or_else(|| {
                            self.returned_by_default(class.as_str(), name.as_str(), || {
                                facts.arity()
                            })
                        })
                        .or_else(|| {
                            self.returned_class_named_by_argument(
                                offset,
                                class.as_str(),
                                name.as_str(),
                                arguments,
                            )
                        })
                        // Last of the four, because a branch is allowed to say nothing: Drupal's
                        // container is declared `@return ($id is class-string<T> ? T : object)`, and
                        // a service id names no class -- so the honest branch is `object`, which
                        // must not stand in front of the catalog that knows which class that id is.
                        .or_else(|| {
                            self.returned_by_method_contract(
                                class.as_str(),
                                name.as_str(),
                                called_on.clone(),
                                &facts,
                            )
                        });
                    let reached = match bound {
                        Some(bound) => bound,
                        // The declaration is read once and used twice: a forwarded contract replaces
                        // the class in it and keeps its shape, so both answers come from here.
                        None => {
                            let declared = self.member_held_on(
                                class.as_str(),
                                MemberKind::Method,
                                name.as_str(),
                                called_on,
                                &supplied,
                            );
                            // The lookback is asked for only when some framework contracted this
                            // method name, which is a scan of a handful of short strings. Every
                            // other call -- which is nearly all of them -- pays that and nothing.
                            let produced_by = (!self
                                .skeleton
                                .returns()
                                .forwarded_bindings(name.as_str())
                                .is_empty()
                                || self.has_tabled_reads())
                            .then(|| self.producing_call(offset, hir, path, index))
                            .flatten();
                            let by_key = produced_by.as_ref().map(|(m, key)| (m, key));
                            self.returned_through(
                                hir,
                                class.as_str(),
                                name.as_str(),
                                by_key,
                                declared.as_ref(),
                            )
                            // The whole table at once: `getValue()` hands back one entry per
                            // delta, each holding every property, which is a list of the shape
                            // the properties describe.
                            .or_else(|| self.tabled_rows(class.as_str(), name.as_str(), by_key))
                            .or(declared)?
                        }
                    };
                    // And a failure branch a literal the caller wrote out rules out, which no
                    // declaration of the method can state. See `ruled_out_failures`.
                    self.without_a_ruled_out_failure(
                        class.as_str(),
                        name.as_str(),
                        arguments,
                        reached,
                    )
                }
                PathStep::ClassConstant(name) => self.member_through(
                    offset,
                    hir,
                    &held,
                    MemberKind::ClassConstant,
                    name.as_str(),
                    forwarding.take(),
                )?,
                // PHP's own rule: calling an object calls its `__invoke`. The step names no method
                // because the layer that read it has no interner; this is the layer that knows.
                //
                // Except for a value whose type already said what invoking it gives, where there
                // is no class to look the method up on and none is needed. PHP's own `Closure`
                // declares `__invoke(): mixed`, so going through the class would replace the one
                // answer anybody wanted with the widest type there is.
                PathStep::Invoked { call } => {
                    // A value whose own *type* says what invoking it gives: a closure literal, a
                    // first-class callable, or a `@param callable(Item): string`. All three arrive
                    // in different shapes and answer the same thing -- see `invoked_return`.
                    match self.invoked_return(hir, &held) {
                        Some(returns) => {
                            forwarding = None;
                            returns
                        }
                        // An array is invoked as the `[$object, 'method']` pair it may be, which
                        // only its path still spells -- see `invoked_pair`. The root only: a pair
                        // reached through a step was written somewhere a binding does not lead.
                        None if index == 0
                            && self.kind_of_held(hir, &held)
                                == Some(phrost_model::BuiltinTy::Array) =>
                        {
                            forwarding = None;
                            let root = phrost_body::ValuePath {
                                root: path.root.clone(),
                                steps: Vec::new(),
                                may_be_absent: false,
                            };
                            self.invoked_pair(offset, hir, &root, budget)?
                        }
                        None => {
                            // The same ladder, and the same blindness before it: a value written
                            // `A|B` where only `B` declares `__invoke` is invocable, and reducing
                            // the union to a common ancestor answered nothing.
                            let class = self.owner_of_member(
                                offset,
                                hir,
                                &held,
                                MemberKind::Method,
                                "__invoke",
                            )?;
                            self.member_held_on(
                                class.as_str(),
                                MemberKind::Method,
                                "__invoke",
                                forwarding.take(),
                                &self.arguments_of_held(hir, &held),
                            )
                            .or_else(|| {
                                self.returned_by_method_contract(
                                    class.as_str(),
                                    "__invoke",
                                    None,
                                    &crate::symbols::CallFacts {
                                        offset,
                                        hir,
                                        literals: &[],
                                        call: *call,
                                        budget,
                                        receiver: &self.arguments_of_held(hir, &held),
                                        // The call is an expression here, so its own arguments are read.
                                        written: None,
                                    },
                                )
                            })?
                        }
                    }
                }
                PathStep::IterationKey => {
                    forwarding = None;
                    self.iteration_key_of_held(offset, hir, &held)?
                }
                PathStep::Element(_) => {
                    forwarding = None;
                    let subscript = match step {
                        PathStep::Element(Some(_)) => crate::resolve::Subscript::Written,
                        _ => crate::resolve::Subscript::Unknown,
                    };
                    match self.indexed_of_held(offset, hir, &held, named_key(step), subscript) {
                        Some(indexed) => indexed,
                        // The declaration says nothing about this slot, which for an array built one
                        // slot at a time is every slot: `$rows = []` describes nothing and the writes
                        // that follow describe everything. Asked only here, so a declaration that did
                        // say what it holds keeps saying it.
                        None => match self.slot_written(offset, hir, path, index, budget)? {
                            Written::Slot(slot) => slot,
                            // The whole path, answered whole -- so the steps after this one are
                            // already in it and there is nothing left to walk.
                            Written::Whole(whole) => return Some(whole),
                        },
                    }
                }
            };

            // A framework said which branch of the declared union this call actually answers, and
            // the chain said which way. Applied after the step rather than instead of it, so the
            // declaration is still read and this only narrows what it produced.
            if let Some(branch) = chained {
                held = Held::Builtin(branch);
            }
            if let Some((many, counted)) = picked {
                held = self.narrowed_union(offset, hir, held, &|operand| {
                    let is_many = operand.class.as_ref().map(|class| {
                        class.as_str().eq_ignore_ascii_case(&many)
                            || self.descends_from(
                                class.as_str(),
                                &many,
                                crate::resolve::HIERARCHY_DEPTH,
                            )
                    });
                    match is_many {
                        Some(is_many) => (is_many == counted).then_some(operand.held),
                        None => Some(operand.held),
                    }
                });
            }

            // And a guard written about the slot this step just reached. `resolve_path` asks that
            // of the path as a whole, which answers for `$this->subject` and not for the
            // `$this->subject->info()` a line below it -- so `if (!$this->subject instanceof
            // Special) { return; } $info = $this->subject->info();` resolved the call, because the
            // chain walk asks the same question of the receiver, and left the *local* untyped.
            // Two readings of one rule, and the property is the only step a guard can name.
            if matches!(step, PathStep::Property(_))
                && let Some(narrowed) = self.narrowed_prefix(offset, hir, path, index, budget)
            {
                forwarding = None;
                held = narrowed;
            }
        }

        Some(self.contract_narrowed(offset, hir, path, budget, held))
    }

    /// What a guard says about the slot `path` reaches at `index`, when one does.
    ///
    /// The prefix through that step, asked of `slot_value` exactly as [`IndexView::resolve_path`]
    /// asks it of a whole path. Only a *guard* -- `written_at.is_none()` -- because a write to the
    /// slot is already what the declaration-reading walk is producing, and preferring the write
    /// here would read it twice.
    fn narrowed_prefix(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        index: usize,
        budget: u8,
    ) -> Option<Held> {
        let budget = budget.checked_sub(1)?;
        let prefix = phrost_body::ValuePath {
            root: path.root.clone(),
            steps: path.steps[..=index].to_vec(),
            may_be_absent: false,
        };
        if !prefix.can_be_guarded() {
            return None;
        }
        let settled = phrost_body::slot_value(
            hir,
            offset,
            &prefix,
            &|hir, expr| self.call_never_returns(offset, hir, expr),
            &self.asserting(),
            &self.assigned_properties(hir),
        )?;
        if settled.written_at.is_some() {
            return None;
        }
        self.resolve_path(offset, hir, &settled.value, budget)
    }

    /// A call before `path` whose `@assert` tag (or by-reference parameter) says something about
    /// what it was passed, refining `held` rather than replacing it: after
    /// `$this->assertNotNull($leading)` the value is still whatever it was assigned, minus the
    /// null.
    ///
    /// Split out of the tail of [`Self::held_after_steps`] so a caller that already has a `held`
    /// from a different walk over the same steps -- `phrost_view::chain`'s scanner reads a
    /// receiver's characters rather than an expression, because the statement being typed does
    /// not parse, and that walk is the one that must carry the arguments a call was written with
    /// -- asks this exact question instead of a second copy of it. Two answers to the same
    /// question is the bug this replaced: a bare local asked it and a subscript did not, purely
    /// because only one route went through here.
    ///
    /// Asked only about a value whose declaration was vague, which is what keeps this out of the
    /// hot path: nearly every value an editor resolves already names one class and cannot be
    /// null, and those two are also what a contract is written for -- nobody writes one about a
    /// value that already says what it is.
    pub(crate) fn contract_narrowed(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
        held: Held,
    ) -> Held {
        if budget == 0 {
            return held;
        }
        // The prefilter is the *call list*, not the value. A value already knowing its class was
        // the old gate, and it was wrong for the commonest assertion there is: `assertInstanceOf`
        // narrows a declared interface to the concrete class a test just built, so every one of
        // them was skipped for saying something -- 326 findings on one real checkout, all inside
        // the assertion that rules them out. What is expensive here is resolving a callee, and
        // `passed_to_calls` already answers "is this value handed to any statement call" from the
        // body alone, which is no for nearly every value in a file.
        let sites = self.passed_to_calls(hir, offset, path);
        if sites.is_empty() {
            return held;
        }

        // Resolved once and walked three times: the passes below ask about the same calls in
        // different orders, and looking a callee up is the expensive half of this.
        let calls = sites
            .into_iter()
            .filter_map(|call| {
                let declared = self.declaration_called(offset, hir, &call, budget)?;
                Some((call, declared))
            })
            .collect::<Vec<_>>();

        // What a method said it leaves in the value it was called on, first and newest first: it
        // replaces the value outright, so only the calls after it have anything left to say.
        // `$box->replace($next)` is a `MutableBox<string>` from there on, whatever it was built
        // as. The receiver is resolved where that call was made, so a self-out before it is
        // already part of what it was written with.
        let newest_left = calls
            .iter()
            .enumerate()
            .rev()
            .find_map(|(at, (call, declared))| {
                Some((at, self.left_in_receiver_by(hir, declared, call, budget)?))
            });
        let (held, calls) = match newest_left {
            Some((at, left)) => (left, &calls[at + 1..]),
            None => (held, &calls[..]),
        };
        let vague = matches!(self.null_reach_of(hir, &held), SentinelReach::Possible)
            || self.class_of_held(offset, hir, &held).is_none();

        // A by-reference parameter first, because it is the plainest claim of the two: the callee
        // may have written a value of that type into the caller's variable, and
        // `function f(Baz &$bar)` says so in the signature rather than in a comment.
        //
        // Newest first, because two calls can write one name and the last one is what is there:
        // `preg_match($p, $s, $m); preg_match_all($q, $s, $m);` leaves `$m` in the second one's
        // shape, and reading the list in source order gave the first tag whichever order they
        // were written in.
        // Only for a value that says little. A by-reference parameter is a bound on what the
        // callee *accepts* rather than a claim about what it left, so believing it about a value
        // that already knows its class can only widen -- which is the reading
        // `written_through_reference` documents at length and declines.
        if vague {
            for (call, declared) in calls.iter().rev() {
                if let Some(written) =
                    self.written_through_reference(offset, hir, declared, call, Some(&held))
                {
                    return written;
                }
            }
        }

        // Then what a call's contract says, in the order the calls were written.
        //
        // Unconditional only. A `@assert-if-true` says nothing about a call written as a statement,
        // where nobody looked at what it returned.
        //
        // The list is the *narrow* one, so every entry is already a call written as a statement of
        // its own -- which is what makes reading a tag as unconditional honest here. The wide walk
        // exists for the by-reference root and deliberately does not feed this.
        for (call, declared) in calls {
            if let Some(asserted) = declared.asserts_about(call)
                && asserted.when_true.is_none()
                && let Some(claimed) =
                    self.asserted_held(offset, hir, held.clone(), call, &asserted)
                && (vague || self.narrows_to(offset, hir, &held, &claimed))
            {
                return self.claimed_keeping_arguments(offset, hir, &held, claimed);
            }
        }

        held
    }
}

impl IndexView<'_> {
    /// The statement calls before `offset` that `path` was passed to, read through the file's
    /// [`phrost_body::PassedPaths`], built once per view.
    ///
    /// Only for the current file's tree, which is the one the index was built from: any other tree
    /// is walked without it, which is the answer that cannot be wrong.
    fn passed_to_calls(
        &self,
        hir: &FileHir,
        offset: TextSize,
        path: &phrost_body::ValuePath,
    ) -> Vec<phrost_body::PassedTo> {
        // Which method names may say what they leave in their receiver: a scan of the handful a
        // project declares, so a call on a value is recorded only where it could matter.
        let leaves_receiver = |name: &str| self.skeleton.declares_receiver_out(name);
        let Some(open) = self.current().filter(|open| std::ptr::eq(open.hir(), hir)) else {
            let every: Vec<phrost_hir::StmtId> =
                phrost_body::PassedPaths::of(hir, &leaves_receiver)
                    .statements(path)
                    .to_vec();
            return phrost_body::passed_to_calls(hir, offset, path, &every, &leaves_receiver);
        };
        let cached = self.passed_paths.borrow().clone();
        let index = match cached {
            Some(index) => index,
            None => {
                let index =
                    std::sync::Arc::new(phrost_body::PassedPaths::of(open.hir(), &leaves_receiver));
                *self.passed_paths.borrow_mut() = Some(std::sync::Arc::clone(&index));
                index
            }
        };
        phrost_body::passed_to_calls(hir, offset, path, index.statements(path), &leaves_receiver)
    }

    /// An asserted subclass, with the type arguments the value already had carried down to it -- the
    /// rule an `instanceof` guard follows, and for the same reason: `assertInstanceOf(Lazy::class,
    /// $c)` on a `Collection<int, User>` is a `Lazy<int, User>`, and the bare class the tag names
    /// took away what the declaration said.
    fn claimed_keeping_arguments(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        claimed: Held,
    ) -> Held {
        // A bare class, however it was spelled: one the tag wrote arguments for, or wrapped in a
        // `?`, says more than this would rebuild.
        let bare = match &claimed {
            Held::Class(_) => true,
            Held::Stored { ty, .. } => {
                matches!(self.skeleton.types().get(*ty), phrost_ty::TypeRef::Named(_))
            }
            _ => false,
        };
        let (true, Some(now), Some(had)) = (
            bare,
            self.class_of_held(offset, hir, &claimed),
            self.class_of_held(offset, hir, held),
        ) else {
            return claimed;
        };
        match had != now
            && self.descends_from(now.as_str(), had.as_str(), crate::view::FOLLOW_LIMIT)
        {
            true => self.subclass_keeping_arguments(hir, held, &had, &now),
            false => claimed,
        }
    }

    /// Whether an assertion's claim is narrower than what the value already said.
    ///
    /// An assertion is a claim about what the value *is*, so it may only make the answer more
    /// precise. Where the value already knows its class, a tag naming an ancestor of it -- which is
    /// what `assertSame($expected, $actual)` produces when the expected side is declared by an
    /// interface -- would take the subclass away and report the methods the test is calling.
    ///
    /// Only asked where the value knows its class. A vague value has nothing to lose, and the
    /// comparison would decline the tags this exists to read.
    fn narrows_to(&self, offset: TextSize, hir: &FileHir, held: &Held, claimed: &Held) -> bool {
        let Some(had) = self.class_of_held(offset, hir, held) else {
            return true;
        };
        let Some(now) = self.class_of_held(offset, hir, claimed) else {
            return false;
        };
        // The class the value already is says nothing new, and replacing the value with it takes
        // away what it was written with: `assertInstanceOf(Builder::class, $results)` on a
        // `Builder<User>` made every `$results->first()` a `?Model`. Unless the value carried no
        // arguments, where a tag that names some -- `@phpstan-assert Collection<int, User>` -- is
        // the more precise of the two.
        if now == had {
            return self.arguments_of_held(hir, held).is_empty();
        }
        self.descends_from(now.as_str(), had.as_str(), crate::view::FOLLOW_LIMIT)
    }
}

/// Which of the two paths a write answered for. See [`IndexView::slot_written`].
enum Written {
    Slot(Held),
    Whole(Held),
}

/// The key a step names, as text.
///
/// `None` for a variable key, which reaches whichever slot it holds and nothing here knows which --
/// so it is read as no key at all.
fn named_key(step: &PathStep) -> Option<&str> {
    match step {
        PathStep::Element(Some(phrost_body::ElementKey::Literal(written))) => {
            Some(written.as_str())
        }
        _ => None,
    }
}
