//! The keys a value is written with.
//!
//! A shape is the one PHP type that names its own keys, and it is the only place a reader can be
//! told what is legal inside `$row['...']`. Everything else -- `array<string, mixed>`, a plain
//! `array` -- says every key holds the same thing and names none of them, so there is nothing to
//! offer and nothing is offered.

use phrost_model::{Name, TextSize};

use crate::{
    chain::{ChainRoot, ChainStep},
    resolve::Held,
    view::IndexView,
};

/// One key a shape names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShapeKey {
    /// As the docblock wrote it, which is what has to end up between the quotes.
    pub name: Name,
    /// What that key holds, rendered. `None` when the field's type names nothing this can render.
    pub ty: Option<Box<str>>,
    /// Written `id?: int` -- the key may be absent, which is worth showing beside it.
    pub optional: bool,
}

impl IndexView<'_> {
    /// The keys of the shape a written receiver ends at.
    ///
    /// Empty for anything that is not a shape, which is the same answer as a class with no members:
    /// there is nothing to say, and a list of guesses inside a subscript would be worse than none.
    pub fn shape_keys_at(
        &self,
        offset: TextSize,
        root: ChainRoot<'_>,
        steps: &[ChainStep<'_>],
    ) -> Vec<ShapeKey> {
        let Some(hir) = self.current().map(|open| open.hir()) else {
            return Vec::new();
        };

        let mut keys: Vec<ShapeKey> = Vec::new();
        for held in self.chain_held(offset, root, steps) {
            for key in self.keys_of_held(offset, hir, &held) {
                // A value that is two shapes at once offers both, and a key they share is one key.
                if !keys.iter().any(|seen| seen.name == key.name) {
                    keys.push(key);
                }
            }
        }
        keys
    }

    pub(crate) fn keys_of_held(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        held: &Held,
    ) -> Vec<ShapeKey> {
        match held {
            Held::Object(_) => Vec::new(),
            // Indexing a `?array{...}` is what the author meant even though PHP would warn about
            // it; the keys are the same either way.
            Held::Nullable(inner) => self.keys_of_held(offset, hir, inner),
            // A literal says its own keys, which is the half of this that needs no docblock.
            Held::Record(fields) => fields
                .iter()
                .map(|(key, value)| ShapeKey {
                    name: key.clone(),
                    ty: self
                        .render_held(offset, hir, value, crate::variables::Naming::Short)
                        .map(Box::from),
                    optional: false,
                })
                .collect(),
            Held::Stored { ty, .. } => self
                .skeleton
                .types()
                .shape_keys(*ty)
                .into_iter()
                .map(|(name, ty, optional)| ShapeKey {
                    name: name.clone(),
                    ty: Some(self.skeleton.types().render(ty).into()),
                    optional,
                })
                .collect(),
            Held::Written(id) => {
                let phrost_hir::TypeRefData::Shape { fields, .. } = hir.type_ref(*id) else {
                    return Vec::new();
                };
                hir.type_ref_list(*fields)
                    .iter()
                    .filter_map(|field| match hir.type_ref(*field) {
                        phrost_hir::TypeRefData::ShapeField { key, optional, ty } => {
                            Some(ShapeKey {
                                name: key.clone(),
                                // A shape's fields are scalars as often as classes, and a key that
                                // showed nothing beside it for `count: int` would leave out the
                                // half a reader is choosing by.
                                ty: match hir.type_ref(*ty) {
                                    phrost_hir::TypeRefData::Builtin(builtin) => {
                                        Some(builtin.as_str().into())
                                    }
                                    _ => self
                                        .class_of_type_ref(offset, hir, *ty)
                                        .map(|class| class.as_str().into()),
                                },
                                optional: *optional,
                            })
                        }
                        _ => None,
                    })
                    .collect()
            }
            // Two shapes at once offer both, and a key they share is one key.
            Held::OneOf(operands) => {
                let mut keys: Vec<ShapeKey> = Vec::new();
                for operand in operands {
                    for key in self.keys_of_held(offset, hir, operand) {
                        if !keys.iter().any(|seen| seen.name == key.name) {
                            keys.push(key);
                        }
                    }
                }
                keys
            }
            // An intersection offers the keys of whichever operand is a shape: `array{a: int}&Foo`
            // is a shape, and the other half saying nothing is not a disagreement.
            Held::AllOf(operands) => operands
                .iter()
                .find_map(|operand| {
                    let keys = self.keys_of_held(offset, hir, operand);
                    (!keys.is_empty()).then_some(keys)
                })
                .unwrap_or_default(),
            Held::Class(_)
            | Held::Builtin(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Callable(_)
            | Held::Applied { .. } => Vec::new(),
        }
    }
}
