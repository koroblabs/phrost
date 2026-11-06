//! Lowering a parsed docblock type into the written-type representation.
//!
//! [`Lowerer::lower_doc_type`] is the one function every other module here ends at: whatever tag
//! or contract a sibling reads, the type it found still has to become a [`TypeRefId`] the rest of
//! the crate can hold, and that translation -- which docblock shapes have a written-PHP equivalent,
//! which are recorded as unmodelled, how a name inside one resolves against the file's imports --
//! lives only here so it exists in exactly one place.

use mago_phpdoc_syntax::cst::r#type::{ReferenceKind, Type};
use mago_span::HasSpan as _;
use phrost_hir::{NameAnchor, NameRefData, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, RelativeTy, TextRange};

use crate::lowerer::Lowerer;

use super::type_alias::TypeAliasTable;
use super::{shift, slice};

impl Lowerer<'_, '_> {
    /// The fields of a shape -- an array's or an object's -- each lowered to a
    /// [`TypeRefData::ShapeField`]. A field whose key or type does not lower is left out, which the
    /// caller reads as the shape being open.
    fn shape_fields<'f, 'arena: 'f>(
        &mut self,
        written: impl Iterator<Item = &'f mago_phpdoc_syntax::cst::r#type::ShapeField<'arena>>,
        base: phrost_model::TextSize,
        text: &str,
        aliases: &TypeAliasTable,
    ) -> Vec<TypeRefId> {
        let mut fields = Vec::new();
        // PHP's next free integer key, for the entries that name none.
        let mut next = 0_u64;
        for field in written {
            // A field with no key is a positional one -- `array{int, string}` -- and PHP
            // gives it the next free integer key, which is exactly what `array{0: int,
            // 1: string}` writes out. So it is *numbered* rather than dropped, and every
            // reader above works unchanged: `shape_field` looks a key up by name, and
            // `$row[0]` on a shape keyed `0` already resolved before this did.
            //
            // Dropped, with the stated reason that nothing reads a shape by position, it
            // took the whole shape with it -- a shape with no readable field falls back to
            // a bare `array` below. So `array{Thing, string}` said nothing at all, and
            // `array<array{Thing, string}>` said `array[]`. The reason was circular:
            // nothing read a position because nothing stored one. Counted in real code
            // before changing it -- 209 in Symfony's own source, 176 in one project's
            // vendor tree, 44 in Laravel's, 26 in that project's own code.
            //
            // Materialising what the spelling means rather than adding a vocabulary for it,
            // which is the same move §8 records for `list<V>` becoming `array<int, V>`.
            let (key, optional) = match field.key.as_ref() {
                Some(written) => {
                    let Some(key) = shape_key(&written.key) else {
                        continue;
                    };
                    // An explicit integer key advances the counter, as it does in an array
                    // literal: `array{0: A, B}` gives `B` the key 1, not 0.
                    if let Ok(explicit) = key.parse::<u64>() {
                        next = next.max(explicit.saturating_add(1));
                    }
                    (key, written.question_mark.is_some())
                }
                None => {
                    let key = next.to_string();
                    next += 1;
                    // A positional entry has no key node to carry a `?`, so there is
                    // nowhere for one to be written.
                    (key, false)
                }
            };
            let Some(ty) = self.lower_doc_type(field.value, base, text, aliases) else {
                continue;
            };
            fields.push(self.builder.add_type_ref(TypeRefData::ShapeField {
                key: self.interner.name(&key),
                optional,
                ty,
            }));
        }
        fields
    }

    /// Lowers one docblock type into the written-type representation.
    ///
    /// `aliases` is the enclosing class's `@phpstan-type` table (see
    /// [`super::type_alias::TypeAliasTable`]), or [`TypeAliasTable::EMPTY`] wherever there is no
    /// class to have declared one -- a free function, a `@throws`, a body-level `@var`. Threaded
    /// through every recursive call here rather than read from `self`, because an alias may appear
    /// anywhere a type is written, nested inside a union or a generic argument included.
    pub(crate) fn lower_doc_type(
        &mut self,
        doc_type: &Type<'_>,
        base: phrost_model::TextSize,
        text: &str,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        let range = shift(doc_type.span(), base);

        let data = match doc_type {
            // Grouping, not structure.
            Type::Parenthesized(inner) => {
                return self.lower_doc_type(inner.inner, base, text, aliases);
            }
            Type::Nullable(nullable) => {
                TypeRefData::Nullable(self.lower_doc_type(nullable.inner, base, text, aliases)?)
            }
            Type::Union(union) => {
                let left = self.lower_doc_type(union.left, base, text, aliases)?;
                let right = self.lower_doc_type(union.right, base, text, aliases)?;
                let parts = self.builder.alloc_type_ref_list(&[left, right]);
                TypeRefData::Union(parts)
            }
            Type::Intersection(intersection) => {
                let left = self.lower_doc_type(intersection.left, base, text, aliases)?;
                let right = self.lower_doc_type(intersection.right, base, text, aliases)?;
                let parts = self.builder.alloc_type_ref_list(&[left, right]);
                TypeRefData::Intersection(parts)
            }
            Type::Reference(reference) => {
                // A name the enclosing class gave to a whole type stands for that type wherever it
                // is used -- so it is checked before anything that would otherwise treat the same
                // identifier as a class. `UserList<T>` is nothing this scheme supports (an alias is
                // not itself generic), so the substitution wins outright and whatever arguments were
                // written beside it are dropped along with the identifier.
                if let ReferenceKind::Identifier(identifier) = &reference.kind {
                    let written = slice(text, identifier.span());
                    if let Some(substituted) = aliases.resolve(written, self.interner) {
                        return Some(substituted);
                    }
                    // A name the class imported from another file, which nothing here can expand.
                    // Declining is the whole of what recording it buys: lowered as a reference it
                    // would be a class the project does not declare, and every value written for it
                    // reported. See `type_alias`.
                    if aliases.is_imported(written) {
                        return None;
                    }
                }
                let named = self.doc_reference_base(&reference.kind, range, text);
                // `Collection<User>`: a class with members *and* a thing to iterate, so both halves
                // are kept.
                return Some(self.with_arguments(
                    named,
                    reference.parameters.as_ref(),
                    aliases,
                    base,
                    text,
                ));
            }
            // Every shape that is an array in written PHP, with its arguments kept: the base is what
            // the value is, and the last argument is what iterating it yields.
            Type::Array(array) => {
                return self.generic(
                    BuiltinTy::Array,
                    array.parameters.as_ref(),
                    aliases,
                    base,
                    text,
                    range,
                );
            }
            Type::NonEmptyArray(array) => {
                let array = self.generic(
                    BuiltinTy::Array,
                    array.parameters.as_ref(),
                    aliases,
                    base,
                    text,
                    range,
                )?;
                return Some(self.non_empty(array));
            }
            Type::AssociativeArray(array) => {
                return self.generic(
                    BuiltinTy::Array,
                    array.parameters.as_ref(),
                    aliases,
                    base,
                    text,
                    range,
                );
            }
            Type::List(list) => {
                return self.list(list.parameters.as_ref(), aliases, base, text);
            }
            Type::NonEmptyList(list) => {
                let list = self.list(list.parameters.as_ref(), aliases, base, text)?;
                return Some(self.non_empty(list));
            }
            // `User[]` -- the same thing written the other way round.
            Type::Slice(slice) => {
                let element = self.lower_doc_type(slice.inner, base, text, aliases)?;
                let array = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::Array));
                let args = self.builder.alloc_type_ref_list(&[element]);
                return Some(self.builder.add_type_ref(TypeRefData::Generic {
                    base: array,
                    args,
                    non_empty: false,
                }));
            }
            // A shape says what its keys are, not what iterating yields uniformly -- which is the
            // whole reason it is kept as a shape rather than flattened to `array`: `array<string,
            // mixed>` says every key holds the same thing, and this says which key holds what.
            Type::Shape(shape) => {
                let fields = self.shape_fields(shape.fields.iter(), base, text, aliases);
                // Two things used to arrive here as one, and the difference is the whole of what
                // `array{}|array{...}` means. `array{}` *written* is a statement -- there is
                // nothing in it -- and PHP reads any key off it as null. A shape whose fields all
                // failed to lower is an absence of knowledge and is still an array, which is what
                // the declaration says. Collapsed into a bare `array`, the first became the second
                // and the bare arm then vetoed every key read of the union it was in: 29 of the 38
                // `Offset 'x' might not exist` findings PHPStan reports on the reference project
                // are that shape, written by hand in the project's own `@return`.
                //
                // A shape is sealed unless it says `...`, which is how PHPStan and Psalm read it. One
                // that lost a field above is open whatever it says, because the key it lost is one
                // the array may hold -- sealed, it would read as an extra key on every caller that
                // passes it. And `array{...}` with nothing named is the bare array it says.
                let open = shape.additional_fields.is_some() || fields.len() < shape.fields.len();
                let list = matches!(
                    shape.kind,
                    mago_phpdoc_syntax::cst::r#type::ShapeTypeKind::List
                        | mago_phpdoc_syntax::cst::r#type::ShapeTypeKind::NonEmptyList
                );
                match (shape.fields.is_empty(), fields.is_empty(), open) {
                    (true, _, false) => TypeRefData::Shape {
                        fields: self.builder.alloc_type_ref_list(&[]),
                        open: false,
                        list,
                    },
                    (true, _, true) | (false, true, _) => TypeRefData::Builtin(BuiltinTy::Array),
                    (false, false, _) => TypeRefData::Shape {
                        fields: self.builder.alloc_type_ref_list(&fields),
                        open,
                        list,
                    },
                }
            }
            Type::Iterable(iterable) => {
                return self.generic(
                    BuiltinTy::Iterable,
                    iterable.parameters.as_ref(),
                    aliases,
                    base,
                    text,
                    range,
                );
            }
            Type::Callable(callable) => return self.callable(callable, aliases, base, text),
            Type::CallableString(_) => TypeRefData::Builtin(BuiltinTy::Callable),
            // An object whose properties are named says what each holds, which is the half a bare
            // `object` cannot: `TRelatedModel&object{pivot: TPivotModel}` is how Laravel's
            // `BelongsToMany` states what it loads. Every property lost to lowering leaves the
            // object it still is.
            Type::Object(object) => match object.properties.as_ref() {
                Some(properties) => {
                    let fields = self.shape_fields(properties.fields.iter(), base, text, aliases);
                    match fields.is_empty() {
                        true => TypeRefData::Builtin(BuiltinTy::Object),
                        false => TypeRefData::ObjectShape {
                            fields: self.builder.alloc_type_ref_list(&fields),
                        },
                    }
                }
                None => TypeRefData::Builtin(BuiltinTy::Object),
            },
            Type::StringableObject(_) => TypeRefData::Builtin(BuiltinTy::Object),
            Type::Null(_) => TypeRefData::Builtin(BuiltinTy::Null),
            Type::Void(_) => TypeRefData::Builtin(BuiltinTy::Void),
            Type::Never(_) => TypeRefData::Builtin(BuiltinTy::Never),
            Type::True(_) => TypeRefData::Builtin(BuiltinTy::True),
            Type::False(_) => TypeRefData::Builtin(BuiltinTy::False),
            Type::Bool(_) => TypeRefData::Builtin(BuiltinTy::Bool),
            Type::Float(_) | Type::LiteralFloat(_) | Type::UnspecifiedLiteralFloat(_) => {
                TypeRefData::Builtin(BuiltinTy::Float)
            }
            // Every way the docblock language has of narrowing an int. All of them are `int` in
            // written PHP, and the narrowing is a constraint rather than a different type.
            Type::Int(_)
            | Type::PositiveInt(_)
            | Type::NegativeInt(_)
            | Type::NonPositiveInt(_)
            | Type::NonNegativeInt(_)
            | Type::NonZeroInt(_)
            | Type::LiteralInt(_)
            | Type::UnspecifiedLiteralInt(_)
            | Type::IntRange(_)
            | Type::IntMask(_)
            | Type::IntMaskOf(_) => TypeRefData::Builtin(BuiltinTy::Int),
            // And every way it has of narrowing a string. A `class-string<T>` is a string at
            // runtime whatever it names, which is why the family belongs in this fold too -- but
            // which class it names is a real fact `new $var` and `$var::method()` need, so those
            // five keep their argument. See [`Self::class_string`].
            //
            // What a string is known to *hold* is kept, though: `non-empty-string` and
            // `non-falsy-string` are types of their own -- see [`BuiltinTy::NonEmptyString`] -- and
            // every narrowing that also says the string is not empty reads as the non-empty one,
            // which is the weaker claim that is still true.
            //
            // `numeric-string` is not among them, though no empty string spells a number. Read as
            // non-empty it would make every `@param numeric-string` a contract a plain `string`
            // fails -- `bcadd()`'s two operands, 36 findings on the reference project alone -- and
            // whether a string spells a number is a question this does not ask anywhere else. The
            // values that are numeric strings and must count as holding something say so another
            // way: `(string) $int` by its operand, and `is_numeric()` by the guard it is.
            Type::NonEmptyString(_)
            | Type::NonEmptyLowercaseString(_)
            | Type::NonEmptyUppercaseString(_)
            | Type::NonEmptyUnspecifiedLiteralString(_) => {
                TypeRefData::Builtin(BuiltinTy::NonEmptyString)
            }
            Type::TruthyString(_) | Type::NonFalsyString(_) => {
                TypeRefData::Builtin(BuiltinTy::NonFalsyString)
            }
            // A literal names its own text, and the text says which of the three it is.
            Type::LiteralString(literal) => {
                TypeRefData::Builtin(BuiltinTy::string_holding(literal.value))
            }
            Type::String(_)
            | Type::NumericString(_)
            | Type::LowercaseString(_)
            | Type::UppercaseString(_)
            | Type::UnspecifiedLiteralString(_)
            | Type::LowercaseCallableString(_)
            | Type::UppercaseCallableString(_) => TypeRefData::Builtin(BuiltinTy::String),
            // `array-key` is `int|string`, and it is written where a key goes. Read as `string`
            // alone it makes an int-keyed array a finding against it, which is the one thing a key
            // that is either must never do.
            Type::ArrayKey(_) => {
                let int = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::Int));
                let string = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::String));
                let parts = self.builder.alloc_type_ref_list(&[int, string]);
                TypeRefData::Union(parts)
            }
            Type::ClassString(class_string) => {
                return self.class_string(class_string.parameter.as_ref(), aliases, base, text);
            }
            Type::ClassLikeString(class_string) => {
                return self.class_string(class_string.parameter.as_ref(), aliases, base, text);
            }
            Type::InterfaceString(class_string) => {
                return self.class_string(class_string.parameter.as_ref(), aliases, base, text);
            }
            Type::EnumString(class_string) => {
                return self.class_string(class_string.parameter.as_ref(), aliases, base, text);
            }
            Type::TraitString(class_string) => {
                return self.class_string(class_string.parameter.as_ref(), aliases, base, text);
            }
            // `numeric` is `int|float|numeric-string` and `scalar` is those plus `bool`, and every
            // operand already exists -- so this is a lowering rule rather than a new `BuiltinTy`,
            // which matters: that vocabulary is closed by convention and adding to it is a
            // semantics change needing the ratchet.
            //
            // `numeric-string` folds to plain `string` two arms up, so the union written here is
            // the honest weaker claim: `array_sum(['a'])` is still accepted where `array<int|float>`
            // rejected it, and `array_sum([true])` is not. Folding both to `mixed` accepted
            // everything, which was inert until mago-prelude 1.48.0 respelled `array_sum` as
            // `@param array<numeric>` and quietly took the check with it.
            Type::Numeric(_) => {
                return self.one_of(&[BuiltinTy::Int, BuiltinTy::Float, BuiltinTy::String]);
            }
            Type::Scalar(_) => {
                return self.one_of(&[
                    BuiltinTy::Int,
                    BuiltinTy::Float,
                    BuiltinTy::String,
                    BuiltinTy::Bool,
                ]);
            }
            Type::Mixed(_) | Type::NonEmptyMixed(_) => TypeRefData::Builtin(BuiltinTy::Mixed),
            // `$this` in a docblock means the receiver, which is what `static` means natively.
            Type::ThisVariable(_) => TypeRefData::Relative(RelativeTy::Static),
            // `Status::PENDING` and `Status::KIND_*` -- a class constant used as a type. What it
            // *is* is the constant's own value, which this layer does not model, so there is no type
            // here. The class is still a name somebody wrote, and one they can ctrl-click and can
            // import: read as no name at all, `use App\Status;` beside `@param Status::KIND_* $k`
            // was an unused import.
            Type::MemberReference(member) => {
                let base_range = shift(member.kind.span(), base);
                self.doc_reference_base(&member.kind, base_range, text);
                return None;
            }
            // `($x is class-string<Rule> ? A : B)`. Which branch a call takes is read by the
            // contract reader out of the tag, and most calls settle nothing -- the argument is a
            // variable nobody typed. What comes back is then still one of the two branches, which
            // is the type this declares: `Arr::two($mixed)` under `($v is string ? string : array)`
            // is a `string|array`, where reading the conditional as no type at all answered nothing
            // for every such call.
            //
            // All four parts are lowered whatever happens to the branches, because each is a name
            // somebody wrote -- and the `is` target is only ever written here, so no other reader
            // walks it. A branch that is `never` leaves nothing behind to be part of the answer, and
            // one that is `mixed` is the whole answer: `string|array|mixed` read as a union would
            // judge its `array` against a `string` parameter, for a value that may be anything.
            Type::Conditional(conditional) => {
                let [_, _, then, otherwise] = [
                    conditional.subject,
                    conditional.target,
                    conditional.then,
                    conditional.r#else,
                ]
                .map(|part| self.lower_doc_type(part, base, text, aliases));
                let is = |lowerer: &Self, branch: TypeRefId, kind: BuiltinTy| {
                    matches!(
                        lowerer.builder.type_ref(branch),
                        TypeRefData::Builtin(builtin) if *builtin == kind
                    )
                };
                let (then, otherwise) = (then?, otherwise?);
                if is(self, then, BuiltinTy::Mixed) || is(self, otherwise, BuiltinTy::Never) {
                    return Some(then);
                }
                if is(self, otherwise, BuiltinTy::Mixed) || is(self, then, BuiltinTy::Never) {
                    return Some(otherwise);
                }
                TypeRefData::Union(self.builder.alloc_type_ref_list(&[then, otherwise]))
            }
            // `key-of<Config>` and `value-of<Config>`: the same, one level in.
            Type::KeyOf(key_of) => {
                self.lower_doc_type(&key_of.parameter.entry.inner, base, text, aliases);
                return None;
            }
            Type::ValueOf(value_of) => {
                self.lower_doc_type(&value_of.parameter.entry.inner, base, text, aliases);
                return None;
            }
            // All three spellings are the same value to everything above this line: whether the
            // handle is still open is a refinement nothing here models, and reading `open-resource`
            // as unmodelled loses the whole type rather than the refinement -- which is what
            // `fopen`'s `@return open-resource|false` had been doing.
            Type::Resource(_) | Type::ClosedResource(_) | Type::OpenResource(_) => {
                TypeRefData::Builtin(BuiltinTy::Resource)
            }
            // A template parameter, an int mask: real types with no written-PHP equivalent and no
            // class named inside them. Recording them as unmodelled says so, rather than picking a
            // neighbour and being confidently wrong about what has members.
            _ => return None,
        };

        Some(self.builder.add_type_ref(data))
    }

    /// The class a docblock reference names, without whatever arguments were written on it.
    ///
    /// Its own step because a contract wants the two halves apart: `@return Bar<T>` resolves `Bar`
    /// like any other name and reads `T` off an argument, so the base has to be reachable without the
    /// arguments coming with it.
    pub(crate) fn doc_reference_base(
        &mut self,
        kind: &ReferenceKind<'_>,
        range: TextRange,
        text: &str,
    ) -> TypeRefId {
        let data = match kind {
            ReferenceKind::Identifier(identifier) => {
                let written = slice(text, identifier.span());
                // `\int` is a keyword with a slash in front, not a class in the root namespace: PHP
                // refuses the spelling on a native type, and in a docblock it is spelling. Read as a
                // class, it was a name nothing declares and every argument for it went unjudged.
                match written.strip_prefix('\\').and_then(BuiltinTy::from_name) {
                    Some(builtin) => TypeRefData::Builtin(builtin),
                    None => TypeRefData::Named(self.doc_name_ref(written, range)),
                }
            }
            ReferenceKind::Self_(_) => TypeRefData::Relative(RelativeTy::SelfTy),
            ReferenceKind::Static(_) => TypeRefData::Relative(RelativeTy::Static),
            ReferenceKind::Parent(_) => TypeRefData::Relative(RelativeTy::Parent),
        };
        self.builder.add_type_ref(data)
    }

    /// `list<V>`, which is `array<int, V>` and now says so.
    ///
    /// Materialised with the key it means rather than left as the one-argument shape, because `V[]`
    /// interned to that same shape and the two do not say the same thing: brackets state a container
    /// and nothing about how it is keyed, where a list states that every key is an int. Handing an
    /// int-keyed array to a parameter expecting string keys is a real finding, and telling these two
    /// spellings apart is the whole of what makes it one -- see `phrost_ty::compat`'s key rule.
    ///
    /// Only the one-argument spelling. A bare `list` names nothing to hold and `list<K, V>` is not a
    /// type anybody can write; either way the base alone is what is known.
    fn list(
        &mut self,
        parameters: Option<&mago_phpdoc_syntax::cst::r#type::generics::GenericParameters<'_>>,
        aliases: &TypeAliasTable,
        base_offset: phrost_model::TextSize,
        text: &str,
    ) -> Option<TypeRefId> {
        let array = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Array));
        let written: Vec<_> = parameters
            .map(|parameters| parameters.entries.iter().collect())
            .unwrap_or_default();
        let [entry] = written.as_slice() else {
            return Some(array);
        };
        let Some(element) = self.lower_doc_type(&entry.inner, base_offset, text, aliases) else {
            return Some(array);
        };
        let key = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Int));
        let args = self.builder.alloc_type_ref_list(&[key, element]);
        Some(self.builder.add_type_ref(TypeRefData::Generic {
            base: array,
            args,
            non_empty: false,
        }))
    }

    /// The same array, said to hold an element: `non-empty-array<K, V>`, `non-empty-list<V>`.
    ///
    /// Only where the arguments made it a `Generic`. A bare `non-empty-array` is the bare base, and
    /// a node with no arguments to carry the flag is a degenerate form every reader would have to
    /// learn -- so that spelling says no more than `array` did.
    fn non_empty(&mut self, array: TypeRefId) -> TypeRefId {
        match self.builder.type_ref(array).clone() {
            TypeRefData::Generic { base, args, .. } => {
                self.builder.add_type_ref(TypeRefData::Generic {
                    base,
                    args,
                    non_empty: true,
                })
            }
            _ => array,
        }
    }

    /// A builtin base with the arguments a docblock gave it.
    fn generic(
        &mut self,
        builtin: BuiltinTy,
        parameters: Option<&mago_phpdoc_syntax::cst::r#type::generics::GenericParameters<'_>>,
        aliases: &TypeAliasTable,
        base_offset: phrost_model::TextSize,
        text: &str,
        _range: TextRange,
    ) -> Option<TypeRefId> {
        let builtin = self.builder.add_type_ref(TypeRefData::Builtin(builtin));
        Some(self.with_arguments(builtin, parameters, aliases, base_offset, text))
    }

    /// Attaches type arguments to an already-lowered base, or returns the base unchanged.
    ///
    /// An argument this representation cannot hold drops the whole list rather than the type: a
    /// `Collection<T>` whose `T` is a template parameter is still a `Collection`, and forgetting
    /// that would lose the members along with the element.
    fn with_arguments(
        &mut self,
        base: TypeRefId,
        parameters: Option<&mago_phpdoc_syntax::cst::r#type::generics::GenericParameters<'_>>,
        aliases: &TypeAliasTable,
        base_offset: phrost_model::TextSize,
        text: &str,
    ) -> TypeRefId {
        let Some(parameters) = parameters else {
            return base;
        };

        let mut args = Vec::new();
        for entry in parameters.entries.iter() {
            match self.lower_doc_type(&entry.inner, base_offset, text, aliases) {
                Some(id) => args.push(id),
                None => return base,
            }
        }
        // Arguments that are all `mixed` say nothing the base does not: `array<mixed, mixed>` is the
        // bare `array` spelled out, and a third spelling of it is one more a reader has to know. It
        // arrives when a template stands for `mixed`, and judged as written it was an array keyed by
        // `mixed` rather than one whose keys nothing said.
        let mixed = |lowerer: &Self, id: TypeRefId| {
            matches!(
                lowerer.builder.type_ref(id),
                TypeRefData::Builtin(BuiltinTy::Mixed)
            )
        };
        if args.is_empty() || args.iter().all(|&arg| mixed(self, arg)) {
            return base;
        }

        let args = self.builder.alloc_type_ref_list(&args);
        self.builder.add_type_ref(TypeRefData::Generic {
            base,
            args,
            non_empty: false,
        })
    }

    /// `class-string<T>` and its four siblings: a `string` base with the class it names as its one
    /// argument.
    ///
    /// The same [`TypeRefData::Generic`] shape `array<T>` and `Collection<T>` already use, and
    /// deliberately so: every reader that peels a generic down to its base already exists --
    /// [`phrost_ty::compat`] treats the value and the expectation identically either way, so a
    /// `class-string<Rule>` keeps satisfying a plain `string` parameter without a line of new
    /// code there. What is new is reading the *argument* rather than the base, which is a
    /// different question -- see `class_named_by_class_string` in `phrost-view`.
    ///
    /// Bare when no `<T>` was written, or when `T` has no written-PHP equivalent: still a string
    /// either way, and [`Self::with_arguments`]'s rule -- drop the list, keep the base -- is the
    /// same rule here.
    /// `callable(Item): string` -- a callable that says what invoking it gives back.
    ///
    /// Kept as a [`TypeRefData::Generic`] over the `callable` builtin, exactly as
    /// [`Self::class_string`] keeps its argument over `string`: every reader that peels a generic to
    /// its base already treats it as a plain `callable`, so nothing that checks a `callable`
    /// parameter changes, and the one reader that wants the *argument* is the one that asks what
    /// invoking the value produces.
    ///
    /// The return is the only part that becomes the **type**. What `$fn($x)` needs is the result;
    /// the parameters would only be read to check the call's own arguments, which is a separate
    /// feature nothing asks for yet -- and a shape that carried them would have to say which
    /// argument is which, where a one-argument generic says nothing ambiguous. The renderer spells
    /// it `callable(...): string`, which is how a first-class callable already reads.
    ///
    /// The parameters are still *lowered*, and their ids thrown away. A type is not the only thing
    /// lowering a type produces: it also records a name reference, which is what makes a class
    /// navigable and what tells `unused-import` the name was used. Dropping the parameters whole
    /// meant `use Symfony\Component\HttpFoundation\Request;` beside
    /// `@param \Closure(Request): void` read as unused -- **62 of Symfony's 404 `unused-import`
    /// findings**, on imports its own source needs. The same half-measure `@see` needed: the
    /// reference costs the index nothing, because `NameRefData` lives in a `FileHir` and the ids
    /// here are arena entries nothing keeps.
    ///
    /// Bare when no return type was written: `callable(Item)` promises nothing about its result, and
    /// a `Generic` with no argument is a degenerate form the base already expresses.
    fn callable(
        &mut self,
        callable: &mago_phpdoc_syntax::cst::r#type::CallableType<'_>,
        aliases: &TypeAliasTable,
        base_offset: phrost_model::TextSize,
        text: &str,
    ) -> Option<TypeRefId> {
        let bare = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Callable));

        // `Closure(int): string` names a class as well as a signature, and the class is what an
        // import of `Closure` is for: read as a bare `callable`, `use Closure;` above it was reported
        // unused. Recorded as a reference and nothing more, for the reason the parameters below are.
        if matches!(
            callable.kind,
            mago_phpdoc_syntax::cst::r#type::callable::CallableTypeKind::Closure
        ) {
            let range = shift(callable.keyword.span, base_offset);
            let written = std::str::from_utf8(callable.keyword.value).unwrap_or("Closure");
            self.doc_name_ref(written, range);
        }

        // For the names in them and nothing else -- see this function's own doc.
        if let Some(spec) = callable.specification.as_ref() {
            for parameter in spec.parameters.entries.iter() {
                if let Some(written) = parameter.parameter_type.as_ref() {
                    self.lower_doc_type(written, base_offset, text, aliases);
                }
            }
        }

        let Some(returned) = callable
            .specification
            .as_ref()
            .and_then(|spec| spec.return_type.as_ref())
        else {
            return Some(bare);
        };
        let Some(returns) = self.lower_doc_type(returned.return_type, base_offset, text, aliases)
        else {
            return Some(bare);
        };
        let args = self.builder.alloc_type_ref_list(&[returns]);
        Some(self.builder.add_type_ref(TypeRefData::Generic {
            base: bare,
            args,
            non_empty: false,
        }))
    }

    /// A union of builtins, for the docblock spellings that name a set PHP has no single word for.
    fn one_of(&mut self, operands: &[BuiltinTy]) -> Option<TypeRefId> {
        let ids: Vec<TypeRefId> = operands
            .iter()
            .map(|builtin| self.builder.add_type_ref(TypeRefData::Builtin(*builtin)))
            .collect();
        let parts = self.builder.alloc_type_ref_list(&ids);
        Some(self.builder.add_type_ref(TypeRefData::Union(parts)))
    }

    fn class_string(
        &mut self,
        parameter: Option<&mago_phpdoc_syntax::cst::r#type::generics::SingleGenericParameter<'_>>,
        aliases: &TypeAliasTable,
        base_offset: phrost_model::TextSize,
        text: &str,
    ) -> Option<TypeRefId> {
        let string = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::String));
        // A bare `class-string` is a class-string of whatever class it names, `object` being the
        // spelling for that -- and kept as a class-string rather than folded to `string`, because
        // what it holds is a fact a value needs: a class's name is never `''` and never `'0'`, so
        // `get_class($x)` is a string a `non-empty-string` parameter takes. A declaration of one
        // still admits any string, as the base is what is judged on that side.
        let argument = match parameter {
            Some(parameter) => {
                self.lower_doc_type(&parameter.entry.inner, base_offset, text, aliases)
            }
            None => None,
        };
        let argument = argument.unwrap_or_else(|| {
            self.builder
                .add_type_ref(TypeRefData::Builtin(BuiltinTy::Object))
        });
        let args = self.builder.alloc_type_ref_list(&[argument]);
        Some(self.builder.add_type_ref(TypeRefData::Generic {
            base: string,
            args,
            non_empty: false,
        }))
    }

    /// A name reference for a class named inside a docblock.
    ///
    /// Anchored like any written name, so the file's imports apply: `@return Repo` in a file that
    /// imports `App\Repo` means that class, exactly as a native hint would.
    fn doc_name_ref(&mut self, written: &str, range: TextRange) -> phrost_hir::NameRefId {
        let (anchor, path) = NameAnchor::of(written);
        let path = self.interner.name(path);
        self.add_name_ref(NameRefData {
            path,
            anchor,
            kind: phrost_hir::ImportKind::ClassLike,
            range,
            in_docblock: true,
            cross_reference: false,
        })
    }
}

/// The key a shape field is written under, when it is one this can name.
///
/// A string or an integer, both of which a PHP array really is keyed by. A `Foo::BAR` key names a
/// constant whose value only the index knows, and resolving one here would mean resolving names
/// during lowering -- so it is left out and the field with it.
fn shape_key(key: &mago_phpdoc_syntax::cst::r#type::ShapeKey<'_>) -> Option<String> {
    use mago_phpdoc_syntax::cst::r#type::ShapeKey;

    match key {
        ShapeKey::String { value, .. } => {
            let written = String::from_utf8_lossy(value).into_owned();
            // Quoted keys are written both ways: `array{'id': int}` and `array{id: int}` are the
            // same key.
            Some(
                written
                    .strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
                    .or_else(|| {
                        written
                            .strip_prefix('"')
                            .and_then(|rest| rest.strip_suffix('"'))
                    })
                    .unwrap_or(&written)
                    .to_owned(),
            )
        }
        ShapeKey::Integer { value, .. } => Some(value.to_string()),
        ShapeKey::ClassLikeConstant { .. } => None,
    }
}
