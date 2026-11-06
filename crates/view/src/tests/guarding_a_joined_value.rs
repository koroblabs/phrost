//! A guard written after a conditional write has to reach inside the join.
//!
//! Joining is how a write in a branch stops being read as the value everywhere, and it puts a
//! *union* where a declaration used to be. Every guard here works by taking a union apart and
//! keeping the operands that survive -- so an operand that is itself a union answers nothing about
//! its own kind or class, and a positive test drops it without ever looking inside. The visible
//! effect is that a write in front of a guard silently defeats the guard:
//!
//! ```php
//! !$t && $t = time();
//! if (is_string($t)) { takesString($t); }   // $t read as int|string|null, guard powerless
//! ```
//!
//! Two things have to hold for that not to happen, and both are pinned below: a union built from a
//! join is flat, and a null taken off a written `A|B|null` leaves the operands rather than the
//! union it could not rewrite.

use expect_test::expect;

use super::{Query, check_view};

/// Two classes sharing one member, beside a scalar, so a union of them can be told apart.
const CLASSES: &str = r#"
//- /src/classes.php
<?php
class Alpha
{
    public function shared(): void {}
    public function onlyAlpha(): void {}
}
class Beta
{
    public function shared(): void {}
    public function onlyBeta(): void {}
}
"#;

#[test]
fn an_instanceof_reaches_an_operand_of_a_joined_union() {
    // `$v` here is `Alpha` (the write) or `Alpha|Beta` (the declaration, minus the null the branch's
    // own guard ruled out). The `instanceof` has to see three operands and keep the one that is a
    // `Beta`; seeing two, one of them an opaque union, it kept nothing and left the value as it was.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Alpha|Beta|null $v): void
{{
    !$v && $v = new Alpha();
    if ($v instanceof Beta) {{
        $v->on@here@lyBeta();
    }}
}}
"#
        ),
        &[Query::cursor("instanceof over a join", "here")],
        expect![[r#"
            instanceof over a join
            - member access Beta::onlyBeta [method]
        "#]],
    );
}

#[test]
fn a_type_predicate_reaches_an_operand_of_a_joined_union() {
    // The same for the other kind of guard, and the shape a real project writes:
    // `!$t && $t = time();` in front of `if (is_string($t))`. A predicate asks each operand what
    // *sort* of value it is, which a nested union has no answer to.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Alpha|string|null $v): void
{{
    !$v && $v = 'fallback';
    if (is_object($v)) {{
        $v->on@here@lyAlpha();
    }}
}}
"#
        ),
        &[Query::cursor("a predicate over a join", "here")],
        expect![[r#"
            a predicate over a join
            - member access Alpha::onlyAlpha [method]
        "#]],
    );
}

#[test]
fn a_null_guard_over_a_join_still_leaves_both_operands() {
    // The reduction must not go further than the guard did. Ruling out null leaves `Alpha|Beta`,
    // and only what both declare is reachable through it -- so this pins that peeling a written
    // union operand by operand did not quietly collapse it to whichever one came first.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Alpha|Beta|null $v, bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
    }}
    if ($v !== NULL) {{
        $v->sh@shared@ared();
    }}
}}
"#
        ),
        &[Query::cursor("shared by both operands", "shared")],
        expect![[r#"
            shared by both operands
            - member access Alpha::shared [method]
        "#]],
    );
}

/// The other half of the same claim, and the one that fails if the peel collapses the union.
///
/// `onlyBeta` is on one operand, so it resolves *to that operand* -- `Beta`, not the ancestor the
/// union used to be reduced to and not `Alpha`. Naming `Beta` is what makes goto land somewhere real
/// and what stops the checker reporting a call that may well run; and if the peel had collapsed the
/// union to a single operand, this would answer `Alpha` or nothing instead.
#[test]
fn a_null_guard_over_a_join_reaches_a_member_only_one_operand_has() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Alpha|Beta|null $v, bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
    }}
    if ($v !== NULL) {{
        $v->on@here@lyBeta();
    }}
}}
"#
        ),
        &[Query::cursor("declared by one operand", "here")],
        expect![[r#"
            declared by one operand
            - member access Beta::onlyBeta [method]
        "#]],
    );
}
