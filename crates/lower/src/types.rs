use mago_syntax::cst::{Hint, Identifier};
use phrost_hir::{TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, RelativeTy};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_> {
    /// Lowers one native type hint.
    pub(crate) fn hint(&mut self, hint: &Hint<'_>) -> TypeRefId {
        let data = match hint {
            // mago gives most reserved types their own variant, so no name matching is needed.
            Hint::Null(_) => TypeRefData::Builtin(BuiltinTy::Null),
            Hint::True(_) => TypeRefData::Builtin(BuiltinTy::True),
            Hint::False(_) => TypeRefData::Builtin(BuiltinTy::False),
            Hint::Array(_) => TypeRefData::Builtin(BuiltinTy::Array),
            Hint::Callable(_) => TypeRefData::Builtin(BuiltinTy::Callable),
            Hint::Void(_) => TypeRefData::Builtin(BuiltinTy::Void),
            Hint::Never(_) => TypeRefData::Builtin(BuiltinTy::Never),
            Hint::Float(_) => TypeRefData::Builtin(BuiltinTy::Float),
            Hint::Bool(_) => TypeRefData::Builtin(BuiltinTy::Bool),
            Hint::Integer(_) => TypeRefData::Builtin(BuiltinTy::Int),
            Hint::String(_) => TypeRefData::Builtin(BuiltinTy::String),
            Hint::Object(_) => TypeRefData::Builtin(BuiltinTy::Object),
            Hint::Mixed(_) => TypeRefData::Builtin(BuiltinTy::Mixed),
            Hint::Iterable(_) => TypeRefData::Builtin(BuiltinTy::Iterable),

            Hint::Static(_) => TypeRefData::Relative(RelativeTy::Static),
            Hint::Self_(_) => TypeRefData::Relative(RelativeTy::SelfTy),
            Hint::Parent(_) => TypeRefData::Relative(RelativeTy::Parent),

            Hint::Identifier(identifier) => return self.named_hint(identifier),

            Hint::Nullable(nullable) => {
                let inner = self.hint(nullable.hint);
                TypeRefData::Nullable(inner)
            }

            // Parentheses only exist to express DNF (`(A&B)|C`). The grouping survives as the
            // nested node, so the parens themselves carry no further meaning.
            Hint::Parenthesized(parenthesized) => return self.hint(parenthesized.hint),

            // mago nests these as pairs; phrost stores the operand list. Flattening here means no
            // consumer has to walk a binary tree to answer "what are the members of this union".
            Hint::Union(_) => {
                let mut parts = Vec::new();
                self.flatten_union(hint, &mut parts);
                let list = self.builder.alloc_type_ref_list(&parts);
                TypeRefData::Union(list)
            }
            Hint::Intersection(_) => {
                let mut parts = Vec::new();
                self.flatten_intersection(hint, &mut parts);
                let list = self.builder.alloc_type_ref_list(&parts);
                TypeRefData::Intersection(list)
            }
        };

        self.builder.add_type_ref(data)
    }

    /// A hint that names something: a class, or a reserved word mago did not give its own variant.
    fn named_hint(&mut self, identifier: &Identifier<'_>) -> TypeRefId {
        let name_ref = self.name_ref(identifier);
        let written = self.hir_name_ref_path(name_ref);

        // A reserved word can still arrive as an identifier -- `int` in some positions, and
        // anything mago lexes as a plain name. Recognizing it here is what keeps `int` from being
        // looked up as a class and reported undefined.
        let data = if let Some(builtin) = BuiltinTy::from_name(written.as_str()) {
            TypeRefData::Builtin(builtin)
        } else if let Some(relative) = RelativeTy::from_name(written.as_str()) {
            TypeRefData::Relative(relative)
        } else {
            TypeRefData::Named(name_ref)
        };

        self.builder.add_type_ref(data)
    }

    fn flatten_union(&mut self, hint: &Hint<'_>, out: &mut Vec<TypeRefId>) {
        match hint {
            Hint::Union(union) => {
                self.flatten_union(union.left, out);
                self.flatten_union(union.right, out);
            }
            other => {
                let id = self.hint(other);
                out.push(id);
            }
        }
    }

    fn flatten_intersection(&mut self, hint: &Hint<'_>, out: &mut Vec<TypeRefId>) {
        match hint {
            Hint::Intersection(intersection) => {
                self.flatten_intersection(intersection.left, out);
                self.flatten_intersection(intersection.right, out);
            }
            other => {
                let id = self.hint(other);
                out.push(id);
            }
        }
    }
}
