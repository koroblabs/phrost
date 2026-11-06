//! What `$value[$key]` gives back.
//!
//! The one read that depends on the key as well as the container, which is what separates it from
//! [`super::element`]: a shape answers the key it was named with, a string indexed is a string,
//! and only what is neither falls through to the element type. That ordering is the whole
//! behaviour -- a shape that does not name the key is a miss and must not be answered from the
//! element -- so it is stated once, here, rather than at each caller.

use phrost_hir::{FileHir, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, Name, TextSize};

use crate::{resolve::Held, view::IndexView};

/// Whether an element read is a subscript somebody wrote, or may be an iteration.
///
/// The two share a step -- `foreach ($rows as $row)` and `$rows[$id]` both read an element -- and
/// on an array they are one question. On an object they are two: a subscript runs `offsetGet` and
/// iterating runs `getIterator`, and WordPress's cookie `Jar` answers `string|null` from one and
/// hands out `Cookie` objects from the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Subscript {
    /// `$value[$key]` as written, whatever the key is.
    Written,
    /// An element step with no key, which is how an iteration is spelled too.
    Unknown,
}

impl IndexView<'_> {
    /// What reading `held[key]` gives back.
    ///
    /// Three questions in one, which is why it is one method: the index operation is written the
    /// same way whatever it is applied to, and a reader that had to know which of the three it was
    /// asking would have to know the type first.
    ///
    /// A shape answers the key it was given, because that is the whole reason it is kept as a
    /// shape. A string indexed is a string -- PHP's offset read, and the one case where the
    /// container and the element are the same type. Everything else holds the same thing at every
    /// slot, so the key identified one and the element type is the answer.
    pub(crate) fn indexed_of_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        key: Option<&str>,
        subscript: Subscript,
    ) -> Option<Held> {
        match key {
            // A key the shape does not name is a miss and stays one: the shape is the whole
            // statement of what is in there, so falling through to the element type would answer a
            // question the type already said no to.
            Some(key) => {
                if let Some(found) = self.keyed_of_held(offset, hir, held, key) {
                    return Some(found);
                }
                // A class whose subscript is a lookup: `$app['config']` is `offsetGet('config')`,
                // whose body is `return $this->make($offset);`. The same reader a call with a
                // literal first argument goes through, reached from the one spelling that has no
                // call to read -- so a framework states the fact once and both answer.
                //
                // The binding list is asked before the class is resolved, and that order is the
                // whole of what makes it affordable: it is a scan of a handful of short names, and
                // in a project whose framework binds nothing it is empty, so a subscript pays one
                // failed comparison instead of a receiver resolution.
                if !self
                    .skeleton
                    .returns()
                    .argument_bindings("offsetGet")
                    .is_empty()
                    && let Some(class) = self.class_of_held(offset, hir, held)
                    && let Some(found) =
                        self.returned_from_argument(class.as_str(), "offsetGet", Some(key))
                {
                    return Some(found);
                }
            }
            // A key nothing here can name reaches every slot at once. On a shape that is one answer
            // only when every field holds the same thing -- and when they do not, saying so is the
            // honest half of not being able to express a union.
            None => {
                if let Some(values) = self.shape_values(offset, hir, held) {
                    return Some(values);
                }
            }
        }

        if self.holds_a_string(hir, held) {
            return Some(Held::Builtin(BuiltinTy::String));
        }

        let element = self.element_of_held(offset, hir, held);
        match subscript {
            Subscript::Written => self
                .offset_get_given(offset, hir, held)
                .or(element)
                .or_else(|| self.offset_get_of_held(offset, hir, held)),
            Subscript::Unknown => element,
        }
    }

    /// What a class's own `offsetGet` says a subscript gives back, when no template said it first.
    ///
    /// PHP runs `offsetGet` for `$object[$key]`, so its return is the subscript's type -- and a
    /// class written `offsetGet(mixed $offset): Item` with no `@implements ArrayAccess<K, V>` is
    /// how most of them are written. Last, because a template argument on the value or on an
    /// `@implements` is the more specific claim and `element_of_held` has already read it.
    fn offset_get_of_held(&self, offset: TextSize, hir: &FileHir, held: &Held) -> Option<Held> {
        let class = self.class_of_held(offset, hir, held)?;
        self.member_held(class.as_str(), phrost_sema::MemberKind::Method, "offsetGet")
    }

    /// What `offsetGet` gives back for a value that carries its own type arguments.
    ///
    /// Before what iterating the value yields, because the two are different slots of one class and
    /// the subscript is PHP running `offsetGet`: `SplObjectStorage<Item, Meta>` iterates its objects
    /// and maps each to a `Meta`, so `$storage[$item]` is the `Meta` -- and reading the element
    /// answered `Item`. Only with arguments to substitute, since without them the return is the
    /// class's own template, and never `mixed`, which is what a class writes when it has nothing to
    /// say and would hide the element behind it.
    fn offset_get_given(&self, offset: TextSize, hir: &FileHir, held: &Held) -> Option<Held> {
        let arguments = self.arguments_of_held(hir, held);
        if arguments.is_empty() {
            return None;
        }
        let class = self.class_of_held(offset, hir, held)?;
        self.member_held_on(
            class.as_str(),
            phrost_sema::MemberKind::Method,
            "offsetGet",
            None,
            &arguments,
        )
        .filter(|found| !matches!(found, Held::Builtin(BuiltinTy::Mixed)))
    }

    /// Whether the value is a string, however the type reached this layer.
    ///
    /// Three spellings for one answer: a literal produces a builtin, a native hint a written type,
    /// and a declaration in another file an interned one. Indexing is the one operation where the
    /// difference matters, because `$word[0]` is a string and everything else's element is not.
    fn holds_a_string(&self, hir: &FileHir, held: &Held) -> bool {
        match held {
            Held::Object(_) => false,
            // What the string holds is no matter: a character of it is a string either way.
            Held::Builtin(builtin) if builtin.is_string() => true,
            Held::Nullable(inner) => self.holds_a_string(hir, inner),
            Held::Written(id) => {
                matches!(hir.type_ref(*id), TypeRefData::Builtin(builtin) if builtin.is_string())
            }
            Held::Stored { ty, .. } => matches!(
                self.skeleton.types().get(*ty),
                phrost_ty::TypeRef::Builtin(builtin) if builtin.is_string()
            ),
            Held::Builtin(_)
            | Held::Class(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Record(_)
            | Held::Callable(_) => false,
            Held::OneOf(operands) => operands
                .iter()
                .all(|operand| self.holds_a_string(hir, operand)),
            // An intersection is every operand at once, so one of them being a string makes it one.
            Held::AllOf(operands) => operands
                .iter()
                .any(|operand| self.holds_a_string(hir, operand)),
            Held::Applied { .. } => false,
        }
    }

    /// What a shape holds under a key nothing here can name: one of its values.
    ///
    /// A key the reader cannot name reaches every slot at once, so the answer is every value the
    /// shape declares -- and the one value when they all declare the same, which is most shapes
    /// anybody writes a computed key against.
    ///
    /// An interned shape compares by id, which is what interning is for. A written one cannot: the
    /// two `Money`s in `array{normal: Money, member: Money}` are two nodes in a file's arena, which
    /// nothing dedupes, so they are compared by what they name instead.
    pub(crate) fn shape_values(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Held> {
        match held {
            Held::Object(_) => None,
            Held::Nullable(inner) => self.shape_values(offset, hir, inner),
            Held::Record(fields) => (!fields.is_empty())
                .then(|| Self::one_of(fields.iter().map(|(_, value)| value.clone()).collect())),
            Held::Stored { ty, scope } => {
                let fields = self.skeleton.types().shape_keys(*ty);
                let mut values = Vec::new();
                for (_, ty, _) in &fields {
                    if !values.contains(ty) {
                        values.push(*ty);
                    }
                }
                (!values.is_empty()).then(|| {
                    Self::one_of(
                        values
                            .into_iter()
                            .map(|ty| {
                                // As a nullable the union constructor can take apart, so a
                                // `?int` field beside a `string` one is `string|int|null`.
                                match self.skeleton.types().get(ty) {
                                    phrost_ty::TypeRef::Nullable(inner) => {
                                        Self::nullable(Held::Stored {
                                            ty: *inner,
                                            scope: scope.clone(),
                                        })
                                    }
                                    _ => Held::Stored {
                                        ty,
                                        scope: scope.clone(),
                                    },
                                }
                            })
                            .collect(),
                    )
                })
            }
            Held::Written(id) => {
                let TypeRefData::Shape { fields, .. } = hir.type_ref(*id) else {
                    return None;
                };
                let mut values: Vec<(Option<Result<Name, BuiltinTy>>, TypeRefId)> = Vec::new();
                for field in hir.type_ref_list(*fields) {
                    let TypeRefData::ShapeField { ty, .. } = hir.type_ref(*field) else {
                        continue;
                    };
                    // A field this cannot name is kept as itself: two of them may be the same type
                    // and nothing here can tell, so they stay two operands rather than one.
                    let named = self.named_or_builtin(offset, hir, *ty);
                    if named.is_none() || !values.iter().any(|(seen, _)| *seen == named) {
                        values.push((named, *ty));
                    }
                }
                (!values.is_empty()).then(|| {
                    Self::one_of(
                        values
                            .into_iter()
                            .map(|(_, ty)| match hir.type_ref(ty) {
                                TypeRefData::Nullable(inner) => {
                                    Self::nullable(Held::Written(*inner))
                                }
                                _ => Held::Written(ty),
                            })
                            .collect(),
                    )
                })
            }
            Held::Class(_)
            | Held::Builtin(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Callable(_) => None,
            Held::OneOf(_) | Held::AllOf(_) | Held::Applied { .. } => None,
        }
    }

    /// What a written type names, as something two of them can be compared by.
    fn named_or_builtin(
        &self,
        offset: TextSize,
        hir: &FileHir,
        id: TypeRefId,
    ) -> Option<Result<Name, BuiltinTy>> {
        match hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => Some(Err(*builtin)),
            _ => self.class_of_type_ref(offset, hir, id).map(Ok),
        }
    }

    /// What one key of a shape holds, when the value is a shape and names that key.
    ///
    /// `array{app: Application, count: int}` indexed by `app` is an `Application`, and by `count` an
    /// `int` -- which is the whole difference from `array<string, mixed>`, where every key holds the
    /// same thing and no single element type could say both.
    ///
    /// An *optional* key still holds its type: `id?: int` says the key may be absent, and PHP's answer
    /// for reading an absent key is a warning and null. Saying `?int` here would be modelling that,
    /// and modelling it as the same thing as `id: ?int` -- which it is not. Left as written until
    /// something is prepared to report the difference.
    ///
    /// `None` when the value is not a shape, or is one and does not name the key. The second is the
    /// honest answer for `$row['typo']`, and the fact a report could be built on later.
    pub(crate) fn keyed_of_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        key: &str,
    ) -> Option<Held> {
        // Read before the match, for the reason `element_of_held` reads it before its own: a union
        // arrives in three spellings and only one of them is a `Held::OneOf`. A `@return
        // array{...}|array{}` in another file is *interned* and a `@param A|B` in this one is
        // *written*, and those two are how nearly every union in real code reaches here -- so the
        // arm below spoke only for the spelling a docblock never produces. `union_operands` is what
        // already knows all three, and it is the same line `element_of_held` opens with.
        if let Some(operands) = self.union_operands(offset, hir, held) {
            return self.keyed_of_union(offset, hir, &operands, key);
        }

        match held {
            Held::Object(_) => None,
            Held::Nullable(inner) => self.keyed_of_held(offset, hir, inner, key),
            // Exactly as a stored shape answers, and a miss stays a miss for the same reason:
            // the literal is the whole statement of what is in there.
            Held::Record(fields) => fields
                .iter()
                .find(|(named, _)| named.as_str() == key)
                .map(|(_, value)| value.clone()),
            Held::Written(id) => Self::keyed_of_type_ref(hir, *id, key).map(Held::Written),
            Held::Stored { ty, scope } => {
                self.skeleton
                    .types()
                    .shape_field(*ty, key)
                    .map(|(ty, _optional)| Held::Stored {
                        ty,
                        // The scope carries over, as it does for an element: `array{one: static}` keyed
                        // still means the class it meant before the key was taken.
                        scope: scope.clone(),
                    })
            }
            // A key of an intersection is a key of whichever operand has one, for the reason its
            // element is.
            Held::AllOf(operands) => operands
                .iter()
                .find_map(|operand| self.keyed_of_held(offset, hir, operand, key)),
            // Answered by `union_operands` above, which recognises this spelling along with the
            // two written ones.
            Held::OneOf(_) => None,
            Held::Class(_)
            | Held::Builtin(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Callable(_)
            | Held::Applied { .. } => None,
        }
    }

    /// The key of a union: the key of each operand, as one value.
    ///
    /// The sentinels are looked through exactly as the `Nullable` arm looks through a null.
    /// `reset()` answers `array{date: string}|false` for every array PHP cannot prove non-empty,
    /// and a `false` has no keys by construction: requiring it to name one is requiring the
    /// impossible, and the branch where it arrives is what `possibly-false-return` and its siblings
    /// are for rather than something a key lookup reports by answering nothing.
    ///
    /// An **empty shape** answers `null`, and every other miss ends the read. `array{}` names no
    /// keys because there are none, and PHP reads any key off it as null with a warning -- so
    /// `@return array{type: string, ...}|array{}`, which is how a project writes "the whole thing,
    /// or nothing", makes `$address['city']` a `?string`. A *populated* shape that does not name
    /// the key may simply not list it, which is the reading a lone shape already gets, so it stays
    /// a miss and takes the union with it.
    fn keyed_of_union(
        &self,
        offset: TextSize,
        hir: &FileHir,
        operands: &[crate::resolve::UnionOperand],
        key: &str,
    ) -> Option<Held> {
        let mut keyed = Vec::with_capacity(operands.len());
        for operand in operands {
            if Self::names_a_sentinel(&operand.held) {
                continue;
            }
            match self.keyed_of_held(offset, hir, &operand.held, key) {
                Some(found) => keyed.push(found),
                None if self.is_empty_shape(hir, &operand.held) => {
                    keyed.push(Held::Builtin(phrost_model::BuiltinTy::Null));
                }
                None => return None,
            }
        }
        // Every operand was a sentinel, so there was nothing to take a key of.
        (!keyed.is_empty()).then(|| Self::one_of(keyed))
    }

    /// Whether a value is a shape that states it holds nothing.
    ///
    /// `array{}` and nothing else. A shape with fields is a claim about the keys it lists and says
    /// nothing about the ones it does not; a shape with none has nothing left to be incomplete
    /// about, which is what makes it the one spelling a key read can be answered from.
    ///
    /// Written and stored, because a union's operands arrive in both spellings -- a `@return` in
    /// another file is interned and a `@param` in this one is a reference into its arena.
    pub(crate) fn is_empty_shape(&self, hir: &FileHir, held: &Held) -> bool {
        match held {
            Held::Nullable(inner) => self.is_empty_shape(hir, inner),
            Held::Written(id) => matches!(
                hir.type_ref(*id),
                TypeRefData::Shape { fields, .. } if hir.type_ref_list(*fields).is_empty()
            ),
            Held::Stored { ty, .. } => matches!(
                self.skeleton.types().get(*ty),
                phrost_ty::TypeRef::Shape { fields, .. } if self.skeleton.types().operands(*fields).is_empty()
            ),
            // Never empty by construction -- see `Held::Record`.
            _ => false,
        }
    }

    /// Whether an operand is one of the two values PHP hands back to mean *there is nothing here*.
    ///
    /// Both, because a project meets both: `null` from a lookup that missed and `false` from the
    /// standard library, which §8 records as one absence split across two spellings.
    fn names_a_sentinel(held: &Held) -> bool {
        matches!(
            held,
            Held::Builtin(phrost_model::BuiltinTy::Null | phrost_model::BuiltinTy::False)
        )
    }

    /// The same, for a type as written.
    fn keyed_of_type_ref(hir: &FileHir, id: TypeRefId, key: &str) -> Option<TypeRefId> {
        match hir.type_ref(id) {
            TypeRefData::Nullable(inner) => Self::keyed_of_type_ref(hir, *inner, key),
            TypeRefData::Shape { fields, .. } => {
                hir.type_ref_list(*fields).iter().find_map(|field| {
                    match hir.type_ref(*field) {
                        // Exactly: a PHP array key is case-sensitive, so `$row['ID']` is a different
                        // key from `$row['id']`.
                        TypeRefData::ShapeField { key: named, ty, .. } if named.as_str() == key => {
                            Some(*ty)
                        }
                        _ => None,
                    }
                })
            }
            _ => None,
        }
    }
}
