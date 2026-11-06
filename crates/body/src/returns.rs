//! What a body's `return` statements hand back.
//!
//! Statements only, and that is the whole of the rule: a closure's body hangs off an *expression*,
//! so descending through statements is what keeps a `return` written inside one attributed to the
//! closure that wrote it rather than to the function around it. Every walk in this crate states the
//! same rule, and a nested named function or class is unmodelled and keeps only its expressions, so
//! neither is descended into either.
//!
//! A bare `return;` is kept as `None` rather than dropped, because the two readers want opposite
//! things of it. Judging a returned value against a declared type has nothing to judge; inferring
//! what a closure produces needs exactly that null, and losing it would turn "sometimes nothing"
//! into a confident claim the code did not make.

use lspkit_arena::IdSlice;
use phrost_hir::{ExprId, FileHir, StmtId, body::StmtData};

/// Every `return` in this body, in source order — the value it hands back, or `None` for a bare one.
pub fn returned_values(hir: &FileHir, statements: IdSlice<StmtId>) -> Vec<Option<ExprId>> {
    return_sites(hir, statements)
        .into_iter()
        .map(|(value, _)| value)
        .collect()
}

/// The same, with each `return`'s own range — for a reader that has to underline the statement
/// rather than judge the value.
pub fn return_sites(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
) -> Vec<(Option<ExprId>, phrost_model::TextRange)> {
    let mut out = Vec::new();
    collect(hir, statements, &mut out);
    out
}

fn collect(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    out: &mut Vec<(Option<ExprId>, phrost_model::TextRange)>,
) {
    for &id in hir.stmt_list(statements) {
        match hir.stmt(id) {
            StmtData::Return { value, range } => out.push((*value, *range)),
            StmtData::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect(hir, *then_branch, out);
                collect(hir, *else_branch, out);
            }
            StmtData::Foreach { body, .. }
            | StmtData::While { body, .. }
            | StmtData::For { body, .. }
            | StmtData::Block { body, .. } => collect(hir, *body, out),
            StmtData::Try {
                body,
                catches,
                finally,
                ..
            } => {
                collect(hir, *body, out);
                for &catch in hir.catch_list(*catches) {
                    collect(hir, hir.catch(catch).body, out);
                }
                collect(hir, *finally, out);
            }
            StmtData::Switch { cases, .. } => {
                for &case in hir.switch_case_list(*cases) {
                    collect(hir, hir.switch_case(case).body, out);
                }
            }
            StmtData::Expr { .. }
            | StmtData::Annotation { .. }
            | StmtData::Leave { .. }
            | StmtData::Bind { .. }
            | StmtData::Unmodelled { .. } => {}
        }
    }
}
