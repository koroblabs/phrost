//! Calls whose result nothing takes.
//!
//! The statement `f($x);` throws the return value away, which for most calls is the point -- and
//! for a call PHP 8.5 marks `#[\NoDiscard]` is a warning at runtime. Whether a *particular* callee
//! is one of those is a question for a catalog; which calls discard is a question about the shape
//! of the body, so it is answered here.

use phrost_hir::{
    FileHir,
    body::{ExprData, StmtData},
};
use phrost_model::BuiltinTy;

use crate::calls::{CallSite, site_of};

/// Every call in the file that stands alone as a statement, with a `(void)` cast taken as consent.
///
/// `(void) f();` is what PHP 8.5 gives a caller to say the result was meant to go -- the runtime
/// warning names it -- so a cast around the call is the author answering, and this does not report
/// it. Nothing else counts as consent: a result assigned to a variable, passed on, or tested is not
/// discarded at all and never reaches here.
///
/// Every body, closures included: a result thrown away inside a callback is thrown away.
pub fn discarded_calls(hir: &FileHir) -> Vec<CallSite> {
    let mut discarded = Vec::new();
    for (_, body) in hir.bodies() {
        crate::walk::visit_every_statement(hir, body.statements, &mut |hir, id| {
            let StmtData::Expr { expr, .. } = hir.stmt(id) else {
                return;
            };
            if let ExprData::Cast {
                to: BuiltinTy::Void,
                ..
            } = hir.expr(*expr)
            {
                return;
            }
            if let Some(site) = site_of(hir, *expr) {
                discarded.push(site);
            }
        });
    }
    discarded
}
