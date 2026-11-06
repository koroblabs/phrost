//! What a container holds, read off its type arguments.
//!
//! Three reads of one node: the last argument is what iterating yields, the first is what names
//! each slot, and the whole list is what the value was given for its class's `@template`s. One
//! file because they are the same lookup with different projections, and because the rules that
//! decide them -- a written argument beats a declared bound, a scope travels with an element --
//! have to agree. Reading by an explicit key is [`super::indexing`]'s: it answers from a shape
//! rather than from an argument.

use phrost_hir::{FileHir, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, TextSize};
use phrost_ty::TypeId;

use crate::{
    iteration::{Bound, IteratedSlot},
    resolve::Held,
    view::IndexView,
};

impl IndexView<'_> {
    /// What iterating a held type yields, when the type says.
    /// Two sources, in order of how specific they are.
    ///
    /// A written type argument settles it outright: `array<Repo>` and `Collection<User>` say what
    /// they hold, and nothing needs to be looked up. Failing that, the class the type names may
    /// still declare what it holds, through a `@template` on it or on an interface above it -- which
    /// is the only way a PHP container can say so, and the reason `foreach` over a Drupal field list
    /// used to yield nothing. See [`Self::iterated_element`].
    ///
    /// The order matters and is not a fallback in disguise: an argument is what this value holds,
    /// a bound is what every value of this type could hold, so the argument wins wherever it exists.
    /// Whether the value is a container at all, whether or not what it holds is known.
    ///
    /// The test that tells *which argument is the container* apart from *which argument has a known
    /// element type*, and they are not the same question. `array_slice($rows, 0, 3)` is declared
    /// `array` with nothing said about its elements, so reading the second as the first walks past
    /// it -- and in `array_map($cb, array_slice($timestamps, 0, 3), [500.00, 250.00])` the next
    /// argument along is a list of floats, so the callback's first parameter answered `float`. A
    /// wrong answer where there had been no answer, which is the worse of the two.
    ///
    /// A kind of `array` or `iterable` settles it; a class needs asking, because a `Traversable`
    /// is a container and answers `object` to the kind question.
    pub(crate) fn holds_elements(&self, offset: TextSize, hir: &FileHir, held: &Held) -> bool {
        matches!(
            self.kind_of_held(hir, held),
            Some(phrost_model::BuiltinTy::Array | phrost_model::BuiltinTy::Iterable)
        ) || self.element_of_held(offset, hir, held).is_some()
    }

    /// What a container handed over whole holds: its element, or -- for a literal, which resolves
    /// as a shape -- every value the shape declares.
    ///
    /// The question a template bound by `array<K, V>` asks of an argument, which is not the keyed
    /// read [`Self::element_of_held`] answers: there a shape's missing key must stay a miss, and here
    /// nothing is read by key at all. `plain_first([$row])` and `array_first([$row])` bound nothing,
    /// because `[$row]` is a shape with a `0` in it -- while `new Coll([$row])` bound its template all
    /// along, through its own copy of this rule in the generics reader.
    pub(crate) fn value_contained(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Held> {
        self.element_of_held(offset, hir, held)
            .or_else(|| self.shape_values(offset, hir, held))
    }

    pub(crate) fn element_of_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Held> {
        // Read before the match, because a union arrives in three spellings and takes one rule.
        // Only the built one is a `Held::OneOf`; a docblock `@return A|B` is an interned union and
        // a `@param A|B` in this file is a written one, and those two are how nearly every union in
        // real code reaches here. `union_operands` is what already knows all three.
        if let Some(operands) = self.union_operands(offset, hir, held) {
            return self.element_of_union(offset, hir, &operands);
        }

        match held {
            Held::Object(_) => None,
            Held::Class(fqn) => self.iterated_element(fqn.as_str()),
            // A bare builtin says nothing, and it names no class to ask.
            Held::Builtin(_) => None,
            // A callable is not a container. What it gives back may be one, but only once it has
            // been called -- `foreach ($make as $row)` iterates the closure, not the query.
            Held::Callable(_) => None,
            // Iterating a null is not something anyone means; what the author meant to iterate is
            // what is inside.
            Held::Nullable(inner) => self.element_of_held(offset, hir, inner),
            Held::ListOf(inner) => Some((**inner).clone()),
            Held::MapOf { value, .. } => Some((**value).clone()),
            // Nothing, which is what a shape answers here and for the same reason. A record has no
            // uniform element: its slots are named, and the keyed read that misses one must stay a
            // miss rather than be answered from what the *other* keys hold. Reached exactly when
            // `indexed_of_held` has already failed to find the key, so answering the union here
            // turned `$row['nope']` into `int|DateTimeImmutable` -- a type for a slot that is not
            // there. Every slot at once is a real question and `shape_values` is where it is asked,
            // from the arm that knows the key was never written.
            Held::Record(_) => None,
            Held::Written(id)
                if let Some(stated) = self.written_iterated(offset, hir, *id, Bound::Value) =>
            {
                Some(stated)
            }
            Held::Written(id) => {
                let class = self.class_of_type_ref(offset, hir, *id);
                // The convention below is the last argument, which a class PHP does not traverse
                // gives no loop -- see `iterates_its_properties`.
                if class
                    .as_ref()
                    .is_some_and(|class| self.iterates_its_properties(class.as_str()))
                {
                    return None;
                }
                match Self::element_of_type_ref(hir, *id) {
                    Some(element) => Some(Held::Written(element)),
                    None => class.and_then(|class| self.iterated_element(class.as_str())),
                }
            }
            // The scope carries over: `list<static>` iterated yields `static`, and it means the same
            // class it meant before the element was taken.
            Held::Stored { ty, scope } => {
                match self.iterated_argument(*ty, crate::iteration::Bound::Value) {
                    Some(element) => Some(Held::Stored {
                        ty: element,
                        scope: scope.clone(),
                    }),
                    None => self
                        .class_of_stored(*ty, scope.as_ref())
                        .and_then(|class| self.iterated_element(class.as_str())),
                }
            }
            // Iterating an instance yields what it was given, which is the whole reason the argument
            // travelled this far: `new Collection(new Item())` iterated is an `Item`. At the position
            // the class's own tags name, and otherwise the last, for the reason `iterated_element`
            // takes it -- `<Key, Value>` is the only pair anybody writes.
            Held::Applied { class, arguments } => {
                if self.iterates_its_properties(class.as_str()) {
                    return None;
                }
                match self.iterated_slot(class.as_str(), Bound::Value) {
                    Some(slot) => Self::applied_slot(slot, Bound::Value, arguments),
                    None => match arguments.last() {
                        Some(last) => Some(last.clone()),
                        None => self.iterated_element(class.as_str()),
                    },
                }
            }
            // Iterating an intersection is iterating whichever operand can be: `Payment&Traversable`
            // is iterable because one half says so, and the other half saying nothing is not a
            // disagreement.
            Held::AllOf(operands) => operands
                .iter()
                .find_map(|operand| self.element_of_held(offset, hir, operand)),
            // Answered by `union_operands` above, which recognises this spelling along with the
            // two written ones.
            Held::OneOf(_) => None,
        }
    }

    /// What iterating a union yields: a union of what its operands yield.
    ///
    /// The rule turns on a distinction [`union_of_held`](Self::union_of_held) never has to make,
    /// because there both cases refuse and both refusals are right. Here they part, and which one
    /// an operand falls into is decided by whether it names a class.
    ///
    /// A `string`, a bare `array`, a `callable` -- anything this layer has described in full --
    /// that produced no element has none, and it rules the whole union out. Without that,
    /// `string|Foo[]` read by index would answer `Foo` for a value half the callers pass a string
    /// for: a wrong answer where there had been a miss.
    ///
    /// A class that produced none has said nothing either way, because the only way a PHP
    /// container can say is a `@template` its author may simply not have written. It is passed
    /// over rather than allowed to veto. `ConditionInterface[]|ConditionPluginCollection` is
    /// PHPDoc's ordinary spelling of "an iterable of T, concretely this class", and refusing there
    /// discarded the half that answered in order to honour a half that had not spoken.
    ///
    /// `None` when nothing answers, rather than an empty union: `one_of` would build a
    /// `Held::OneOf` with no operands, and every reader of one would have to learn to recognise it
    /// as "nothing".
    fn element_of_union(
        &self,
        offset: TextSize,
        hir: &FileHir,
        operands: &[crate::resolve::UnionOperand],
    ) -> Option<Held> {
        let mut elements = Vec::with_capacity(operands.len());
        for operand in operands {
            match self.element_of_held(offset, hir, &operand.held) {
                Some(element) => elements.push(element),
                None if operand.class.is_some() => {}
                // Nor does a sentinel veto: a `false` or a `null` beside a container is the call
                // saying it failed or found nothing, and what the author meant to read is the other
                // half. The `Held::Nullable` arm already answers that way for the one spelling of
                // null it sees, and `preg_split`'s `list<string>|false` is how the other spelling
                // arrives -- iterated, it typed nothing, and every call the loop variable reached
                // settled no conditional. Whether the sentinel itself arrives is the null and false
                // vocabulary's question, and `possibly-false-iteration` is where it is asked.
                None if matches!(operand.kind, Some(BuiltinTy::False | BuiltinTy::Null)) => {}
                None => return None,
            }
        }
        (!elements.is_empty()).then(|| Self::one_of(elements))
    }

    /// What iterating names each slot by: the `$k` of `foreach ($rows as $k => $row)`.
    ///
    /// PHP's rules, and they are decided by how the container was written rather than by what is in
    /// it. `array<string, T>` names its keys with strings. A shape names its keys individually, and
    /// they are all of one sort or the type is `int|string`.
    ///
    /// **A container written with one argument states no key**, and says nothing here. `list<T>` is
    /// the exception and does not reach that arm any more: lowering materialises it as
    /// `array<int, T>`, so its `int` comes from the key half being written down like any other. What
    /// is left with one argument is `T[]` and `array<T>`, and neither says anything about keys --
    /// which is the position this project already holds where keys are *compared*, and held in only
    /// one of the two readers until now.
    ///
    /// Answering `int` there was a guess, and wrong for every associative array anybody spells with
    /// brackets. PSR-7's `getHeaders(): array` is `@return string[][]` and is keyed by header name;
    /// believing the `int` made `strtolower($name)` an `incompatible-argument` **error** on three
    /// correct lines of one small project, because a file with `declare(strict_types=1)` does not
    /// coerce an int to a string and non-Drupal PHP declares it as a matter of course.
    ///
    /// Not `int|string` either, which is what the keys of a keyless array truly are: measured, that
    /// keeps all three findings and only changes their wording, because a union in an argument
    /// position is judged branch by branch. Unstated is the honest answer and the safe one -- a miss
    /// rather than an invention.
    ///
    /// A bare `array` says nothing here for the same reason.
    pub(crate) fn iteration_key_of_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Held> {
        match held {
            Held::Object(_) => None,
            Held::Nullable(inner) => self.iteration_key_of_held(offset, hir, inner),
            Held::ListOf(_) => Some(Held::Builtin(BuiltinTy::Int)),
            Held::MapOf { key, .. } => Some((**key).clone()),
            // Every key of a record is written out, so its keys are known the way a written
            // shape's are and are read by the same rule: `['one' => 1]` is keyed by a string.
            Held::Record(fields) => {
                Self::shape_key_kind(fields.iter().map(|(key, _)| key.as_str()))
            }
            Held::Written(id) => match hir.type_ref(*id) {
                TypeRefData::Nullable(inner) => {
                    self.iteration_key_of_held(offset, hir, &Held::Written(*inner))
                }
                TypeRefData::Generic { .. }
                    if let Some(key) = self.written_iterated(offset, hir, *id, Bound::Key) =>
                {
                    Some(key)
                }
                TypeRefData::Generic { args, .. } => {
                    match hir.type_ref_list(*args) {
                        // `array<K, V>`: the key half is written down.
                        [key, ..] if hir.type_ref_list(*args).len() > 1 => {
                            Some(Held::Written(*key))
                        }
                        // One argument states the element and nothing about the slot.
                        _ => None,
                    }
                }
                TypeRefData::Shape { fields, .. } => {
                    Self::shape_key_kind(hir.type_ref_list(*fields).iter().filter_map(|field| {
                        match hir.type_ref(*field) {
                            TypeRefData::ShapeField { key, .. } => Some(key.as_str()),
                            _ => None,
                        }
                    }))
                }
                // A class states its key where it states its element, and the same walk finds both.
                _ => self
                    .class_of_type_ref(offset, hir, *id)
                    .and_then(|class| self.iterated_key(class.as_str())),
            },
            Held::Stored { ty, scope } => match self.skeleton.types().get(*ty) {
                phrost_ty::TypeRef::Nullable(inner) => self.iteration_key_of_held(
                    offset,
                    hir,
                    &Held::Stored {
                        ty: *inner,
                        scope: scope.clone(),
                    },
                ),
                phrost_ty::TypeRef::Generic { .. } => self
                    .iterated_argument(*ty, crate::iteration::Bound::Key)
                    .map(|key| Held::Stored {
                        ty: key,
                        scope: scope.clone(),
                    }),
                phrost_ty::TypeRef::Shape { fields, .. } => {
                    let types = self.skeleton.types();
                    Self::shape_key_kind(types.operands(*fields).iter().filter_map(|field| {
                        match types.get(*field) {
                            phrost_ty::TypeRef::Field { key, .. } => Some(key.as_str()),
                            _ => None,
                        }
                    }))
                }
                _ => self
                    .class_of_stored(*ty, scope.as_ref())
                    .and_then(|class| self.iterated_key(class.as_str())),
            },
            Held::Class(fqn) => self.iterated_key(fqn.as_str()),
            Held::Builtin(_) | Held::Callable(_) => None,
            Held::Applied { class, arguments } => self
                .iterated_slot(class.as_str(), Bound::Key)
                .and_then(|slot| Self::applied_slot(slot, Bound::Key, arguments)),
            Held::OneOf(_) | Held::AllOf(_) => None,
        }
    }

    /// The argument a written container gives a `foreach` for `bound`, at the position its class's
    /// own tags name -- see [`iterated_template`](IndexView::iterated_template).
    fn written_iterated(
        &self,
        offset: TextSize,
        hir: &FileHir,
        id: TypeRefId,
        bound: Bound,
    ) -> Option<Held> {
        match hir.type_ref(id) {
            TypeRefData::Nullable(inner) => self.written_iterated(offset, hir, *inner, bound),
            TypeRefData::Generic { args, .. } => {
                let class = self.class_of_type_ref(offset, hir, id)?;
                match self.iterated_slot(class.as_str(), bound)? {
                    IteratedSlot::Argument(position) => {
                        let written = hir.type_ref_list(*args);
                        written
                            .get(IteratedSlot::written_position(
                                position,
                                bound,
                                written.len(),
                            )?)
                            .copied()
                            .map(Held::Written)
                    }
                    IteratedSlot::Fixed(ty) => Some(Held::Stored { ty, scope: None }),
                }
            }
            _ => None,
        }
    }

    /// What an instance's arguments give a `foreach` at the slot its class names.
    fn applied_slot(slot: IteratedSlot, bound: Bound, arguments: &[Held]) -> Option<Held> {
        match slot {
            IteratedSlot::Argument(position) => arguments
                .get(IteratedSlot::written_position(
                    position,
                    bound,
                    arguments.len(),
                )?)
                .cloned(),
            IteratedSlot::Fixed(ty) => Some(Held::Stored { ty, scope: None }),
        }
    }

    /// What sort of key a shape names its fields with, when they are all of one sort -- and for
    /// strings, what every one of them holds, which the shape spells out.
    fn shape_key_kind<'k>(mut keys: impl Iterator<Item = &'k str>) -> Option<Held> {
        let first = keys.next()?;
        let all_numeric = phrost_model::is_an_integer_key(first);
        let mut holds = BuiltinTy::string_holding(first.as_bytes());
        keys.all(|key| {
            holds = holds
                .string_join(BuiltinTy::string_holding(key.as_bytes()))
                .unwrap_or(holds);
            phrost_model::is_an_integer_key(key) == all_numeric
        })
        .then_some(match all_numeric {
            true => Held::Builtin(BuiltinTy::Int),
            false => Held::Builtin(holds),
        })
    }

    /// The last type argument of a written type, which is where every shape puts the element.
    fn element_of_type_ref(hir: &FileHir, id: TypeRefId) -> Option<TypeRefId> {
        match hir.type_ref(id) {
            TypeRefData::Generic { args, .. } => hir.type_ref_list(*args).last().copied(),
            TypeRefData::Nullable(inner) => Self::element_of_type_ref(hir, *inner),
            _ => None,
        }
    }

    /// A stored type, as a class name. `None` for a scalar, a union, or an unannotated member.
    /// With `self`/`static`/`parent` resolved against where the type was declared.
    ///
    /// `None` for the scope means the caller has no declaration context, so a relative type stays
    /// unresolved rather than being pointed at whatever class is nearby.
    /// The type arguments a value carries, as against the ones its class declares.
    ///
    /// `@param Collection<Item> $items` says what *this* container holds; the declaration's own
    /// `@extends` says what every value of the class holds, and a `@template`'s bound says only what
    /// every argument it could be given has in common. The one written at the value is the most
    /// specific of the three, and the only one that answers `$items->get()`.
    ///
    /// Empty for everything else, which is the same answer as "the declaration stands as written".
    pub(crate) fn arguments_of_held(&self, hir: &FileHir, held: &Held) -> Vec<Held> {
        match held {
            Held::Nullable(inner) => self.arguments_of_held(hir, inner),
            // Each argument kept in the world it was written in, which is the whole reason `Held` has
            // two: a parameter's `Collection<Item>` lives in this file's arena and a stored return
            // type is an id in the project's table, and neither can be turned into the other by a
            // reader.
            Held::Stored { ty, scope } => self
                .stored_arguments(*ty)
                .into_iter()
                .map(|ty| Held::Stored {
                    ty,
                    scope: scope.clone(),
                })
                .collect(),
            Held::Written(id) => Self::written_arguments(hir, *id)
                .into_iter()
                .map(Held::Written)
                .collect(),
            // Already what this answers: an instance carries its arguments outright.
            Held::Applied { arguments, .. } => arguments.to_vec(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn stored_arguments(&self, id: TypeId) -> Vec<TypeId> {
        match self.skeleton.types().get(id) {
            phrost_ty::TypeRef::Nullable(inner) => self.stored_arguments(*inner),
            phrost_ty::TypeRef::Generic { args, .. } => {
                self.skeleton.types().operands(*args).to_vec()
            }
            _ => Vec::new(),
        }
    }

    fn written_arguments(hir: &FileHir, id: TypeRefId) -> Vec<TypeRefId> {
        match hir.type_ref(id) {
            TypeRefData::Nullable(inner) => Self::written_arguments(hir, *inner),
            TypeRefData::Generic { args, .. } => hir.type_ref_list(*args).to_vec(),
            _ => Vec::new(),
        }
    }
}
