//! What a held type *is*, as a test of it would see.
//!
//! Yes/no questions with no class in the answer: which builtin kind a value has, whether null can
//! reach it, whether a call comes back at all. Grouped because they share one shape -- fold the
//! same question over every operand of a composite, with a rule per composite for how the answers
//! combine -- and because they are what the guards in [`super::narrowing`] and the null
//! diagnostics consult, neither of which wants a class.

use phrost_hir::{ExprId, FileHir, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, TextSize};
use phrost_ty::TypeId;

use crate::{references::SentinelReach, resolve::Held, view::IndexView};

/// What a value of one kind answers to a test for another. See [`IndexView::kind_answers`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Answers {
    Yes,
    No,
    /// Neither: the kinds overlap without either implying the other, so the operand survives both
    /// branches of the test.
    Maybe,
}

impl IndexView<'_> {
    /// What kind of value a held type is, as a type predicate would see it.
    ///
    /// A refined string is a `string` here, as it is to `is_string`: what a string is known to hold
    /// is a fact about its value and not about its kind, and every reader of a kind -- the guards,
    /// the impossible-use checks, a conditional's `is string` -- asks the question PHP's predicates
    /// answer. The three spellings each widen it, so none of them answers differently.
    pub(crate) fn kind_of_held(&self, hir: &FileHir, held: &Held) -> Option<BuiltinTy> {
        match held {
            Held::Object(_) => Some(phrost_model::BuiltinTy::Object),
            Held::Builtin(builtin) => Some(builtin.unrefined()),
            Held::Class(_) => Some(BuiltinTy::Object),
            Held::ListOf(_) | Held::MapOf { .. } | Held::Record(_) => Some(BuiltinTy::Array),
            // Exactly what a bare `callable` answers, which is the point: knowing what invoking it
            // gives back says nothing about what sort of value it is. A callable is a string, a
            // two-element array or an object, and `is_callable` is the only test that settles it.
            Held::Callable(_) => Some(BuiltinTy::Callable),
            Held::Nullable(inner) => self.kind_of_held(hir, inner),
            Held::Written(id) => Self::written_kind(hir, *id),
            Held::Stored { ty, .. } => self.stored_kind(*ty),
            // Several kinds at once is no single kind, which is the honest answer: a predicate about
            // one of them says nothing about the value as a whole. Several alternatives of *one*
            // kind are that kind, though, and a union of them is built more often than it reads:
            // a join of `string` written in one branch with the `?string` a call returned in
            // another is two spellings `union` cannot fold into one, and answering nothing for it
            // made `preg_replace($p, $r, $joined)` settle neither branch of its `$subject is
            // string` -- a `string[]` on a value that is never one.
            Held::OneOf(operands) => {
                let mut kinds = operands
                    .iter()
                    .map(|operand| self.kind_of_held(hir, operand));
                let first = kinds.next()??;
                kinds.all(|kind| kind == Some(first)).then_some(first)
            }
            // An intersection is every operand at once, so any kind one of them names is the kind of
            // the whole -- and two operands naming two kinds is a type nothing can inhabit.
            Held::AllOf(operands) => operands
                .iter()
                .find_map(|operand| self.kind_of_held(hir, operand)),
            Held::Applied { .. } => Some(BuiltinTy::Object),
        }
    }

    /// What kind of value a written type is, as a type predicate would see it.
    pub(super) fn written_kind(hir: &FileHir, id: TypeRefId) -> Option<BuiltinTy> {
        match hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => Some(builtin.unrefined()),
            // `list<Foo>` is an array as surely as `array` is: the arguments say what is in it and
            // the base says what it is.
            TypeRefData::Generic { base, .. } => Self::written_kind(hir, *base),
            TypeRefData::Shape { .. } => Some(BuiltinTy::Array),
            TypeRefData::ObjectShape { .. } => Some(BuiltinTy::Object),
            TypeRefData::Nullable(inner) => Self::written_kind(hir, *inner),
            // PHPDoc has spellings for an array that PHP has no keyword for, so they arrive as
            // class names. This is the one question where that matters.
            TypeRefData::Named(name) => {
                match is_an_array_spelling(hir.name_ref(*name).path.as_str()) {
                    true => Some(BuiltinTy::Array),
                    // A class instance answers `is_object` and nothing else here: whether it is
                    // callable depends on an `__invoke` this does not look for.
                    false => Some(BuiltinTy::Object),
                }
            }
            TypeRefData::Relative(_) => Some(BuiltinTy::Object),
            _ => None,
        }
    }

    /// The same, for an interned type.
    pub(crate) fn stored_kind(&self, id: TypeId) -> Option<BuiltinTy> {
        match self.skeleton.types().get(id) {
            phrost_ty::TypeRef::Builtin(builtin) => Some(builtin.unrefined()),
            phrost_ty::TypeRef::Generic { base, .. } => self.stored_kind(*base),
            phrost_ty::TypeRef::Shape { .. } => Some(BuiltinTy::Array),
            // An object whose properties are named is an object first.
            phrost_ty::TypeRef::ObjectShape { .. } => Some(BuiltinTy::Object),
            phrost_ty::TypeRef::Nullable(inner) => self.stored_kind(*inner),
            phrost_ty::TypeRef::Named(name) => match is_an_array_spelling(name.as_str()) {
                true => Some(BuiltinTy::Array),
                false => Some(BuiltinTy::Object),
            },
            _ => None,
        }
    }

    /// What a value is known to hold when every value it can be is a string: one of the three string
    /// types, the weakest its alternatives agree on. `None` where it may be something else -- a
    /// null included, which is no string at all.
    ///
    /// The question [`Self::kind_of_held`] deliberately does not answer, since a kind is the same for
    /// all three. Asked through every spelling at once, because a refinement arrives in each: a
    /// built literal, a written `@param`, an interned return -- and a `class-string<T>`, which is a
    /// generic over `string` naming a class and so never empty and never `'0'`.
    pub(crate) fn string_held(&self, hir: &FileHir, held: &Held) -> Option<BuiltinTy> {
        match held {
            Held::Builtin(builtin) => builtin.is_string().then_some(*builtin),
            Held::Written(id) => Self::written_string_held(hir, *id),
            Held::Stored { ty, .. } => self.stored_string_held(*ty),
            Held::OneOf(operands) => operands
                .iter()
                .try_fold(BuiltinTy::NonFalsyString, |seen, operand| {
                    seen.string_join(self.string_held(hir, operand)?)
                }),
            _ => None,
        }
    }

    fn written_string_held(hir: &FileHir, id: TypeRefId) -> Option<BuiltinTy> {
        match hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => builtin.is_string().then_some(*builtin),
            TypeRefData::Generic { base, args, .. }
                if !hir.type_ref_list(*args).is_empty()
                    && matches!(hir.type_ref(*base), TypeRefData::Builtin(BuiltinTy::String)) =>
            {
                Some(BuiltinTy::NonFalsyString)
            }
            TypeRefData::Union(parts) => hir
                .type_ref_list(*parts)
                .iter()
                .try_fold(BuiltinTy::NonFalsyString, |seen, &part| {
                    seen.string_join(Self::written_string_held(hir, part)?)
                }),
            _ => None,
        }
    }

    fn stored_string_held(&self, id: TypeId) -> Option<BuiltinTy> {
        let types = self.skeleton.types();
        match types.get(id) {
            phrost_ty::TypeRef::Builtin(builtin) => builtin.is_string().then_some(*builtin),
            phrost_ty::TypeRef::Generic { base, args, .. }
                if !args.is_empty()
                    && matches!(
                        types.get(*base),
                        phrost_ty::TypeRef::Builtin(BuiltinTy::String)
                    ) =>
            {
                Some(BuiltinTy::NonFalsyString)
            }
            phrost_ty::TypeRef::Union(parts) => types
                .operands(*parts)
                .iter()
                .try_fold(BuiltinTy::NonFalsyString, |seen, &part| {
                    seen.string_join(self.stored_string_held(part)?)
                }),
            _ => None,
        }
    }

    /// What a value is known to hold as a piece of a string a `.` or an interpolation builds: what
    /// [`Self::string_held`] says, or the digits a number is spelled in, or nothing.
    ///
    /// Never `None`, because a piece that holds nothing as far as this knows is still a piece -- it
    /// just adds nothing to what the others hold.
    pub(crate) fn piece_held(&self, hir: &FileHir, held: &Held) -> BuiltinTy {
        if let Some(string) = self.string_held(hir, held) {
            return string;
        }
        if self.null_reach_of(hir, held) != SentinelReach::Impossible {
            return BuiltinTy::String;
        }
        match self.kind_of_held(hir, held) {
            Some(BuiltinTy::Int | BuiltinTy::Float) => BuiltinTy::NonEmptyString,
            Some(BuiltinTy::True) => BuiltinTy::NonFalsyString,
            _ => BuiltinTy::String,
        }
    }

    /// What a value of one kind answers to a test for another.
    ///
    /// Equality is the wrong test on both sides of a predicate, because two of PHP's kinds are not
    /// families of their own. An **array is iterable**, so `is_iterable($x)` on an `array|string`
    /// keeps the array and drops the string -- read as equality it dropped both and the union was
    /// left standing whole. An **object may be** iterable or callable and may not, so it survives
    /// either branch: which it is depends on a `Traversable` or an `__invoke` this does not look
    /// for.
    ///
    /// Three answers rather than two, because a `Maybe` has to survive *both* branches. Dropping an
    /// object from the `!is_callable` side would claim every object callable, which is the mirror of
    /// the bug this fixes.
    pub(crate) fn kind_answers(operand: BuiltinTy, tested: BuiltinTy) -> Answers {
        use BuiltinTy::{Array, Bool, Callable, False, Iterable, Mixed, Object, String, True};

        if operand == tested {
            return Answers::Yes;
        }
        match (tested, operand) {
            // Nothing is known about what it is, so nothing is known about what it answers.
            (_, Mixed) | (Mixed, _) => Answers::Maybe,
            // An array is iterable by the language; an object is iterable if it is a `Traversable`.
            (Iterable, Array) => Answers::Yes,
            (Iterable, Object) | (Array | Object, Iterable) => Answers::Maybe,
            // A callable is a function name, a two-element array or an object with `__invoke`, and
            // which of the three it is is not a fact about the value's kind.
            (Callable, String | Array | Object) | (String | Array | Object, Callable) => {
                Answers::Maybe
            }
            // `true` and `false` are bools, and a bool is one of them.
            (Bool, True | False) => Answers::Yes,
            (True | False, Bool) => Answers::Maybe,
            _ => Answers::No,
        }
    }

    /// Whether two kinds rule each other out -- whether one value can answer to both.
    ///
    /// `is_string` and `is_array` cannot both be true, so a guard naming one of them contradicts a
    /// value believed to be the other, and the guard is then the better claim: it is about this
    /// position and the declaration is about every one. `callable` and `iterable` name no family
    /// of their own and so exclude nothing -- a callable is a string, a two-element array or an
    /// object depending on how it was written, and an iterable is an array or a `Traversable` --
    /// so a guard naming either says something *beside* what the value already was, and keeping
    /// the class is what lets `is_callable($handler)` leave a handler resolvable.
    ///
    /// `mixed` is not a claim at all, so nothing excludes it and nothing it excludes; the caller
    /// that cares treats it as the absence of one.
    pub(crate) fn kinds_exclude(one: BuiltinTy, other: BuiltinTy) -> bool {
        match (Self::exclusive_family(one), Self::exclusive_family(other)) {
            (Some(one), Some(other)) => one != other,
            _ => false,
        }
    }

    /// Which family of mutually exclusive kinds a kind belongs to, if it is in one.
    ///
    /// `true` and `false` are in `bool`'s, because a value that is one is a bool and a guard about
    /// booleans rules neither out.
    fn exclusive_family(kind: BuiltinTy) -> Option<u8> {
        Some(match kind {
            BuiltinTy::Int => 0,
            BuiltinTy::Float => 1,
            BuiltinTy::String | BuiltinTy::NonEmptyString | BuiltinTy::NonFalsyString => 2,
            BuiltinTy::Bool | BuiltinTy::True | BuiltinTy::False => 3,
            BuiltinTy::Array => 4,
            BuiltinTy::Object => 5,
            BuiltinTy::Null => 6,
            // Its own family: a resource is not an object, and `is_object($fh)` is false for one.
            BuiltinTy::Resource => 7,
            BuiltinTy::Callable
            | BuiltinTy::Iterable
            | BuiltinTy::Mixed
            | BuiltinTy::Void
            | BuiltinTy::Never => return None,
        })
    }

    /// Whether null can be in a held type.
    ///
    /// Structural, and about the type alone: whether a *guard* took the null off already is settled
    /// before this is asked, because narrowing is what produced the held type in the first place.
    /// That is the whole reason this check is affordable to turn on -- without truthiness narrowing
    /// it would fire on every `if ($x) { $x->m(); }` in a project.
    pub(crate) fn null_reach_of(&self, hir: &FileHir, held: &Held) -> SentinelReach {
        self.sentinel_reach_of(hir, held, BuiltinTy::Null)
    }

    /// Whether `false` can be in a held type.
    ///
    /// The other value PHP's standard library hands back to mean *not there*: `reset()` on an array
    /// nothing proved non-empty, a `strpos()` that was not compared, `end()`, `next()`. Answered by
    /// the same walk as the null question and named separately, because one name for both would lie
    /// at one of the two call sites and two walks would drift apart.
    pub(crate) fn false_reach_of(&self, hir: &FileHir, held: &Held) -> SentinelReach {
        self.sentinel_reach_of(hir, held, BuiltinTy::False)
    }

    /// Whether a held type *proves* the sentinel absent, which the two reach questions above do not
    /// say on their own: they answer `Impossible` for `mixed` too, because a check about a value
    /// nobody typed has to stay quiet. A guard that reads another value's type to narrow this one
    /// must not take that silence for proof -- `$x === Config::ANY` against a `const mixed ANY`
    /// leaves `$x` whatever it was.
    pub(crate) fn proves_absent(&self, hir: &FileHir, held: &Held, sentinel: BuiltinTy) -> bool {
        !self.held_absorbs(hir, held)
            && self.sentinel_reach_of(hir, held, sentinel) == SentinelReach::Impossible
    }

    fn sentinel_reach_of(&self, hir: &FileHir, held: &Held, sentinel: BuiltinTy) -> SentinelReach {
        self.held_reach_within(hir, held, sentinel, false)
    }

    /// `bool_takes_false` carries the same distinction the other two walks make -- see
    /// [`Self::written_reach_within`]. A union is a union in three spellings here:
    /// [`Held::OneOf`], an interned `TypeRef::Union` and a written `TypeRefData::Union`, and the
    /// flag has to travel through all three or the one it does not reach answers differently.
    fn held_reach_within(
        &self,
        hir: &FileHir,
        held: &Held,
        sentinel: BuiltinTy,
        bool_takes_false: bool,
    ) -> SentinelReach {
        if self.held_absorbs(hir, held) {
            return SentinelReach::Impossible;
        }
        match held {
            Held::Object(_) => self.held_reach_within(
                hir,
                &Held::Builtin(phrost_model::BuiltinTy::Object),
                sentinel,
                bool_takes_false,
            ),
            // A class name and a cast both state something a sentinel is not.
            Held::Class(_) => SentinelReach::Impossible,
            Held::Builtin(builtin) if *builtin == sentinel => SentinelReach::Certain,
            Held::Builtin(BuiltinTy::Bool) if sentinel == BuiltinTy::False && bool_takes_false => {
                SentinelReach::Possible
            }
            Held::Builtin(_) => SentinelReach::Impossible,
            // The wrapper *is* the null and says nothing about what it wraps. To a reader asking
            // about another sentinel it is one alternative and the inside is the other, so the
            // inside is what answers -- and whatever was certain there is only possible here,
            // because the value may be the null instead.
            Held::Nullable(inner) => match sentinel {
                BuiltinTy::Null => SentinelReach::Possible,
                _ => self
                    .held_reach_within(hir, inner, sentinel, bool_takes_false)
                    .at_most_possible(),
            },
            Held::ListOf(_) | Held::MapOf { .. } | Held::Record(_) => SentinelReach::Impossible,
            // A closure literal is a value as surely as a `new` is. What it returns may well be
            // null, and that is a claim about the *invocation*, which carries its own type.
            Held::Callable(_) => SentinelReach::Impossible,
            Held::Written(id) => Self::written_reach_within(hir, *id, sentinel, bool_takes_false),
            Held::Stored { ty, .. } => self.stored_reach_within(*ty, sentinel, bool_takes_false),
            // A `new` never gives back null, whatever it was given.
            Held::Applied { .. } => SentinelReach::Impossible,
            // Null cannot reach an intersection unless it reaches every operand: the value answers
            // to all of them, so one that cannot be null settles it -- which an `instanceof` guard
            // always does.
            Held::AllOf(operands) => {
                operands
                    .iter()
                    .fold(SentinelReach::Certain, |reach, operand| {
                        match (
                            reach,
                            self.held_reach_within(hir, operand, sentinel, bool_takes_false),
                        ) {
                            (SentinelReach::Impossible, _) | (_, SentinelReach::Impossible) => {
                                SentinelReach::Impossible
                            }
                            (SentinelReach::Certain, SentinelReach::Certain) => {
                                SentinelReach::Certain
                            }
                            _ => SentinelReach::Possible,
                        }
                    })
            }
            // Null reaches a union if it reaches any operand: the value is one of them and nothing
            // here says which.
            Held::OneOf(operands) => {
                // The third spelling of a union, and the flag has to reach it: `object|bool`
                // arrives here when a guard has taken the type apart, and a `bool` operand beside
                // another is the `false` a docblock meant.
                let bool_takes_false = bool_takes_false || operands.len() > 1;
                operands
                    .iter()
                    .fold(SentinelReach::Impossible, |reach, operand| {
                        match (
                            reach,
                            self.held_reach_within(hir, operand, sentinel, bool_takes_false),
                        ) {
                            // Certain only when every operand is: one that cannot be null makes the whole
                            // value merely possible.
                            (SentinelReach::Certain, SentinelReach::Certain) => {
                                SentinelReach::Certain
                            }
                            (SentinelReach::Impossible, SentinelReach::Impossible) => {
                                SentinelReach::Impossible
                            }
                            _ => SentinelReach::Possible,
                        }
                    })
            }
        }
    }

    /// `bool_takes_false` says whether this type is an operand of a union, which is the whole of what
    /// makes a `bool` a *sentinel* rather than a boolean.
    ///
    /// `object|bool` is how a docblock spells `object|false`, and that `bool` is a failure nobody
    /// checked. A parameter declared plainly `bool` is not: it is a boolean, and passing one where
    /// an `int` is expected is ordinary weak-mode PHP -- `takesInt($flag)` in a file without
    /// `strict_types` coerces, which was run against 8.5.10. Reading the bare `bool` as a sentinel
    /// reported every such call as a call nobody checked.
    fn written_reach_within(
        hir: &FileHir,
        id: TypeRefId,
        sentinel: BuiltinTy,
        bool_takes_false: bool,
    ) -> SentinelReach {
        if Self::written_absorbs(hir, id) {
            return SentinelReach::Impossible;
        }
        match hir.type_ref(id) {
            TypeRefData::Nullable(inner) => match sentinel {
                BuiltinTy::Null => SentinelReach::Possible,
                _ => Self::written_reach_within(hir, *inner, sentinel, bool_takes_false)
                    .at_most_possible(),
            },
            TypeRefData::Builtin(builtin) if *builtin == sentinel => SentinelReach::Certain,
            TypeRefData::Builtin(BuiltinTy::Bool)
                if sentinel == BuiltinTy::False && bool_takes_false =>
            {
                SentinelReach::Possible
            }
            // `Foo|null` is how a docblock spells `?Foo`, and far more often. A union of nothing but
            // null is `null`, which a generated docblock does produce.
            TypeRefData::Union(parts) => {
                // Recursive, because an operand can be nullable in its own right. `string|?int` is
                // how a docblock spells this far more often than `string|int|null`, and counting
                // only bare `null` operands read it as never-null -- which silently under-reported
                // every nullable hidden inside a union.
                let parts = hir.type_ref_list(*parts);
                // Beside another operand, which is what makes a `bool` here the `false` spelling.
                // A union of nothing but bools is still a boolean.
                let bool_takes_false = bool_takes_false || parts.len() > 1;
                let reaches = parts
                    .iter()
                    .map(|&part| Self::written_reach_within(hir, part, sentinel, bool_takes_false))
                    .collect::<Vec<_>>();
                if reaches.iter().all(|reach| *reach == SentinelReach::Certain) {
                    SentinelReach::Certain
                } else if reaches
                    .iter()
                    .any(|reach| *reach != SentinelReach::Impossible)
                {
                    SentinelReach::Possible
                } else {
                    SentinelReach::Impossible
                }
            }
            // Everything else names something that is not null. `mixed` included: it does hold null,
            // but it names no class either, so the access has no members to be reached at all and
            // nothing here would be reported about it.
            _ => SentinelReach::Impossible,
        }
    }

    /// Whether a declared type accepts a sentinel.
    ///
    /// `mixed` counts for either, being PHP's "anything". `bool` counts for `false` alone, and is
    /// why this is not just the reach walk: a declaration saying `bool` names the `false` without
    /// spelling it, where nothing names a null without spelling it.
    pub(crate) fn type_admits(&self, ty: phrost_ty::TypeId, sentinel: BuiltinTy) -> bool {
        // `mixed` counts for either sentinel and is tested only here rather than in the walk, which
        // answers the *other* question: whether a value can be one. A `mixed` value can be null and
        // names no class, so the receiver check has nothing to say about it and the reach walk
        // deliberately answers `Impossible` -- see [`Self::written_reach_within`].
        //
        // Through a union, because `string|null` and `?string` are one claim written two ways and
        // the table keeps whichever was written.
        self.stored_absorbs(ty)
            || self.stored_reach_within(ty, sentinel, true) != SentinelReach::Impossible
    }

    /// Whether a type is `mixed` in every way that matters to a sentinel: `mixed` itself, or a union
    /// or nullable with `mixed` inside.
    ///
    /// `mixed` already holds every value, which is why PHP refuses to compile `mixed|null` or
    /// `?mixed` natively. A docblock can still write them -- WordPress's
    /// `WP_REST_Request::offsetGet` says `@return mixed|null`, Laravel's `Gate` takes
    /// `Authenticatable|mixed` -- and read operand by operand, the first became a claim that null is
    /// *possible* where a bare `mixed` claims nothing, and the second a parameter refusing the null
    /// its `mixed` takes. The union is kept as written, because `Row|mixed` still names the class a
    /// reader wants completed; it is only the sentinel questions that have to see it as `mixed`.
    fn held_absorbs(&self, hir: &FileHir, held: &Held) -> bool {
        match held {
            Held::Object(_) => false,
            Held::Builtin(BuiltinTy::Mixed) => true,
            Held::Nullable(inner) => self.held_absorbs(hir, inner),
            Held::OneOf(operands) => operands
                .iter()
                .any(|operand| self.held_absorbs(hir, operand)),
            Held::Written(id) => Self::written_absorbs(hir, *id),
            Held::Stored { ty, .. } => self.stored_absorbs(*ty),
            Held::Builtin(_)
            | Held::Class(_)
            | Held::Applied { .. }
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Record(_)
            | Held::Callable(_)
            | Held::AllOf(_) => false,
        }
    }

    /// [`Self::held_absorbs`], written in this file's arena.
    fn written_absorbs(hir: &FileHir, id: TypeRefId) -> bool {
        match hir.type_ref(id) {
            TypeRefData::Builtin(BuiltinTy::Mixed) => true,
            TypeRefData::Nullable(inner) => Self::written_absorbs(hir, *inner),
            TypeRefData::Union(parts) => hir
                .type_ref_list(*parts)
                .iter()
                .any(|&part| Self::written_absorbs(hir, part)),
            _ => false,
        }
    }

    /// [`Self::held_absorbs`], interned.
    fn stored_absorbs(&self, ty: phrost_ty::TypeId) -> bool {
        let types = self.skeleton.types();
        match types.get(ty) {
            phrost_ty::TypeRef::Builtin(BuiltinTy::Mixed) => true,
            phrost_ty::TypeRef::Nullable(inner) => self.stored_absorbs(*inner),
            phrost_ty::TypeRef::Union(operands) => types
                .operands(*operands)
                .iter()
                .any(|&operand| self.stored_absorbs(operand)),
            _ => false,
        }
    }

    /// The same question of a declaration that was never interned.
    ///
    /// A closure's `fn (Row $r): Item` is an index into the file's own arena -- see
    /// [`crate::compat::Expectation`] for why a reading view cannot intern one -- and asking this of
    /// the two spellings through one entry is what keeps them one rule. The `@var` branch was
    /// blind to two rules for a year by being asked somewhere else.
    pub(crate) fn expectation_admits(
        &self,
        hir: &FileHir,
        expected: crate::compat::Expectation,
        sentinel: BuiltinTy,
    ) -> bool {
        match expected {
            crate::compat::Expectation::Stored(ty) => self.type_admits(ty, sentinel),
            crate::compat::Expectation::Written(id) => {
                Self::written_absorbs(hir, id)
                    || Self::written_reach_within(hir, id, sentinel, true)
                        != SentinelReach::Impossible
            }
        }
    }

    /// See [`Self::written_reach_within`] for what `bool_takes_false` decides.
    fn stored_reach_within(
        &self,
        ty: phrost_ty::TypeId,
        sentinel: BuiltinTy,
        bool_takes_false: bool,
    ) -> SentinelReach {
        if self.stored_absorbs(ty) {
            return SentinelReach::Impossible;
        }
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => match sentinel {
                BuiltinTy::Null => SentinelReach::Possible,
                _ => self
                    .stored_reach_within(*inner, sentinel, bool_takes_false)
                    .at_most_possible(),
            },
            phrost_ty::TypeRef::Builtin(builtin) if *builtin == sentinel => SentinelReach::Certain,
            // The same reading of `bool` the written side gives, and the same condition on it.
            phrost_ty::TypeRef::Builtin(BuiltinTy::Bool)
                if sentinel == BuiltinTy::False && bool_takes_false =>
            {
                SentinelReach::Possible
            }
            // The same nesting the written side has: an interned `string|?int` hides its null one
            // level down, and reading the operands only for a bare `null` misses it.
            phrost_ty::TypeRef::Union(operands) => {
                let types = self.skeleton.types();
                let operands = types.operands(*operands);
                let bool_takes_false = bool_takes_false || operands.len() > 1;
                let reaches = operands
                    .iter()
                    .map(|&operand| self.stored_reach_within(operand, sentinel, bool_takes_false))
                    .collect::<Vec<_>>();
                if reaches.iter().all(|reach| *reach == SentinelReach::Certain) {
                    SentinelReach::Certain
                } else if reaches
                    .iter()
                    .any(|reach| *reach != SentinelReach::Impossible)
                {
                    SentinelReach::Possible
                } else {
                    SentinelReach::Impossible
                }
            }
            _ => SentinelReach::Impossible,
        }
    }

    /// Whether a call written as a statement leaves the function for good.
    ///
    /// The half of "this block cannot fall through" that syntax cannot answer. `return`, `throw`
    /// and `exit` are decided where the statements are; a call to something declared `: never` --
    /// `Assert::fail()`, a project's own `abort()` helper -- is written exactly like a call that
    /// comes back, so only the callee's declaration says otherwise.
    ///
    /// Guarded against re-entry: answering this resolves the call's receiver, and resolving
    /// anything consults the guards before it, one of which may be this very call.
    pub(crate) fn call_never_returns(&self, at: TextSize, hir: &FileHir, expr: ExprId) -> bool {
        // The name first, and the resolution only if the name could possibly answer yes.
        //
        // This is what makes the question affordable rather than a thing every caller has to budget
        // for. `never` is a property of the callee's *declaration*, and finding the callee means
        // resolving its receiver -- a full chain resolution to learn one bit that is almost always
        // the same bit. The project-wide list of names anything returns `never` from is four entries
        // on a real Drupal checkout, two of them syntax that never reaches here, so a scan of a
        // handful of short strings answers for every call in the project bar a handful.
        //
        // A computed name (`$obj->$method()`) resolves nothing here and is not a miss: nothing else
        // in this layer claims to know what such a call reaches either.
        let Some(name) = Self::called_name(hir, expr) else {
            return false;
        };
        if !self.skeleton.declares_never_returning(name) {
            return false;
        }
        // Past the name, the answer depends on where it is asked and on whether another such
        // question is open -- which the narrowing memo has to know, so it keeps nothing computed
        // across this. See `phrost_body::ProvedMemo`.
        self.proved.note_position_dependent();
        if self.answering_never.get() {
            return false;
        }
        self.answering_never.set(true);
        let answer = phrost_body::value_path_of(hir, expr)
            .and_then(|path| self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT))
            .is_some_and(|held| self.holds_never(&held));
        self.answering_never.set(false);
        answer
    }

    /// Whether a called function's declaration promises something about one of its arguments.
    ///
    /// The gate in front of a narrowing root, and the reason it is a name scan rather than a
    /// resolution is the one [`Self::call_never_returns`] gives: `if (f($x))` is everywhere, what
    /// `f` promises is in another file, and the answer is no for nearly every name. Unlike that
    /// one this needs no resolution at all afterwards -- the tag is read later, at the root, by
    /// whoever resolves the declaration -- so there is nothing here to guard against re-entry.
    ///
    /// The short name, because that is what both sides of the comparison hold: the project's list
    /// is filled from a declaration's short name and a call writes one. A namespaced call to a
    /// function of the same short name therefore passes the gate and is settled by the resolution
    /// at the root, which is the safe direction -- a superset costs one lookup.
    pub(crate) fn call_asserts_argument(&self, hir: &FileHir, name: phrost_hir::NameRefId) -> bool {
        self.skeleton
            .declares_argument_assertion(phrost_model::short_name(hir.name_ref(name).path.as_str()))
    }

    /// The name a call writes, read off the syntax rather than resolved.
    ///
    /// `None` for a call whose name is computed and for anything that is not a call -- both of which
    /// this layer has nothing to say about anyway.
    fn called_name(hir: &FileHir, expr: ExprId) -> Option<&str> {
        match hir.expr(expr) {
            phrost_hir::body::ExprData::MethodCall { name, .. }
            | phrost_hir::body::ExprData::StaticCall { name, .. } => {
                name.as_ref().map(|name| name.as_str())
            }
            phrost_hir::body::ExprData::FunctionCall { name, .. } => {
                Some(phrost_model::short_name(hir.name_ref(*name).path.as_str()))
            }
            _ => None,
        }
    }

    fn holds_never(&self, held: &Held) -> bool {
        match held {
            Held::Builtin(BuiltinTy::Never) => true,
            Held::Stored { ty, .. } => matches!(
                self.skeleton.types().get(*ty),
                phrost_ty::TypeRef::Builtin(BuiltinTy::Never)
            ),
            _ => false,
        }
    }
}

/// Whether a name is one of PHPDoc's words for an array.
///
/// PHP has no keyword for these, so they are lowered as class names like any other. Nothing else
/// treats them specially -- what a `list<T>` holds is read off its arguments the same way.
fn is_an_array_spelling(name: &str) -> bool {
    ["list", "non-empty-list", "non-empty-array"]
        .iter()
        .any(|spelling| name.eq_ignore_ascii_case(spelling))
}

impl<'a> IndexView<'a> {
    /// The reader's answers to the narrowing walk's questions about calls, with its memo.
    pub(crate) fn asserting(&self) -> ViewAsserts<'_, 'a> {
        ViewAsserts { view: self }
    }
}

/// What the narrowing walk asks about calls, answered by a view -- see [`IndexView::asserting`].
pub(crate) struct ViewAsserts<'v, 'a> {
    view: &'v IndexView<'a>,
}

impl phrost_body::Asserts for ViewAsserts<'_, '_> {
    fn asserts(&self, hir: &FileHir, name: phrost_hir::NameRefId) -> bool {
        self.view.call_asserts_argument(hir, name)
    }

    fn memo(&self) -> Option<&phrost_body::ProvedMemo> {
        Some(&self.view.proved)
    }
}
