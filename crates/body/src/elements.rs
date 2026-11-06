//! What was written into one slot of an array.
//!
//! Most arrays in PHP are built a slot at a time -- `$rows = []; $rows[] = $user;` -- and that is the
//! one shape no declaration describes: the literal the variable was assigned is empty, and every
//! word about what it holds is in the writes that follow.
//!
//! The other shape is the literal itself: `$rows = [new Pen()]` fills every slot at once, and it is
//! the same question asked of a different statement -- so it is answered here rather than by a second
//! mechanism somewhere else.
//!
//! Kept apart from the guards, which say what a value *is* here. A write says what was put somewhere,
//! and the two are ranked differently: a declaration that names an element type wins, because it
//! describes every slot rather than one.
//!
//! Which the reader only reaches for a container whose declaration said nothing -- and once anything
//! writes an element of a *declared* variable, `bound_by` has already reduced it to "an array", so
//! there is nothing left for the declaration to win with. That is a ranking question one layer down
//! and not this one's to answer.

use lspkit_arena::IdSlice;
use phrost_hir::{
    ExprId, FileHir,
    body::{ArrayElementKind, ExprData, StmtData},
};
use phrost_model::TextSize;

use crate::{
    path::{PathRoot, PathStep, ValuePath, path_of},
    scope::Scope,
    walk::visit_statements_before,
};

/// One write into a slot: what was put there, and where.
///
/// The position travels with the value for the reason a joined value's operands carry theirs: the
/// value means what it meant *where it was written*. `$apps[$n] = $application` inside an `if` whose
/// guard threw on a null `$application` puts an `Application` in the slot, and reading `$application`
/// where the slot is read -- after the `if`, where that guard no longer holds -- finds a `?Application`
/// that was never what went in.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ElementWrite {
    pub at: TextSize,
    pub value: ValuePath,
}

/// The value last written into one slot of `container`, when something wrote one.
///
/// The key comes in as text rather than as an interned name, because the reader that needs this most
/// read it out of the file: a key nothing in the project declares has no name to look up, and the
/// write's own key is the only thing to compare it against.
///
/// The last write, because a later one replaces an earlier one. Writes inside a branch count: a slot
/// filled conditionally is still a slot the author means to read, and the alternative is saying
/// nothing at all about `$options['config']`.
pub fn element_written(
    hir: &FileHir,
    offset: TextSize,
    container: &ValuePath,
    key: Option<&str>,
) -> Option<ElementWrite> {
    let facts = crate::facts::BodyFacts::of(hir);
    let mut found: Option<(TextSize, ValuePath, bool)> = None;
    // A write that reached *through* the container, which neither reading below can see and which
    // makes both of them stale -- everything before it, and nothing after. See [`writes_through`].
    //
    // Its position and not a flag, and that is the whole of this: a slot write *after* a
    // through-write replaces the whole slot, and whatever the through-write put one level inside
    // goes with it. `$order['Field2']['coCustomer'] = $sanctions;` is stale the moment
    // `$order['Field2'] = json_encode($order['Field2']);` runs three hundred lines later, and the
    // flag made every write for the rest of the body invisible -- so `strlen($order['Field2'])` on
    // the next line was judged against nothing at all.
    let mut through: Option<TextSize> = None;
    // A parameter arrives full, so a write into one of its slots is one contributor among however
    // many the caller put there. Noted before the walk because it is the one source no statement in
    // this scope shows.
    let mut foreign = local_parameter(hir, offset, container);
    for scope in Scope::chain_at(hir, offset).iter() {
        visit_statements_before(
            hir,
            hir.body(scope.body()).statements,
            offset,
            &mut |hir, statement| {
                let StmtData::Expr { expr, .. } = hir.stmt(statement) else {
                    return;
                };
                let ExprData::Assign { target, value, .. } = hir.expr(*expr) else {
                    return;
                };
                // Whatever else this statement is, an assignment of something that is not a literal
                // to the container itself is where its contents came from, and they are not visible
                // here.
                if fills_container_from_elsewhere(hir, *target, *value, container) {
                    foreign = true;
                }
                if writes_through(hir, *target, container) {
                    let at = hir.expr(*expr).range().start();
                    if through.is_none_or(|seen| seen < at) {
                        through = Some(at);
                    }
                }
                // A write in a branch the offset is not in did not run: an `if`/`elseif` chain that
                // rewrites `$callback` in one arm says nothing about the arm beside it. Every other
                // reader of a write asks this first; this walk did not ask at all.
                let ran = facts.writes(scope.body()).iter().any(|site| {
                    site.expr == *expr && facts.reach_of(hir, site, offset, &|_, _| false).is_some()
                });
                if !ran {
                    return;
                }
                let at = hir.expr(*expr).range().start();
                if found.as_ref().is_some_and(|(seen, _, _)| *seen > at) {
                    return;
                }

                let written = filled_slot(hir, *target, *value, container, key)
                    .map(|value| (value, true))
                    .or_else(|| {
                        whole_literal(hir, *target, *value, container, key)
                            .map(|value| (value, false))
                    });

                // A write whose value this layer cannot name leaves an earlier one standing: it says
                // the slot was filled and not with what, and the earlier answer is still the only one.
                if let Some((written, into_a_slot)) = written {
                    found = Some((at, written, into_a_slot));
                }
            },
        );
    }

    found
        .filter(|(at, _, _)| through.is_none_or(|stale| stale < *at))
        .filter(|(_, _, into_a_slot)| !(*into_a_slot && foreign))
        .map(|(at, value, _)| ElementWrite { at, value })
}

/// Whether a write reached *through* the container, into something it holds.
///
/// `$rows[$i][1] = $version` replaces one field of one of `$rows`'s elements, and neither reading
/// below can see it: the write's base is `$rows[$i]` rather than `$rows`, so it fills no slot of
/// the container -- and what the container holds is no longer what the literal that built it said.
///
/// Answered as "say nothing" rather than as a write of its own, because what it put there is one
/// level further in than the question being asked: the honest answer is that the earlier claim no
/// longer stands, not a narrower one built from a slot nobody asked about.
///
/// Symfony builds an array of tuples with a placeholder and fills the placeholder exactly this way
/// -- `$requiredPackages[] = [$options, $response, $name, $path, null]` and then
/// `$requiredPackages[$i][4] = $version` -- so reading the literal alone said the version was
/// certainly null, on a line that passes a string.
fn writes_through(hir: &FileHir, target: ExprId, container: &ValuePath) -> bool {
    let ExprData::Index { base, .. } = hir.expr(target) else {
        return false;
    };
    // A write whose base *is* the container fills one of its slots, which `filled_slot` reads --
    // however deep the container itself is written. What this catches is the base reaching the
    // container through a further index, which is a slot of a slot and past anything nameable here.
    if path_of(hir, *base).as_ref() == Some(container) {
        return false;
    }
    reaches_into(hir, *base, container)
}

/// Whether following an index chain down from `expr` arrives at `container`.
fn reaches_into(hir: &FileHir, expr: ExprId, container: &ValuePath) -> bool {
    if path_of(hir, expr).as_ref() == Some(container) {
        return true;
    }
    match hir.expr(expr) {
        ExprData::Index { base, .. } => reaches_into(hir, *base, container),
        _ => false,
    }
}

/// What a write into one slot of `container` put there.
///
/// `$rows['x'] = $user`, `$rows[] = $user`. The same container by the ordinary rules: a write into
/// `$a['x']` says nothing about `$b['x']`, and `$this->rows[]` is a different slot from `$rows[]`.
fn filled_slot(
    hir: &FileHir,
    target: ExprId,
    value: ExprId,
    container: &ValuePath,
    key: Option<&str>,
) -> Option<ValuePath> {
    let ExprData::Index {
        base,
        key: written_key,
        ..
    } = hir.expr(target)
    else {
        return None;
    };
    if path_of(hir, *base).as_ref() != Some(container) || !fills(key, written_key.as_ref()) {
        return None;
    }
    path_of(hir, value)
}

/// The slot a literal assigned *to* `container` puts under `key`.
///
/// `$rows = [new Pen()]` fills every slot at once, so the literal answers for the one being asked
/// about. And -- this is the half that was missing -- so does `$rows['a'] = ['pen' => new Pen()]`
/// when what is being asked about is `$rows['a']`: the write's target *is* the container, and its
/// value names the key.
///
/// The two readings are one question asked of two containers, and dispatching on the *target's*
/// shape is what hid the second one. A write whose target is an `Index` was read only as "a write
/// into a slot of the container", so a keyed literal in a slot could be reached at one level and
/// never at two -- `$rows['a']` answered the literal and `$rows['a']['pen']` answered nothing. It is
/// invisible from either end: `$rows = ['pen' => new Pen()]` works, and so does
/// `$rows['a'] = new Pen()`, and the shape that fails is the two of them composed, which is how
/// every array of records in PHP is built.
///
/// Disjoint from [`filled_slot`] by construction rather than by ordering: one compares the
/// container against the write's *base* and the other against the whole target, and a path cannot
/// equal both.
///
/// Matched on the value first because asking what the target names is not free, and a right-hand
/// side that is not a literal says nothing about a slot however the left-hand side was written.
fn whole_literal(
    hir: &FileHir,
    target: ExprId,
    value: ExprId,
    container: &ValuePath,
    key: Option<&str>,
) -> Option<ValuePath> {
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(value) else {
        return None;
    };
    if path_of(hir, target).as_ref() != Some(container) {
        return None;
    }
    literal_element(hir, *elements, key)
}

/// What an array literal puts in the slot `key` names, when every element that could be that slot
/// agrees on what it holds.
///
/// Agreement rather than the last element, which is the whole difference between a literal and a
/// write: a write *replaces* what was there, so it has a last, while a literal fills every slot at
/// once and `[new Pen(), new Pencil()]` really does hold both. Answering with the later one would
/// offer a `Pencil`'s members on a value as likely to be a `Pen`, and the union of the two is a claim
/// this layer cannot spell -- so it says nothing, which is what it said before this existed.
///
/// Compared by equality and not by [`same_slot`](crate::path::same_slot): that answers whether two
/// paths are the same *value*, where an unnameable slot matches nothing, and this asks whether two
/// elements hold the same *type*, where two reads of one unnameable slot agree.
fn literal_element(
    hir: &FileHir,
    elements: IdSlice<ExprId>,
    key: Option<&str>,
) -> Option<ValuePath> {
    let mut agreed: Option<ValuePath> = None;

    for &element in hir.expr_list(elements) {
        let held = match hir.expr(element) {
            ExprData::ArrayElement {
                kind: ArrayElementKind::LiteralKey { name, .. },
                value,
                ..
            } => {
                if !fills(key, Some(name)) {
                    continue;
                }
                path_of(hir, *value)
            }
            // A key nothing here can name reaches whichever slot it holds, so it is as likely to be
            // the one being read as any other -- the rule `fills` applies to a variable key.
            ExprData::ArrayElement {
                kind: ArrayElementKind::ComputedKey { .. },
                value,
                ..
            } => path_of(hir, *value),
            // `...$rest` contributes what *its* elements are rather than itself, so what reaches this
            // slot is one step further in -- the same step `$rest[0]` takes, resolved by the reader
            // that already resolves that.
            ExprData::ArrayElement {
                kind: ArrayElementKind::Spread,
                value,
                ..
            } => path_of(hir, *value).map(|mut path| {
                path.steps.push(PathStep::Element(None));
                path
            }),
            // A bare element is keyed by position, which nothing here counts -- so it is as likely to
            // be the slot being read as any other, the rule `fills` applies to an append.
            _ => path_of(hir, element),
        };

        // An element whose value cannot be named is still in the array and could be the slot being
        // read, so a literal holding one says nothing certain about any slot.
        let held = held?;
        agreed = match agreed {
            None => Some(held),
            Some(seen) if seen == held => Some(seen),
            // Two literals are two strings, which is the agreement asked about here -- only one of
            // them could say which class it names, and neither is the slot's on its own.
            Some(seen) if seen.is_a_plain_string() && held.is_a_plain_string() => Some(
                ValuePath::of(PathRoot::Builtin(phrost_model::BuiltinTy::String)),
            ),
            Some(_) => return None,
        };
    }

    agreed
}

/// Whether a write under `written` fills the slot a read of `key` asks for.
///
/// Named keys have to agree: `$a['x'] = $foo` says nothing about `$a['y']`. Anything else does fill
/// it -- an append lands at whatever index is next, and a variable key names a slot nothing here can
/// identify, so both are as likely to be the slot being read as any other.
///
/// The opposite of [`same_slot`](crate::path::same_slot), deliberately. That answers whether two
/// paths are the same *value*, where an unnameable slot must not match; this answers whether a write
/// could have been the one being read, where it must.
fn fills(key: Option<&str>, written: Option<&phrost_model::Name>) -> bool {
    match (key, written) {
        (Some(key), Some(written)) => key == written.as_str(),
        _ => true,
    }
}

/// Whether the container is a bare local this scope declares as a parameter.
///
/// A parameter arrives with whatever the caller put in it, so a write into one of its slots says
/// what *one* slot now holds and nothing about the rest.
fn local_parameter(hir: &FileHir, offset: TextSize, container: &ValuePath) -> bool {
    let Some(name) = bare_local(container) else {
        return false;
    };
    Scope::chain_at(hir, offset).iter().any(|scope| {
        hir.param_list(scope.params())
            .iter()
            .any(|&id| hir.param(id).name.as_str() == name)
    })
}

/// Whether this assignment fills the container from a source whose contents are not written here.
///
/// `$calls = $value->getMethodCalls();` is the shape: a later `$calls[] = [$constructor, $args]` says
/// what was appended and nothing about the calls that were already in it, and reading the push as the
/// element type reported eight working lines on one framework. A literal is the opposite -- it says
/// what the whole array is, which is why the module's other reading exists.
fn fills_container_from_elsewhere(
    hir: &FileHir,
    target: ExprId,
    value: ExprId,
    container: &ValuePath,
) -> bool {
    let Some(name) = bare_local(container) else {
        return false;
    };
    let ExprData::Variable { name: written, .. } = hir.expr(target) else {
        return false;
    };
    written.as_str() == name && !matches!(hir.expr(value), ExprData::ArrayLiteral { .. })
}

/// The name of a container that is one whole local, and nothing else.
///
/// Only that shape is judged. A property or a deeper slot keeps the older reading: its declaration
/// names no element type either, so the writes really are the only evidence there is -- which is what
/// a memoising `static $cache = []` in a service depends on.
fn bare_local(container: &ValuePath) -> Option<&str> {
    match (&container.root, container.steps.is_empty()) {
        (crate::path::PathRoot::Local(name), true) => Some(name.as_str()),
        _ => None,
    }
}
