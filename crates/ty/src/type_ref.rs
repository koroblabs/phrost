use phrost_model::{BuiltinTy, Name, RelativeTy};

use crate::table::{TypeId, TypeList};

/// One node of a resolved type expression.
///
/// The same shape as the written form in the HIR, with two differences that are the whole point:
/// a class name is a resolved fully-qualified [`Name`] rather than a per-file reference, and every
/// child is a [`TypeId`] into a shared table rather than a per-file arena index. Both are what let
/// a type outlive the file it was written in.
///
/// `Nullable` is kept as a wrapper rather than desugared into `T|null` so that what is shown to a
/// person still matches what they wrote.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TypeRef {
    Builtin(BuiltinTy),
    /// `self`, `static`, `parent` — left unresolved on purpose, because they mean different classes
    /// depending on the receiver a call is made through.
    Relative(RelativeTy),
    /// A class-like name, fully qualified and without a leading separator.
    Named(Name),
    /// `?T`.
    Nullable(TypeId),
    /// `A|B|C`.
    Union(TypeList),
    /// `A&B`.
    Intersection(TypeList),
    /// A type with arguments: `list<User>`, `array<int, User>`, `Collection<User>`.
    ///
    /// Written only in docblocks, and kept because the two halves answer two questions: the base is
    /// what the value is, and the last argument is what iterating it yields.
    Generic {
        base: TypeId,
        args: TypeList,
        /// `non-empty-array<K, V>` and `non-empty-list<V>`. See the HIR's own `Generic::non_empty`
        /// for why it is a flag rather than a base.
        non_empty: bool,
    },
    /// `array{id: int, name: string}` -- an array whose keys are known individually.
    ///
    /// Not a `Generic` over one element type, because that is the whole difference: `array<string,
    /// mixed>` says every key holds the same thing and this says which key holds what. The operands
    /// are [`Self::Field`]s and nothing else.
    Shape {
        fields: TypeList,
        /// The array may hold keys the shape does not name -- `array{id: int, ...}`, or a shape
        /// some of whose written keys could not be lowered. A sealed shape names every key there is.
        open: bool,
        /// Written `list{...}`: its keys are `0` upwards in order, so a value keyed otherwise -- the
        /// same keys out of order included -- is not one.
        list: bool,
    },
    /// `object{pivot: Pivot}` -- an object whose named properties are known. See the HIR's own
    /// `ObjectShape` for why it is not a flag on [`Self::Shape`]. The operands are [`Self::Field`]s.
    ObjectShape {
        fields: TypeList,
    },
    /// One field of a [`Self::Shape`]: `id: int`, or `id?: int` when the key may be absent.
    ///
    /// A node rather than a side table, so a shape's fields are interned, hashed and compared by the
    /// same machinery every other operand list uses -- and cost the same. It never appears outside a
    /// shape's operands, which is why it names no type on its own.
    Field {
        key: Name,
        optional: bool,
        ty: TypeId,
    },
}
