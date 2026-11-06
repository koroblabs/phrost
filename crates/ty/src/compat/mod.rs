//! Whether a value of one type may be used where another is expected.
//!
//! The judgement the checker is missing. Null is already checked on its own; everything else about
//! a type is currently unexamined, so `needs_string($anInt)` passes silently.
//!
//! # What lives here, and what cannot
//!
//! PHP's assignability is two separate questions wearing one name. One is about the *connectives* —
//! how `?T`, `A|B`, `A&B` and the builtin vocabulary compose, plus which scalars the engine will
//! convert and when. That is a question about the type table and nothing else, so it lives here.
//!
//! The other is nominal: whether `Foo` is a `BarInterface`. That needs a class hierarchy, and this
//! crate may depend on `phrost-model` and `lspkit-memsize` and nothing more — the same reason the
//! crate docs give for inference living above `phrost-sema`. So it is a hole, spelled
//! [`ClassHierarchy`], which a layer that *can* see the hierarchy fills in. Composition here,
//! meaning up there; no new edge in the crate graph either way.
//!
//! # Why there are three answers and not two
//!
//! [`Fit::Unknown`] is the load-bearing one. A checker is judged by its false positives, and a
//! wrong "expects string, int given" on framework code teaches a person to stop reading the list.
//! So the rule this module is built on is:
//!
//! > **Reject only what can be proven wrong.** If some value of the declared type could legally
//! > arrive, the answer is `Unknown` and the caller stays quiet.
//!
//! That makes one whole class of judgement deliberately lenient: passing something *broader* than
//! what is asked for. `object` where `Foo` is expected, `iterable` where `array` is expected, `bool`
//! where `true` is expected — each of those is a narrowing the value might well satisfy at runtime,
//! so none of them is reported. Other checkers do report them. This is a policy and not an
//! oversight; revisit it with a measurement against a real checkout, which is what the roadmap asks
//! of anything in this area.

#[cfg(test)]
mod tests;

use phrost_model::{BuiltinTy, Name};

use crate::{TypeId, TypeRef, table::TypeTable};

/// The verdict on one "can this be used there" question.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    /// Every value of the type is usable where it is going.
    Accepted,
    /// No value of the type can be, so a caller may report it.
    Rejected,
    /// Not decidable from what is known. Never report one of these.
    Unknown,
}

impl Fit {
    /// The verdict when *one* alternative is enough: a union on the expected side, an intersection
    /// on the value's side.
    ///
    /// `Unknown` outranks `Rejected` so that one undecidable branch is enough to keep quiet: the
    /// branch we could not judge may well have been the one that fits.
    fn any_of(verdicts: impl IntoIterator<Item = Self>) -> Self {
        let mut seen_unknown = false;
        for verdict in verdicts {
            match verdict {
                Self::Accepted => return Self::Accepted,
                Self::Unknown => seen_unknown = true,
                Self::Rejected => {}
            }
        }
        if seen_unknown {
            Self::Unknown
        } else {
            Self::Rejected
        }
    }

    /// The verdict when *every* alternative must hold: a union on the value's side, an intersection
    /// on the expected side.
    ///
    /// `Rejected` outranks `Unknown` here, and that is the asymmetry with [`Self::any_of`]: one
    /// branch proven impossible makes the whole thing impossible, whatever the others do.
    fn all_of(verdicts: impl IntoIterator<Item = Self>) -> Self {
        let mut seen_unknown = false;
        for verdict in verdicts {
            match verdict {
                Self::Rejected => return Self::Rejected,
                Self::Unknown => seen_unknown = true,
                Self::Accepted => {}
            }
        }
        if seen_unknown {
            Self::Unknown
        } else {
            Self::Accepted
        }
    }
}

/// A value's type, as a layer that could not intern it can describe one.
///
/// The counterpart to a [`TypeId`] for the value side of a question. Inference above this crate
/// produces types the table cannot hold -- a `Collection<Item>` built at a `new`, a class written
/// in the file being read, a union with one operand narrowed away -- and interning them would need
/// the table open for writing, which a reader does not have. Describing one costs an allocation
/// only for the shapes that actually compose; a plain class or builtin is a leaf.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ValueTy {
    /// An instance of this class. Type arguments, if the caller knew any, are deliberately dropped:
    /// the judgement ignores them on interned types too.
    Named(Name),
    Builtin(BuiltinTy),
    /// A type that *is* in the table, which is most of them.
    Interned(TypeId),
    /// An array whose element type nothing here knows.
    Array,
    /// An array whose element type the caller *does* know: `Row[]`, a list literal of one class.
    ///
    /// Apart from [`Self::Array`] because the two answer differently against an element-typed
    /// expectation, and only when *both* sides know is there anything to compare -- see
    /// [`Compatibility::arguments_fit`].
    ///
    /// `key` is `None` for every spelling that states a container without stating how it is keyed:
    /// `Row[]`, `array<Row>`, and anything a layer above inferred. Only `array<K, V>` fills it in.
    ArrayOf {
        key: Option<Box<ValueTy>>,
        element: Box<ValueTy>,
    },
    /// An array whose keys are known, each with what it holds: a record literal.
    ///
    /// Apart from [`Self::ArrayOf`] for the reason that one is apart from [`Self::Array`]: it is the
    /// only spelling that can be judged against a declared `array{...}` key by key, and against
    /// anything else it is simply an array.
    ///
    /// A record literal names every key it holds -- it wrote them, and the writes after it are
    /// folded in -- and so does a sealed shape written in the file being read; `open` is a shape
    /// written with `...`, which may hold others. Against a sealed `array{...}` a key it does not
    /// name is a rejection -- see [`Compatibility::record_against_shape`].
    Record {
        fields: Vec<RecordField>,
        open: bool,
    },
    /// `A|B` -- the value is one of these and nothing knows which.
    OneOf(Vec<ValueTy>),
    /// `A&B` -- the value answers to all of these at once.
    AllOf(Vec<ValueTy>),
    Nullable(Box<ValueTy>),
    /// Nothing could be determined. Always [`Fit::Unknown`], never a finding.
    Unknown,
}

/// One key of a [`ValueTy::Record`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RecordField {
    pub key: Name,
    pub value: ValueTy,
    /// Written `key?:` -- the array may not hold it at all. Never set for a literal.
    pub optional: bool,
}

/// Which representation the expected side is in.
///
/// A parameter, a property and a return are interned; a `/** @var T */` is an index into the file's
/// own arena and cannot be. Both are peeled the same way, and the two peels meet at the same leaf
/// rules -- which is the point of naming this rather than writing a second judgement.
#[derive(Clone, Copy)]
enum Expected<'a> {
    Interned(TypeId),
    Described(&'a ValueTy),
}

/// Which representation a *value* is in, for the one question both spellings answer alike.
///
/// [`Expected`] already names the same split for the other side. This one exists only so
/// [`Compatibility::element_fit`] can be written once instead of twice.
#[derive(Clone, Copy)]
enum Side<'a> {
    Interned(TypeId),
    Described(&'a ValueTy),
}

impl<'a> Side<'a> {
    /// The expected side, for the questions both sides answer alike.
    ///
    /// How a container is keyed is asked of the value and of the expectation in exactly the same
    /// words, so this is where one function can serve both instead of the pair
    /// [`Compatibility::expected_element`] and [`Compatibility::value_element`] already are.
    fn of(expected: Expected<'a>) -> Self {
        match expected {
            Expected::Interned(id) => Interned(id),
            Expected::Described(ty) => Described(ty),
        }
    }
}

use Side::{Described, Interned};

/// One key a value is known to hold, whichever spelling the value is in, for
/// [`Compatibility::keys_against_shape`].
struct HeldKey<'a> {
    key: &'a str,
    value: Side<'a>,
    /// Written `key?:` -- the value may not hold it at all.
    optional: bool,
}

/// Which of PHP's two key types an array's keys can be.
///
/// The vocabulary is closed at two: the engine normalises every key it accepts to an int or a
/// string, so `array<float, V>` is not a third kind and `array<Foo, V>` is not a type. That closure
/// is what makes the key question decidable -- and what makes anything outside the two *unstated*
/// rather than a kind of its own. `array<K, V>` is 114 of the prelude's signatures and says nothing
/// about keys.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct KeyKinds {
    int: bool,
    string: bool,
}

impl KeyKinds {
    const INT: Self = Self {
        int: true,
        string: false,
    };
    const STRING: Self = Self {
        int: false,
        string: true,
    };

    /// The two key types, and nothing else is one.
    fn of(builtin: BuiltinTy) -> Option<Self> {
        match builtin {
            BuiltinTy::Int => Some(Self::INT),
            // What a string holds says nothing about which kind of key it makes.
            string if string.is_string() => Some(Self::STRING),
            _ => None,
        }
    }

    fn either(self, other: Self) -> Self {
        Self {
            int: self.int || other.int,
            string: self.string || other.string,
        }
    }

    /// Every key this side can carry is one the other side accepts.
    fn within(self, other: Self) -> bool {
        (!self.int || other.int) && (!self.string || other.string)
    }

    /// No key either side could produce is one the other would recognise.
    fn shares_nothing_with(self, other: Self) -> bool {
        !(self.int && other.int) && !(self.string && other.string)
    }
}

/// A value reduced to something the expected side can be walked against.
#[derive(Clone, Copy)]
enum Leaf<'a> {
    Named(&'a Name),
    Builtin(BuiltinTy),
}

impl Leaf<'_> {
    /// A builtin as it behaves *as a value*.
    ///
    /// Only `void` differs, and it differs because PHP has no void value: a call to a `: void`
    /// function evaluates to NULL like any function that falls off its end, so consuming one where
    /// `?int` is wanted runs and yields NULL. Judging it as null keeps `?T` and a bare `null`
    /// target accepting it, and keeps `int` refusing it for the reason PHP would give.
    ///
    /// The *expected* side is untouched: returning a value from a `: void` declaration is a fatal,
    /// and that is a different question asked in the other direction.
    fn of(builtin: BuiltinTy) -> Self {
        match builtin {
            BuiltinTy::Void => Self::Builtin(BuiltinTy::Null),
            other => Self::Builtin(other),
        }
    }
}

/// The nominal half of the judgement, answered by a layer that can walk a hierarchy.
///
/// Both methods may return [`Fit::Unknown`], and an implementation is expected to: a class the
/// index has never seen has to be undecidable rather than absent, or every unresolved name becomes
/// a false finding.
pub trait ClassHierarchy {
    /// Whether a value declared `subject` may be used where `expected` is required.
    ///
    /// More than an ancestor lookup, which is why the whole question is delegated rather than just
    /// `is_subclass_of`. `Foo` where a `BarInterface` is expected is *not* provably wrong unless
    /// `Foo` is final: a subclass of `Foo` may implement `BarInterface`, and that subclass is what
    /// the caller actually passed. Only the implementor knows about finality, so only the
    /// implementor can tell a rejection from an unknown.
    fn relate(&self, subject: &Name, expected: &Name) -> Fit;

    /// Whether a value declared `subject` satisfies a builtin: `Traversable` for `iterable`,
    /// `Stringable` for `string` in weak mode, `__invoke` for `callable`.
    fn satisfies(&self, subject: &Name, expected: BuiltinTy) -> Fit;

    /// Whether the index holds a declaration for this class.
    ///
    /// Asked before a *builtin* is rejected for not being one, which is the half [`Self::relate`]
    /// could not answer: that one compares two classes and already says nothing when either is
    /// undeclared, and a builtin against a class never reached it. An `@return Row` whose `Row` is
    /// an imported type alias nothing expanded, or a `@template` nobody declared, names a class the
    /// project does not have -- and the array the method really hands back was rejected against it.
    ///
    /// A class nothing declares says nothing, which is the same rule as [`Self::relate`]'s and the
    /// same reason: silence keeps an unresolved name from becoming a finding.
    fn declared(&self, class: &Name) -> bool;
}

/// Judges one file's calls, under that file's `strict_types` setting.
///
/// Built per request rather than stored: it borrows the table and holds one bit, so constructing it
/// costs nothing, and binding the strict-types flag at construction is what stops a caller from
/// accidentally judging a call by the callee's file — see `IndexView::strict_types`.
pub struct Compatibility<'a, H: ClassHierarchy + ?Sized> {
    types: &'a TypeTable,
    classes: &'a H,
    /// Whether the file the *call* is written in declared `declare(strict_types=1)`.
    strict_types: bool,
}

impl<H: ClassHierarchy + ?Sized> std::fmt::Debug for Compatibility<'_, H> {
    /// The hierarchy is caller-supplied and has no rendering of its own; the directive is the only
    /// held state that changes an answer.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compatibility")
            .field("strict_types", &self.strict_types)
            .finish_non_exhaustive()
    }
}

impl<'a, H: ClassHierarchy + ?Sized> Compatibility<'a, H> {
    pub fn new(types: &'a TypeTable, classes: &'a H, strict_types: bool) -> Self {
        Self {
            types,
            classes,
            strict_types,
        }
    }

    /// Whether a value of `value` may be used where `expected` is declared.
    ///
    /// Recursion is bounded by construction: the table is built bottom-up and interned, so a type
    /// can never contain itself and the walk is as deep as the type is written.
    /// Whether a value of `value` may be used where `expected` is declared.
    ///
    /// Recursion is bounded by construction: the table is built bottom-up and interned, so a type
    /// can never contain itself and the walk is as deep as the type is written.
    pub fn fit(&self, value: TypeId, expected: TypeId) -> Fit {
        // Interning makes this exact rather than a shortcut: two ids are equal precisely when the
        // types are.
        if value == expected {
            return Fit::Accepted;
        }
        self.written_against(value, Expected::Interned(expected))
    }

    /// The same question when the value's type could not be interned.
    ///
    /// The expected side of a real site is always an interned type -- a parameter, a property, a
    /// return. The *value* side frequently is not: an inferred `Collection<Item>` would need the
    /// table open for writing, and a class written in the file being read is an index into that
    /// file's own arena. So a caller that has inferred a value describes it with [`ValueTy`] and
    /// gets the same judgement, through the same rules, without interning anything.
    pub fn fit_value(&self, value: &ValueTy, expected: TypeId) -> Fit {
        self.described_against(value, Expected::Interned(expected))
    }

    /// The same again when *neither* side could be interned.
    ///
    /// A `/** @var File $file */` is a type in the file's own arena, so an assignment checked
    /// against one has an uninterned expectation as well as an uninterned value. Nothing about the
    /// rules changes; only how the expected side is spelled.
    pub fn fit_described(&self, value: &ValueTy, expected: &ValueTy) -> Fit {
        self.described_against(value, Expected::Described(expected))
    }

    /// Peels a described value, whichever way the expectation is spelled.
    fn described_against(&self, value: &ValueTy, expected: Expected<'_>) -> Fit {
        if let Some(fit) = self.arguments_fit(Described(value), expected) {
            return fit;
        }
        // A bare `mixed` has its own rules below, and only the unions it absorbs are settled here.
        if !matches!(value, ValueTy::Builtin(BuiltinTy::Mixed)) && self.described_absorbs(value) {
            return self.leaf_against(Leaf::Builtin(BuiltinTy::Mixed), expected);
        }
        match value {
            ValueTy::Unknown => Fit::Unknown,
            ValueTy::Interned(id) => self.written_against(*id, expected),
            ValueTy::Named(name) => self.leaf_against(Leaf::Named(name), expected),
            ValueTy::Builtin(builtin) => self.leaf_against(Leaf::of(*builtin), expected),
            // An array of something is an array; what it holds is asked separately, above.
            ValueTy::Array | ValueTy::ArrayOf { .. } => {
                self.leaf_against(Leaf::Builtin(BuiltinTy::Array), expected)
            }
            // Key by key where the expectation names keys, and an ordinary array everywhere else.
            ValueTy::Record { fields, open } => match self.shape_awaited(expected) {
                Some(shape) => self.record_against_shape(fields, *open, shape),
                None => self.leaf_against(Leaf::Builtin(BuiltinTy::Array), expected),
            },
            // The null half has to land somewhere too, exactly as for a written `?T`.
            ValueTy::Nullable(inner) => Fit::all_of([
                self.leaf_against(Leaf::Builtin(BuiltinTy::Null), expected),
                self.described_against(inner, expected),
            ]),
            ValueTy::OneOf(operands) => Fit::all_of(
                operands
                    .iter()
                    .map(|operand| self.described_against(operand, expected)),
            ),
            // Against an intersection, part by part -- see [`Self::expected_parts`].
            ValueTy::AllOf(_) if let Some(parts) = self.expected_parts(expected) => Fit::all_of(
                parts
                    .into_iter()
                    .map(|part| self.described_against(value, part)),
            ),
            ValueTy::AllOf(operands) => Fit::any_of(
                operands
                    .iter()
                    .map(|operand| self.described_against(operand, expected)),
            ),
        }
    }

    /// The parts of an intersection the value is expected to be, looking through a `?`: a value
    /// that is itself an intersection is never null, so the null half has nothing to take.
    ///
    /// Asked before an intersection *value* is peeled, because the two quantifiers do not commute:
    /// `MockObject&Connection` reaching a `MockObject&Connection` is each expected part met by *some*
    /// value part, and peeling the value first asks whether one value part meets *every* expected
    /// part -- which no part of a two-part intersection does. That was invisible while a value
    /// typed as an interface answered `Unknown` against everything.
    fn expected_parts(&self, expected: Expected<'a>) -> Option<Vec<Expected<'a>>> {
        match expected {
            Expected::Interned(id) => {
                let id = match self.types.get(id) {
                    TypeRef::Nullable(inner) => *inner,
                    _ => id,
                };
                match self.types.get(id) {
                    TypeRef::Intersection(parts) => Some(
                        self.types
                            .operands(*parts)
                            .iter()
                            .map(|&part| Expected::Interned(part))
                            .collect(),
                    ),
                    _ => None,
                }
            }
            Expected::Described(ty) => {
                let ty = match ty {
                    ValueTy::Nullable(inner) => inner,
                    _ => ty,
                };
                match ty {
                    ValueTy::AllOf(parts) => Some(parts.iter().map(Expected::Described).collect()),
                    _ => None,
                }
            }
        }
    }

    /// Peels the *value* side down to a leaf, then hands over.
    ///
    /// Split from [`Self::leaf_against`] so that both entry points share one copy of every rule.
    /// The two sides are peeled separately and in this order because the quantifiers differ: each
    /// branch of a union on the value's side must find *some* home, where a union on the expected
    /// side needs only *one* branch to take the value. Interleaving them in one match made that
    /// easy to get backwards.
    fn written_against(&self, value: TypeId, expected: Expected<'_>) -> Fit {
        if let Some(fit) = self.arguments_fit(Interned(value), expected) {
            return fit;
        }
        if !matches!(self.types.get(value), TypeRef::Builtin(BuiltinTy::Mixed))
            && self.interned_absorbs(value)
        {
            return self.leaf_against(Leaf::Builtin(BuiltinTy::Mixed), expected);
        }
        match self.types.get(value) {
            // A call that returns `never` does not come back, so there is nothing to be wrong about.
            TypeRef::Builtin(BuiltinTy::Never) => Fit::Accepted,
            // `self`, `static` and `parent` mean a different class per receiver and are left
            // unresolved by design. A field never stands on its own.
            TypeRef::Relative(_) | TypeRef::Field { .. } => Fit::Unknown,
            TypeRef::Nullable(inner) => Fit::all_of([
                self.leaf_against(Leaf::Builtin(BuiltinTy::Null), expected),
                self.written_against(*inner, expected),
            ]),
            // Each branch has to land somewhere: a value that is `A|B` may be either.
            TypeRef::Union(branches) => Fit::all_of(
                self.types
                    .operands(*branches)
                    .iter()
                    .map(|&branch| self.written_against(branch, expected)),
            ),
            // Against an intersection, part by part -- see [`Self::expected_parts`].
            TypeRef::Intersection(_) if let Some(parts) = self.expected_parts(expected) => {
                Fit::all_of(
                    parts
                        .into_iter()
                        .map(|part| self.written_against(value, part)),
                )
            }
            // Any part is enough: the value answers to all of them at once.
            TypeRef::Intersection(parts) => Fit::any_of(
                self.types
                    .operands(*parts)
                    .iter()
                    .map(|&part| self.written_against(part, expected)),
            ),
            // A `class-string<T>`: a string, and one naming a class, which no empty string and no
            // `'0'` does. The base stays `string` so that the declaration side keeps accepting any
            // string where a class-string is asked for -- see `ParamRow::rejects` -- and the value
            // side says what it is known to hold.
            TypeRef::Generic { base, args, .. }
                if !args.is_empty()
                    && matches!(self.types.get(*base), TypeRef::Builtin(BuiltinTy::String)) =>
            {
                self.leaf_against(Leaf::Builtin(BuiltinTy::NonFalsyString), expected)
            }
            // Type arguments are docblock-only and unchecked everywhere else, so reading them here
            // would invent findings nothing else can corroborate. The base is what the value *is*.
            TypeRef::Generic { base, .. } => self.written_against(*base, expected),
            // A shape is an array whose keys are known: key by key against another shape, and
            // wherever an array goes otherwise.
            TypeRef::Shape { .. } => match self.shape_awaited(expected) {
                Some(shape) => self.shape_against_shape(value, shape),
                None => self.leaf_against(Leaf::Builtin(BuiltinTy::Array), expected),
            },
            // An object, wherever an object goes. What its properties hold is not judged: nothing
            // that expects one names them.
            TypeRef::ObjectShape { .. } => {
                self.leaf_against(Leaf::Builtin(BuiltinTy::Object), expected)
            }
            TypeRef::Named(name) => self.leaf_against(Leaf::Named(name), expected),
            TypeRef::Builtin(builtin) => self.leaf_against(Leaf::of(*builtin), expected),
        }
    }

    /// Whether a value is `mixed` for this judgement: `mixed`, or a union or nullable with `mixed`
    /// inside.
    ///
    /// `mixed` holds every value already, which is why PHP will not compile `mixed|null`; a
    /// docblock writes it anyway, and taken apart operand by operand its null half was judged on
    /// its own and refused where a bare `mixed` is judged not at all. A wider union keeps its other
    /// operands for every reader but this one -- `Row|mixed` still completes as a `Row`.
    fn described_absorbs(&self, value: &ValueTy) -> bool {
        match value {
            ValueTy::Builtin(BuiltinTy::Mixed) => true,
            ValueTy::Interned(id) => self.interned_absorbs(*id),
            ValueTy::Nullable(inner) => self.described_absorbs(inner),
            ValueTy::OneOf(operands) => operands
                .iter()
                .any(|operand| self.described_absorbs(operand)),
            _ => false,
        }
    }

    /// [`Self::described_absorbs`], interned.
    fn interned_absorbs(&self, value: TypeId) -> bool {
        match self.types.get(value) {
            TypeRef::Builtin(BuiltinTy::Mixed) => true,
            TypeRef::Nullable(inner) => self.interned_absorbs(*inner),
            TypeRef::Union(branches) => self
                .types
                .operands(*branches)
                .iter()
                .any(|&branch| self.interned_absorbs(branch)),
            _ => false,
        }
    }

    /// What the two sides' *type arguments* say, when both of them say something.
    ///
    /// `Row[]` given a `string[]` is the commonest real bug an array can carry in PHP, and both
    /// halves of it are knowable now: a parameter states its element in a docblock, and a value's
    /// element comes from an annotation, an inferred container or a list literal. The judgement used
    /// to drop both -- deliberately, and the doc comments said so -- because for a long time neither
    /// side knew, and reading one alone invents findings nothing can corroborate.
    ///
    /// `None` unless **both** know, which is what keeps that true. An array of unknown elements is
    /// still an array and answers exactly as it did; so is a shape, and so is `array<string, mixed>`,
    /// which is 290 of the 526 element-typed parameters on the reference project.
    ///
    /// Two questions, asked separately because a container can state either half without the other:
    /// `array<int, mixed>` says how it is keyed and nothing about what it holds, and `Row[]` the
    /// reverse. One answer is enough to judge on.
    ///
    /// The base is judged as well, and first in the pairing: `iterable<Row>` against `Row[]` is a
    /// question about `iterable` against `array` before it is one about `Row`.
    fn arguments_fit(&self, value: Side<'_>, expected: Expected<'_>) -> Option<Fit> {
        // `array<int>|array<string>` is all ints or all strings, and asking for "the element" of the
        // union answers nothing -- so `[1, 'b']` passed for an array. Each operand is the whole
        // question in turn, and any one of them is enough.
        if self.one_container(value)
            && let Some(branches) = self.union_of_containers(expected)
        {
            return Some(Fit::any_of(branches.iter().map(|&branch| {
                self.side_against(value, Expected::Interned(branch))
            })));
        }
        let element = self.element_fit(value, expected);
        let keys = self.key_fit(value, expected);
        if element.is_none() && keys.is_none() {
            return None;
        }
        Some(Fit::all_of(
            [
                Some(self.leaf_against(Leaf::Builtin(BuiltinTy::Array), expected)),
                element,
                keys,
            ]
            .into_iter()
            .flatten(),
        ))
    }

    /// The operands of an expected union that is a choice between containers -- each of them, but
    /// a null, a container stating what it holds or how it is keyed.
    ///
    /// That is the union written to say *which* array: `array<int>|array<string>`. One that mixes a
    /// container with a scalar or a class -- `array[]|string`, `string|iterable<string>` -- is
    /// written to say *whether* it is an array at all, and its container's element is where real
    /// docblocks are loosest: WordPress's `wp_kses()` takes `array[]|string` and every caller hands
    /// it `['code' => true]`. Measured on six corpora, judging those reported 34 lines nobody would
    /// change, and the element of such a union stays unjudged as it was.
    fn union_of_containers(&self, expected: Expected<'_>) -> Option<&[TypeId]> {
        let id = match expected {
            Expected::Interned(id) => id,
            Expected::Described(ValueTy::Interned(id)) => *id,
            Expected::Described(_) => return None,
        };
        let TypeRef::Union(branches) = self.types.get(id) else {
            return None;
        };
        let branches = self.types.operands(*branches);
        branches
            .iter()
            .all(|&branch| {
                matches!(self.types.get(branch), TypeRef::Builtin(BuiltinTy::Null))
                    || self.expected_element(Expected::Interned(branch)).is_some()
                    || self.stated_key(Interned(branch)).is_some()
            })
            .then_some(branches)
    }

    /// Whether a value is one container rather than a union of things: the expectation's union is
    /// distributed only over such a value, because a value's own union has to be taken apart first
    /// -- each of its operands finding *some* home -- and distributing the other side over it
    /// asked whether one operand of the expectation takes all of the value at once.
    fn one_container(&self, value: Side<'_>) -> bool {
        match value {
            Described(ValueTy::Record { .. } | ValueTy::ArrayOf { .. }) => true,
            Described(ValueTy::Interned(id)) => matches!(
                self.types.get(*id),
                TypeRef::Generic { .. } | TypeRef::Shape { .. }
            ),
            Interned(id) => matches!(
                self.types.get(id),
                TypeRef::Generic { .. } | TypeRef::Shape { .. }
            ),
            Described(_) => false,
        }
    }

    /// One value against one expectation, whichever spelling the value is in.
    fn side_against(&self, value: Side<'_>, expected: Expected<'_>) -> Fit {
        match value {
            Interned(id) => self.written_against(id, expected),
            Described(ty) => self.described_against(ty, expected),
        }
    }

    /// What the two sides' elements say.
    ///
    /// Covariant, and for PHP that is sound rather than convenient: an array is passed by value, so a
    /// callee writing into it cannot reach the caller's copy. `RowSub[]` therefore satisfies `Row[]`.
    fn element_fit(&self, value: Side<'_>, expected: Expected<'_>) -> Option<Fit> {
        let wanted = self.expected_element(expected)?;
        // A record has no single element -- each slot is its own -- so every slot is judged and all
        // of them have to fit. Not a union of them: a union is what the *value* would have to be
        // for any one slot, and against `Row[]` a `['a', new Row()]` is wrong about the first
        // whatever the second holds. Reached only where the expectation states an element, so a
        // declared `array{...}` still goes key by key through `record_against_shape`.
        if let Described(ValueTy::Record { fields, .. }) = value {
            return Some(Fit::all_of(fields.iter().map(|field| {
                self.described_against(&field.value, Expected::Interned(wanted))
            })));
        }
        Some(match self.value_element(value)? {
            Interned(id) => self.written_against(id, Expected::Interned(wanted)),
            Described(ty) => self.described_against(ty, Expected::Interned(wanted)),
        })
    }

    /// What the two sides' *keys* say, when both of them say what they are.
    ///
    /// Its own rule rather than the assignability one, and that is not a shortcut: PHP does not
    /// convert an array's keys the way it converts an argument. Handing `array<int, Row>` to a
    /// parameter declared `array<string, Row>` gives the callee int keys whatever `strict_types`
    /// says, so the question is whether the two key sets can be the same keys -- disjointness, with
    /// the directive not entering into it and no scalar coercion either.
    fn key_fit(&self, value: Side<'_>, expected: Expected<'_>) -> Option<Fit> {
        let wanted = self.key_kinds(self.stated_key(Side::of(expected))?)?;
        let held = match value {
            // A literal states its keys outright, and a string one is a claim nobody can mistake:
            // `['name' => $n]` handed to a `list<string>` has a string key on every run. An integer
            // key says nothing on its own, for [`Self::stated_key`]'s reason -- most are PHP's own
            // numbering of a bracketed list, which no writer meant as a key type, and the record
            // cannot tell `[0 => $a]` from `[$a]`. Beside a string key it does say something: a
            // literal that writes `'href' => ...` is a map, and its positional entry is an int key
            // of that map on every run -- `['disabled', 'href' => $url]` is neither a list nor a
            // string-keyed map, which PHPStan and PHPantom both reject.
            //
            // And every key of a literal is there, so one the expectation cannot hold is a key it
            // is handed: rejected outright, where a container that only *may* hold such keys is
            // forgiven below.
            Described(ValueTy::Record { fields, .. }) => {
                fields
                    .iter()
                    .any(|field| !phrost_model::is_an_integer_key(field.key.as_str()))
                    .then_some(())?;
                let held = KeyKinds {
                    int: fields
                        .iter()
                        .any(|field| phrost_model::is_an_integer_key(field.key.as_str())),
                    string: true,
                };
                return Some(match held.within(wanted) {
                    true => Fit::Accepted,
                    false => Fit::Rejected,
                });
            }
            _ => self.key_kinds(self.stated_key(value)?)?,
        };
        Some(if held.within(wanted) {
            Fit::Accepted
        } else if held.shares_nothing_with(wanted) {
            Fit::Rejected
        } else {
            // Wider than what is asked for, which every other broadening here is forgiven for: an
            // `int|string` key may well arrive as the int the callee wanted.
            Fit::Unknown
        })
    }

    /// The key half of a container that states one, on either side.
    ///
    /// Exactly two type arguments, which is what tells `array<K, V>` from `array<V>`, `V[]` and
    /// every other one-argument spelling: those name a container and say nothing about how it is
    /// keyed. PHP's implicit numbering is not a claim the writer made, and reading it as one turns
    /// every bracketed array in a codebase into a finding against a string-keyed parameter -- which
    /// is what PHPStan does here and why its answer is noise on framework code. A `list<V>`, by
    /// contrast, states `int` outright, and lowering now materialises that key so it can be read.
    fn stated_key<'v>(&self, side: Side<'v>) -> Option<Side<'v>> {
        match side {
            Interned(id) => match self.types.get(id) {
                TypeRef::Nullable(inner) => self.stated_key(Interned(*inner)),
                TypeRef::Generic { base, args, .. } => matches!(
                    self.types.get(*base),
                    TypeRef::Builtin(BuiltinTy::Array | BuiltinTy::Iterable)
                )
                .then(|| match self.types.operands(*args) {
                    [key, _] => Some(Interned(*key)),
                    _ => None,
                })
                .flatten(),
                _ => None,
            },
            // A described array carries a key only when the layer above read one written down.
            // `Held::ListOf` is built for Drupal's `loadMultiple` as well as for a list literal, and
            // that one is keyed by entity id -- so an inferred container states no key here.
            Described(ValueTy::ArrayOf { key, .. }) => key.as_deref().map(Described),
            Described(ValueTy::Interned(id)) => self.stated_key(Interned(*id)),
            Described(ValueTy::Nullable(inner)) => self.stated_key(Described(inner)),
            Described(_) => None,
        }
    }

    /// Which key types a written key reduces to, or `None` if it is not a key type at all.
    ///
    /// A template, a class, a shape and `mixed` all answer `None`, and that is the whole safety of
    /// the rule: the only keys judged are the ones somebody spelled as an int or a string.
    fn key_kinds(&self, keys: Side<'_>) -> Option<KeyKinds> {
        match keys {
            Interned(id) => match self.types.get(id) {
                TypeRef::Builtin(builtin) => KeyKinds::of(*builtin),
                // Every narrowed key -- `non-empty-string`, `non-negative-int`, `class-string` --
                // lowers to a builtin or to a generic over one, and the base is what the engine uses
                // as the key.
                TypeRef::Generic { base, .. } => self.key_kinds(Interned(*base)),
                TypeRef::Union(branches) => self
                    .types
                    .operands(*branches)
                    .iter()
                    .try_fold(None, |seen: Option<KeyKinds>, &branch| {
                        let kinds = self.key_kinds(Interned(branch))?;
                        Some(Some(match seen {
                            Some(seen) => seen.either(kinds),
                            None => kinds,
                        }))
                    })
                    .flatten(),
                _ => None,
            },
            Described(ValueTy::Builtin(builtin)) => KeyKinds::of(*builtin),
            Described(ValueTy::Interned(id)) => self.key_kinds(Interned(*id)),
            Described(ValueTy::OneOf(branches)) => branches
                .iter()
                .try_fold(None, |seen: Option<KeyKinds>, branch| {
                    let kinds = self.key_kinds(Described(branch))?;
                    Some(Some(match seen {
                        Some(seen) => seen.either(kinds),
                        None => kinds,
                    }))
                })
                .flatten(),
            Described(_) => None,
        }
    }

    /// The element an expectation states, when it is an array-like that states one.
    ///
    /// The *last* type argument, which is what a container holds however it was keyed: `array<K, V>`,
    /// `list<V>` and `V[]` all say `V`. A `?T` is looked through -- the null lands on the base -- and
    /// a union or an intersection is not, because which operand's element would answer is exactly
    /// what nothing here knows.
    fn expected_element(&self, expected: Expected<'_>) -> Option<TypeId> {
        let id = match expected {
            Expected::Interned(id) => id,
            Expected::Described(ValueTy::Interned(id)) => *id,
            Expected::Described(_) => return None,
        };
        match self.types.get(id) {
            TypeRef::Nullable(inner) => self.expected_element(Expected::Interned(*inner)),
            TypeRef::Generic { base, args, .. } => {
                let holds = matches!(
                    self.types.get(*base),
                    TypeRef::Builtin(BuiltinTy::Array | BuiltinTy::Iterable)
                );
                holds
                    .then(|| self.types.operands(*args).last().copied())
                    .flatten()
                    .filter(|element| {
                        // `array<string, mixed>` states nothing about its elements, and comparing
                        // against `mixed` would accept everything anyway -- so it is not an element
                        // this has to walk for.
                        !matches!(self.types.get(*element), TypeRef::Builtin(BuiltinTy::Mixed))
                    })
            }
            _ => None,
        }
    }

    /// The element a value carries, when it is an array-like that carries one.
    fn value_element<'v>(&self, value: Side<'v>) -> Option<Side<'v>> {
        match value {
            Interned(id) => match self.types.get(id) {
                TypeRef::Nullable(inner) => self.value_element(Interned(*inner)),
                TypeRef::Generic { base, args, .. } => matches!(
                    self.types.get(*base),
                    TypeRef::Builtin(BuiltinTy::Array | BuiltinTy::Iterable)
                )
                .then(|| self.types.operands(*args).last().copied())
                .flatten()
                .map(Interned),
                _ => None,
            },
            Described(ValueTy::ArrayOf { element, .. }) => Some(Described(element)),
            Described(ValueTy::Interned(id)) => self.value_element(Interned(*id)),
            Described(ValueTy::Nullable(inner)) => self.value_element(Described(inner)),
            Described(_) => None,
        }
    }

    /// Peels the *expected* side, for a value already reduced to a leaf.
    fn leaf_against(&self, leaf: Leaf<'_>, expected: Expected<'_>) -> Fit {
        match expected {
            Expected::Interned(id) => self.leaf_against_interned(leaf, id),
            Expected::Described(ty) => self.leaf_against_described(leaf, ty),
        }
    }

    /// The shape an expectation names, through the one wrapper a declaration puts around one.
    ///
    /// `@return array{...}` and `@return array{...}|null` are what real code writes; anything
    /// further out -- a union of two shapes, a generic over one -- is left to the array rule, which
    /// is the answer this gave before shapes on the value side existed and is never wrong.
    fn shape_awaited(&self, expected: Expected<'_>) -> Option<TypeId> {
        let Expected::Interned(id) = expected else {
            return None;
        };
        match self.types.get(id) {
            TypeRef::Shape { .. } => Some(id),
            TypeRef::Nullable(inner) => match self.types.get(*inner) {
                TypeRef::Shape { .. } => Some(*inner),
                _ => None,
            },
            _ => None,
        }
    }

    /// A record literal against the shape a declaration named, key by key.
    ///
    /// **A key the shape requires and the record lacks is a rejection.** It was not while the record
    /// held only what the literal wrote: `$r = ['a' => 1]; $r['b'] = 'x'; return $r;` is the
    /// commonest way a record is assembled, and refusing it for a missing `b` reported correct code.
    /// The writes are folded into the record one layer down now, so a key it lacks is one nothing
    /// put there -- and a key written in a branch counts as present, which keeps the doubt on the
    /// side that says nothing. Measured over six projects: eleven findings, every one a docblock and
    /// a call that disagree -- a Symfony test helper declaring `scrollOffset` for an array every
    /// caller and the renderer spell `scroll_offset`.
    ///
    /// A key the shape does not name is a rejection when the shape is sealed, and a record names
    /// every key it holds: the literal wrote them all and the fold added the rest.
    fn record_against_shape(&self, fields: &[RecordField], open: bool, shape: TypeId) -> Fit {
        let held = fields
            .iter()
            .map(|field| HeldKey {
                key: field.key.as_str(),
                value: Described(&field.value),
                optional: field.optional,
            })
            .collect::<Vec<_>>();
        self.keys_against_shape(&held, open, shape)
    }

    /// A declared shape against the shape a declaration named: `array{foo: int, buz: float}` handed
    /// to a parameter that takes `array{foo: int}`.
    ///
    /// The same judgement as a record's, through the same function, and only the source of the keys
    /// differs -- one that may be absent, and a value that may hold keys it does not name, are the
    /// two things a declaration can say and a literal cannot.
    fn shape_against_shape(&self, value: TypeId, shape: TypeId) -> Fit {
        let held = self
            .types
            .shape_keys(value)
            .into_iter()
            .map(|(key, ty, optional)| HeldKey {
                key: key.as_str(),
                value: Interned(ty),
                optional,
            })
            .collect::<Vec<_>>();
        self.keys_against_shape(&held, self.types.shape_is_open(value), shape)
    }

    /// Keys a value is known to hold against a declared shape, key by key; `held_open` when the
    /// value may hold keys beyond those.
    ///
    /// Every key the shape requires has to be there for certain, with the type the shape gives it.
    /// A key the shape does not name is not judged at all, sealed or not: that is how PHPStan reads
    /// a shape it is handed unless `bleedingEdge` says otherwise, and it is what a shape on a
    /// parameter means in the code it is written in -- `@param array{city?: mixed, state?: mixed}`
    /// is the keys a helper reads, and its callers pass the whole address. Rejecting the extra keys
    /// was 36 errors on the reference project, none of them a defect and none of them PHPStan's at
    /// level 8 with the project's own configuration. A value that may hold keys it does not name
    /// cannot be shown to lack a required key, so that question is left undecided for it.
    fn keys_against_shape(&self, held: &[HeldKey<'_>], held_open: bool, shape: TypeId) -> Fit {
        let named = self.types.shape_keys(shape);
        let wanted = named.iter().map(|(key, ty, optional)| {
            match held.iter().find(|held| held.key == key.as_str()) {
                // A key that may be absent, where the shape needs it, is the missing key on the runs
                // that leave it out.
                Some(held) if held.optional && !optional => Fit::Rejected,
                Some(held) => self.side_against(held.value, Expected::Interned(*ty)),
                None if *optional => Fit::Accepted,
                None if held_open => Fit::Unknown,
                None => Fit::Rejected,
            }
        });
        // A list's keys are `0` upwards in the order they were written, and the same keys in another
        // order are not one: `[1 => 'x', 0 => 'y']` has both keys of `list{string, string}` and
        // `array_is_list()` says no.
        let listed = (self.types.shape_is_list(shape)
            && held
                .iter()
                .enumerate()
                .any(|(index, held)| held.key != index.to_string()))
        .then_some(Fit::Rejected);
        Fit::all_of(wanted.chain(listed))
    }

    /// The expected side as the table holds it.
    fn leaf_against_interned(&self, leaf: Leaf<'_>, expected: TypeId) -> Fit {
        match self.types.get(expected) {
            // `mixed` is every type.
            TypeRef::Builtin(BuiltinTy::Mixed) => Fit::Accepted,
            TypeRef::Relative(_) | TypeRef::Field { .. } => Fit::Unknown,
            // `?T` is the one place a bare null is welcome.
            TypeRef::Nullable(inner) => match leaf {
                Leaf::Builtin(BuiltinTy::Null) => Fit::Accepted,
                _ => self.leaf_against_interned(leaf, *inner),
            },
            // One branch taking the value is enough.
            TypeRef::Union(branches) => Fit::any_of(
                self.types
                    .operands(*branches)
                    .iter()
                    .map(|&branch| self.leaf_against_interned(leaf, branch)),
            ),
            // Every part has to be satisfied at once.
            TypeRef::Intersection(parts) => Fit::all_of(
                self.types
                    .operands(*parts)
                    .iter()
                    .map(|&part| self.leaf_against_interned(leaf, part)),
            ),
            TypeRef::Generic { base, .. } => self.leaf_against_interned(leaf, *base),
            // An object may or may not carry these properties, and a class is an object; nothing
            // that is not an object can.
            TypeRef::ObjectShape { .. } => match leaf {
                Leaf::Builtin(BuiltinTy::Object | BuiltinTy::Mixed) | Leaf::Named(_) => {
                    Fit::Unknown
                }
                Leaf::Builtin(BuiltinTy::Never) => Fit::Accepted,
                _ => Fit::Rejected,
            },
            // An array may or may not carry these keys; nothing else can.
            TypeRef::Shape { .. } => match leaf {
                Leaf::Builtin(BuiltinTy::Array | BuiltinTy::Iterable | BuiltinTy::Mixed) => {
                    Fit::Unknown
                }
                Leaf::Builtin(BuiltinTy::Never) => Fit::Accepted,
                _ => Fit::Rejected,
            },
            TypeRef::Named(target) => match leaf {
                Leaf::Named(subject) => {
                    match subject.as_str().eq_ignore_ascii_case(target.as_str()) {
                        true => Fit::Accepted,
                        false => self.classes.relate(subject, target),
                    }
                }
                Leaf::Builtin(BuiltinTy::Never) => Fit::Accepted,
                Leaf::Builtin(builtin) => self.builtin_as_class(builtin, target),
            },
            TypeRef::Builtin(to) => match leaf {
                Leaf::Named(subject) => self.named_as(subject, *to),
                Leaf::Builtin(from) => self.builtins(from, *to),
            },
        }
    }

    /// A class-like value against a builtin expectation.
    fn named_as(&self, subject: &Name, expected: BuiltinTy) -> Fit {
        // The same question from the value's side, and the same answer: a name the index holds
        // nothing for may not be a class, so nothing about it can be ruled out.
        if !self.classes.declared(subject) {
            return Fit::Unknown;
        }
        match expected {
            // Every class-like is an object, hierarchy or not -- interfaces and enums included.
            BuiltinTy::Object => Fit::Accepted,
            // No object is any of these, whatever it inherits.
            BuiltinTy::Array | BuiltinTy::Int | BuiltinTy::Float | BuiltinTy::Bool => Fit::Rejected,
            BuiltinTy::True | BuiltinTy::False | BuiltinTy::Null | BuiltinTy::Void => Fit::Rejected,
            // `Stringable` is a coercion, and strict mode is exactly the switch that turns
            // coercions off -- a Stringable object is still a `TypeError` for a `string` parameter
            // under `strict_types=1`.
            string if string.is_string() && self.strict_types => Fit::Rejected,
            // A `__toString()` is declared to return a `string`, which may be the empty one -- so a
            // class that converts at all may convert to what a refinement refuses, and what it
            // returns on this call is not knowable from the declaration.
            string if string.is_string() => {
                match self.classes.satisfies(subject, BuiltinTy::String) {
                    Fit::Accepted if string != BuiltinTy::String => Fit::Unknown,
                    fit => fit,
                }
            }
            // Everything left is a question about what the class declares.
            _ => self.classes.satisfies(subject, expected),
        }
    }

    /// A builtin value against a class-like expectation.
    fn builtin_as_class(&self, value: BuiltinTy, expected: &Name) -> Fit {
        match value {
            // Broader than what is asked for: the object may well be of that class.
            BuiltinTy::Object | BuiltinTy::Mixed => Fit::Unknown,
            // A `callable` may be a `Closure`, and an `iterable` may be a `Traversable`.
            BuiltinTy::Callable | BuiltinTy::Iterable => Fit::Unknown,
            BuiltinTy::Never => Fit::Accepted,
            // Nothing else is ever an object -- as long as the expectation really names a class.
            // See [`ClassHierarchy::declared`]: a name the index holds nothing for may not be a
            // class at all, and rejecting against it is a report built out of an absence.
            _ if !self.classes.declared(expected) => Fit::Unknown,
            _ => Fit::Rejected,
        }
    }

    /// The expected side as a layer that could not intern it describes it.
    ///
    /// The same shape as [`Self::leaf_against_interned`], because it is the same peel over the same
    /// connectives -- only the spelling differs. What must not differ is where the two land, so both
    /// finish in `relate`, `named_as`, `builtin_as_class` and `builtins`, which hold the rules once.
    fn leaf_against_described(&self, leaf: Leaf<'_>, expected: &ValueTy) -> Fit {
        match expected {
            // Nothing was determined about what is wanted, so nothing can be wrong.
            ValueTy::Unknown => Fit::Unknown,
            ValueTy::Interned(id) => self.leaf_against_interned(leaf, *id),
            ValueTy::Builtin(BuiltinTy::Mixed) => Fit::Accepted,
            ValueTy::Nullable(inner) => match leaf {
                Leaf::Builtin(BuiltinTy::Null) => Fit::Accepted,
                _ => self.leaf_against_described(leaf, inner),
            },
            ValueTy::OneOf(branches) => Fit::any_of(
                branches
                    .iter()
                    .map(|branch| self.leaf_against_described(leaf, branch)),
            ),
            ValueTy::AllOf(parts) => Fit::all_of(
                parts
                    .iter()
                    .map(|part| self.leaf_against_described(leaf, part)),
            ),
            // What it holds is asked before the peel, by `arguments_fit`, and only when both
            // sides say -- here the two are the same array. A record is one of these: keys on the
            // *expected* side can only be judged against keys on the value's, and a leaf has none.
            ValueTy::Array | ValueTy::ArrayOf { .. } | ValueTy::Record { .. } => match leaf {
                Leaf::Named(subject) => self.named_as(subject, BuiltinTy::Array),
                Leaf::Builtin(from) => self.builtins(from, BuiltinTy::Array),
            },
            ValueTy::Named(target) => match leaf {
                Leaf::Named(subject) => {
                    match subject.as_str().eq_ignore_ascii_case(target.as_str()) {
                        true => Fit::Accepted,
                        false => self.classes.relate(subject, target),
                    }
                }
                Leaf::Builtin(BuiltinTy::Never) => Fit::Accepted,
                Leaf::Builtin(builtin) => self.builtin_as_class(builtin, target),
            },
            ValueTy::Builtin(to) => match leaf {
                Leaf::Named(subject) => self.named_as(subject, *to),
                Leaf::Builtin(from) => self.builtins(from, *to),
            },
        }
    }

    /// Two builtins, which is where `strict_types` actually decides something.
    fn builtins(&self, value: BuiltinTy, expected: BuiltinTy) -> Fit {
        use BuiltinTy::{
            Array, Bool, Callable, False, Float, Int, Iterable, Mixed, Never, NonEmptyString,
            NonFalsyString, Null, Object, Resource, String, True, Void,
        };

        match (value, expected) {
            (from, to) if from == to => Fit::Accepted,
            (_, Mixed) | (Never, _) => Fit::Accepted,
            // Broader than what is asked for.
            (Mixed, _) => Fit::Unknown,

            // Two strings, which differ only in what they are known to hold. `string` is
            // `''|non-empty-string`, and the empty half is a value that cannot land where a
            // non-empty one is declared -- rejected for the reason `bool` is against a declared
            // `true` below, and the way PHPStan reads it. A refinement is a string wherever a
            // string goes, and `non-falsy-string` a `non-empty-string`.
            (from, to) if from.is_string() && to.is_string() => {
                match from.string_rank() >= to.string_rank() {
                    true => Fit::Accepted,
                    false => Fit::Rejected,
                }
            }
            // Weak mode converts a scalar to a string, and what the conversion gives is known
            // without the value: an int or a float always spells some digits -- `0` spells `'0'`,
            // which is not empty and is falsy -- `true` spells `'1'`, and `false` spells `''`. So a
            // refinement refuses exactly the scalars whose conversion can land outside it. Strict
            // mode refuses them all, which is the scalar table's rule below.
            (Int | Float | Bool | True | False, to)
                if to.is_string() && to != String && !self.strict_types =>
            {
                let spelled = match value {
                    Int | Float => NonEmptyString,
                    True => NonFalsyString,
                    _ => String,
                };
                match spelled.string_rank() >= to.string_rank() {
                    true => Fit::Accepted,
                    false => Fit::Rejected,
                }
            }

            // Nothing can be returned where `void` is declared. The other direction cannot reach
            // here any more: a `void` *value* is a null, which `Leaf::of` settles before this.
            (_, Void) => Fit::Rejected,
            (Void, _) => Fit::Rejected,
            // A bare `null` reaching a non-nullable target. `?T` was handled before this.
            (Null, _) | (_, Null) => Fit::Rejected,

            // `true` and `false` are the two halves of `bool`, and a `bool` is one of them only on
            // some runs -- the half that is not is a union branch that cannot land, rejected here
            // for the reason `int|string` is against an `int`.
            (True | False, Bool) => Fit::Accepted,
            (Bool, True | False) => Fit::Rejected,
            (True, False) | (False, True) => Fit::Rejected,
            // Nor does weak mode convert anything *to* one of the halves. Measured on PHP 8.5.10
            // without `strict_types`: `1`, `1.0` and `"x"` passed to a declared `true`, and `0`
            // returned from a declared `false`, are each a `TypeError` -- the scalar table below
            // converts to `bool`, never to a literal of it.
            (Int | Float | String | NonEmptyString | NonFalsyString, True | False) => Fit::Rejected,

            // A `callable` is written as a string, an array, or a `Closure`, so almost nothing
            // about one can be ruled out from the vocabulary alone.
            (Callable, _) | (_, Callable) => Fit::Unknown,

            // A resource shares no values with anything else in the vocabulary, and unlike every
            // other pair here that is true in weak mode too. Measured on PHP 8.5.10: a file handle
            // passed to a declared `string`, `int`, `bool`, `array` or `object` parameter is a
            // `TypeError` either way -- the coercions that let a `false` reach a `string` do not
            // exist for one. An explicit `(string) $fh` still gives `Resource id #5`, which is a
            // different question and is why the cast rules are elsewhere.
            (Resource, _) | (_, Resource) => Fit::Rejected,

            (Array, Iterable) => Fit::Accepted,
            // An `iterable` may be a `Traversable` rather than an array.
            (Iterable, Array) => Fit::Unknown,
            (Object, Iterable) | (Iterable, Object) => Fit::Unknown,

            // The scalar table, and the only place the directive changes an answer.
            (from, to) if Self::is_scalar(from) && Self::is_scalar(to) => {
                if self.strict_types {
                    // The one widening PHP keeps under strict types.
                    match (from, to) {
                        (Int, Float) => Fit::Accepted,
                        _ => Fit::Rejected,
                    }
                } else {
                    // Weak mode converts between all of them. Whether the *value* survives -- "abc"
                    // to an `int` -- is a runtime question about data we do not have.
                    Fit::Accepted
                }
            }

            // An object may define `__toString`; under strict types that no longer helps, which
            // `named_as` handles for a class we can name.
            (Object, to) if Self::is_scalar(to) => {
                if self.strict_types {
                    Fit::Rejected
                } else {
                    Fit::Unknown
                }
            }

            // Arrays, iterables and objects share no values with scalars or with each other.
            (Array | Iterable | Object, _) | (_, Array | Iterable | Object) => Fit::Rejected,

            _ => Fit::Unknown,
        }
    }

    /// The types PHP will convert between in weak mode.
    fn is_scalar(ty: BuiltinTy) -> bool {
        ty.is_string()
            || matches!(
                ty,
                BuiltinTy::Int
                    | BuiltinTy::Float
                    | BuiltinTy::Bool
                    | BuiltinTy::True
                    | BuiltinTy::False
            )
    }
}
