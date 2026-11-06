//! Every `match` the file writes over a value, with the constants its arms name.
//!
//! A `match` with no arm for the value it is handed throws `UnhandledMatchError`, which is the one
//! way PHP turns a missing case into a crash rather than a default. Over an enum that is decidable:
//! the cases are declared, the arms name them, and what is left over is what the code will fall
//! through on.
//!
//! What travels is the *names* the arms wrote and the path to the subject. Which class those names
//! belong to, and whether it is an enum at all, takes a catalog to say -- the same division every
//! other projection here follows.

use phrost_hir::{FileHir, body::ExprData};
use phrost_model::{Name, TextRange};

use crate::{ValuePath, path::value_path_of, scope::Scope};

/// One `match` written over a value, when every arm of it names a class constant.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MatchSite {
    /// The `match` keyword through its subject, which is what a report underlines.
    pub range: TextRange,
    /// The value being matched on.
    pub subject: ValuePath,
    /// The constants the arms name, in source order and without duplicates removed.
    ///
    /// Only the name. `Stage::Draft` and `self::Draft` reach the same case, and which class each
    /// spelling means is a question for the layer that has one.
    pub named: Vec<Name>,
    /// Every class constant the enclosing body names anywhere else.
    ///
    /// What a `match` leaves out is often handled *before* it: `if (TextAlign::Left !== $align) {
    /// ... match ($align) { Center, Right } }` and an early `return` for one case are both
    /// ordinary, and in both the arm is missing because the value cannot arrive there.
    ///
    /// Reading the guard itself would need narrowing over enum cases, which this layer has no
    /// vocabulary for -- a case is a value rather than a class or a null. What it can say is
    /// weaker and enough: a case the body *mentions* has been thought about, and the whole value of
    /// this check is the case nobody thought about. So a mention anywhere in the body is taken as
    /// handled, and what is left is the case the author never wrote down.
    pub named_elsewhere: Vec<Name>,
    /// Every literal the enclosing body compares with a `->value`: `if ($suit->value === 'H')`.
    ///
    /// The same mention, spelled by the case's backing value rather than its name -- which case a
    /// value is takes the enum's declaration to say, so what travels is where each literal is.
    pub values_elsewhere: Vec<TextRange>,
}

/// Every exhaustively-checkable `match` in the file.
///
/// Three refusals, and each is a `match` whose completeness this layer cannot settle:
///
/// - **A `default` arm**, which handles everything left and makes the question moot.
/// - **An arm naming anything but a class constant** -- a literal, a call, a variable. Such a
///   `match` is over a value whose alternatives are not a declared set.
/// - **A `match (true)`**, which is a chain of guards rather than a comparison against a value; the
///   narrowing reader already treats it as one.
pub fn match_sites(hir: &FileHir) -> Vec<MatchSite> {
    let mut found: Vec<MatchSite> = Vec::new();

    for scope in Scope::every_scope(hir) {
        let body = hir.body(scope.body()).statements;
        // Every constant the body names, collected once per scope: the arms of the `match` are in
        // here too, which costs nothing -- a case an arm names is handled either way.
        let mut mentioned: Vec<Name> = Vec::new();
        let mut values: Vec<TextRange> = Vec::new();
        crate::walk::visit_statements(hir, body, &mut |hir, expr| match hir.expr(expr) {
            ExprData::ClassConstAccess {
                name: Some(constant),
                ..
            } => mentioned.push(constant.clone()),
            ExprData::Comparison { left, right, .. } => {
                values.extend(compared_with_a_value(hir, *left, *right));
            }
            _ => {}
        });
        let first = found.len();
        collect_matches(hir, body, &mut found);
        for site in &mut found[first..] {
            site.named_elsewhere = mentioned.clone();
            site.values_elsewhere = values.clone();
        }
    }

    found.sort_by_key(|site| site.range.start());
    found
}

fn collect_matches(
    hir: &FileHir,
    body: lspkit_arena::IdSlice<phrost_hir::StmtId>,
    found: &mut Vec<MatchSite>,
) {
    crate::walk::visit_statements(hir, body, &mut |hir, expr| {
        let ExprData::Match {
            subject,
            arms,
            range,
        } = hir.expr(expr)
        else {
            return;
        };
        if matches!(hir.expr(*subject), ExprData::Literal { .. }) {
            return;
        }
        let Some(path) = value_path_of(hir, *subject) else {
            return;
        };

        let mut named = Vec::new();
        for &arm in hir.match_arm_list(*arms) {
            let conditions = hir.expr_list(hir.match_arm(arm).conditions);
            if conditions.is_empty() {
                return;
            }
            for &condition in conditions {
                let ExprData::ClassConstAccess {
                    name: Some(constant),
                    ..
                } = hir.expr(condition)
                else {
                    return;
                };
                named.push(constant.clone());
            }
        }
        if named.is_empty() {
            return;
        }

        found.push(MatchSite {
            // The subject rather than the whole expression: a `match` is many lines and its arms
            // are where the eye goes, so the report sits on what is being matched.
            range: hir.expr(*subject).range(),
            subject: path,
            named,
            named_elsewhere: Vec::new(),
            values_elsewhere: Vec::new(),
        });
        let _ = range;
    });
}

/// The literal one side of a comparison is, where the other side reads a `->value`.
fn compared_with_a_value(
    hir: &FileHir,
    left: phrost_hir::ExprId,
    right: phrost_hir::ExprId,
) -> Option<TextRange> {
    let reads_value = |expr: phrost_hir::ExprId| {
        matches!(
            hir.expr(expr),
            ExprData::PropertyAccess { name: Some(name), .. } if name.as_str() == "value"
        )
    };
    let literal = |expr: phrost_hir::ExprId| match hir.expr(expr) {
        ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::String | phrost_hir::body::LiteralKind::Int,
            range,
        } => Some(*range),
        _ => None,
    };
    match (reads_value(left), reads_value(right)) {
        (true, false) => literal(right),
        (false, true) => literal(left),
        _ => None,
    }
}
