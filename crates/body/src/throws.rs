//! Where a body throws, for the docblock that should say so.

use lspkit_arena::IdSlice;
use phrost_hir::{
    FileHir, NameRefId, StmtId, TypeRefData, TypeRefId,
    body::{ClassTarget, ExprData},
};
use phrost_model::TextRange;

use crate::walk::{visit_every_statement, visit_statements};

/// One `throw new X(...)` written directly in a body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ThrowSite {
    /// The class as the `new` wrote it, for the reader with a scope to resolve it in.
    pub class: ClassTarget,
    /// The whole `throw` expression, which is what a report about it underlines.
    pub range: TextRange,
}

/// Every `throw new X(...)` under `statements`, in source order, not counting one inside a closure
/// written there -- the closure's docblock is the one that should say so.
///
/// Only a `new` is read. `throw $e` rethrows what a `catch` bound and `throw $this->error()` throws
/// what a callee built; what class either is would be a resolution rather than a reading, and the
/// tag a docblock wants is the one a person can see in the body. A `throw` written as an expression
/// -- `$x ?? throw new E()` -- counts the same as one written as a statement, because the walk is
/// over expressions.
pub fn throw_sites(hir: &FileHir, statements: IdSlice<StmtId>) -> Vec<ThrowSite> {
    let mut found = Vec::new();
    visit_statements(hir, statements, &mut |hir, expr| {
        let ExprData::Throw { value, range, .. } = hir.expr(expr) else {
            return;
        };
        let ExprData::New { class, .. } = hir.expr(*value) else {
            return;
        };
        found.push(ThrowSite {
            class: *class,
            range: *range,
        });
    });
    found.sort_by_key(|site| site.range.start());
    found
}

/// One `try` block, and the classes its `catch` clauses name.
///
/// The guarded span is the `try { ... }` block **alone**. A throw written in a `catch` body is not
/// caught by the clause it is in, and one in `finally` is caught by nothing here -- both are
/// outside this span, which is what makes a range test right rather than approximate.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatchScope {
    pub guarded: TextRange,
    /// Every type some clause catches, as written. A multi-catch (`catch (A|B $e)`) contributes
    /// each operand, because catching either of them catches the throw.
    pub caught: Vec<NameRefId>,
}

/// Every `try` block under `statements`, innermost last, with what each one catches.
///
/// Not descending into a closure written inside, for the reason [`throw_sites`] does not: its body
/// is a scope of its own and its own docblock is what should speak for it.
pub fn catch_scopes(hir: &FileHir, statements: IdSlice<StmtId>) -> Vec<CatchScope> {
    let mut found = Vec::new();
    visit_every_statement(hir, statements, &mut |hir, id| {
        let phrost_hir::body::StmtData::Try { body, catches, .. } = hir.stmt(id) else {
            return;
        };
        // The span of the block's own statements. An empty `try {}` has none and guards nothing,
        // which is the right answer rather than a missing one: nothing can be thrown in it.
        let statements = hir.stmt_list(*body);
        let (Some(first), Some(last)) = (statements.first(), statements.last()) else {
            return;
        };
        let guarded = TextRange::new(
            hir.stmt(*first).range().start(),
            hir.stmt(*last).range().end(),
        );

        let mut caught = Vec::new();
        for &catch in hir.catch_list(*catches) {
            if let Some(written) = hir.catch(catch).type_ref {
                collect_named(hir, written, &mut caught);
            }
        }
        found.push(CatchScope { guarded, caught });
    });
    found
}

/// Every class-like name a catch's written type names, flattening a multi-catch union.
fn collect_named(hir: &FileHir, written: TypeRefId, out: &mut Vec<NameRefId>) {
    match hir.type_ref(written) {
        TypeRefData::Named(name) => out.push(*name),
        TypeRefData::Union(operands) => {
            for &operand in hir.type_ref_list(*operands) {
                collect_named(hir, operand, out);
            }
        }
        // Nothing else can appear in a `catch`: PHP allows a class name or a union of them, and a
        // parse that produced anything else is a file with a syntax error in it.
        _ => {}
    }
}
