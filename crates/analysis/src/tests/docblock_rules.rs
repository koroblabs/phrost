//! The documentation rules, which a project has to ask for.
//!
//! Every test here is as much about the silence as about the finding: a rule that fires on a
//! `{@inheritdoc}` or on a `void` method is a rule people switch off in the first hour.

use expect_test::expect;

use super::diagnostics::check_asked_for;

#[test]
fn nothing_is_reported_until_a_project_asks() {
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * Finds a row.
     */
    public function find(int $id, bool $strict): string
    {
        return '';
    }
}
"#,
        &[],
        expect![[r#""#]],
    );
}

#[test]
fn a_parameter_with_no_param_tag_and_a_return_with_no_return_tag() {
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * Finds a row.
     *
     * @param int $id
     *   The identifier.
     */
    public function find(int $id, bool $strict): string
    {
        return '';
    }
}
"#,
        &["doc-missing-tags"],
        expect![[r#"
            10:21-10:25 warning [doc-missing-tags] returns a value and has no @return
            10:40-10:47 warning [doc-missing-tags] $strict has no @param
        "#]],
    );
}

#[test]
fn a_param_naming_a_parameter_that_was_renamed() {
    // A rename that stopped half way. The tag reads as authoritative and documents nothing.
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * @param int $identifier
     *   The identifier.
     */
    public function find(int $id): void {}
}
"#,
        &["doc-param-name"],
        expect![[r#"
            8:21-8:25 warning [doc-param-name] @param $identifier names no parameter of this declaration
        "#]],
    );
}

#[test]
fn an_inheritdoc_ends_both_rules() {
    // The tag means the declaration above documents this one. Demanding the tags again is demanding
    // the copy the tag exists to avoid.
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * {@inheritdoc}
     */
    public function find(int $id, bool $strict): string
    {
        return '';
    }
}
"#,
        &["doc-missing-tags", "doc-param-name"],
        expect![[r#""#]],
    );
}

#[test]
fn a_void_method_needs_no_return_tag() {
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * Deletes a row.
     *
     * @param int $id
     *   The identifier.
     */
    public function delete(int $id): void {}
}
"#,
        &["doc-missing-tags"],
        expect![[r#""#]],
    );
}

#[test]
fn a_return_inside_a_closure_is_the_closures() {
    // Only the body's own statements. A `return` in a callback written inside a method says nothing
    // about what the method gives back.
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * Runs.
     */
    public function run(): void
    {
        $fn = function () {
            return 1;
        };
        $fn();
    }
}
"#,
        &["doc-missing-tags"],
        expect![[r#""#]],
    );
}

#[test]
fn a_declaration_with_no_docblock_at_all_is_left_alone() {
    // A rule about *presence* is a different rule, and mago's `missing-docs` already is one.
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): string
    {
        return '';
    }
}
"#,
        &["doc-missing-tags", "doc-param-name"],
        expect![[r#""#]],
    );
}

#[test]
fn a_variadic_or_by_reference_param_is_the_same_parameter() {
    // `...$rest` and `&$out` are the parameter written with what the call does to it. Reading the
    // sigils as part of the name reported every variadic constructor in a real project.
    check_asked_for(
        r#"
//- /app.php
<?php
class Repo
{
    /**
     * Collects.
     *
     * @param string ...$names
     *   The names.
     * @param int &$count
     *   How many.
     */
    public function collect(string ...$names): void {}
}
"#,
        &["doc-missing-tags", "doc-param-name"],
        expect![[r#"
            12:21-12:28 warning [doc-param-name] @param $count names no parameter of this declaration
        "#]],
    );
}
