//! Calls whose result PHP 8.5 says should not be thrown away.
//!
//! `#[\NoDiscard]` marks a declaration whose return value is the point of calling it -- a
//! `tryLock()` that answers whether it locked, a validator that answers whether it passed. PHP 8.5
//! raises a warning at runtime for `tryLock();` written as a statement, and accepts `(void)
//! tryLock();` as the author saying the result was meant to go.
//!
//! Which calls discard is [`phrost_body::discarded_calls`]'s question; whether the callee is one of
//! these is this module's, because deciding it needs the declaration and the declaration is usually
//! in another file.

use phrost_body::{CallSite, CallTarget};
use phrost_model::TextRange;
use phrost_sema::MemberKind;

use crate::{view::FOLLOW_LIMIT, view::IndexView};

/// One call whose result was thrown away against the declaration's wishes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiscardedResult {
    /// The callee's name as written, which is what a report underlines.
    pub range: TextRange,
    /// What was called, spelled the way a reader would write it: `App\Repo::tryLock` for a
    /// method, the qualified name for a free function.
    pub callee: String,
}

impl IndexView<'_> {
    /// Every call in the current file that discards a `#[\NoDiscard]` result.
    pub fn discarded_results(&self) -> Vec<DiscardedResult> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::discarded_calls(hir)
            .into_iter()
            .filter_map(|call| self.discarded_result(hir, &call))
            .collect()
    }

    fn discarded_result(
        &self,
        hir: &phrost_hir::FileHir,
        call: &CallSite,
    ) -> Option<DiscardedResult> {
        let at = call.receiver_at();
        match &call.target {
            CallTarget::Member { receiver, name } => {
                let held = self.resolve_path(at, hir, receiver, FOLLOW_LIMIT)?;
                let class =
                    self.declaring_class(at, hir, &held, MemberKind::Method, name.as_str())?;
                let table = self.members(class.as_str());
                let found = table.find(MemberKind::Method, name.as_str())?;
                // The class that *declares* it, which is the one whose attribute this is -- the
                // same rule the refused-write check follows for a property's modifiers.
                let owner = match &found.provenance {
                    phrost_sema::MemberProvenance::Own
                    | phrost_sema::MemberProvenance::Trait(_) => class.clone(),
                    phrost_sema::MemberProvenance::Inherited(fqn)
                    | phrost_sema::MemberProvenance::Interface(fqn) => fqn.clone(),
                };
                found
                    .row
                    .modifiers
                    .is_no_discard()
                    .then(|| DiscardedResult {
                        range: call.callee_range,
                        callee: format!("{owner}::{name}"),
                    })
            }
            CallTarget::Function(reference) => {
                let written = hir.name_ref(*reference);
                let location = self.resolve_name(
                    at,
                    written.path.as_str(),
                    written.anchor,
                    phrost_hir::ImportKind::Function,
                )?;
                self.function_is_no_discard(location.fqn.as_str())
                    .then(|| DiscardedResult {
                        range: call.callee_range,
                        callee: location.fqn.as_str().to_owned(),
                    })
            }
            // `new Repo();` for its side effects is ordinary PHP, and the attribute is about a
            // return value -- a constructor has none.
            CallTarget::Constructor(_) => None,
            // `#[NoDiscard]` is read off a *declaration*, and a value being invoked names none to
            // read it from.
            CallTarget::Invoked(_) => None,
        }
    }
}
