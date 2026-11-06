//! The literal a call was made with, as part of the path.
//!
//! For a handful of methods the first argument *is* the type: a container asked for one service id
//! holds a different class than the same container asked for another. This layer does not know which
//! methods those are -- it carries the id so the layer that does can use it.

use expect_test::expect;

use super::check;

#[test]
fn a_string_first_argument_travels_with_the_step() {
    check(
        r#"<?php
class Caller
{
    public function run(): void
    {
        $manager = $this->container->get('app.manager');
        $0
    }
}
"#,
        "manager",
        expect!["$this->$container->get(@0 'app.manager')"],
    );
}

#[test]
fn a_static_call_carries_one_too() {
    check(
        r#"<?php
class Caller
{
    public function run(): void
    {
        $manager = \Drupal::service('app.manager');
        $0
    }
}
"#,
        "manager",
        expect!["Drupal->service(@0 'app.manager')"],
    );
}

/// An escape is resolved, because what the container was asked for is the value and not the spelling.
#[test]
fn an_escape_is_resolved_to_the_value_it_means() {
    check(
        r#"<?php
class Caller
{
    public function run(): void
    {
        $thing = $this->container->get("a\\b");
        $0
    }
}
"#,
        "thing",
        expect![[r#"$this->$container->get(@0 'a\b')"#]],
    );
}

/// A named argument carries the name it filled rather than its position, which for one of them is
/// the one thing that says nothing: which parameter it fills takes the callee's signature to decide,
/// and that is the reader's job.
#[test]
fn a_named_argument_carries_the_name_it_filled() {
    check(
        r#"<?php
class Caller
{
    public function run(): void
    {
        $thing = $this->container->get(id: 'app.manager');
        $0
    }
}
"#,
        "thing",
        expect![[r#"$this->$container->get(id: 'app.manager')"#]],
    );
}

/// Anything whose value is not written at the call carries nothing, rather than carrying a guess.
#[test]
fn a_variable_and_a_spread_carry_nothing() {
    for (label, call) in [
        ("a variable", "$this->container->get($id)"),
        ("a spread", "$this->container->get(...$args)"),
        ("no argument at all", "$this->container->get()"),
    ] {
        let source = format!(
            r#"<?php
class Caller
{{
    public function run(): void
    {{
        $thing = {call};
        $0
    }}
}}
"#
        );
        let rendered = super::rendered(&source, "thing");
        assert_eq!(
            rendered, "$this->$container->get()",
            "{label} must carry no literal"
        );
    }
}

/// A number and a global constant carry, because a conditional return type tests one.
///
/// `pathinfo($path, PATHINFO_EXTENSION)` hands back a string where the same call without the flag
/// hands back an array, and the stub says so with `($flags is 15 ? array : string)` -- so the flag
/// has to reach the reader. The constant carries its *name*: what it stands for is in whichever
/// file declares it, and lowering reads one file.
#[test]
fn a_number_and_a_constant_carry_what_a_flag_is_written_as() {
    check(
        r#"<?php
class Caller
{
    public function run(): void
    {
        $thing = $this->container->get(7);
        $0
    }
}
"#,
        "thing",
        expect![[r#"$this->$container->get(@0 '7')"#]],
    );
    check(
        r#"<?php
class Caller
{
    public function run(): void
    {
        $thing = $this->container->get(PATHINFO_EXTENSION);
        $0
    }
}
"#,
        "thing",
        expect![[r#"$this->$container->get(@0 'PATHINFO_EXTENSION')"#]],
    );
}

#[test]
fn a_literal_carries_where_it_was_written() {
    // Which argument decides is the callee's business: a catalog contract means the first, and a
    // `class-string<T>` parameter can be anywhere. So the path records the position and the reader
    // compares it -- see `forwarded_catalog` for the first half of that.
    check(
        r#"<?php
class Caller
{
    public function run($id): void
    {
        $thing = $this->container->get($id, 'app.manager');
        $th$0ing;
    }
}
"#,
        "thing",
        expect!["$this->$container->get(@1 'app.manager')"],
    );
}

/// And an interpolated one travels too, which is what tells two calls apart.
///
/// The shape it was found by: `$wpdb->get_var($wpdb->prepare('SHOW TABLES LIKE %s'))` guarding
/// `$wpdb->get_var("SELECT domain FROM $wpdb->site ...")` two lines below. Until the skeleton was
/// carried, an interpolated argument was *no* argument, so both steps rendered `->get_var()` and
/// compared equal -- one slot, and the guard on the first narrowed the `null` out of the second. The
/// two calls ask the database different questions, and PHPStan reports the same return.
#[test]
fn an_interpolated_first_argument_travels_as_a_skeleton() {
    check(
        r#"<?php
class Caller
{
    public function run(string $table): void
    {
        $row = $this->db->query("SELECT domain FROM $table LIMIT 1");
        $0
    }
}
"#,
        "row",
        expect![[r#"$this->$db->query(@0 "SELECT domain FROM ${} LIMIT 1")"#]],
    );
}

/// Two calls to one method with different skeletons are two values, rendered side by side.
///
/// Both halves, because what this is about is a *comparison*: the paths have to differ, and a
/// snapshot of one of them says nothing about whether they do.
#[test]
fn two_calls_with_different_skeletons_are_two_values() {
    const SOURCE: &str = r#"<?php
class Caller
{
    public function run(string $table): void
    {
        $domain = $this->db->query("SELECT domain FROM $table LIMIT 1");
        $name = $this->db->query("SELECT name FROM $table LIMIT 1");
        $0
    }
}
"#;

    check(
        SOURCE,
        "domain",
        expect![[r#"$this->$db->query(@0 "SELECT domain FROM ${} LIMIT 1")"#]],
    );
    check(
        SOURCE,
        "name",
        expect![[r#"$this->$db->query(@0 "SELECT name FROM ${} LIMIT 1")"#]],
    );
}
