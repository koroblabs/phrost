//! What a `@return ID_TABLE[T]` gives back for one call: the entry of the table at the key passed.
//!
//! The reading half of `Contract::ConstantEntry`. The key is what the call wrote at the parameter,
//! or the parameter's default for a call that wrote nothing there; the table is the constant's own
//! stored type; and the entry is read off it the way a subscript written in code is --
//! `ID_TABLE['mutable']` and `lookUp('mutable')` are one question.

use phrost_sema::{ConstantTable, MemberKind};

use super::CallFacts;
use crate::{
    resolve::{Held, RelativeScope, Subscript},
    view::IndexView,
};

impl IndexView<'_> {
    /// The entry `table` holds at the key `at`'s call passes for `parameter`.
    ///
    /// `None` -- the declared return then answers -- for a key the call does not write as a literal
    /// or a constant, for a table nothing declares, and for a key a sealed table does not name.
    pub(super) fn constant_entry_held(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        table: &ConstantTable,
        parameter: u16,
        scope: Option<&RelativeScope>,
    ) -> Option<Held> {
        let key = self.string_passed_or_defaulted(at, params, parameter)?;
        let held = match table {
            // The first name PHP would find, as a constant written in code is looked up.
            ConstantTable::Global(names) => {
                let found = names
                    .iter()
                    .find(|name| self.constant_location(name.as_str()).is_some())?;
                self.constant_held(at.offset, at.hir, found.as_str())?
            }
            ConstantTable::Member { class, name } => {
                let class = self.class_of_stored(*class, scope)?;
                self.member_held(class.as_str(), MemberKind::ClassConstant, name.as_str())?
            }
        };
        self.indexed_of_held(at.offset, at.hir, &held, Some(&key), Subscript::Written)
    }
}
