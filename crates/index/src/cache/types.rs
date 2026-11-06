//! The project's type table, written and read back.
//!
//! Ids are indices, and every member row stored beside the table holds them -- so the nodes go out
//! in id order and come back the same way. Nothing else about the table is stored: its lookup maps
//! are the same information a second time, and they are rebuilt on load.

use phrost_model::{BuiltinTy, Interner, RelativeTy};
use phrost_ty::{TypeId, TypeList, TypeRef, TypeTable};

use super::bytes::{Reader, Writer};

/// Tags for the node kinds, so a decoder never has to guess.
const BUILTIN: u8 = 0;
const RELATIVE: u8 = 1;
const NAMED: u8 = 2;
const NULLABLE: u8 = 3;
const UNION: u8 = 4;
const INTERSECTION: u8 = 5;
const GENERIC: u8 = 6;
const SHAPE: u8 = 7;
const FIELD: u8 = 8;
const OBJECT_SHAPE: u8 = 9;

pub(super) fn write(out: &mut Writer<'_>, types: &TypeTable) {
    out.list(types.nodes(), |out, node| match node {
        TypeRef::Builtin(builtin) => {
            out.u8(BUILTIN);
            out.u8(builtin_tag(*builtin));
        }
        TypeRef::Relative(relative) => {
            out.u8(RELATIVE);
            out.u8(relative_tag(*relative));
        }
        TypeRef::Named(name) => {
            out.u8(NAMED);
            out.str(name.as_str());
        }
        TypeRef::Nullable(inner) => {
            out.u8(NULLABLE);
            out.u32(inner.as_raw());
        }
        TypeRef::Union(list) => {
            out.u8(UNION);
            write_list(out, *list);
        }
        TypeRef::Intersection(list) => {
            out.u8(INTERSECTION);
            write_list(out, *list);
        }
        TypeRef::Generic {
            base,
            args,
            non_empty,
        } => {
            out.u8(GENERIC);
            out.u32(base.as_raw());
            write_list(out, *args);
            out.u8(u8::from(*non_empty));
        }
        TypeRef::Shape { fields, open, list } => {
            out.u8(SHAPE);
            write_list(out, *fields);
            out.u8(u8::from(*open) | (u8::from(*list) << 1));
        }
        TypeRef::ObjectShape { fields } => {
            out.u8(OBJECT_SHAPE);
            write_list(out, *fields);
        }
        TypeRef::Field { key, optional, ty } => {
            out.u8(FIELD);
            out.str(key.as_str());
            out.u8(u8::from(*optional));
            out.u32(ty.as_raw());
        }
    });
    out.list(types.list_items(), |out, id| out.u32(id.as_raw()));
}

pub(super) fn read(input: &mut Reader<'_>, interner: &mut Interner) -> Option<TypeTable> {
    let nodes = input.list(|input| {
        Some(match input.u8()? {
            BUILTIN => TypeRef::Builtin(builtin_of(input.u8()?)?),
            RELATIVE => TypeRef::Relative(relative_of(input.u8()?)?),
            // Interned into the project's own interner, which is what makes one name one
            // allocation and lets a restored name compare equal to a freshly built one.
            NAMED => TypeRef::Named(interner.name(input.str()?)),
            NULLABLE => TypeRef::Nullable(TypeId::from_raw(input.u32()?)),
            UNION => TypeRef::Union(read_list(input)?),
            INTERSECTION => TypeRef::Intersection(read_list(input)?),
            GENERIC => TypeRef::Generic {
                base: TypeId::from_raw(input.u32()?),
                args: read_list(input)?,
                non_empty: input.u8()? != 0,
            },
            SHAPE => {
                let fields = read_list(input)?;
                let flags = input.u8()?;
                TypeRef::Shape {
                    fields,
                    open: flags & 1 != 0,
                    list: flags & 2 != 0,
                }
            }
            OBJECT_SHAPE => TypeRef::ObjectShape {
                fields: read_list(input)?,
            },
            FIELD => TypeRef::Field {
                key: interner.name(input.str()?),
                optional: input.u8()? != 0,
                ty: TypeId::from_raw(input.u32()?),
            },
            _ => return None,
        })
    })?;
    let list_items = input.list(|input| Some(TypeId::from_raw(input.u32()?)))?;

    Some(TypeTable::from_parts(nodes, list_items))
}

fn write_list(out: &mut Writer<'_>, list: TypeList) {
    let (start, len) = list.parts();
    out.u32(start);
    out.u32(len);
}

fn read_list(input: &mut Reader<'_>) -> Option<TypeList> {
    Some(TypeList::from_parts(input.u32()?, input.u32()?))
}

/// Tags rather than the discriminant: a reordered enum must not change what an old file means.
///
/// The one copy, which every row that stores a builtin writes through. §8 records that adding a
/// `BuiltinTy` variant compiles clean and changes behaviour silently; a second table here was the
/// same hazard in the codec, two lists that had to learn each variant separately.
pub(super) fn builtin_tag(builtin: BuiltinTy) -> u8 {
    match builtin {
        BuiltinTy::Int => 0,
        BuiltinTy::Float => 1,
        BuiltinTy::String => 2,
        BuiltinTy::Bool => 3,
        BuiltinTy::Array => 4,
        BuiltinTy::Object => 5,
        BuiltinTy::Callable => 6,
        BuiltinTy::Iterable => 7,
        BuiltinTy::Mixed => 8,
        BuiltinTy::Void => 9,
        BuiltinTy::Never => 10,
        BuiltinTy::Null => 11,
        BuiltinTy::True => 12,
        BuiltinTy::False => 13,
        BuiltinTy::Resource => 14,
        BuiltinTy::NonEmptyString => 15,
        BuiltinTy::NonFalsyString => 16,
    }
}

pub(super) fn builtin_of(tag: u8) -> Option<BuiltinTy> {
    Some(match tag {
        0 => BuiltinTy::Int,
        1 => BuiltinTy::Float,
        2 => BuiltinTy::String,
        3 => BuiltinTy::Bool,
        4 => BuiltinTy::Array,
        5 => BuiltinTy::Object,
        6 => BuiltinTy::Callable,
        7 => BuiltinTy::Iterable,
        8 => BuiltinTy::Mixed,
        9 => BuiltinTy::Void,
        10 => BuiltinTy::Never,
        11 => BuiltinTy::Null,
        12 => BuiltinTy::True,
        13 => BuiltinTy::False,
        14 => BuiltinTy::Resource,
        15 => BuiltinTy::NonEmptyString,
        16 => BuiltinTy::NonFalsyString,
        _ => return None,
    })
}

pub(super) fn absence_tag(absence: phrost_model::Absence) -> u8 {
    match absence {
        phrost_model::Absence::Null => 0,
        phrost_model::Absence::False => 1,
        phrost_model::Absence::NullOrFalse => 2,
    }
}

pub(super) fn absence_of(tag: u8) -> Option<phrost_model::Absence> {
    Some(match tag {
        0 => phrost_model::Absence::Null,
        1 => phrost_model::Absence::False,
        2 => phrost_model::Absence::NullOrFalse,
        _ => return None,
    })
}

fn relative_tag(relative: RelativeTy) -> u8 {
    match relative {
        RelativeTy::SelfTy => 0,
        RelativeTy::Static => 1,
        RelativeTy::Parent => 2,
    }
}

fn relative_of(tag: u8) -> Option<RelativeTy> {
    Some(match tag {
        0 => RelativeTy::SelfTy,
        1 => RelativeTy::Static,
        2 => RelativeTy::Parent,
        _ => return None,
    })
}
