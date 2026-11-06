//! Every assignment in a file, and what this layer can say about its two ends.
//!
//! The projection a type check for assignments needs. What is asserted is mostly what it *declines*
//! to name: a target it cannot name, or a value it cannot path, has to come out as nothing rather
//! than as a guess -- a wrong claim here becomes a wrong "expects int, string given" on somebody's
//! property.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use crate::{AssignmentTarget, assignment_sites};

/// Every assignment site, one per line, as target then value.
fn check(source: &str, expect: Expect) {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    let hir = &lowered.hir;

    let mut out = String::new();
    for site in assignment_sites(hir) {
        let target = match &site.target {
            AssignmentTarget::Variable(name) => format!("${name}"),
            AssignmentTarget::Property { name, .. } => format!("<value>->{name}"),
            AssignmentTarget::StaticProperty { name, .. } => format!("<class>::${name}"),
        };
        let _ = writeln!(
            out,
            "{target} = {}{}",
            match site.value {
                Some(_) => "<path>",
                None => "<no path>",
            },
            match site.annotation {
                Some(_) => "  [annotated]",
                None => "",
            }
        );
    }
    if out.is_empty() {
        out.push_str("<no assignments>\n");
    }

    expect.assert_eq(&out);
}

#[test]
fn a_property_target_is_named_where_the_older_walk_saw_nothing() {
    // The gap this exists to close: the coverage walk matches only `ExprData::Variable` targets and
    // returns on anything else, so a typed property was never reachable from above.
    check(
        r#"<?php
class Invoice
{
    public int $total = 0;

    public function set(Invoice $other): void
    {
        $this->total = 1;
        $other->total = 2;
        $local = 3;
    }
}
"#,
        expect![[r#"
            <value>->total = <path>
            <value>->total = <path>
            $local = <path>
        "#]],
    );
}

#[test]
fn a_static_property_is_named_too() {
    check(
        r#"<?php
class Counter
{
    public static int $seen = 0;

    public static function bump(): void
    {
        self::$seen = 1;
        Counter::$seen = 2;
    }
}
"#,
        expect![[r#"
            <class>::$seen = <path>
            <class>::$seen = <path>
        "#]],
    );
}

#[test]
fn an_annotation_is_kept_beside_the_value_rather_than_replacing_it() {
    // The half that made the check impossible before. For *inference* the annotation is the better
    // answer and `path_of` substitutes it, which is right there and wrong here: a check needs the
    // annotation as what is expected and the value as what arrived, and collapsing the two makes
    // every annotated assignment agree with itself.
    check(
        r#"<?php
function go(): void
{
    /** @var \App\Repo $repo */
    $repo = make();
    $plain = make();
}
"#,
        expect![[r#"
            $repo = <path>  [annotated]
            $plain = <path>
        "#]],
    );
}

#[test]
fn a_compound_assignment_is_not_an_assignment() {
    // `+=` and `.=` produce what the operator decides, not what the right-hand side holds. Lowering
    // already refuses them; this is the test that says so out loud.
    check(
        r#"<?php
function go(int $n, string $s): void
{
    $total = 0;
    $total += $n;
    $label = '';
    $label .= $s;
}
"#,
        expect![[r#"
            $total = <path>
            $label = <path>
        "#]],
    );
}

#[test]
fn a_null_coalescing_assignment_is_an_assignment() {
    // `??=` is the one compound assignment whose result the right-hand side does describe: the
    // target keeps what it had unless that was null, so the value is the union of the two and the
    // name is bound here exactly as `=` binds it. Every other projection -- read-before-write,
    // writes in a branch, unused variables -- wants that same answer.
    check(
        r#"<?php
function go(int $n): void
{
    $total ??= $n;
}
"#,
        expect![[r#"
            $total = <path>
        "#]],
    );
}

#[test]
fn a_target_this_layer_cannot_name_is_left_alone() {
    // A computed member names no property, a destructuring assigns several things at once, and an
    // append names no key. Each would have to be guessed at, and a guess here is a wrong finding on
    // somebody's code.
    check(
        r#"<?php
function go(object $obj, array $rows, string $field): void
{
    $obj->$field = 1;
    [$a, $b] = $rows;
    $rows[] = 2;
}
"#,
        expect![[r#"
            <no assignments>
        "#]],
    );
}

#[test]
fn an_assignment_inside_a_closure_still_counts() {
    check(
        r#"<?php
function go(): callable
{
    return function (Invoice $invoice) {
        $invoice->total = 1;
    };
}
"#,
        expect![[r#"
            <value>->total = <path>
        "#]],
    );
}
