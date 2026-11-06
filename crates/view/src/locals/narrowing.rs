//! What a guard leaves of a value.
//!
//! A null check, an `instanceof`, a type predicate and an `@assert` all say the same kind of thing:
//! the declaration listed what the value may be, and something in the code has ruled some of it
//! out. Each is a rewrite of a type into a narrower type, which is why they are one file and not
//! spread across the roots that reach them -- the rules compose, and `also_of_class` reaches for
//! two of its siblings.

use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, Name, TextSize};

use crate::{resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// The same value with an empty shape taken off it.
    ///
    /// `array{}` is falsy, exactly as `null` and `false` are, so a truthiness guard rules it out
    /// along with them -- and it has to, because the whole reason a project writes
    /// `@return array{...}|array{}` is to be checked for emptiness before it is read.
    /// `if ($address === []) { return; }` above `$address['city']` is the shape every one of these
    /// sites takes, and without this the branch that made the keys readable reported every one of
    /// them.
    ///
    /// Not a peel like [`Self::without_null`]: an empty shape is an operand and never a wrapper, so
    /// there is nothing to look through. A value that is *only* an empty shape is left as it was --
    /// a guard proving a value cannot exist is not something to answer a type question with.
    pub(crate) fn without_empty_shape(
        &self,
        at: phrost_model::TextSize,
        hir: &FileHir,
        held: Held,
    ) -> Held {
        let Some(operands) = self.union_operands(at, hir, &held) else {
            return held;
        };
        let rest = operands
            .into_iter()
            .map(|operand| operand.held)
            .filter(|operand| !self.is_empty_shape(hir, operand))
            .collect::<Vec<_>>();
        match rest.is_empty() {
            true => held,
            false => Self::one_of(rest),
        }
    }

    /// Whether null is one of the things `held` can be.
    ///
    /// Asked *through* [`Self::without_null`] rather than beside it, which is the point: null
    /// arrives in six spellings -- a bare `Held::Builtin(Null)`, a `Held::Nullable`, an operand of
    /// a `OneOf`, a stored `TypeRef::Nullable`, a stored `Union` holding a null, and a written
    /// `TypeRefData` in either of the last two shapes -- and the function that removes it is the
    /// one thing that already knows all six. A second reader of the same question is how the two
    /// halves come to disagree.
    pub(crate) fn takes_a_null_off(&self, held: &Held) -> bool {
        self.without_null(held.clone()).as_ref() != Some(held)
    }

    /// What is left once a guard has ruled out the sentinels `refused` names.
    ///
    /// Null always. `false` for every guard but the strictest null test, which is the whole of how
    /// the standard library and Drupal's database layer report "nothing": `fetchAssoc()` returns
    /// `array|false`, and `if ($row)` above it leaves an `array`. Reusing the predicate machinery
    /// rather than a second peel -- `$x !== FALSE` removes the same operand through the same call.
    ///
    /// And the empty shape for a truthiness test only, since it is the third falsy thing a union of
    /// shapes can hold -- see [`Self::without_empty_shape`] -- and `is_countable([])` is true.
    ///
    /// `None` where the value was exactly null, as for [`Self::without_null`].
    pub(crate) fn without_sentinels(
        &self,
        at: TextSize,
        hir: &FileHir,
        held: Held,
        refused: phrost_body::Refused,
    ) -> Option<Held> {
        let held = self.without_null(held)?;
        if !refused.takes_the_false() {
            return Some(held);
        }
        let held = self.of_kind(at, hir, held, BuiltinTy::False, false, true);
        let held = match refused.leaves_strings() {
            Some(holding) => self.with_strings_holding(at, hir, held, holding),
            None => held,
        };
        Some(match refused.proves_truth() {
            true => self.without_empty_shape(at, hir, held),
            false => held,
        })
    }

    /// The same value with every string it may be known to hold at least `holding`: what a guard
    /// that ruled out `''` -- and, for a truthiness test, `'0'` -- leaves of a string.
    ///
    /// A refinement rather than a filter, which is why it is not [`Self::of_kind`]: the string
    /// operand survives, and what changes is what it is known to hold. Every other alternative is
    /// untouched, a null included -- `$name !== ''` on a `?string` is a `?non-empty-string`. A value
    /// nothing described is left as it was, for the reason `of_kind` leaves one: the guard is true of
    /// a `mixed`, and saying so would type a great deal of untyped PHP.
    ///
    /// Through the one helper every spelling of a string answers to, so a refinement written in the
    /// file, one interned from another, and one a literal built all rise the same way; one that
    /// already holds more keeps what it holds.
    pub(crate) fn with_strings_holding(
        &self,
        at: TextSize,
        hir: &FileHir,
        held: Held,
        holding: BuiltinTy,
    ) -> Held {
        let raised = |held: Held| match self.string_held(hir, &held) {
            Some(holds) if holds.string_rank() < holding.string_rank() => Held::Builtin(holding),
            _ => held,
        };
        let nullable = self.null_reach_of(hir, &held) != crate::SentinelReach::Impossible;
        let Some(bare) = self.without_null(held.clone()) else {
            return held;
        };
        let refined = match self.union_operands(at, hir, &bare) {
            Some(_) => {
                self.narrowed_union(at, hir, bare.clone(), &|operand| Some(raised(operand.held)))
            }
            None => raised(bare.clone()),
        };
        match (refined == bare, nullable) {
            // Nothing was a string to raise, so the value is what it was -- in the spelling it was.
            (true, _) => held,
            (false, true) => Self::nullable(refined),
            (false, false) => refined,
        }
    }

    /// What is left of a union once a test that names no kind has ruled `kinds` out of it -- each
    /// one read the way a failed type predicate is, as a filter over what the declaration listed.
    pub(crate) fn none_of_kinds(
        &self,
        at: TextSize,
        hir: &FileHir,
        held: Held,
        kinds: &[BuiltinTy],
    ) -> Held {
        kinds.iter().fold(held, |held, &kind| {
            self.of_kind(at, hir, held, kind, false, false)
        })
    }

    /// The same value with the possibility of null taken off it.
    ///
    /// Unchanged when there was no null to take: a guard that rules out something the type never
    /// held is not an error, and answering nothing for it would lose the type the reader had.
    ///
    /// `None` only for a value believed to be *exactly* null, where the guard and the belief
    /// contradict each other -- which means the belief is incomplete, most often because a loop
    /// carries an assignment written below the guard back round to it:
    ///
    /// ```php
    /// $lowest = NULL;
    /// foreach ($grades as $grade) {
    ///     if ($lowest === NULL || $grade->getMinRating() < $lowest->getMinRating()) {
    ///         $lowest = $grade;
    ///     }
    /// }
    /// ```
    ///
    /// Nothing is lost by declining: null has no members, so there was no answer to give.
    pub(crate) fn without_null(&self, held: Held) -> Option<Held> {
        Some(match held {
            Held::Builtin(phrost_model::BuiltinTy::Null) => return None,
            // Recursive, because the two nulls can stack: `$c ? File::load($id) : NULL` wraps a
            // return type that is itself `?File`, and stripping one layer leaves the other.
            Held::Nullable(inner) => return self.without_null(*inner),
            // A union whose null is *inside* an operand rather than beside them. `one_of` folds a
            // bare null operand out into a `Nullable` wrapper, and everything here reads it there
            // -- but `?Node` joined with `Node` is a union of two operands and neither of them is
            // that bare null, so the wrapper never appears and the guard had nothing to take off.
            // Peeling each operand is the same claim applied to each alternative, and one that was
            // nothing but null drops out: the guard ruled that alternative out entirely.
            Held::OneOf(operands) => {
                let rest = operands
                    .into_vec()
                    .into_iter()
                    .filter_map(|held| self.without_null(held))
                    .collect::<Vec<_>>();
                return match rest.is_empty() {
                    true => None,
                    false => Some(Self::one_of(rest)),
                };
            }
            Held::Stored { ty, scope } => match self.skeleton.types().get(ty) {
                // A stored `null`, which a conditional's branch is: `array_shift()`'s `V : null`
                // joined as stored operands kept the null where no guard could reach it.
                phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Null) => return None,
                phrost_ty::TypeRef::Nullable(inner) => Held::Stored { ty: *inner, scope },
                _ => Held::Stored { ty, scope },
            },
            Held::Written(id) => {
                let Some(hir) = self.current().map(|open| open.hir()) else {
                    return Some(Held::Written(id));
                };
                match hir.type_ref(id) {
                    phrost_hir::TypeRefData::Builtin(phrost_model::BuiltinTy::Null) => return None,
                    phrost_hir::TypeRefData::Nullable(inner) => Held::Written(*inner),
                    // `Repo|null`, which is how a docblock spells it. One survivor points at that
                    // operand rather than at a rebuilt type, which keeps everything it carried,
                    // generic arguments included; several are rebuilt operand by operand, because
                    // there is no way to write a narrowed type into another file's arena and the
                    // alternative is answering with the null still on it. That used to be the
                    // answer, on the grounds that a union of two classes names no single class
                    // either way -- but it does reach a *predicate*, and `is_string($t)` on a
                    // joined `int|string|null` was left with a null nobody could take off.
                    //
                    // Operand by operand and all the way down, because a docblock's union arrives
                    // nested: `string|array|null` is `string|(array|null)` to the parser, so the
                    // null is never an operand of the outer union and taking off only those left
                    // it standing. `$content ?? ''` over a `@param View|string|array|null` read as
                    // possibly null, and every call it was handed to was reported.
                    phrost_hir::TypeRefData::Union(parts) => {
                        let rest = hir
                            .type_ref_list(*parts)
                            .to_vec()
                            .into_iter()
                            .filter_map(|part| self.without_null(Held::Written(part)))
                            .collect::<Vec<_>>();
                        match rest.is_empty() {
                            true => return None,
                            false => Self::one_of(rest),
                        }
                    }
                    _ => Held::Written(id),
                }
            }
            other => other,
        })
    }

    /// What is left of a value once a type predicate has said which sort it is.
    ///
    /// A union is where the filtering says anything: `Foo|string` inside `if (is_string($x))` is a
    /// `string`, which is the shape a PHP function that accepts either is written in.
    ///
    /// # A positive test proves the value is there, and is the whole answer where nothing it held
    /// was of the kind
    ///
    /// Null answers `false` to every one of PHP's predicates, so `if (is_string($s))` on a
    /// `?string` says as much about the null as `!== null` does -- and this used to leave it on,
    /// because a `?string` is not a union to [`Self::union_operands`] while
    /// [`Self::kind_of_held`] reads through the `?` and found the kind it wanted. Every guard
    /// written that way then reported the null it had just ruled out.
    ///
    /// And where nothing the value held is of the kind, filtering keeps nothing and leaving the
    /// value as it was is the contradiction rather than the answer: a `$v` believed to be null
    /// inside `if (is_array($v))` was passed on as null, so the call the guard exists to protect
    /// was reported as passing one. The guard is about this position and the belief is about every
    /// one, so the guard wins -- the same rule [`Self::of_one_of_classes`] states for a class and
    /// [`Self::also_of_class`] for one class, and the reader of a `@phpstan-assert` tag had it and
    /// this one did not. A kind that merely says something *beside* what the value is keeps it:
    /// `is_callable($handler)` on a class is not a contradiction, and answering `callable` would
    /// take away the members. See [`IndexView::kinds_exclude`].
    ///
    /// Both of those are for a guard that *named* the kind, which is what `settled` carries: a
    /// claim that only ruled some alternatives out filters and nothing more. `!in_array($x, ['a',
    /// 'b'], true)` reaches here as "an object", which is a way of removing the strings from a
    /// declared union and is not true of anything else -- so taken as the whole answer it would
    /// make an object of every value nothing else described.
    /// What is left of a `bool` once a test has said which half of it the value is.
    ///
    /// `bool` is `true|false` spelled as one type, so ruling one of the two out leaves the other:
    /// `$handler ?: SIG_DFL` over a `bool|resource` cannot hand back the `false` it tested for, and
    /// keeping `bool` said it could. And the passing side names its half outright: inside `if ($a
    /// === false)` a `bool $a` is `false`, which kept as `bool` was a value a declared `false`
    /// refuses.
    fn bool_part(kind: BuiltinTy, holds: bool, settled: bool) -> Option<BuiltinTy> {
        match (kind, holds, settled) {
            (BuiltinTy::False, false, true) => Some(BuiltinTy::True),
            (BuiltinTy::True, false, true) => Some(BuiltinTy::False),
            (BuiltinTy::False | BuiltinTy::True, true, _) => Some(kind),
            _ => None,
        }
    }

    pub(crate) fn of_kind(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        kind: BuiltinTy,
        holds: bool,
        settled: bool,
    ) -> Held {
        let survives = |operand: crate::resolve::UnionOperand| {
            if operand.kind == Some(BuiltinTy::Bool)
                && let Some(left) = Self::bool_part(kind, holds, settled)
            {
                return Some(Held::Builtin(left));
            }
            let answers = operand
                .kind
                .map_or(crate::resolve::Answers::Maybe, |operand| {
                    Self::kind_answers(operand, kind)
                });
            // An object is callable or iterable by what its class declares, so the class answers
            // where the kind alone could only say "maybe" -- and a closed class that declares
            // neither is not one of them. `is_string` is not asked this way: no object is a string
            // to it, `Stringable` or not.
            let answers = match (&operand.class, kind, answers) {
                (
                    Some(class),
                    BuiltinTy::Callable | BuiltinTy::Iterable,
                    crate::resolve::Answers::Maybe,
                ) => self.class_answers_kind(class.as_str(), kind),
                _ => answers,
            };
            let kept = match answers {
                // An operand that neither answers the test nor refuses it survives either branch.
                crate::resolve::Answers::Maybe => true,
                crate::resolve::Answers::Yes => holds,
                crate::resolve::Answers::No => !holds,
            };
            kept.then_some(operand.held)
        };
        // A value that is *only* the kind the guard rules out cannot enter the branch at all:
        // `if (!is_array($rows))` where `$rows` is an `array`. Handing back what it was says the
        // opposite of what the guard proved, and the checks then judge an unreachable line against
        // a type the guard excluded. `mixed` is the honest answer -- the one type every check reads
        // as no claim -- and this layer proves things about values rather than deciding which
        // branches run. Only for a settled test, since an unsettled one has not ruled anything out.
        if !holds
            && settled
            && self.union_operands(offset, hir, &held).is_none()
            && self.kind_of_held(hir, &held) == Some(kind)
        {
            return Held::Builtin(BuiltinTy::Mixed);
        }
        // A lone `bool` is no union for the walk to take apart, and is still two values. A `?bool`
        // reads as one here too, and its null is neither half: gone where the test named a half
        // and passed, and still there where it ruled one out.
        if self.union_operands(offset, hir, &held).is_none()
            && self.kind_of_held(hir, &held) == Some(BuiltinTy::Bool)
            && let Some(left) = Self::bool_part(kind, holds, settled)
        {
            let left = Held::Builtin(left);
            return match !holds
                && self.null_reach_of(hir, &held) != crate::SentinelReach::Impossible
            {
                true => Self::nullable(left),
                false => left,
            };
        }
        if !holds || !settled {
            return self.narrowed_union(offset, hir, held, &survives);
        }
        let Some(held) = self.without_null(held) else {
            return Held::Builtin(kind);
        };
        match self.union_operands(offset, hir, &held) {
            Some(operands) => match operands
                .iter()
                // An operand that is itself a union answers nothing about its own kind, and
                // `narrowed_union` takes it apart in turn -- so it counts as an alternative that
                // may yet be of this kind rather than as one that is not.
                .any(|operand| {
                    operand.kind.is_none_or(|operand| {
                        Self::kind_answers(operand, kind) != crate::resolve::Answers::No
                    }) || self.union_inside(&operand.held)
                }) {
                true => self.narrowed_union(offset, hir, held, &survives),
                false => Held::Builtin(kind),
            },
            // Only where the two *contradict*. A value nothing described -- `mixed`, or a name with
            // no hint -- is left as it was, although the guard is a true claim about it and saying
            // so would type a great deal of untyped PHP. It was measured doing that and reporting
            // eleven working lines: a claim built by `Otherwise` for the path that *skipped* a
            // conditional write takes the complement of whatever guard narrows the name at the
            // write, including one that encloses the read as well -- so
            // `if (!is_array($t)) { if (...) { $t = str_replace(...); } trim($t); }` answers that
            // the skipped path had an array, which is the one thing that branch rules out. That
            // claim is wrong either way and was inert only while a kind nothing held could not be
            // an answer. Widening this arm is worth revisiting once it is right.
            None => match self.kind_of_held(hir, &held) {
                Some(already) if Self::kinds_exclude(already, kind) => Held::Builtin(kind),
                _ => held,
            },
        }
    }

    /// What is left of a union once a guard has named which alternatives are possible.
    ///
    /// `Foo|Bar|Other` inside `if ($x instanceof Foo || $x instanceof Bar)` is a `Foo` or a `Bar`.
    /// An operand that *descends* from one of them counts: a `Special` is a `Foo`, so the guard did
    /// not rule it out.
    ///
    /// # When there is no union to narrow, the guard is the whole answer
    ///
    /// This was written as a filter over alternatives the declaration already listed, and filtering
    /// nothing leaves nothing -- so `if ($v instanceof Baz || $v instanceof Boo)` proved a `Baz|Boo`
    /// for a parameter declared `Baz|Boo|int` and **nothing at all** for one declared `mixed` or with
    /// no hint, which is every real shape the guard is written in. Three fixtures of a borrowed corpus
    /// are that one gap: an `||` of two `instanceof`, the same through `assert`, and the negated
    /// conjunction that means the same thing by De Morgan.
    ///
    /// [`Self::also_of_class`] has the missing half already, for one class rather than several --
    /// "nothing named, so there is nothing to keep beside the guard". Two constructors of one rule,
    /// and only one of them knowing it.
    ///
    /// A value that *does* name a class keeps it, unchanged from before. `if ($v instanceof Baz ||
    /// $v instanceof Boo)` on a declared `Foo` is an intersection of `Foo` with a union, which is a
    /// larger claim than this filter makes and is not what any measured shape needs.
    pub(crate) fn of_one_of_classes(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        named: &[Name],
    ) -> Held {
        // The null goes first, exactly as it does in `also_of_class`: `instanceof` is false for null,
        // so a guard that passed proves the value is there.
        let Some(held) = self.without_null(held) else {
            return Self::one_of_classes(named);
        };
        if self.union_operands(offset, hir, &held).is_none() {
            match self.class_of_held(offset, hir, &held) {
                // Nothing named, so there is nothing to filter and the guard is the whole answer.
                None => return Self::one_of_classes(named),
                // Every class the guard named is below what the value already names, so the guard is
                // strictly the more specific claim and replaces rather than filters. `also_of_class`
                // has this branch for a single class -- "a value whose class the guard's descends
                // from is narrowed to the guard's" -- and this is the same rule with several. It is
                // what `Zero $zero` guarded by `instanceof One || instanceof Two` needs, which is the
                // ordinary shape of an early-return guard over a sealed hierarchy, and the third
                // place this same asymmetry between one class and several had to be closed.
                Some(already)
                    if named.iter().all(|class| {
                        class.as_str().eq_ignore_ascii_case(already.as_str())
                            || self.descends_from(
                                class.as_str(),
                                already.as_str(),
                                crate::resolve::HIERARCHY_DEPTH,
                            )
                    }) =>
                {
                    return Self::one_of_classes(named);
                }
                // Anything else keeps what it had: an intersection of a class with a union is a
                // larger claim than this filter makes.
                Some(_) => {}
            }
        }

        self.narrowed_union(offset, hir, held, &|operand| {
            // An operand naming no class is not an instance of anything, so the guard ruled it out.
            let held_class = operand.class.as_ref()?;
            // The guard restates what this operand already was, or names a supertype of it: a
            // `Special` is a `Foo`, so the guard did not rule it out and did not sharpen it either.
            if named.iter().any(|class| {
                held_class.as_str().eq_ignore_ascii_case(class.as_str())
                    || self.descends_from(
                        held_class.as_str(),
                        class.as_str(),
                        crate::resolve::HIERARCHY_DEPTH,
                    )
            }) {
                return Some(operand.held);
            }
            // The guard is the *more specific* claim, so the operand becomes what the guard named --
            // the third hierarchy relation, and the one a filter could not express. `Base|false`
            // under `instanceof Sub` has no operand that is already a `Sub`, and answering "keep
            // nothing" left the whole union standing with its `false` in it. Several, when several
            // of the guarded classes sit below this operand: the value is one of them.
            let below = named
                .iter()
                .filter(|class| {
                    self.descends_from(
                        class.as_str(),
                        held_class.as_str(),
                        crate::resolve::HIERARCHY_DEPTH,
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            (!below.is_empty()).then(|| Self::one_of_classes(&below))
        })
    }

    /// The classes a guard named, as one value.
    ///
    /// Through [`Held::one_of`] rather than built by hand, so the degenerate forms fold: one class is
    /// a `Held::Class` and not a union of one, which is the distinction every narrowing after this one
    /// depends on being able to see.
    pub(crate) fn one_of_classes(named: &[Name]) -> Held {
        Self::one_of(named.iter().cloned().map(Held::Class).collect())
    }

    /// What a value is once a guard has proved it is a `class`, beside what it already held.
    ///
    /// Three answers, and which one applies is a question about the hierarchy. A value that already
    /// names the class, or a subclass of it, learns nothing -- the guard is a restatement. A value
    /// whose class the guard's *descends* from is narrowed to the guard's. And a value whose class
    /// has no relation to it is both at once, which is what an intersection is: PHP has one type for
    /// that, and this is where it gets built.
    ///
    /// A union narrows instead, to the operands that can be the class. That is the same walk
    /// [`Self::of_one_of_classes`] does, with one class instead of several.
    pub(crate) fn also_of_class(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        class: &Name,
    ) -> Held {
        // The null goes first, and in every branch below: `instanceof` is false for null, so a guard
        // that passed proves the value is there. Keeping it cost 57 phantom possibly-null-receiver
        // findings the first time this composed instead of replacing -- `if ($x instanceof Foo)` on a
        // `?Foo` used to answer `Foo` outright, and answering "what it was" brought the null back.
        let Some(held) = self.without_null(held) else {
            return Held::Class(class.clone());
        };

        if self.union_operands(offset, hir, &held).is_some() {
            return self.of_one_of_classes(offset, hir, held, std::slice::from_ref(class));
        }

        // An intersection is each of its operands at once, so the guard is one more of them --
        // unless one already is the class. It names no single class, and taken for one that names
        // nothing it was *replaced* by the guard: `$this instanceof Contract` inside a trait read
        // as `$this` and lost every member the trait declares.
        if let Held::AllOf(operands) = &held {
            let already = operands.iter().any(|operand| {
                self.class_of_held(offset, hir, operand)
                    .is_some_and(|named| {
                        named.as_str().eq_ignore_ascii_case(class.as_str())
                            || self.descends_from(
                                named.as_str(),
                                class.as_str(),
                                crate::resolve::HIERARCHY_DEPTH,
                            )
                    })
            });
            if already {
                return held;
            }
            let mut operands = operands.to_vec();
            operands.push(Held::Class(class.clone()));
            return Self::all_of(operands);
        }

        let Some(already) = self.class_of_held(offset, hir, &held) else {
            // Nothing named, so there is nothing to keep beside the guard -- and a `mixed` or an
            // array kept alongside would offer members of a value that is neither.
            return Held::Class(class.clone());
        };
        if already.as_str().eq_ignore_ascii_case(class.as_str())
            || self.descends_from(
                already.as_str(),
                class.as_str(),
                crate::resolve::HIERARCHY_DEPTH,
            )
        {
            return held;
        }
        if self.descends_from(
            class.as_str(),
            already.as_str(),
            crate::resolve::HIERARCHY_DEPTH,
        ) {
            return self.subclass_keeping_arguments(hir, &held, &already, class);
        }
        // Two unrelated *classes* cannot both be true: PHP has single inheritance, so no object is an
        // instance of both, and the useful reading of a guard that says otherwise is the guard --
        // `if ($this instanceof Special)` inside a sibling class means the code meant `Special`.
        //
        // An interface on either side is the opposite case, and the one worth composing: a value the
        // signature typed as `MockInterface` really is also the class the guard named, which is what
        // every test double in a project is.
        match self.is_interface(already.as_str()) || self.is_interface(class.as_str()) {
            true => Self::all_of(vec![held, Held::Class(class.clone())]),
            false => Held::Class(class.clone()),
        }
    }

    fn is_interface(&self, fqn: &str) -> bool {
        self.class_header(fqn)
            .is_some_and(|header| header.kind == phrost_sema::ClassKind::Interface)
    }

    /// Whether a value that is a `fqn` is one exactly: a final class, or an enum, which PHP makes
    /// final without saying so.
    pub(crate) fn nothing_extends(&self, fqn: &str) -> bool {
        self.class_header(fqn)
            .is_some_and(|header| header.is_final || header.kind == phrost_sema::ClassKind::Enum)
    }

    /// What is left once an `instanceof` guard has ruled a class out.
    ///
    /// The other side of the claim `Narrowed` makes, and it says something only because a union
    /// says what the alternatives were: after `if ($x instanceof Foo) { return; }` a `Foo|Bar` is a
    /// `Bar`. An operand that *descends* from the class goes too -- a `Sub` is a `Foo`, and keeping
    /// it would offer members of a class the value cannot be.
    pub(crate) fn not_of_class(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: Held,
        class: &str,
    ) -> Held {
        self.narrowed_union(offset, hir, held, &|operand| {
            let ruled_out = operand.class.as_ref().is_some_and(|named| {
                named.as_str().eq_ignore_ascii_case(class)
                    || self.descends_from(named.as_str(), class, crate::resolve::HIERARCHY_DEPTH)
            });
            // An operand naming no class is not an instance of anything, so the guard did not rule
            // it out.
            (!ruled_out).then_some(operand.held)
        })
    }

    /// `null`, when that is the only falsy thing the value could have been.
    ///
    /// `if (!$repo)` on a `?Repo` proves null, because an object is always truthy in PHP and null is
    /// the only other thing a `?Repo` holds. `if (!$count)` on a `?int` proves nothing: zero is falsy
    /// too. So this answers for the first and declines for the second, rather than reporting a value
    /// the reader would then act on.
    pub(crate) fn only_null(&self, held: &Held) -> Option<Held> {
        let object_or_null = match held {
            Held::Nullable(inner) => return self.only_null(inner),
            Held::Stored { ty, .. } => match self.skeleton.types().get(*ty) {
                phrost_ty::TypeRef::Nullable(inner) => self.is_object(*inner),
                _ => false,
            },
            Held::Written(id) => {
                let hir = self.current()?.hir();
                match hir.type_ref(*id) {
                    phrost_hir::TypeRefData::Nullable(inner) => self.is_written_object(*inner),
                    _ => false,
                }
            }
            _ => false,
        };

        object_or_null.then_some(Held::Builtin(phrost_model::BuiltinTy::Null))
    }

    /// What a value can still be where it tested falsy: everything it was but what is always truthy.
    ///
    /// [`Self::only_null`] first, which is the exact answer where null is all that is left. Past it
    /// this used to decline, and declining took the whole type out of the branch: `if (!$tag ||
    /// is_wp_error($tag))` on `array|WP_Error|null` left the falsy half with nothing, so the union of
    /// the two halves was nothing too, and every `$tag->get_error_message()` below it resolved
    /// nowhere. An object or a `true` cannot be falsy and goes; an `array`, an `int` or a string may
    /// be, and stays -- `?int` after `if (!$count)` is the `0` or the null it is. A `bool` is its
    /// `false`, and a shape that lists a key it always has is as truthy as an object.
    ///
    /// An array is the `array{}` it can only be -- a record with no fields, which is how an empty
    /// array is spelled here -- where `empty_array` says the guard was about this very value, and
    /// the bare `array` elsewhere. Dropping it instead would say a `?array` tested falsy is the null,
    /// and report every `if (!$rows) { return $rows; }` as returning null.
    ///
    /// `None` where nothing can be falsy at all: the branch cannot be entered with this value.
    pub(crate) fn falsy_part(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        empty_array: bool,
    ) -> Option<Held> {
        if let Some(null) = self.only_null(held) {
            return Some(null);
        }
        let nullable = self.null_reach_of(hir, held) != crate::SentinelReach::Impossible;
        let Some(bare) = self.without_null(held.clone()) else {
            return Some(Held::Builtin(phrost_model::BuiltinTy::Null));
        };
        let falsy = |kind: Option<phrost_model::BuiltinTy>, held: Held| match kind {
            Some(phrost_model::BuiltinTy::Object | phrost_model::BuiltinTy::True) => None,
            Some(phrost_model::BuiltinTy::Bool) => {
                Some(Held::Builtin(phrost_model::BuiltinTy::False))
            }
            Some(phrost_model::BuiltinTy::Array) if self.lists_a_required_key(hir, &held) => None,
            // `[]` is the one falsy array, so a falsy array is a record with no fields -- the
            // `array{}` this used to have no spelling for. Where the caller can vouch that the
            // guard was about this value and not a slot a call stands for; see `reads_no_call`.
            Some(phrost_model::BuiltinTy::Array) if empty_array => Some(Held::Record(Box::new([]))),
            _ => Some(held),
        };
        let rest = match self.union_operands(offset, hir, &bare) {
            Some(operands) => {
                let kept = operands
                    .into_iter()
                    .filter_map(|operand| falsy(operand.kind, operand.held))
                    .collect::<Vec<_>>();
                (!kept.is_empty()).then(|| Self::one_of(kept))
            }
            None => falsy(self.kind_of_held(hir, &bare), bare),
        };
        match (rest, nullable) {
            (Some(rest), true) => Some(Self::nullable(rest)),
            (Some(rest), false) => Some(rest),
            (None, true) => Some(Held::Builtin(phrost_model::BuiltinTy::Null)),
            (None, false) => None,
        }
    }

    /// Whether a value is a shape with a key it never leaves out, and so is never empty.
    fn lists_a_required_key(&self, hir: &FileHir, held: &Held) -> bool {
        match held {
            Held::Record(_) => true,
            Held::Written(id) => match hir.type_ref(*id) {
                phrost_hir::TypeRefData::Shape { fields, .. } => {
                    hir.type_ref_list(*fields).iter().any(|field| {
                        matches!(
                            hir.type_ref(*field),
                            phrost_hir::TypeRefData::ShapeField {
                                optional: false,
                                ..
                            }
                        )
                    })
                }
                _ => false,
            },
            Held::Stored { ty, .. } => {
                let types = self.skeleton.types();
                match types.get(*ty) {
                    phrost_ty::TypeRef::Shape { fields, .. } => {
                        types.operands(*fields).iter().any(|field| {
                            matches!(
                                types.get(*field),
                                phrost_ty::TypeRef::Field {
                                    optional: false,
                                    ..
                                }
                            )
                        })
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Whether every value of this type is an object, and so always truthy.
    fn is_object(&self, ty: phrost_ty::TypeId) -> bool {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Named(_) | phrost_ty::TypeRef::Relative(_) => true,
            phrost_ty::TypeRef::Generic { base, .. } => self.is_object(*base),
            // Every operand: `A|B` is always truthy, `A|int` is not.
            phrost_ty::TypeRef::Union(parts) | phrost_ty::TypeRef::Intersection(parts) => self
                .skeleton
                .types()
                .operands(*parts)
                .to_vec()
                .iter()
                .all(|part| self.is_object(*part)),
            _ => false,
        }
    }

    /// The same, for a type as written in the file rather than as stored.
    fn is_written_object(&self, id: phrost_hir::TypeRefId) -> bool {
        let Some(hir) = self.current().map(|open| open.hir()) else {
            return false;
        };
        match hir.type_ref(id) {
            phrost_hir::TypeRefData::Named(_) | phrost_hir::TypeRefData::Relative(_) => true,
            phrost_hir::TypeRefData::Generic { base, .. } => self.is_written_object(*base),
            phrost_hir::TypeRefData::Union(parts)
            | phrost_hir::TypeRefData::Intersection(parts) => hir
                .type_ref_list(*parts)
                .to_vec()
                .iter()
                .all(|part| self.is_written_object(*part)),
            _ => false,
        }
    }
}
