//! What a global or namespaced constant holds, read off the value its declaration stored.
//!
//! Most values state a type outright and the index keeps that type. A value built from other
//! constants -- `define('HOUR_IN_SECONDS', 60 * MINUTE_IN_SECONDS)`, `const SEPARATOR =
//! DIRECTORY_SEPARATOR;` -- is kept as its parts, and settled here against the index it is asked
//! in: which declaration a name reaches is a fact about every file, and this is where every file is
//! known.

use phrost_hir::FileHir;
use phrost_index::constant::ConstantValue;
use phrost_model::TextSize;

use crate::{resolve::Held, view::IndexView};

/// How many constants deep a value is followed: `YEAR_IN_SECONDS` is four from a literal, and a
/// constant naming itself -- which is how the stubs write what the engine fills in -- must stop.
const CONSTANT_DEPTH: u8 = 8;

impl IndexView<'_> {
    /// What the constant `fqn` holds, or `None` where nothing it is built from says.
    pub(crate) fn constant_held(&self, offset: TextSize, hir: &FileHir, fqn: &str) -> Option<Held> {
        self.constant_followed(offset, hir, fqn, CONSTANT_DEPTH)
    }

    fn constant_followed(
        &self,
        offset: TextSize,
        hir: &FileHir,
        fqn: &str,
        depth: u8,
    ) -> Option<Held> {
        let depth = depth.checked_sub(1)?;
        let value = self.constant_value(fqn)?;
        self.constant_part(offset, hir, value, depth)
    }

    fn constant_part(
        &self,
        offset: TextSize,
        hir: &FileHir,
        value: &ConstantValue,
        depth: u8,
    ) -> Option<Held> {
        match value {
            ConstantValue::Ty(ty) => Some(Held::Stored {
                ty: *ty,
                scope: None,
            }),
            // The first name PHP would find, and only that one: a namespaced constant that exists
            // and says nothing does not hand the question on to the global one it shadows.
            ConstantValue::Constant(names) => {
                let found = names
                    .iter()
                    .find(|name| self.constant_location(name.as_str()).is_some())?;
                self.constant_followed(offset, hir, found.as_str(), depth)
            }
            ConstantValue::Arithmetic(operator, operands) => {
                let left = self.constant_part(offset, hir, &operands[0], depth)?;
                let right = self.constant_part(offset, hir, &operands[1], depth)?;
                self.arithmetic_result(offset, hir, *operator, &left, &right)
            }
            ConstantValue::Bitwise(operands) => {
                let left = self.constant_part(offset, hir, &operands[0], depth)?;
                let right = self.constant_part(offset, hir, &operands[1], depth)?;
                self.bitwise_result(offset, hir, &left, &right)
            }
        }
    }
}
