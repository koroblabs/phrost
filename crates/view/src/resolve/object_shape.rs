//! A property an `object{...}` shape names: `->pivot` on what Laravel's `BelongsToMany` loads.
//!
//! The shape is no class, so no member table holds the property; it is read off the shape itself,
//! in whichever of its spellings the value arrives -- built with its templates filled
//! ([`Held::Object`]), interned from another file's declaration, or written in this one -- and
//! through the intersection it is nearly always part of: `TRelatedModel&object{pivot: TPivotModel}`.

use phrost_hir::{FileHir, TypeRefData};

use crate::{resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What reading `name` off this value holds, where an object shape in it names the property.
    pub(crate) fn object_shape_property(
        &self,
        hir: &FileHir,
        held: &Held,
        name: &str,
    ) -> Option<Held> {
        match held {
            Held::Object(fields) => fields
                .iter()
                .find(|(field, _)| field.as_str() == name)
                .map(|(_, value)| value.clone()),
            Held::AllOf(operands) => operands
                .iter()
                .find_map(|operand| self.object_shape_property(hir, operand, name)),
            // The read itself is what a null receiver is reported against; the property holds the
            // same thing either way.
            Held::Nullable(inner) => self.object_shape_property(hir, inner, name),
            Held::Stored { ty, scope } => {
                self.stored_object_property(*ty, name)
                    .map(|ty| Held::Stored {
                        ty,
                        scope: scope.clone(),
                    })
            }
            Held::Written(id) => Self::written_object_property(hir, *id, name).map(Held::Written),
            Held::Class(_)
            | Held::Builtin(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Record(_)
            | Held::Callable(_)
            | Held::OneOf(_)
            | Held::Applied { .. } => None,
        }
    }

    /// The same, of an interned type: the shape itself, or one inside an intersection or a `?`.
    fn stored_object_property(
        &self,
        ty: phrost_ty::TypeId,
        name: &str,
    ) -> Option<phrost_ty::TypeId> {
        let types = self.skeleton.types();
        match types.get(ty) {
            phrost_ty::TypeRef::ObjectShape { .. } => {
                types.object_shape_field(ty, name).map(|(field, _)| field)
            }
            phrost_ty::TypeRef::Nullable(inner) => self.stored_object_property(*inner, name),
            phrost_ty::TypeRef::Intersection(operands) => types
                .operands(*operands)
                .iter()
                .find_map(|&operand| self.stored_object_property(operand, name)),
            _ => None,
        }
    }

    /// The same, of a type written in this file.
    fn written_object_property(
        hir: &FileHir,
        id: phrost_hir::TypeRefId,
        name: &str,
    ) -> Option<phrost_hir::TypeRefId> {
        match hir.type_ref(id) {
            TypeRefData::ObjectShape { fields } => {
                hir.type_ref_list(*fields)
                    .iter()
                    .find_map(|&field| match hir.type_ref(field) {
                        TypeRefData::ShapeField { key, ty, .. } if key.as_str() == name => {
                            Some(*ty)
                        }
                        _ => None,
                    })
            }
            TypeRefData::Nullable(inner) => Self::written_object_property(hir, *inner, name),
            TypeRefData::Intersection(parts) => hir
                .type_ref_list(*parts)
                .iter()
                .find_map(|&part| Self::written_object_property(hir, part, name)),
            _ => None,
        }
    }
}
