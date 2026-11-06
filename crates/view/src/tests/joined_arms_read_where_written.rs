//! Each arm of a join resolved at its own position, not at the join's.
//!
//! A ternary and a `match` *select* an arm by a condition, so the condition holds inside the arm it
//! chose. Reading every arm at one offset ignores that and unions the value the condition was
//! written to split -- which is invisible for as long as one arm resolves to nothing, and becomes a
//! false positive the moment inference improves upstream:
//!
//! ```php
//! /** @param array<int, string|array<int, string>> $validations */
//! $methodName = is_array($validationMethod) ? $validationMethod[1] : $validationMethod;
//! ```
//!
//! That line is production code, and read at the join it types as `string|string|array<int,
//! string>` -- the array put back by the else arm, which `is_array` had just ruled out, beside a
//! `string` counted twice. Read where each arm is written it is a `string`.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use super::{Query, check_view};
use crate::IndexView;

/// Every argument in `app.php` whose type the parameter cannot take, with the type as reported.
///
/// The rendered type and not merely the count, because the type *is* the finding here: the shape
/// this file is about produced a union with an operand nobody wrote, and a count would read the
/// same whether that operand was there or not.
fn check_arguments(fixture: &str, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let found = view
            .incompatible_arguments()
            .iter()
            .map(|argument| {
                format!(
                    "{} ${} takes {}, given {}\n",
                    world.render_range(file, argument.range),
                    argument.parameter,
                    argument.declared,
                    argument.given
                )
            })
            .collect::<String>();

        match found.is_empty() {
            true => "<nothing incompatible>\n".to_owned(),
            false => found,
        }
    });

    expect.assert_eq(&rendered);
}

/// Two classes sharing one member, so a union of them can be told apart from either alone.
const CLASSES: &str = r#"
//- /src/classes.php
<?php
namespace App;

class Alpha
{
    public function shared(): void {}
    public function onlyAlpha(): void {}
}

class Beta
{
    public function shared(): void {}
}
"#;

#[test]
fn the_arm_a_predicate_did_not_choose_is_not_put_back() {
    // The production shape, structurally verbatim, twice.
    //
    // `validateField` is the line as written: `$validationMethod[1]` needs the `is_array` to make
    // the receiver a single array before it can be indexed at all, so read at the join it answered
    // nothing and the whole join with it. That silence is what made the defect invisible.
    //
    // `named` is the same join with an arm that resolves without it, which is where the defect was
    // first seen: read at the join it reported `string|string|array<int, string>` -- the array the
    // condition had just ruled out, beside a `string` counted twice.
    //
    // `$validationMethod` passed on its own is the control. It really is both, so it stays a
    // finding, and the two lines above passing is not the harness declining to answer.
    check_arguments(
        r#"
//- /app.php
<?php
namespace App;

class Validator
{
    /**
     * @param array<int, string|array<int, string>> $validations
     */
    public function validateField(array $validations): void
    {
        foreach ($validations as $validationMethod) {
            $methodName = is_array($validationMethod) ? $validationMethod[1] : $validationMethod;
            $this->fail($methodName);
            $this->fail($validationMethod);
        }
    }

    /**
     * @param array<int, string|array<int, string>> $validations
     */
    public function named(array $validations): void
    {
        foreach ($validations as $validationMethod) {
            $label = is_array($validationMethod) ? 'a list' : $validationMethod;
            $this->fail($label);
        }
    }

    private function fail(string $name): void {}
}
"#,
        expect![[r#"
            14:25-14:42 $name takes string, given string|array<int, string>
        "#]],
    );
}

#[test]
fn an_arm_keeps_the_class_the_condition_proved_it_had() {
    // The then side. `$v instanceof Alpha ? $v : new Alpha()` is one class either way, and read at
    // the join the first arm is the whole declaration again -- so the union was `Alpha|Beta|Alpha`
    // and `onlyAlpha` was unreachable through a value that is always an `Alpha`.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
namespace App;

class Uses
{{
    /**
     * @param Alpha|Beta $v
     */
    public function run($v): void
    {{
        $x = $v instanceof Alpha ? $v : new Alpha();
        $x->onlyA@here@lpha();
    }}
}}
"#
        ),
        &[Query::goto("the arm the guard chose", "here")],
        expect![[r#"
            the arm the guard chose
            - method App\Alpha::onlyAlpha @ src/classes.php 7:21-7:30
        "#]],
    );
}

#[test]
fn the_else_arm_keeps_what_the_negated_condition_left() {
    // The other side, and the one the production shape turns on: the else arm is reached only where
    // the condition was false, so `is_string($v)` leaves an `Alpha` there.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
namespace App;

class Uses
{{
    /**
     * @param Alpha|string $v
     */
    public function run($v): void
    {{
        $x = is_string($v) ? new Alpha() : $v;
        $x->onlyA@here@lpha();
    }}
}}
"#
        ),
        &[Query::goto("the arm the negation left", "here")],
        expect![[r#"
            the arm the negation left
            - method App\Alpha::onlyAlpha @ src/classes.php 7:21-7:30
        "#]],
    );
}

#[test]
fn a_join_whose_arms_disagree_is_still_a_union() {
    // The direction that would be lost by collapsing to whichever arm came first. Here the
    // condition genuinely splits the value in two, so only what both classes declare is reachable
    // and what one of them declares alone is not.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
namespace App;

class Uses
{{
    /**
     * @param Alpha|Beta $v
     */
    public function run($v): void
    {{
        $x = $v instanceof Alpha ? $v : $v;
        $x->sha@both@red();
        $x->onlyA@one@lpha();
    }}
}}
"#
        ),
        &[
            Query::goto("declared by both arms", "both"),
            Query::goto("declared by one arm", "one"),
        ],
        expect![[r#"
            declared by both arms
            - method App\Alpha::shared @ src/classes.php 6:21-6:27
            declared by one arm
            - method App\Alpha::onlyAlpha @ src/classes.php 7:21-7:30
        "#]],
    );
}

#[test]
fn two_arms_that_answer_the_same_scalar_are_that_scalar() {
    // Reading each arm where it was written makes the arms *agree* far more often than they used
    // to, and the agreement has to be spelled as one type. `string|string` reads in a message as
    // though the code offered a choice, and it was half of what the production shape reported.
    check_arguments(
        r#"
//- /app.php
<?php
namespace App;

class Alpha {}

class Uses
{
    /**
     * @param Alpha|string $v
     */
    public function run($v, bool $c): void
    {
        $name = $c ? 'literal' : (string) $v;
        $this->withAlpha($name);
    }

    private function withAlpha(Alpha $a): void {}
}
"#,
        expect![[r#"
            14:26-14:31 $a takes \App\Alpha, given string
        "#]],
    );
}
