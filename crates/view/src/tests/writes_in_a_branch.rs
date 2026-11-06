//! What a variable holds where a write to it is in another branch.
//!
//! The body decides which writes reach a position; this is what a feature sees once they have been
//! resolved. Two answers matter and they pull opposite ways, which is why both are pinned here:
//! a write the position is not on the path of must not be read as the value, and a write that
//! *may* have run must not throw away the value either -- `if ($c) { $v = new Alpha(); } $v->` is
//! how PHP is written and the members of Alpha are what somebody typing there is asking for.

use expect_test::expect;

use super::{Query, check_view};

/// Two classes sharing one member, so a union can be told from either operand alone.
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
fn the_other_branch_of_an_if_keeps_the_declared_type() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v, bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
    }} else {{
        $v@here@;
    }}
}}
"#
        ),
        &[Query::variable_class("in the else branch", "here", "v")],
        expect![[r#"
            in the else branch
            - Beta
        "#]],
    );
}

#[test]
fn a_later_if_sees_a_union_of_the_write_and_what_was_there() {
    // Two separate `if`s: nothing structural says the first did not run, so the value is one or the
    // other and only what both declare can be reached through it.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v, bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
    }}
    if (!$c) {{
        $v->sh@shared@ared();
    }}
}}
"#
        ),
        &[Query::cursor("declared by both", "shared")],
        expect![[r#"
            declared by both
            - member access Alpha::shared [method]
        "#]],
    );
}

/// The other half of the union, and the half that proves it is one.
///
/// Both operands are still there: the write in the earlier `if` *may* have run, and if it did not
/// then `$v` is the parameter. So the type is `Alpha|Beta` and each operand's own member resolves to
/// that operand -- which is the evidence, and it is why the type is asked for here as well. Before
/// the join existed, the write was simply the value and `onlyBeta` would answer nothing; if the join
/// collapsed to a common ancestor, neither would.
#[test]
fn a_later_if_holds_both_operands_and_each_ones_own_member() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v, bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
    }}
    if (!$c) {{
        $w@which@ = $v;
        $v->on@alpha@lyAlpha();
        $v->on@beta@lyBeta();
    }}
}}
"#
        ),
        &[
            Query::variable_type("both operands", "which"),
            Query::cursor("declared by the write", "alpha"),
            Query::cursor("declared by the parameter", "beta"),
        ],
        expect![[r#"
            both operands
            - Alpha|Beta
            declared by the write
            - member access Alpha::onlyAlpha [method]
            declared by the parameter
            - member access Beta::onlyBeta [method]
        "#]],
    );
}

#[test]
fn a_lone_write_in_a_branch_is_still_the_whole_answer() {
    // The capability the join must not cost. Nothing else ever writes `$v`, so there is no second
    // operand and the branch's own value stands -- including the member only it declares.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
    }}
    $v->on@here@lyAlpha();
}}
"#
        ),
        &[Query::cursor("past a lone conditional write", "here")],
        expect![[r#"
            past a lone conditional write
            - member access Alpha::onlyAlpha [method]
        "#]],
    );
}

#[test]
fn a_write_on_the_way_is_the_value() {
    // The control. Nothing branches, so the write replaces the parameter outright and the member
    // only the written class declares resolves.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v): void
{{
    $v = new Alpha();
    $v->on@here@lyAlpha();
}}
"#
        ),
        &[Query::cursor("after a straight-line write", "here")],
        expect![[r#"
            after a straight-line write
            - member access Alpha::onlyAlpha [method]
        "#]],
    );
}

#[test]
fn another_match_arm_keeps_the_declared_type() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v, string $k): void
{{
    match ($k) {{
        'a' => $v = new Alpha(),
        'b' => $v->on@here@lyBeta(),
        default => null,
    }};
}}
"#
        ),
        &[Query::cursor("in a sibling match arm", "here")],
        expect![[r#"
            in a sibling match arm
            - member access Beta::onlyBeta [method]
        "#]],
    );
}

#[test]
fn a_case_that_broke_keeps_the_declared_type_in_the_next() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v, string $k): void
{{
    switch ($k) {{
        case 'a':
            $v = new Alpha();
            break;

        case 'b':
            $v->on@here@lyBeta();
            break;
    }}
}}
"#
        ),
        &[Query::cursor("in a later switch arm", "here")],
        expect![[r#"
            in a later switch arm
            - member access Beta::onlyBeta [method]
        "#]],
    );
}

#[test]
fn a_branch_that_returns_leaves_the_declared_type_behind_it() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(Beta $v, bool $c): void
{{
    if ($c) {{
        $v = new Alpha();
        return;
    }}
    $v->on@here@lyBeta();
}}
"#
        ),
        &[Query::cursor("past a branch that returns", "here")],
        expect![[r#"
            past a branch that returns
            - member access Beta::onlyBeta [method]
        "#]],
    );
}

#[test]
fn a_branch_entered_because_the_name_was_null_leaves_no_null_behind() {
    // `if (!$v) { $v = new Alpha(); }` writes on the one path where the value was null, so the path
    // that skipped it is the path where it was not. Rejoining the declaration whole would make
    // every use below read as possibly null, which is the opposite of what the branch is for.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(?Alpha $v): void
{{
    if (!$v) {{
        $v = new Alpha();
    }}
    $v->on@here@lyAlpha();
}}
"#
        ),
        &[Query::cursor("past a null-filling branch", "here")],
        expect![[r#"
            past a null-filling branch
            - member access Alpha::onlyAlpha [method]
        "#]],
    );
}

#[test]
fn a_branch_entered_because_the_name_was_not_of_a_class_leaves_that_class_behind() {
    // The commoner half of the same idiom in typed code, and the one a bare `object` parameter
    // makes visible: joining the hint whole would union `Alpha` with something that declares
    // nothing, and the member would resolve to no class at all.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
function run(object $v): void
{{
    if (!$v instanceof Alpha) {{
        $v = new Alpha();
    }}
    $v->on@here@lyAlpha();
}}
"#
        ),
        &[Query::cursor("past a class-filling branch", "here")],
        expect![[r#"
            past a class-filling branch
            - member access Alpha::onlyAlpha [method]
        "#]],
    );
}
