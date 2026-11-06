//! `/** @var T $x */` written on a line of its own.
//!
//! The most-used way to type a value in a framework project, because it is what an author writes
//! when the inferred type is too general: a hook receives an `EntityInterface` and the body knows it
//! is a `User`. 243 of them in one real Drupal site.

use expect_test::expect;

use super::check;

#[test]
fn an_annotation_says_what_a_parameter_holds() {
    check(
        r#"<?php
class Hook
{
    public function run(Basic $entity): void
    {
        /** @var Special $entity */
        $ent$0ity;
    }
}
"#,
        "entity",
        expect!["Special"],
    );
}

/// It applies from where it is written and not before, which is what makes it a statement.
#[test]
fn an_annotation_does_not_reach_backwards() {
    check(
        r#"<?php
class Hook
{
    public function run(Basic $entity): void
    {
        $ent$0ity;
        /** @var Special $entity */
        $entity;
    }
}
"#,
        "entity",
        expect!["Basic"],
    );
}

/// PHP has no block scope for a variable, so an annotation written inside a branch still describes
/// the name after it. Which is the same rule an assignment there follows.
#[test]
fn an_annotation_inside_a_branch_still_applies_after_it() {
    check(
        r#"<?php
class Hook
{
    public function run(Basic $entity, bool $flag): void
    {
        if ($flag) {
            /** @var Special $entity */
            $entity;
        }
        $ent$0ity;
    }
}
"#,
        "entity",
        expect!["Special"],
    );
}

/// A later one replaces an earlier one, like every other binding.
#[test]
fn the_last_annotation_before_the_use_is_the_one() {
    check(
        r#"<?php
class Hook
{
    public function run($entity): void
    {
        /** @var Alpha $entity */
        $entity;
        /** @var Beta $entity */
        $ent$0ity;
    }
}
"#,
        "entity",
        expect!["Beta"],
    );
}

/// An annotation names one variable, and says nothing about any other.
#[test]
fn an_annotation_binds_only_the_name_it_writes() {
    check(
        r#"<?php
class Hook
{
    public function run(Basic $entity, Basic $other): void
    {
        /** @var Special $entity */
        $oth$0er;
    }
}
"#,
        "other",
        expect!["Basic"],
    );
}

/// An annotation written directly above an assignment documents *that* assignment, which is how
/// every other tool reads it -- so the two agree rather than compete.
#[test]
fn an_annotation_above_an_assignment_types_it() {
    check(
        r#"<?php
class Hook
{
    public function run(): void
    {
        /** @var Special $entity */
        $entity = new Other();
        $ent$0ity;
    }
}
"#,
        "entity",
        expect!["Special"],
    );
}

/// An assignment further down is what holds, because it happened later.
#[test]
fn an_assignment_after_an_annotation_replaces_it() {
    check(
        r#"<?php
class Hook
{
    public function run($entity): void
    {
        /** @var Special $entity */
        $entity;
        $entity = new Other();
        $ent$0ity;
    }
}
"#,
        "entity",
        expect!["new Other"],
    );
}

/// A nameless `@var` is not an annotation statement: it belongs to whatever is written below it, and
/// the assignment already reads it. Two representations of one tag would be one too many.
#[test]
fn a_nameless_var_stays_with_the_statement_below_it() {
    check(
        r#"<?php
class Hook
{
    public function run(): void
    {
        /** @var Special */
        $entity = $this->find();
        $ent$0ity;
    }
}
"#,
        "entity",
        expect!["Special"],
    );
}

/// `@var` on a loop names the value variable, which the loop consumes itself.
#[test]
fn an_annotation_above_a_loop_types_the_value_variable() {
    check(
        r#"<?php
class Hook
{
    public function run(array $items): void
    {
        /** @var Special $item */
        foreach ($items as $item) {
            $it$0em;
        }
    }
}
"#,
        "item",
        expect!["Special"],
    );
}
