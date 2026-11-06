//! Where the file reaches into a value with `[...]`, and what it reaches into.
//!
//! The sibling of [`member_sites`](crate::member_sites): that one collects `$x->y`, this collects
//! `$x['y']`, and both hand over the path to the value on the left for a layer with a catalog to
//! resolve. A whole-file projection, because the reader that needs it has to visit every one.
//!
//! # Only reads, and only a variable
//!
//! A **write** is left out, because `$rows['k'] = 1` is legal on a variable that holds nothing at
//! all -- PHP creates the array. So is a `?? ` left operand and an `isset`, which read through a
//! missing slot on purpose.
//!
//! The base is any value a path can be read from -- a variable, a property, a call's result -- and
//! never another subscript. The exclusion is what keeps the cost flat, and it is the whole of what
//! the old rule ("a plain variable and nothing else") was buying: a framework's render array is
//! `$form['a']['b']['#type']` a thousand times per file, and stopping at the innermost `[` is one
//! resolution per chain rather than one per level.
//!
//! Restricting it to a *variable* bought nothing beyond that and cost the two shapes a nullable
//! container is actually written in: `$this->config['endpoints']` on a `?array` property, and
//! `$form_state->getTriggeringElement()['#array_parents']` on a method that declares one. PHPStan
//! reports ~25 of those on one real checkout where this said nothing, and the resolution count is
//! bounded the same way either way -- one per distinct path per scope.
//!
//! For the same reason a variable the file itself *builds* as an array is left out. `$form['x'] = 1`
//! creates the array, which is PHP's own rule and not a guess, so resolving `$form` can only ever
//! answer `array` -- and on a generated template that one exclusion is most of the file.
//!
//! And only the first read of each path in a scope. What its type is is one fact, so
//! `$seller['a']` … `$seller['z']` twenty-nine times is one thing to say, and a report per line
//! would say it twenty-nine times -- the same rule the unbound and early-read checks report under.
//! It is also what stops the cost scaling with how often a name is used: the twenty-nine reads
//! resolve the same expression at twenty-nine positions, and no two of them can share an answer.

use std::collections::HashSet;

use phrost_hir::{
    BodyId, FileHir,
    body::{ExprData, StmtData},
};
use phrost_model::{Name, TextRange};

use crate::{ValuePath, path::value_path_of, scope::Scope};

/// One `[...]` read from a value, rather than written to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SubscriptSite {
    /// The whole `$rows['k']`, for a reader that needs to know how far the expression goes.
    pub range: TextRange,
    /// The value on the left, which is what a report about its type underlines.
    pub base_range: TextRange,
    /// The path to the value being reached into.
    pub base: ValuePath,
    /// The key as a literal wrote it, `None` when it is computed or absent.
    pub key: Option<Name>,
    /// Whether the value on the left is a plain local variable.
    ///
    /// What the two readers of a site disagree about. A *null* is always declared -- `?array`, a
    /// docblock union, a finder that may find nothing -- so it can be judged wherever the value was
    /// reached from. A *kind* often is not: an untyped property's initializer is PHP's starting
    /// value rather than a claim, and `public $update = false;` on WordPress's `WP_Theme` is
    /// written to with an array from outside the class, which is whole-program and not something a
    /// per-file index answers. So "this holds no elements at all" is asked only of a local, whose
    /// writes the file that reads it also holds.
    pub base_is_local: bool,
}

/// The first element read from each distinct value of each scope, in source order.
///
/// One site per *value*, which is what a question about the value's own type needs -- see the
/// module docs. [`keyed_subscript_sites`] is the same collection kept one per key, for the question
/// that is about the key instead.
pub fn subscript_sites(hir: &FileHir) -> Vec<SubscriptSite> {
    collected(hir, |site| site.base.clone())
}

/// The same, kept one per value *and key*, with the computed keys left out.
///
/// Whether a container holds elements at all is one fact about the value, so twenty-nine reads of
/// it are one thing to say. Whether a *key* is declared is a fact about the key, and
/// `$address['address']` beside `$address['city']` are two questions with two answers -- so the
/// rule that keeps the first read of each value would answer one of them and drop the other.
pub fn keyed_subscript_sites(hir: &FileHir) -> Vec<SubscriptSite> {
    collected(hir, |site| (site.base.clone(), site.key.clone()))
        .into_iter()
        .filter(|site| site.key.is_some())
        .collect()
}

fn collected<K: std::hash::Hash + Eq>(
    hir: &FileHir,
    distinct: impl Fn(&SubscriptSite) -> K,
) -> Vec<SubscriptSite> {
    let written = write_targets(hir);
    let asked = crate::members::null_suppressed_ranges(hir);
    let mut sites = Vec::new();

    // Scope by scope, because "the first read of this name" is a question about one scope: two
    // functions writing `$rows` are writing two variables. Closures are scopes of their own and
    // are in this list, so nothing written inside one is missed.
    for scope in Scope::every_scope(hir) {
        let mut found: Vec<SubscriptSite> = Vec::new();

        crate::walk::visit_statements(hir, hir.body(scope.body()).statements, &mut |hir, expr| {
            let ExprData::Index {
                base, key, range, ..
            } = hir.expr(expr)
            else {
                return;
            };
            // Another subscript on the left is a deeper level of one chain, and the innermost is
            // the one that says what the whole chain reaches into.
            let base_expr = base;
            if matches!(hir.expr(*base_expr), ExprData::Index { .. }) {
                return;
            }
            let base_range = hir.expr(*base_expr).range();
            // A write creates the array, and a presence test is willing to find nothing there.
            if written
                .targets
                .iter()
                .chain(asked.iter())
                .any(|region| region.contains_range(*range))
            {
                return;
            }
            let Some(base) = value_path_of(hir, *base) else {
                return;
            };
            // A write creates the container, whatever it is reached through -- in this body, for a
            // local, and anywhere for a slot the whole file shares.
            if written.builds(scope.body(), &base) {
                return;
            }
            found.push(SubscriptSite {
                range: *range,
                base_range,
                // A base is a prefix, for the reason `MemberSite::receiver` gives: `$a?->b()['k']`
                // never indexes when `$a` is null.
                base: ValuePath {
                    may_be_absent: false,
                    ..base
                },
                key: key.clone(),
                base_is_local: matches!(hir.expr(*base_expr), ExprData::Variable { .. }),
            });
        });

        // The expression walk reaches a parent before its children, which is not source order, and
        // which of several reads is *first* is the whole of what is being kept.
        found.sort_by_key(|site| site.range.start());
        let mut seen: HashSet<K> = HashSet::new();
        sites.extend(found.into_iter().filter(|site| seen.insert(distinct(site))));
    }

    sites.sort_by_key(|site| site.range.start());
    sites
}

/// One `(value, key)` pair a scope tests the presence of, and the scope it was tested in.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestedKey {
    /// The body the test was written in, which is how far the claim reaches.
    pub within: TextRange,
    pub base: ValuePath,
    pub key: Name,
}

/// Every `(value, key)` pair the file tests the presence of, scope by scope.
///
/// `isset($row['vin'])`, `array_key_exists('vin', $row)` and `$row['vin'] ?? NULL`: three
/// spellings of a test whose whole purpose is to ask whether a key is there. A body that writes one
/// has said the key may be absent and has written the branch for it.
///
/// Per *scope* rather than per position, which is the reading [`crate::existence_tests`] takes of
/// the same question and for the same reason: the guarded read is often nowhere near the test, and
/// requiring the test to enclose it would report most of exactly the code this exists to stop
/// reporting. Per scope rather than per file, unlike that one, because a local is a different value
/// in every method -- a generated template that coalesces `$address['address']` in twelve methods
/// would otherwise silence a bare read in the thirteenth.
pub fn tested_keys(hir: &FileHir) -> Vec<TestedKey> {
    let mut tested = Vec::new();

    for scope in Scope::every_scope(hir) {
        let within = hir.body(scope.body()).range;
        crate::walk::visit_statements(hir, hir.body(scope.body()).statements, &mut |hir, expr| {
            let indexed = match hir.expr(expr) {
                // `isset($row['vin'])`, and `empty($row['vin'])` through the same node.
                ExprData::NullTest { value, kind, .. } if kind.reads_through_null() => *value,
                // `$row['vin'] ?? NULL`, the spelling a body writes when it wants the value as well
                // as the answer: `if (($offer['amount'] ?? NULL) !== NULL && $offer['amount'] !==
                // '')` tests the key and then reads it again two operands later.
                ExprData::Coalesce { left, .. } => *left,
                // `array_key_exists('vin', $row)`, whose arguments are the other way round.
                ExprData::FunctionCall {
                    name,
                    args,
                    literal_arguments,
                    ..
                } if hir
                    .name_ref(*name)
                    .path
                    .as_str()
                    .eq_ignore_ascii_case("array_key_exists") =>
                {
                    let Some(key) = literal_arguments
                        .iter()
                        .find(|argument| argument.position == 0 && argument.wrote_its_value())
                        .map(|argument| argument.value.clone())
                    else {
                        return;
                    };
                    let Some(&base) = hir.expr_list(*args).get(1) else {
                        return;
                    };
                    if let Some(base) = value_path_of(hir, base) {
                        tested.push(TestedKey {
                            within,
                            base: ValuePath {
                                may_be_absent: false,
                                ..base
                            },
                            key,
                        });
                    }
                    return;
                }
                _ => return,
            };
            let ExprData::Index {
                base,
                key: Some(key),
                ..
            } = hir.expr(indexed)
            else {
                return;
            };
            if let Some(base) = value_path_of(hir, *base) {
                tested.push(TestedKey {
                    within,
                    base: ValuePath {
                        may_be_absent: false,
                        ..base
                    },
                    key: key.clone(),
                });
            }
        });
    }

    tested
}

/// Everywhere the file writes, in the two shapes a reader of subscripts needs.
pub(crate) struct Written {
    /// The range of every assignment target and every `foreach` target.
    ///
    /// By range rather than by shape, because what matters is whether a subscript is *inside* one:
    /// `$rows['a']['b'] = 1` writes through two of them, and only the outermost is the target.
    targets: Vec<TextRange>,
    /// Every value the file writes an element of, which is every value it builds an array out of.
    ///
    /// Keyed by the **body** for a local and by nothing for anything else. A local is a different
    /// value in every scope: a widget that builds `$element['#prefix']` in `formElement` says
    /// nothing about the `$element` another method reads off a `?array`, and a file-wide set
    /// silenced every subscript check on that name in every other scope of the file -- three real
    /// findings on one Drupal widget, and the shape recurs wherever a framework names its arrays
    /// the same thing twice. A property is the same slot wherever it is written, so it keeps the
    /// file-wide reading it always had.
    built: HashSet<(Option<BodyId>, ValuePath)>,
}

pub(crate) fn write_targets(hir: &FileHir) -> Written {
    let mut written = Written {
        targets: Vec::new(),
        built: HashSet::new(),
    };

    // Every scope, closures included, because the reads this answers for are collected the same
    // way: a closure's own `$rows['a'] = 1` was not a write here and read as an element of `?array`.
    for scope in Scope::every_scope(hir) {
        let body = scope.body();
        crate::walk::visit_statements(hir, hir.body(body).statements, &mut |hir, expr| {
            if let ExprData::Assign { target, .. } = hir.expr(expr) {
                written.record(hir, body, *target);
            }
        });
        // A `foreach` target is written by the header, which the expression walk reaches through
        // the statement rather than as part of an assignment.
        crate::walk::visit_every_statement(hir, hir.body(body).statements, &mut |hir, id| {
            if let StmtData::Foreach { key, value, .. } = hir.stmt(id) {
                for target in [*key, Some(*value)].into_iter().flatten() {
                    written.record(hir, body, target);
                }
            }
        });
    }

    written
}

impl Written {
    fn record(&mut self, hir: &FileHir, body: BodyId, target: phrost_hir::ExprId) {
        self.targets.push(hir.expr(target).range());
        // `$rows['k'] = 1` and `$this->rows['k'] = 1` both create the container they write
        // through, so neither says anything about what was there before.
        if let ExprData::Index { base, .. } = hir.expr(target)
            && let Some(path) = value_path_of(hir, *base)
        {
            let scoped = matches!(path.root, crate::path::PathRoot::Local(_)).then_some(body);
            self.built.insert((scoped, path));
        }
    }

    /// Whether `body` builds this value, or the file does where the value is not a local.
    pub(crate) fn builds(&self, body: BodyId, path: &ValuePath) -> bool {
        self.built.contains(&(None, path.clone()))
            || self.built.contains(&(Some(body), path.clone()))
    }
}
