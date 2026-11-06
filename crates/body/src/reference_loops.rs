//! What a loop over a variable by reference wrote back into it.
//!
//! `foreach ($ints as &$item) { $item = 'x'; }` makes each pass's `$item` an alias of one slot of
//! `$ints`, so the writes the body makes to `$item` are writes into `$ints` -- and after the loop the
//! container holds strings, which nothing at the loop's header says. A write, then, but one with no
//! assignment to the container anywhere in the text, which is why it is found here and not among the
//! assignments.

use phrost_hir::{
    FileHir, StmtId,
    body::{ExprData, StmtData},
};
use phrost_model::TextSize;

use crate::{
    binding::{Bound, bound_by},
    facts::BodyFacts,
    path::{OperandSite, PathRoot, ValuePath},
    scope::Scope,
    walk::{NeverReturns, visit_preceding_statements},
};

impl Scope {
    /// The last loop over `name` by reference that ran on the way to `offset`, as the value it left
    /// in `name`, positioned where the loop ends.
    ///
    /// A loop control certainly went through, and only that: `visit_preceding_statements` is the
    /// walk that knows which statements those are. A loop inside a branch the position is past may
    /// not have run, and the container on the path that skipped it is not what it was at the loop's
    /// header -- a guard around the loop may have narrowed it there. Joining the two is what
    /// `maybe_written` does for an assignment, and nothing here pretends to; such a loop is simply
    /// not read, which leaves the container as the declaration says, as it was before this existed.
    ///
    /// `None` too for a loop that writes `name` itself in its body -- the container is then
    /// rebuilt by something other than the reference -- and for one that writes nothing whole to the
    /// reference, which leaves every element as it was. A write *into* an element, `$item['k'] = 1`,
    /// is not read: it changes what one slot holds, not which slots there are, and following it is
    /// the element-write rule's business one level in.
    pub(crate) fn rewritten_by_reference(
        &self,
        hir: &FileHir,
        offset: TextSize,
        name: &str,
        never: NeverReturns<'_>,
    ) -> Option<(TextSize, ValuePath)> {
        let facts = BodyFacts::of(hir);
        if !facts.iterated_by_reference(self.body(), name) {
            return None;
        }
        let mut last: Option<StmtId> = None;
        visit_preceding_statements(
            hir,
            hir.body(self.body()).statements,
            offset,
            never,
            &mut |hir, statement| {
                if Self::reference_loop_over(hir, statement, name).is_some()
                    && last.is_none_or(|previous| {
                        hir.stmt(previous).range().end() <= hir.stmt(statement).range().end()
                    })
                {
                    last = Some(statement);
                }
            },
        );
        let statement = last?;
        let (container, item) = Self::reference_loop_over(hir, statement, name)?;
        let range = hir.stmt(statement).range();

        let inside = |site: &&crate::facts::WriteSite| range.contains_range(site.range);
        // A write rooted at the container, or a destructuring that names it -- the one shape of
        // write with no variable under its target, which `rooted_at` cannot tell apart and so
        // admits for every name.
        if facts
            .writes_that_may_root_at(self.body(), Some(name))
            .filter(inside)
            .any(|site| match &site.root {
                Some(root) => root.as_str() == name,
                None => match hir.expr(site.expr) {
                    ExprData::Assign { target, .. } => {
                        crate::binding::destructured_slot(hir, *target, name).is_some()
                    }
                    _ => false,
                },
            })
        {
            return None;
        }
        if Self::unsets_variable(hir, statement, item) {
            return None;
        }

        let mut writes = Some(Vec::new());
        let mut any = false;
        for site in facts
            .writes_that_may_root_at(self.body(), Some(item))
            .filter(inside)
        {
            let (ExprData::Assign { target, .. } | ExprData::CompoundAssign { target, .. }) =
                hir.expr(site.expr)
            else {
                continue;
            };
            match bound_by(hir, *target) {
                Some(Bound::Whole(bound)) if bound.as_str() == item => {
                    any = true;
                    let value = crate::locals::destructured_value(hir, site.expr, &[]);
                    match (writes.as_mut(), value) {
                        (Some(writes), Some(value)) => writes.push(OperandSite {
                            value,
                            at: site.range.start(),
                        }),
                        (_, None) => writes = None,
                        (None, Some(_)) => {}
                    }
                }
                Some(_) => {}
                // `[$item, $rest] = $pair` writes the reference through a destructuring, which
                // names a slot of a value rather than a value.
                None => {
                    if crate::binding::destructured_slot(hir, *target, item).is_some() {
                        any = true;
                        writes = None;
                    }
                }
            }
        }
        if !any {
            return None;
        }

        Some((
            range.end(),
            ValuePath::of(PathRoot::RewrittenElements {
                before: Box::new(ValuePath::of(PathRoot::Local(container.clone()))),
                at: range.start(),
                writes,
            }),
        ))
    }

    /// Whether the loop's body has an `unset($item)` anywhere in it, which ends the alias: what is
    /// written after it lands in a fresh local rather than in the container.
    ///
    /// Walked here rather than read from the facts, which keep only the unsets that remove a key --
    /// and walked only for a loop that already qualified, which is rare.
    fn unsets_variable(hir: &FileHir, statement: StmtId, item: &str) -> bool {
        let StmtData::Foreach { body, .. } = hir.stmt(statement) else {
            return false;
        };
        let mut found = false;
        crate::walk::visit_every_statement(hir, *body, &mut |hir, id| {
            if let StmtData::Unmodelled {
                inner,
                kind: phrost_hir::body::UnmodelledKind::Unset,
                ..
            } = hir.stmt(id)
            {
                found |= hir.expr_list(*inner).iter().any(|&target| {
                    matches!(hir.expr(target), ExprData::Variable { name, .. } if name.as_str() == item)
                });
            }
        });
        found
    }

    /// The container and the reference variable of `foreach ($name as &$item)`, when `statement`
    /// is one.
    fn reference_loop_over<'h>(
        hir: &'h FileHir,
        statement: StmtId,
        name: &str,
    ) -> Option<(&'h phrost_model::Name, &'h str)> {
        let StmtData::Foreach { subject, value, .. } = hir.stmt(statement) else {
            return None;
        };
        let ExprData::Variable { name: iterated, .. } = hir.expr(*subject) else {
            return None;
        };
        if iterated.as_str() != name {
            return None;
        }
        let ExprData::Reference { value, .. } = hir.expr(*value) else {
            return None;
        };
        let ExprData::Variable { name: item, .. } = hir.expr(*value) else {
            return None;
        };
        Some((iterated, item.as_str()))
    }
}
