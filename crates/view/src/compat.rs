//! The nominal half of type compatibility, answered from the project's hierarchy.
//!
//! `phrost_ty::compat` judges how the connectives compose -- `?T`, `A|B`, `A&B`, the builtins, and
//! which scalars the engine converts under which `strict_types`. It cannot judge what a class *name*
//! means, because that needs a hierarchy and `phrost-ty` may not see one. This is the other half.
//!
//! # Reject only what can be proven wrong
//!
//! The rule the whole judgement is built on, and it bites hardest here. PHP is nominal but it is
//! also open: a value declared `Foo` is not necessarily a `Foo`, it is a `Foo` *or anything below
//! one*. So "not an ancestor" is almost never enough to reject:
//!
//! - `Foo` where a `BarInterface` is wanted is **not** wrong. `class Sub extends Foo implements
//!   BarInterface` is legal, and a `Sub` is what the caller may well have passed.
//! - `Foo` where an unrelated *class* `Baz` is wanted **is** wrong, because PHP has single
//!   inheritance: no class is below both unless one is below the other.
//! - `Foo` where anything is wanted is wrong if `Foo` is `final`, because then there is nothing
//!   below it to save the call.
//!
//! Which is why finality and the class *kind* decide as much here as ancestry does, and why a class
//! the index has never heard of is [`Fit::Unknown`] rather than a rejection.

#[cfg(test)]
mod tests;

use phrost_hir::{FileHir, TypeRefData, TypeRefId};
use phrost_model::{Name, TextSize};
use phrost_sema::ClassKind;
use phrost_ty::{
    TypeId,
    compat::{ClassHierarchy, Compatibility, Fit, RecordField, ValueTy},
};

use crate::{
    IndexView,
    resolve::{HIERARCHY_DEPTH, Held},
};

/// How a site spells the type it expects.
///
/// A parameter, a property and a return are interned in the project's table; a
/// `/** @var File $file */` is an index into this file's own arena and cannot be, because every
/// `TypeTable` constructor needs `&mut` and a reading view does not have one. The same split
/// `phrost_ty`'s judgement names on its own side of the wall -- named here so that the rules about
/// what was *written* are one rule and not one per spelling, which is how the `@var` branch came to
/// be missing both of them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expectation {
    Stored(TypeId),
    Written(TypeRefId),
}

impl Expectation {
    /// The expectation as a message names it: qualified, because a reader has to find the class.
    pub(crate) fn render(self, types: &phrost_ty::TypeTable, hir: &phrost_hir::FileHir) -> String {
        match self {
            Self::Stored(ty) => types.render_qualified(ty),
            Self::Written(id) => phrost_hir::render::render_type_ref(hir, id),
        }
    }
}

impl IndexView<'_> {
    /// Whether a value of `value` may be used where `expected` is declared.
    ///
    /// The `strict_types` of the file the request is about is bound here rather than passed in, for
    /// the reason [`Self::strict_types`] gives: PHP applies the directive where the *call* is
    /// written, so a caller that could choose would eventually choose the callee's file and be
    /// wrong on every cross-file call.
    pub fn fit(&self, value: TypeId, expected: TypeId) -> Fit {
        let classes = Classes(self);
        Compatibility::new(self.skeleton.types(), &classes, self.strict_types())
            .fit(value, expected)
    }

    /// Whether an instance of `subject` is an instance of `expected`: yes, no, or a subclass that
    /// nothing here can see may make it so -- the nominal half of [`Self::fit`] on its own.
    pub(crate) fn relate_classes(&self, subject: &Name, expected: &Name) -> Fit {
        Classes(self).relate(subject, expected)
    }

    /// [`Self::fit_held`], plus the two things a [`Held`] cannot say: that the value is a *literal*
    /// no conversion can make fit, and that it is a *cast* stating its type outright.
    ///
    /// The entry point every check that has the written expression in hand should use, and there are
    /// four of them -- an argument, an assignment to a property, a return, and an assignment checked
    /// against a `@var`. One spelling rather than the rule repeated at each, because two
    /// constructors of one rule is how one of them comes to be missing what the other has: the
    /// fourth reached the judgement through [`Self::fit_held_against_written`], for the good reason
    /// that its expectation is not interned, and was blind to *both* of these rules for a year --
    /// `/** @var string $id */ $id = (int) $n;` and `/** @var int $n */ $n = 'nope';` were silent
    /// while the same two lines written into a typed property were reported. That is what
    /// [`Expectation`] is for.
    pub(crate) fn fit_written_value(
        &self,
        offset: TextSize,
        hir: &FileHir,
        value: phrost_hir::ExprId,
        held: &Held,
        expected: Expectation,
    ) -> Fit {
        let fit = match expected {
            Expectation::Stored(ty) => self.fit_held(offset, hir, held, ty),
            Expectation::Written(id) => self.fit_held_against_written(offset, hir, held, id),
        };
        match fit {
            // `[]` against a shape is `Unknown` to the judgement, which cannot tell an empty array
            // from any other -- and the literal as written is exactly what it could not tell.
            Fit::Accepted | Fit::Unknown if self.empty_literal_refused(hir, value, expected) => {
                Fit::Rejected
            }
            // Only where the type fitted: a value already rejected is reported for the reason the
            // judgement gave, and one that is `Unknown` stays silent whatever was written.
            Fit::Accepted
                if self.literal_cannot_convert(offset, hir, value, expected)
                    || self.cast_states_another_type(offset, hir, value, expected)
                    || self.type_arguments_refused(offset, hir, held, expected)
                    || self.callable_return_refused(offset, hir, held, expected) =>
            {
                Fit::Rejected
            }
            fit => fit,
        }
    }

    /// Whether the **strict** rules refuse a plain `builtin` where the site's type is declared.
    ///
    /// The one question both rules below ask, and asking it this way rather than by peeling the
    /// expectation is what keeps the union, nullable and template cases working: `int|string`
    /// accepts a string as a string and answers `Accepted`, and `mixed` and an unbound `T` answer
    /// `Unknown` -- exactly the two verdicts that must stay silent.
    fn strictly_refuses(
        &self,
        offset: TextSize,
        hir: &FileHir,
        builtin: phrost_model::BuiltinTy,
        expected: Expectation,
    ) -> bool {
        let classes = Classes(self);
        let strict = Compatibility::new(self.skeleton.types(), &classes, true);
        let value = ValueTy::Builtin(builtin);
        let fit = match expected {
            Expectation::Stored(ty) => strict.fit_value(&value, ty),
            Expectation::Written(id) => {
                strict.fit_described(&value, &self.describe_written(offset, hir, id))
            }
        };
        fit == Fit::Rejected
    }

    /// Whether the value written at `expr` is an explicit cast to a type `expected` will not hold.
    ///
    /// The other place [`Self::fit`]'s answer is not the whole answer, and it is a *position* rather
    /// than a fact about the runtime. Weak mode converts between the scalars, so
    /// `add(FileInterface $f, string $id)` called `add($file, (int) $node->id())` runs -- PHP turns
    /// the int back into a string at the boundary and nothing is lost. What makes it worth reporting
    /// anyway is that the cast is the author *writing the type down*: one line says the value is an
    /// int and the declaration says it is a string, and one of the two is a mistake whichever way it
    /// is resolved. Nothing else in a weak-mode file states a scalar type that plainly -- a variable
    /// typed `string` may hold `'12'`, which is why the type judgement is right to stay quiet about
    /// it, and a cast has no such doubt.
    ///
    /// Asked of the **strict** rules, the same way [`Self::literal_cannot_convert`] is asked and for
    /// the same reason: the strict and weak tables differ only over the scalars, so a verdict that
    /// changes between them is exactly a coercion PHP would have performed, and every other
    /// expectation -- `mixed`, `int|string`, a class, an unbound template -- answers identically in
    /// both and is already settled by the judgement above.
    ///
    /// `Int -> Float` survives, because PHP widens it under `strict_types` too and `(int)` where a
    /// `float` is declared is not a claim about anything. An `(array)` or `(object)` cast is not a
    /// scalar and reaches no scalar rule, so this is about the four scalar casts and nothing else.
    pub(crate) fn cast_states_another_type(
        &self,
        offset: TextSize,
        hir: &FileHir,
        expr: phrost_hir::ExprId,
        expected: Expectation,
    ) -> bool {
        let phrost_hir::body::ExprData::Cast { to, .. } = hir.expr(expr) else {
            return false;
        };
        // A string cast states the kind and not what the string holds -- that is the value's, and
        // the judgement above has already read it -- so it is asked as the string that holds the
        // most, which refuses exactly what no string is.
        let stated = match to.is_string() {
            true => phrost_model::BuiltinTy::NonFalsyString,
            false => *to,
        };

        self.strictly_refuses(offset, hir, stated, expected)
    }

    /// Whether the value written at `expr` is a literal no conversion can make fit `expected`.
    ///
    /// The one place [`Self::fit`]'s answer is not the whole answer. PHP's weak mode converts a
    /// string where an `int` is declared, so the judgement accepts one -- and it is right about the
    /// *type*, because a `string` may well hold `'12'`. A **literal** that is not a number cannot:
    /// measured on PHP 8.5, `needs_int('bad')`, `$holder->count = 'bad'` and `return 'bad'` from
    /// `: int` are all a `TypeError` on every run, `strict_types` or not, and `'5 apples'` and `''`
    /// are too. Only `'12'` and `'12.5'` get through, the second with a deprecation.
    ///
    /// So the question is put to [`Self::strictly_refuses`] against a plain `string`: if a string is
    /// rejected under the strict rules, coercion was the only thing that could have saved this
    /// value, and a literal that is not numeric has no coercion.
    ///
    /// Silent for anything but a plain single-quoted-style literal: a backslash or a `$` means the
    /// text as written is not the text at runtime, and this reads the source.
    pub(crate) fn literal_cannot_convert(
        &self,
        offset: TextSize,
        hir: &FileHir,
        expr: phrost_hir::ExprId,
        expected: Expectation,
    ) -> bool {
        let phrost_hir::body::ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::String,
            range,
        } = hir.expr(expr)
        else {
            return false;
        };
        let Some(text) = self.current_text() else {
            return false;
        };
        let Some(written) = text.get(usize::from(range.start())..usize::from(range.end())) else {
            return false;
        };
        if !is_a_plain_string(written) || numeric(unquoted(written)) {
            return false;
        }

        // As the string it is, `''` and `'0'` included: what it holds is part of what it cannot be
        // converted into.
        let literal = self.literal_string_holds(offset, *range);
        self.strictly_refuses(offset, hir, literal, expected)
    }

    /// Whether the value is the literal `[]` and the site is declared `non-empty-array` or
    /// `non-empty-list`, or a shape with a key it requires -- which no conversion fills.
    ///
    /// The judgement itself reads a non-empty array as the array it is, because a value's own type
    /// almost never says whether it holds anything -- a `list<int>` handed to a `non-empty-list<int>`
    /// is unknown rather than wrong. The literal is the one value that says so as written. The same
    /// goes for `array{id: int}`: `[]` is not a record, since the record reading starts at the first
    /// key, and as written it is the one array certain to lack `id`.
    fn empty_literal_refused(
        &self,
        hir: &FileHir,
        expr: phrost_hir::ExprId,
        expected: Expectation,
    ) -> bool {
        let phrost_hir::body::ExprData::ArrayLiteral { elements, .. } = hir.expr(expr) else {
            return false;
        };
        if !hir.expr_list(*elements).is_empty() {
            return false;
        }
        match expected {
            Expectation::Stored(ty) => {
                let types = self.skeleton.types();
                let ty = match types.get(ty) {
                    phrost_ty::TypeRef::Nullable(inner) => *inner,
                    _ => ty,
                };
                match types.get(ty) {
                    phrost_ty::TypeRef::Generic { non_empty, .. } => *non_empty,
                    phrost_ty::TypeRef::Shape { .. } => types
                        .shape_keys(ty)
                        .iter()
                        .any(|(_, _, optional)| !optional),
                    _ => false,
                }
            }
            Expectation::Written(id) => {
                let id = match hir.type_ref(id) {
                    phrost_hir::TypeRefData::Nullable(inner) => *inner,
                    _ => id,
                };
                match hir.type_ref(id) {
                    phrost_hir::TypeRefData::Generic { non_empty, .. } => *non_empty,
                    phrost_hir::TypeRefData::Shape { fields, .. } => {
                        hir.type_ref_list(*fields).iter().any(|&field| {
                            matches!(
                                hir.type_ref(field),
                                phrost_hir::TypeRefData::ShapeField {
                                    optional: false,
                                    ..
                                }
                            )
                        })
                    }
                    _ => false,
                }
            }
        }
    }

    /// The same, for a value this layer inferred rather than read out of the table.
    ///
    /// The entry point every real check needs. A site's *expected* type is always interned -- a
    /// parameter, a property, a return -- but the value's almost never is: [`Held`] exists precisely
    /// because inference produces types the table cannot hold. Translating rather than interning is
    /// what keeps the view read-only.
    pub(crate) fn fit_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        value: &Held,
        expected: TypeId,
    ) -> Fit {
        let classes = Classes(self);
        let described = self.describe(offset, hir, value);
        Compatibility::new(self.skeleton.types(), &classes, self.strict_types())
            .fit_value(&described, expected)
    }

    /// The same when the expectation is written in this file too.
    ///
    /// A `/** @var File $file */` is an index into the file's own arena, so an assignment checked
    /// against one has an uninterned expectation as well as an uninterned value -- and every
    /// `TypeTable` constructor needs `&mut`, which a reading view does not have. Both sides are
    /// described instead, and the judgement is the same judgement.
    pub(crate) fn fit_held_against_written(
        &self,
        offset: TextSize,
        hir: &FileHir,
        value: &Held,
        expected: TypeRefId,
    ) -> Fit {
        let classes = Classes(self);
        let value = self.describe(offset, hir, value);
        let expected = self.describe_written(offset, hir, expected);
        Compatibility::new(self.skeleton.types(), &classes, self.strict_types())
            .fit_described(&value, &expected)
    }

    /// The same when both sides were inferred rather than read out of the table.
    ///
    /// For ranking two claims about one value against each other -- what a slot's declaration
    /// guarantees against what an assignment put there -- where neither side is a written type and
    /// neither is interned.
    pub(crate) fn fit_held_against_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        value: &Held,
        expected: &Held,
    ) -> Fit {
        let classes = Classes(self);
        let value = self.describe(offset, hir, value);
        let expected = self.describe(offset, hir, expected);
        Compatibility::new(self.skeleton.types(), &classes, self.strict_types())
            .fit_described(&value, &expected)
    }

    /// [`Self::fit_held_against_held`] with no conversion admitted, whatever the file declares.
    ///
    /// For comparing two types rather than judging a call. A weak-mode file converts `'5'` to `5`
    /// where a call passes it, and that is a fact about the call: a string written into a slot, or
    /// found by `===`, is a string still. Asked with the file's own mode, a weak-mode file read every
    /// scalar as fitting every other.
    pub(crate) fn fit_held_strictly(
        &self,
        offset: TextSize,
        hir: &FileHir,
        value: &Held,
        expected: &Held,
    ) -> Fit {
        let classes = Classes(self);
        let value = self.describe(offset, hir, value);
        let expected = self.describe(offset, hir, expected);
        Compatibility::new(self.skeleton.types(), &classes, true).fit_described(&value, &expected)
    }

    /// What a held value looks like to the judgement.
    ///
    /// Anything this cannot express becomes [`ValueTy::Unknown`], which is never a finding. That is
    /// the safe direction: a value we failed to describe must not be reported as the wrong type.
    fn describe(&self, offset: TextSize, hir: &FileHir, value: &Held) -> ValueTy {
        match value {
            Held::Object(_) => ValueTy::Builtin(phrost_model::BuiltinTy::Object),
            Held::Class(name) => ValueTy::Named(name.clone()),
            Held::Builtin(builtin) => ValueTy::Builtin(*builtin),
            // Type arguments are dropped here as they are everywhere else in this judgement.
            Held::Applied { class, .. } => ValueTy::Named(class.clone()),
            // The element travels, which is the one type argument this judgement does read: both
            // sides knowing is what makes `Row[]` given a `string[]` a finding rather than a guess.
            Held::ListOf(inner) => ValueTy::ArrayOf {
                // No key: the same `Held` stands for a list literal and for `loadMultiple`'s
                // id-keyed result, and only a written `array<K, V>` says which.
                key: None,
                element: Box::new(self.describe(offset, hir, inner)),
            },
            // The key is known here, and still not stated: what `ArrayOf` compares it against is a
            // written key, and a built one describing a class or a union would be a guess.
            Held::MapOf { value, .. } => ValueTy::ArrayOf {
                key: None,
                element: Box::new(self.describe(offset, hir, value)),
            },
            // The return is dropped, as every other type argument is in this judgement: nothing in
            // the compatibility rules can express a signature, and `callable` is what PHP checks.
            Held::Callable(_) => ValueTy::Builtin(phrost_model::BuiltinTy::Callable),
            // The keys travel, because the judgement can use them: against a declared `array{...}`
            // a record is compared key by key, and against everything else it is the array it
            // always was. Each value is described in turn, so a nested literal keeps its own shape.
            Held::Record(fields) => ValueTy::Record {
                fields: fields
                    .iter()
                    .map(|(key, value)| RecordField {
                        key: key.clone(),
                        value: self.describe(offset, hir, value),
                        optional: false,
                    })
                    .collect(),
                open: false,
            },
            Held::Nullable(inner) => ValueTy::Nullable(Box::new(self.describe(offset, hir, inner))),
            Held::OneOf(operands) => ValueTy::OneOf(
                operands
                    .iter()
                    .map(|operand| self.describe(offset, hir, operand))
                    .collect(),
            ),
            Held::AllOf(operands) => ValueTy::AllOf(
                operands
                    .iter()
                    .map(|operand| self.describe(offset, hir, operand))
                    .collect(),
            ),
            // Interned already, so the judgement reads it straight from the table -- except a
            // relative type, which means nothing without the scope that travelled with it.
            Held::Stored { ty, scope } => match self.skeleton.types().get(*ty) {
                phrost_ty::TypeRef::Relative(_) => {
                    match self.class_of_stored(*ty, scope.as_ref()) {
                        Some(class) => ValueTy::Named(class),
                        None => ValueTy::Unknown,
                    }
                }
                _ => ValueTy::Interned(*ty),
            },
            // A type written in the file being read: an index into that file's own arena, which is
            // exactly what cannot be interned from here.
            Held::Written(id) => self.describe_written(offset, hir, *id),
        }
    }

    /// A type as the current file writes it.
    ///
    /// Walked rather than reduced to a single class, because a written type is where a union or an
    /// intersection is most often spelled out -- a parameter is the commonest value a check asks
    /// about, and its type lives here.
    fn describe_written(&self, offset: TextSize, hir: &FileHir, id: TypeRefId) -> ValueTy {
        match hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => ValueTy::Builtin(*builtin),
            TypeRefData::Named(reference) => {
                let reference = hir.name_ref(*reference);
                match self.resolve_written_name(offset, reference.path.as_str(), reference.anchor) {
                    Some(location) => ValueTy::Named(location.fqn),
                    // A name the index cannot place says nothing about the value.
                    None => ValueTy::Unknown,
                }
            }
            TypeRefData::Nullable(inner) => {
                ValueTy::Nullable(Box::new(self.describe_written(offset, hir, *inner)))
            }
            TypeRefData::Union(parts) => ValueTy::OneOf(
                hir.type_ref_list(*parts)
                    .iter()
                    .map(|&part| self.describe_written(offset, hir, part))
                    .collect(),
            ),
            TypeRefData::Intersection(parts) => ValueTy::AllOf(
                hir.type_ref_list(*parts)
                    .iter()
                    .map(|&part| self.describe_written(offset, hir, part))
                    .collect(),
            ),
            // A container keeps its element *and* its key, and only a container: a `class-string<T>`
            // is a generic over `string`, and what it *names* is a different question from what an
            // array holds. The third spelling of this -- the interned one reads its own arguments and
            // a `Held::ListOf` carries an element -- and a rule taught to two of three governs the
            // two it was tested with. Dropping the key here is what made an element-typed *parameter*
            // answer nothing where the same type reached through a local answered.
            TypeRefData::Generic { base, args, .. }
                if matches!(
                    hir.type_ref(*base),
                    TypeRefData::Builtin(
                        phrost_model::BuiltinTy::Array | phrost_model::BuiltinTy::Iterable
                    )
                ) =>
            {
                match hir.type_ref_list(*args) {
                    // `array<K, V>`: both halves travel. The key is what tells this from `V[]`,
                    // which is the one-argument shape below and states no key at all.
                    [key, element] => ValueTy::ArrayOf {
                        key: Some(Box::new(self.describe_written(offset, hir, *key))),
                        element: Box::new(self.describe_written(offset, hir, *element)),
                    },
                    [.., element] => ValueTy::ArrayOf {
                        key: None,
                        element: Box::new(self.describe_written(offset, hir, *element)),
                    },
                    [] => self.describe_written(offset, hir, *base),
                }
            }
            TypeRefData::Generic { base, .. } => self.describe_written(offset, hir, *base),
            // A shape keeps its keys for the reason a record does, and what it says about them --
            // that one may be absent, or that there may be others -- travels with them. Left
            // undescribed, `array{foo: int, buz: float}` handed on to a parameter that takes
            // `array{foo: int}` was the one spelling of a shape nothing compared.
            TypeRefData::Shape { fields, open, .. } => ValueTy::Record {
                fields: hir
                    .type_ref_list(*fields)
                    .iter()
                    .filter_map(|&field| match hir.type_ref(field) {
                        TypeRefData::ShapeField { key, optional, ty } => Some(RecordField {
                            key: key.clone(),
                            value: self.describe_written(offset, hir, *ty),
                            optional: *optional,
                        }),
                        _ => None,
                    })
                    .collect(),
                open: *open,
            },
            // `self`, `static` and `parent` need the enclosing class, which the offset supplies.
            TypeRefData::Relative(_) => match self.class_of_type_ref(offset, hir, id) {
                Some(class) => ValueTy::Named(class),
                None => ValueTy::Unknown,
            },
            _ => ValueTy::Unknown,
        }
    }
}

/// Answers the nominal questions by walking the skeleton's resolved relations.
struct Classes<'v, 'a>(&'v IndexView<'a>);

impl ClassHierarchy for Classes<'_, '_> {
    fn relate(&self, subject: &Name, expected: &Name) -> Fit {
        let (subject, expected) = (subject.as_str(), expected.as_str());
        // PHP compares class names case-insensitively, so `Foo` and `foo` are one class.
        if subject.eq_ignore_ascii_case(expected) {
            return Fit::Accepted;
        }

        // A class nothing declares says nothing. Silence here is what keeps an unresolved vendor
        // name from becoming a finding on every call that mentions it.
        let Some(subject_header) = self.0.class_header(subject) else {
            return Fit::Unknown;
        };
        if self.0.descends_from(subject, expected, HIERARCHY_DEPTH)
            || self.0.stringable_without_saying(subject, expected)
        {
            return Fit::Accepted;
        }
        let Some(expected_header) = self.0.class_header(expected) else {
            return Fit::Unknown;
        };

        // Nothing below it, so what it is not, it can never be.
        if subject_header.is_final {
            return Fit::Rejected;
        }

        // A trait is not a type anything holds. Judging it would be judging a mistake by guessing
        // what was meant.
        if matches!(subject_header.kind, ClassKind::Trait)
            || matches!(expected_header.kind, ClassKind::Trait)
        {
            return Fit::Unknown;
        }

        // A value a framework says answers any name is a double: a mocking library's interface, with
        // a subclass of whatever was mocked generated behind it at runtime. `Mockery::mock(Store::class)`
        // is documented as returning `MockInterface`, so the class it doubles is in the argument
        // and not in the type -- PHPStan needs `phpstan-mockery` for the same reason. See
        // `dynamic_members`.
        if self.0.skeleton.answers_any_name(subject) {
            return Fit::Unknown;
        }

        // A downcast -- `expected` is below `subject`, so what arrives may be one at runtime.
        // Unproven, not impossible, and the one leniency kept: a parent handed where a child is
        // asked for is how a factory's product reaches its consumer.
        if self.0.descends_from(expected, subject, HIERARCHY_DEPTH)
            || self.0.stringable_without_saying(expected, subject)
        {
            return Fit::Unknown;
        }

        // An ancestor the index does not hold may be the one that is an `expected`, so nothing
        // the declared ancestry leaves out can be ruled out.
        if !self.0.ancestry_is_known(subject, HIERARCHY_DEPTH) {
            return Fit::Unknown;
        }

        // Neither is below the other. A class or interface somewhere could still extend both, and
        // this used to answer `Unknown` for exactly that reason whenever either side was an
        // interface -- but nothing the code says makes the value one, and PHPStan and PHP's own
        // `TypeError` both go by what is declared. The double PHPUnit hands back is a
        // `MockObject&Application`, an intersection, and one part of it is enough.
        Fit::Rejected
    }

    fn declared(&self, class: &Name) -> bool {
        self.0.class_header(class.as_str()).is_some()
    }

    /// Weak mode only for `string` -- `phrost_ty` rejects that outright under `strict_types`,
    /// because a `__toString` is a conversion and that is what the directive turns off.
    fn satisfies(&self, subject: &Name, expected: phrost_model::BuiltinTy) -> Fit {
        match self.0.class_answers_kind(subject.as_str(), expected) {
            crate::resolve::Answers::Yes => Fit::Accepted,
            crate::resolve::Answers::No => Fit::Rejected,
            crate::resolve::Answers::Maybe => Fit::Unknown,
        }
    }
}

/// Whether a literal's source text means at runtime exactly what it says here.
///
/// An escape or an interpolation makes the written text and the value two different things, and this
/// reads the written one. Both are rare in a literal being passed where a number is wanted, and both
/// are cheaper to decline than to model.
fn is_a_plain_string(written: &str) -> bool {
    written.len() >= 2 && !written.contains('\\') && !written[1..].contains('$')
}

/// A literal with its quotes taken off.
fn unquoted(written: &str) -> &str {
    let mut chars = written.chars();
    match (chars.next(), written.chars().next_back()) {
        (Some(open @ ('"' | '\'')), Some(close)) if open == close => {
            &written[open.len_utf8()..written.len() - close.len_utf8()]
        }
        _ => written,
    }
}

/// Whether PHP would read this string as a number.
///
/// Leading and trailing whitespace is allowed, which is PHP 8's rule, and nothing else is: since
/// PHP 8.0 a *leading*-numeric string like `'5 apples'` is a `TypeError` rather than a 5 with a
/// warning.
///
/// Answered by Rust's own float parser, which accepts a superset -- `inf` and `NaN` are numbers to
/// it and not to PHP. That is the safe direction: a value wrongly called numeric produces silence.
fn numeric(text: &str) -> bool {
    text.trim().parse::<f64>().is_ok()
}
