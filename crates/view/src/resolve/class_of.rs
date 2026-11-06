//! Which class a type names -- and what several of them reduce to.
//!
//! One question over three spellings of a type (held, written, interned) and three compositions of
//! it (one class, a union, an intersection). They are one file because the reductions are the
//! answer to the question: a union names the class its operands share, an intersection names none
//! and hands back the operands instead, and a caller that wanted a single class has to be told
//! which of the two it got. Splitting them would put the question in one file and half its answer
//! in another.

use phrost_hir::{FileHir, TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, Name, RelativeTy, TextSize};

use crate::{
    resolve::{Held, RelativeScope},
    view::IndexView,
};

impl IndexView<'_> {
    /// The class a held type names, if it names exactly one.
    pub(crate) fn class_of_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Name> {
        match held {
            Held::Object(_) => None,
            Held::Class(name) => Some(name.clone()),
            Held::Builtin(_) => None,
            // `?Foo` still describes Foo. Whether the value is null is a diagnostic's question; the
            // members being asked about are the same either way.
            Held::Nullable(inner) => self.class_of_held(offset, hir, inner),
            // An array has no members, which is the right answer for `...$items` and a different
            // answer from "not known".
            Held::ListOf(_) | Held::MapOf { .. } => None,
            Held::Record(_) => None,
            // A callable *value* is a `Closure`, and PHP's own class of that name is where its
            // members live: `bindTo`, `call`, `fromCallable`, `__invoke`.
            //
            // Only this arm, which is the whole of why it is safe. It is built in exactly two
            // places -- a closure literal and a first-class callable -- and PHP makes a `Closure`
            // out of both. A value written as a plain `callable` may be a string or a two-element
            // array instead, and it never reaches here: such a declaration arrives as a `Written`
            // or `Stored` type whose builtin names no class, which is the answer it should have.
            Held::Callable(_) => self.class_location("Closure").map(|found| found.fqn),
            Held::Written(id) => self.class_of_type_ref(offset, hir, *id),
            Held::Stored { ty, scope } => self.class_of_stored(*ty, scope.as_ref()),
            // One of several names one class only when they share an ancestor, which is the same
            // reduction a stored union gets and for the same reason -- sentinels dropped along with
            // it, since `one_of` lifts a `null` out into a `Nullable` and leaves a `false` here.
            Held::OneOf(operands) => {
                let named = operands
                    .iter()
                    .filter(|operand| {
                        !matches!(operand, Held::Builtin(builtin) if names_no_class(*builtin))
                    })
                    .map(|operand| self.class_of_held(offset, hir, operand))
                    .collect::<Option<Vec<_>>>()?;
                self.common_ancestor(&named)
            }
            // All of several names no single class, which is the point: a caller that wants members
            // asks `intersection_of_held` and gets every operand's table. Naming one would hide the
            // rest -- unless the rest are object shapes, which have no table to hide:
            // `Vault&object{pivot: Pivot}` is a `Vault` whose `pivot` is known.
            Held::AllOf(operands) => {
                let mut classes = operands
                    .iter()
                    .filter(|operand| !self.is_object_shape(operand));
                let one = classes.next()?;
                if classes.next().is_some() {
                    return None;
                }
                self.class_of_held(offset, hir, one)
            }
            // The class it is an instance of. The arguments say what its templates are, not what it
            // is, so every caller asking for a class gets the same answer it always did.
            Held::Applied { class, .. } => Some(class.clone()),
        }
    }

    /// Whether a held operand is an object shape -- or the bare `object` one leaves when none of
    /// its properties could be placed -- and so adds no class to an intersection.
    fn is_object_shape(&self, held: &Held) -> bool {
        match held {
            Held::Object(_) | Held::Builtin(BuiltinTy::Object) => true,
            Held::Stored { ty, .. } => matches!(
                self.skeleton.types().get(*ty),
                phrost_ty::TypeRef::ObjectShape { .. }
                    | phrost_ty::TypeRef::Builtin(BuiltinTy::Object)
            ),
            _ => false,
        }
    }

    /// The class a member named on this value is on.
    ///
    /// The single class first, so an ordinary receiver costs what it did. An intersection has no
    /// single class, so the operand that declares this member is the answer -- which is why the
    /// member's name has to be passed in: `Application&MockObject` owns `expects()` on one operand
    /// and `getId()` on the other, and there is no answer that does not depend on which is asked
    /// about.
    pub(crate) fn owner_of_member(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        kind: phrost_sema::MemberKind,
        name: &str,
    ) -> Option<Name> {
        // A union before anything else, and only when one of its operands declares the member.
        // Reduced to a common ancestor first, a union answers about a class the source never wrote:
        // `Uc|Ud` extending one base reported `Undefined method UBase::cOnly()` for a call `Uc`
        // declares -- a false positive about a call that may well run -- and goto was dead at the
        // same site, because the ancestor has no such member to go to.
        if let Some(class) = self.declaring_operand(offset, hir, held, kind, name) {
            return Some(class);
        }
        if let Some(class) = self.class_of_held(offset, hir, held) {
            return Some(class);
        }
        if let Some(class) = self
            .intersection_of_held(offset, hir, held)
            .into_iter()
            .find(|class| self.members(class.as_str()).find(kind, name).is_some())
        {
            return Some(class);
        }
        // Every operand of a union having the member is the one case left, and it is what names a
        // class when *none* of them declares it: `catch (\Exception | GuzzleException $e)` both
        // declare `getMessage`, so either is a true answer and the first is the stable one.
        let alternatives = self.union_of_held(offset, hir, held);
        let first = alternatives.first()?;
        alternatives
            .iter()
            .all(|class| self.members(class.as_str()).find(kind, name).is_some())
            .then(|| first.clone())
    }

    /// The class whose declaration of a member a call on this value has to satisfy: the value's own
    /// class, or the operand of an intersection that declares it.
    ///
    /// [`Self::owner_of_member`] without its two union rungs. Which operand of a union *answers* a
    /// name is the right question for goto and for the type a call hands back, and the wrong one for
    /// a signature: `UuidFactory|RandomBasedUuidFactory` is one of the two, and checking
    /// `->create($name)` against the operand whose `create()` takes nothing reports a call the other
    /// accepts. An intersection is all of its operands at once, so whichever declares the member
    /// declares it for the value -- `$this` in a trait that names what composes it is one.
    pub(crate) fn declaring_class(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        kind: phrost_sema::MemberKind,
        name: &str,
    ) -> Option<Name> {
        if let Some(class) = self.class_of_held(offset, hir, held) {
            return Some(class);
        }
        self.intersection_of_held(offset, hir, held)
            .into_iter()
            .find(|class| self.members(class.as_str()).find(kind, name).is_some())
    }

    /// The operand of a union that declares this member.
    ///
    /// `None` for anything that is not a union of classes, which is what makes this affordable on
    /// the path every member access takes: `union_operands` answers nothing for an ordinary receiver
    /// and the walk stops there.
    ///
    /// The *first* operand that declares it, when several do. Two operands of one union declaring
    /// one name is ordinary -- an interface and a class implementing it, two siblings inheriting from
    /// one base -- and `member_location` resolves each of them to the declaration itself, so which
    /// is named changes where goto lands only when the two really are different members.
    ///
    /// An operand that is an intersection names no one class, and its parts are asked instead:
    /// `MockObject&PriceListProduct|MockObject&Product` is what an `if` choosing which class to mock
    /// leaves behind, and `->method()` is declared by the `MockObject` both halves carry.
    fn declaring_operand(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
        kind: phrost_sema::MemberKind,
        name: &str,
    ) -> Option<Name> {
        let operands = self.union_operands(offset, hir, held)?;
        let declares = |class: &Name| self.members(class.as_str()).find(kind, name).is_some();
        operands
            .into_iter()
            .find_map(|operand| match operand.class {
                Some(class) => declares(&class).then_some(class),
                None => self
                    .intersection_of_held(offset, hir, &operand.held)
                    .into_iter()
                    .find(declares),
            })
    }

    /// The classes an intersection names, when the held type is one.
    ///
    /// Empty for everything else, which is what makes this free: it is asked only where
    /// [`Self::class_of_held`] has already answered nothing, and that was a dead end before this
    /// existed. So no member access that resolves pays for it.
    ///
    /// `A&B` says the value is all of them at once, so it has both APIs and reduces to no single
    /// class -- unlike `A|B`, which reduces to what they share. Every PHPUnit mock in a modern test
    /// is written this way: `private Application&MockObject $application` needs `expects()` from one
    /// operand and `getId()` from the other, and picking either loses the whole of the other.
    pub(crate) fn intersection_of_held(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Vec<Name> {
        match held {
            Held::Object(_) => Vec::new(),
            Held::Nullable(inner) => self.intersection_of_held(offset, hir, inner),
            Held::Written(id) => match hir.type_ref(*id) {
                TypeRefData::Intersection(parts) => hir
                    .type_ref_list(*parts)
                    .to_vec()
                    .iter()
                    .filter_map(|part| self.class_of_type_ref(offset, hir, *part))
                    .collect(),
                _ => Vec::new(),
            },
            Held::Stored { ty, scope } => self.stored_intersection(*ty, scope.as_ref()),
            Held::Class(_)
            | Held::Builtin(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Record(_)
            | Held::Callable(_) => Vec::new(),
            // A union is the opposite composition; `union_of_held` is what reads it.
            Held::OneOf(_) => Vec::new(),
            // Already an intersection, so this is the whole of what it reads: each operand's own
            // class, which is what makes a member of any of them reachable -- and each class of an
            // operand that is a union, which is how `$this` in a trait composed into several
            // classes is held: the trait, and one of its hosts.
            Held::AllOf(operands) => operands
                .iter()
                .flat_map(|operand| match self.class_of_held(offset, hir, operand) {
                    Some(class) => vec![class],
                    None => self.union_of_held(offset, hir, operand),
                })
                .collect(),
            // One class, so `class_of_held` is what answers it.
            Held::Applied { .. } => Vec::new(),
        }
    }

    /// Whether two held types name the same single class.
    ///
    /// Asked where two operands of one expression have to be told apart. Only a *single* class
    /// counts: a union or an intersection names none, and two of those are not the same thing just
    /// because neither reduces.
    pub(crate) fn name_one_class(
        &self,
        offset: TextSize,
        hir: &FileHir,
        left: &Held,
        right: &Held,
    ) -> bool {
        match (
            self.class_of_held(offset, hir, left),
            self.class_of_held(offset, hir, right),
        ) {
            (Some(one), Some(other)) => one == other,
            _ => false,
        }
    }

    /// The classes a value is *one of*, when its type is a union of class names.
    ///
    /// The mirror of [`Self::intersection_of_held`] and the opposite reduction: an intersection is
    /// every operand at once, so everything any of them declares can be reached, while a union is
    /// one of them and only what all of them declare can.
    ///
    /// Empty unless every operand names a class. `A|string` reached with `->` is a call on a value
    /// that may be a string, and a list built from `A` alone would say that call is fine.
    ///
    /// Taken apart by [`Self::union_operands`] rather than by a second match on the three spellings
    /// a union arrives in -- which is what this used to be, and it recognised two of them.
    pub(crate) fn union_of_held(&self, offset: TextSize, hir: &FileHir, held: &Held) -> Vec<Name> {
        let Some(operands) = self.union_operands(offset, hir, held) else {
            return Vec::new();
        };
        operands
            .into_iter()
            .map(|operand| operand.class)
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// The same, for an interned type.
    pub(crate) fn stored_intersection(
        &self,
        id: phrost_ty::TypeId,
        scope: Option<&RelativeScope>,
    ) -> Vec<Name> {
        match self.skeleton.types().get(id) {
            phrost_ty::TypeRef::Nullable(inner) => self.stored_intersection(*inner, scope),
            phrost_ty::TypeRef::Intersection(parts) => self
                .skeleton
                .types()
                .operands(*parts)
                .to_vec()
                .iter()
                .filter_map(|part| self.class_of_stored(*part, scope))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn class_of_stored(
        &self,
        id: phrost_ty::TypeId,
        scope: Option<&RelativeScope>,
    ) -> Option<Name> {
        match self.skeleton.types().get(id) {
            phrost_ty::TypeRef::Named(fqn) => Some(fqn.clone()),
            phrost_ty::TypeRef::Nullable(inner) => self.class_of_stored(*inner, scope),
            // `Collection<User>` has members; `array<User>` does not. Either way the base is what
            // the value *is*, and the arguments are what it holds.
            phrost_ty::TypeRef::Generic { base, .. } => self.class_of_stored(*base, scope),
            // The same reduction as a written union, with the sentinels dropped for the reason
            // [`names_no_class`] gives. Interning lifts a `null` out into a `Nullable`, so the one
            // that actually turns up here is `false`.
            phrost_ty::TypeRef::Union(parts) => {
                let operands = self
                    .skeleton
                    .types()
                    .operands(*parts)
                    .to_vec()
                    .iter()
                    .filter(|part| {
                        !matches!(
                            self.skeleton.types().get(**part),
                            phrost_ty::TypeRef::Builtin(builtin) if names_no_class(*builtin)
                        )
                    })
                    .map(|part| self.class_of_stored(*part, scope))
                    .collect::<Option<Vec<_>>>()?;
                self.common_ancestor(&operands)
            }
            phrost_ty::TypeRef::Relative(relative) => self.class_of_relative(*relative, scope?),
            // The same reading as a built intersection: one class beside object shapes is that
            // class. See `class_of_held`.
            phrost_ty::TypeRef::Intersection(parts) => {
                let types = self.skeleton.types();
                let mut classes = types.operands(*parts).iter().filter(|&&part| {
                    !matches!(
                        types.get(part),
                        phrost_ty::TypeRef::ObjectShape { .. }
                            | phrost_ty::TypeRef::Builtin(BuiltinTy::Object)
                    )
                });
                let one = *classes.next()?;
                if classes.next().is_some() {
                    return None;
                }
                self.class_of_stored(one, scope)
            }
            _ => None,
        }
    }

    /// What one of PHP's three relative words means for a value read through this scope.
    ///
    /// Its own method because two readers need the same rule and neither owns it: resolving a
    /// stored type to a class, and *rendering* one for a person -- where a surviving `static` is
    /// never a right answer to "what is this". Written twice, the two would drift.
    pub(crate) fn class_of_relative(
        &self,
        relative: RelativeTy,
        scope: &RelativeScope,
    ) -> Option<Name> {
        match relative {
            RelativeTy::Static => Some(scope.called.clone()),
            RelativeTy::SelfTy => Some(scope.declaring.clone()),
            // The parent of whoever wrote the declaration, which is what `parent` means inside it --
            // not the parent of whatever class the call was made on.
            RelativeTy::Parent => self
                .class_header(scope.declaring.as_str())
                .and_then(|header| header.extends.first().cloned()),
        }
    }

    /// The classes a written union names, with the sentinels dropped -- they name none.
    fn written_union_operands(
        &self,
        offset: TextSize,
        hir: &FileHir,
        parts: &[TypeRefId],
    ) -> Vec<Name> {
        parts
            .iter()
            .filter(|part| {
                !matches!(hir.type_ref(**part), TypeRefData::Builtin(builtin) if names_no_class(*builtin))
            })
            .map(|part| self.class_of_type_ref(offset, hir, *part))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// The class a `class-string<T>` type names -- not what a value of the type would be an
    /// instance of, which [`Self::class_of_type_ref`] already answers correctly as `None`: a
    /// class-string is a string, not a `T`.
    ///
    /// `new $var` asks this instead, when `$var`'s type came from an annotation rather than from a
    /// `Foo::class` literal in the value's own path -- see `class_a_path_names` in
    /// `resolve::target`, the one caller. Lowering keeps a `class-string<T>` as
    /// [`TypeRefData::Generic`] with a `string` base for exactly this: the base is what
    /// [`Self::class_of_type_ref`] and every compat check read, and the argument is what this
    /// reads instead.
    pub(crate) fn class_named_by_class_string(
        &self,
        offset: TextSize,
        hir: &FileHir,
        id: TypeRefId,
    ) -> Option<Name> {
        match hir.type_ref(id) {
            // `?class-string<T>` still names `T`; whether the value might be null is the question
            // [`Self::class_of_type_ref`] answers for `?Foo`, and not this one.
            TypeRefData::Nullable(inner) => self.class_named_by_class_string(offset, hir, *inner),
            // The one generic a docblock ever builds with a plain `string` base: `array<T>` and
            // `Collection<T>` are written with their own base, never with `string`. That is
            // exactly what tells this apart from every other one-argument generic without a flag
            // of its own.
            TypeRefData::Generic { base, args, .. }
                if matches!(hir.type_ref(*base), TypeRefData::Builtin(BuiltinTy::String)) =>
            {
                match hir.type_ref_list(*args) {
                    [argument] => self.class_of_type_ref(offset, hir, *argument),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The class a written type names, if it names exactly one.
    pub(crate) fn class_of_type_ref(
        &self,
        offset: TextSize,
        hir: &FileHir,
        id: TypeRefId,
    ) -> Option<Name> {
        match hir.type_ref(id) {
            TypeRefData::Named(reference) => {
                let reference = hir.name_ref(*reference);
                self.resolve_written_name(offset, reference.path.as_str(), reference.anchor)
                    .map(|location| location.fqn)
            }
            // `?Foo` still describes Foo. Whether the value is null is a diagnostic's question; the
            // members being asked about are the same either way.
            TypeRefData::Nullable(inner) => self.class_of_type_ref(offset, hir, *inner),
            TypeRefData::Relative(relative) => match relative {
                RelativeTy::SelfTy | RelativeTy::Static => self.enclosing_class(offset),
                RelativeTy::Parent => self.parent_of_enclosing_class(offset),
            },
            TypeRefData::Generic { base, .. } => self.class_of_type_ref(offset, hir, *base),
            // `Foo|null` is `?Foo` written the other way round, and PHPDoc writes it that way far
            // more often. Seeing through it is not the guess below: `null` names no class, so a
            // union whose other members are all `null` still names exactly one.
            //
            // Done here rather than by normalising the tree, because the tree is what the author
            // wrote and hover and rename read it back.
            TypeRefData::Union(parts) => {
                let parts = hir.type_ref_list(*parts).to_vec();
                let operands = self.written_union_operands(offset, hir, &parts);
                self.common_ancestor(&operands)
            }
            // A scalar has no members. An intersection names several classes at once, and the value
            // has the members of all of them -- so unlike a union it does not reduce to one class,
            // and picking an operand would lose the other's whole API. `Application&MockObject` in a
            // test needs both, which is a merged member table and a later slice.
            TypeRefData::Builtin(_) | TypeRefData::Intersection(_) => None,
            // A shape is an array. It has keys and no members, which is the same answer `array`
            // gets and a different answer from "not known".
            TypeRefData::Shape { .. } | TypeRefData::ShapeField { .. } => None,
            // An object of no class in particular: the properties it names are read off the shape
            // itself, and there is no member table to look them up in.
            TypeRefData::ObjectShape { .. } => None,
        }
    }
}

/// Whether a union operand is a sentinel standing for the value not being there.
///
/// Dropped rather than declining the union, because "which class is this" and "might the value be
/// missing" are two questions and only the second is about the sentinel -- the same split the
/// `?Foo` arms above already make. `Row|false` names `Row` exactly as `?Row` does, and whether the
/// `false` can arrive is the reach check's to answer.
///
/// One rule for both spellings. The written union and the interned one each had their own list, and
/// each list held `null` alone -- so when `reset`'s `V|false` stopped being folded into a `?V` and
/// arrived as the union it always was, both stopped naming a class and every member on the result of
/// an array-pointer call went unresolved.
fn names_no_class(builtin: BuiltinTy) -> bool {
    matches!(builtin, BuiltinTy::Null | BuiltinTy::False)
}
