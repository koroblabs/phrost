//! Whether the derived facts answer exactly what the walks they replaced answered.
//!
//! `facts.rs` holds the tree's half of two rules and applies the position's half per write; the
//! walks in `reach.rs` and `walk.rs` are the statement of those rules and run only here. Every
//! assignment of every body of every fixture below is asked about at every position a statement or
//! expression begins or ends, and the two answers have to agree -- which write is visited, in what
//! order, and how surely it reached the position -- and so do the guards each says hold there. A
//! rule changed in one place and not the other fails this, which is the point of keeping the walk.

use phrost_hir::{BodyId, ExprId, FileHir, body::ExprData};
use phrost_model::{FileId, Interner, TextSize};

use crate::{
    facts::BodyFacts,
    reach::{Reach, visit_reaching_expressions},
    walk::{NeverReturns, visit_guarding_ifs, visit_guarding_operands, visit_statements},
};

const FIXTURES: &[&str] = &[
    // Branches that leave, nested ifs, elseif chains, an else that writes.
    r#"<?php
function f(?Foo $a, $c, $d) {
    $v = start();
    if ($c) {
        $v = one($v);
        if ($d) { $v = two($v); return $v; } elseif ($a) { $v = three(); } else { $v = four(); }
    } else {
        $v = five();
        throw new E();
    }
    $w = $v;
    if (!$a instanceof Foo) { return; }
    $a->bar($v);
    if ($v instanceof Bar || $w) { $v->baz(); } else { $w = $v; }
}
"#,
    // Loops of every kind, with writes in headers, bodies and after.
    r#"<?php
function g(array $rows) {
    $last = NULL; $n = 0;
    foreach ($rows as $k => $row) { $last = $row; $n = $n + 1; if ($row) { continue; } $last = NULL; }
    while ($item = next($rows)) { $last = $item; if ($item->done()) { break; } }
    for ($i = 0, $j = 1; $i < $n, ($x = $i); $i++, $j = $j * 2) { $last = $i; }
    do { $last = pop($rows); } while ($last);
    return $last;
}
"#,
    // try / catch / finally with writes in each part and uses in the handlers.
    r#"<?php
function h(Client $client) {
    $result = NULL;
    try {
        $result = $client->fetch();
        $count = count($result);
        if ($count === 0) { throw new Nothing(); }
    } catch (Nothing $e) {
        $result = [];
        $count = 0;
    } catch (Failure $f) {
        $result = $f->recover();
        return $result;
    } finally {
        $closed = $client->close();
    }
    $again = $result;
    return [$count, $closed, $again];
}
"#,
    // switch with and without break, fall-through, a default.
    r#"<?php
function s($kind, $v) {
    switch ($kind) {
        case 'a': $v = trim($v); break;
        case 'b': $v = strtoupper($v);
        case 'c': $v = $v . '!'; break;
        case 'd': $v = NULL; return $v;
        default: $v = (string) $v;
    }
    return $v;
}
"#,
    // Writes inside expressions: match, ternary, coalesce, logical, nested.
    r#"<?php
function e($k, $x, $y) {
    $v = match ($k) { 'a' => $x = one(), 'b' => ($x = two()) ?: ($x = three()), default => NULL };
    $w = $k ? ($y = four()) : ($y = five());
    $z = $y ?? ($y = six());
    $ok = $x && ($y = seven()) || ($x = eight());
    $q = $k ?: ($x = nine());
    return [$v, $w, $z, $ok, $q, $x, $y];
}
"#,
    // Closures and arrow functions: their bodies are their own, their captures are not.
    r#"<?php
function c(array $items) {
    $total = 0;
    $add = function ($n) use (&$total) { $total = $total + $n; $local = $n; return $local; };
    $double = fn($m) => $total = $m * 2;
    $names = array_map(function ($item) { $name = $item->name(); if ($name) { $name = trim($name); } return $name; }, $items);
    $total = $add(1);
    return [$double, $names, $total];
}
"#,
    // Property and element writes through a variable, and destructuring.
    r#"<?php
class K {
    private array $rows = [];
    private ?int $count = NULL;
    public function fill(array $in): void {
        $this->count = 0;
        $rows = [];
        foreach ($in as $key => [$a, $b]) {
            $rows[$key] = $a;
            $rows[$key]['b'] = $b;
            $this->rows[$key] = $rows[$key];
            $this->count = $this->count + 1;
        }
        list($first, $second) = $rows;
        if ($this->count > 1) { $this->rows = array_reverse($this->rows); }
        $done = $this->rows;
    }
}
"#,
    // Calls that never return, in every deciding position: last in a branch, followed by dead
    // code, in a switch arm, and beside an ordinary call that only looks like one.
    r#"<?php
function abort(): never { exit(1); }
function m(Foo $x, $c, $k) {
    $v = start();
    if ($c) {
        $v = one();
        abort();
        $v = dead();
    } elseif ($k) {
        $v = two();
        log_it($v);
    } else {
        $v = three();
        if ($k) { abort(); } else { log_it($v); }
    }
    switch ($k) {
        case 'a': $v = four(); abort();
        case 'b': $v = five(); log_it($v);
        case 'c': $v = six(); break;
        default: $v = seven();
    }
    while ($c) { $v = eight(); if ($k) { abort(); $v = dead(); } }
    return $v;
}
"#,
    // A bare block, an unmodelled statement, an annotation, and code at file scope.
    r#"<?php
$config = load();
{
    $config = merge($config, defaults());
}
/** @var Settings $config */
$settings = $config;
if ($settings) {
    echo $settings->name;
    goto done;
}
$settings = NULL;
done:
print $settings;
"#,
];

fn lowered(source: &str) -> FileHir {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );
    lowered.hir
}

/// Every position worth asking about: where each statement and expression of the body begins and
/// ends, and one byte to either side, so that inclusive and exclusive boundaries are both tried.
fn positions(hir: &FileHir, body: BodyId) -> Vec<TextSize> {
    let mut found = Vec::new();
    let range = hir.body(body).range;
    found.push(range.start());
    found.push(range.end());
    crate::walk::visit_every_statement(hir, hir.body(body).statements, &mut |hir, stmt| {
        let range = hir.stmt(stmt).range();
        found.push(range.start());
        found.push(range.end());
    });
    visit_statements(hir, hir.body(body).statements, &mut |hir, expr| {
        let range = hir.expr(expr).range();
        found.push(range.start());
        found.push(range.end());
    });
    let mut all = Vec::new();
    for at in found {
        all.push(at);
        all.push(at + TextSize::from(1));
        if u32::from(at) > 0 {
            all.push(at - TextSize::from(1));
        }
    }
    all.sort();
    all.dedup();
    all
}

/// What a reader that can look a callee up would answer: `abort()` is declared `: never` in the
/// fixture that calls it, and nothing else is.
fn abort_never_returns(hir: &FileHir, expr: ExprId) -> bool {
    matches!(
        hir.expr(expr),
        ExprData::FunctionCall { name, .. } if hir.name_ref(*name).path.as_str() == "abort"
    )
}

#[test]
fn the_facts_answer_what_the_walks_answered_for_every_write_at_every_position() {
    // Both oracles, because the facts keep the `: never` question for lookup time and the two
    // answers exercise different arms: with the pessimistic one every kept question falls
    // through, with the other a call decides whether a branch's writes are dropped.
    let oracles: [(&str, NeverReturns<'_>); 2] = [
        ("nothing never returns", &|_, _| false),
        ("abort never returns", &abort_never_returns),
    ];
    for (which, source) in FIXTURES.iter().enumerate() {
        let hir = lowered(source);
        let facts = BodyFacts::of(&hir);
        let mut bodies_seen = 0;
        for (body, data) in hir.bodies() {
            bodies_seen += 1;
            for offset in positions(&hir, body) {
                for (oracle, never) in oracles {
                    // The oracle: every assignment the walk hands over, in its order, with its
                    // reach.
                    let mut expected: Vec<(ExprId, Reach)> = Vec::new();
                    visit_reaching_expressions(
                        &hir,
                        data.statements,
                        offset,
                        never,
                        &mut |hir, expr, reach| {
                            if matches!(hir.expr(expr), ExprData::Assign { .. }) {
                                expected.push((expr, reach));
                            }
                        },
                    );
                    let actual: Vec<(ExprId, Reach)> = facts
                        .writes(body)
                        .iter()
                        .filter_map(|site| {
                            facts
                                .reach_of(&hir, site, offset, never)
                                .map(|reach| (site.expr, reach))
                        })
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "fixture {which}, body {body:?}, offset {offset:?}, {oracle}: reaching writes differ"
                    );
                }

                let mut expected_ifs: Vec<(ExprId, bool)> = Vec::new();
                visit_guarding_ifs(
                    &hir,
                    data.statements,
                    offset,
                    &mut |_, condition, _, positive| {
                        expected_ifs.push((condition, positive));
                    },
                );
                let actual_ifs: Vec<(ExprId, bool)> = facts
                    .if_guards_at(body, offset)
                    .map(|guard| (guard.condition, guard.positive))
                    .collect();
                assert_eq!(
                    actual_ifs, expected_ifs,
                    "fixture {which}, body {body:?}, offset {offset:?}: if guards differ"
                );

                let mut expected_operands: Vec<(ExprId, bool)> = Vec::new();
                visit_guarding_operands(
                    &hir,
                    data.statements,
                    offset,
                    &mut |_, guard, positive| {
                        expected_operands.push((guard, positive));
                    },
                );
                let actual_operands: Vec<(ExprId, bool)> = facts
                    .operand_guards_at(body, offset)
                    .map(|guard| (guard.condition, guard.positive))
                    .collect();
                assert_eq!(
                    actual_operands, expected_operands,
                    "fixture {which}, body {body:?}, offset {offset:?}: operand guards differ"
                );
            }
        }
        assert!(bodies_seen > 0, "fixture {which} lowered no body");
    }
}
