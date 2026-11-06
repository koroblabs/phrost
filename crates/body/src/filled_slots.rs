//! What was written into a local's slots after the literal that built it.
//!
//! `$args = ['--force']; $args[] = $path;` is one variable written twice, and the binding search
//! answers with the first: an element write is not a binding, so it never replaces one. That is the
//! right rule for *creating* a variable and the wrong one for describing it -- the literal said
//! `array{0: string}` and the variable holds two entries, and a type narrower than the truth is
//! worse than none. A text scan finds roughly one literal in four assigned to a local and written
//! into afterwards, on a Drupal project and on WordPress alike.
//!
//! The same holds for a key an `unset` takes out, and for an array that was not a literal at all: a
//! parameter declared `array{id: int}` and given `$row['name'] = ...` holds two keys afterwards,
//! and judged against a sealed shape by its declaration alone it would be missing one.
//!
//! This only finds the writes. Folding them into what the literal said is a question about types,
//! and belongs to the layer that has them.

use phrost_hir::{FileHir, body::ExprData};
use phrost_model::{Name, TextSize};

use crate::{
    facts::BodyFacts,
    path::{PathRoot, ValuePath, path_of},
    reach::Reach,
    scope::Scope,
    walk::NeverReturns,
};

/// One write into a slot of a local, after the literal that built it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotFill {
    /// Where it was written, which is where its value is read: the names in it mean what they meant
    /// there.
    pub at: TextSize,
    pub slot: FilledSlot,
    /// What went in, as written. `None` for a value that is not a path, and for a compound write,
    /// whose result depends on what the slot held.
    pub value: Option<ValuePath>,
    /// `$x['n'] += 1`: the slot is rewritten in terms of itself.
    pub compound: bool,
    /// Whether every path to the position went through it. A write in a branch may not have run,
    /// so it adds to what the slot may hold rather than replacing it.
    pub certain: bool,
}

/// Which slot a write named.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FilledSlot {
    /// `$rows[] = $row`, which takes the next integer key.
    Appended,
    /// `$row['id'] = $id`, `$row[0] = $first`.
    Named(Name),
    /// `$map[$key] = $value`, a slot nothing here can name.
    Computed,
    /// `$rows[0]['id'] = $id`: one level further in than any slot of this one, so what the literal
    /// said about that slot no longer stands and nothing here can say what does -- only that it is
    /// an array now. The slot written through, when its key is written: `0` here, and nothing for
    /// `$rows[]['id']` or `$rows[$i]['id']`.
    Through(Option<Name>),
    /// `unset($row['tmp'])`: the slot is gone. Nothing for `unset($row[$key])`, which may have
    /// removed any of them.
    Removed(Option<Name>),
}

/// Every write into a slot of `name` after `since` that can have run before `offset`, in source
/// order.
///
/// `since` is where the literal was assigned, and it has to be a write this scope holds: a name
/// bound somewhere else has nothing to find here. A write a loop carries round counts when the loop
/// began after the literal -- `$stack = [$root]; while ($stack) { ...; $stack[] = $child; }` reads
/// the children at the top of every pass but the first -- and not when the literal is inside the
/// loop too, which assigns it afresh before the read on every pass.
pub fn slots_filled_since(
    hir: &FileHir,
    offset: TextSize,
    name: &str,
    since: TextSize,
    never: NeverReturns<'_>,
) -> Vec<SlotFill> {
    let facts = BodyFacts::of(hir);
    let Some(scope) = Scope::chain_at(hir, offset)
        .into_iter()
        .rev()
        .find(|scope| {
            facts
                .writes_that_may_root_at(scope.body(), Some(name))
                .any(|site| site.range.start() == since)
        })
    else {
        return Vec::new();
    };
    fills_in(hir, facts, &scope, offset, name, since, never)
}

/// The same for a variable that was not bound by an assignment this scope holds -- a parameter, a
/// loop's variable, a value handed over from a call -- searched for in the innermost scope that
/// declares `name` as a parameter, or else the innermost one around `offset`. `since` is where the
/// binding was written, and `None` for a parameter, which holds what it was passed before anything
/// in its body runs.
pub fn slots_filled_in_scope(
    hir: &FileHir,
    offset: TextSize,
    name: &str,
    since: Option<TextSize>,
    never: NeverReturns<'_>,
) -> Vec<SlotFill> {
    let facts = BodyFacts::of(hir);
    let chain = Scope::chain_at(hir, offset);
    let declaring = chain.iter().rev().find(|scope| {
        hir.param_list(scope.params())
            .iter()
            .any(|&id| hir.param(id).name.as_str() == name)
    });
    let Some(scope) = declaring.or(chain.last()) else {
        return Vec::new();
    };
    let since = since.unwrap_or(TextSize::from(0));
    fills_in(hir, facts, scope, offset, name, since, never)
}

/// The writes and removals into slots of `name` in `scope`'s own body after `since`, in source order.
fn fills_in(
    hir: &FileHir,
    facts: &BodyFacts,
    scope: &Scope,
    offset: TextSize,
    name: &str,
    since: TextSize,
    never: NeverReturns<'_>,
) -> Vec<SlotFill> {
    // Whether a write or a removal can have run before the position, and whether it certainly did.
    // A write a loop carries round counts when the loop began after the binding.
    let reach = |site: &crate::facts::WriteSite| match site.range.end() >= offset {
        true => match facts.carried_from(hir, site, offset, never) {
            Some(loop_start) if loop_start > since => Some(Reach::Possible(loop_start)),
            _ => None,
        },
        false => facts.reach_of(hir, site, offset, never),
    };

    let mut fills = Vec::new();
    for site in facts.writes_that_may_root_at(scope.body(), Some(name)) {
        if !site.rooted_at(name) || site.range.start() <= since {
            continue;
        }
        let (target, value, compound) = match hir.expr(site.expr) {
            ExprData::Assign { target, value, .. } => (*target, Some(*value), false),
            ExprData::CompoundAssign { target, .. } => (*target, None, true),
            _ => continue,
        };
        // A whole write after the literal would have been the binding, or a conditional one that
        // made it a join -- either way not a literal being filled in, and not this reader's.
        let ExprData::Index {
            base, index, key, ..
        } = hir.expr(target)
        else {
            continue;
        };
        let slot = match path_of(hir, *base) {
            Some(ValuePath {
                root: PathRoot::Local(written),
                steps,
                ..
            }) if steps.is_empty() && written.as_str() == name => match (index, key) {
                (None, _) => FilledSlot::Appended,
                (Some(_), Some(key)) => FilledSlot::Named(key.clone()),
                (Some(_), None) => FilledSlot::Computed,
            },
            _ => match through(hir, *base, name) {
                Some(key) => FilledSlot::Through(key),
                None => continue,
            },
        };
        let Some(reach) = reach(site) else {
            continue;
        };
        fills.push(SlotFill {
            at: site.range.start(),
            slot,
            value: value.and_then(|value| path_of(hir, value)),
            compound,
            certain: matches!(reach, Reach::Certain),
        });
    }
    for site in facts.unsets_rooted_at(scope.body(), name) {
        if site.range.start() <= since {
            continue;
        }
        let ExprData::Index { base, key, .. } = hir.expr(site.expr) else {
            continue;
        };
        let slot = match path_of(hir, *base) {
            Some(ValuePath {
                root: PathRoot::Local(written),
                steps,
                ..
            }) if steps.is_empty() && written.as_str() == name => FilledSlot::Removed(key.clone()),
            // Removing a key further in changes the slot it is in, and the array that slot holds is
            // still an array.
            _ => match through(hir, *base, name) {
                Some(key) => FilledSlot::Through(key),
                None => continue,
            },
        };
        let Some(reach) = reach(site) else {
            continue;
        };
        fills.push(SlotFill {
            at: site.range.start(),
            slot,
            value: None,
            compound: false,
            certain: matches!(reach, Reach::Certain),
        });
    }
    fills.sort_by_key(|fill| fill.at);
    fills
}

/// The first-level slot an index chain reaches the bare local `name` through: `Some(key)` when that
/// slot's key is written, `Some(None)` when it is not, and `None` when the chain does not end at the
/// local at all.
fn through(hir: &FileHir, expr: phrost_hir::ExprId, name: &str) -> Option<Option<Name>> {
    let ExprData::Index { base, key, .. } = hir.expr(expr) else {
        return None;
    };
    match hir.expr(*base) {
        ExprData::Variable { name: written, .. } => (written.as_str() == name).then(|| key.clone()),
        _ => through(hir, *base, name),
    }
}
