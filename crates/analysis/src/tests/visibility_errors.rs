//! A member reached from where PHP will not allow it.
//!
//! The rule is about *classes* and not objects, which is the part easiest to get backwards: inside
//! `Base`, another `Base` instance's private members are reachable. Every silence below is a case
//! where reporting would be wrong, and there are more of those than there are reports.

use expect_test::expect;

use super::diagnostics::check;

const HIERARCHY: &str = r#"
//- /app.php
<?php
class Base
{
    private string $secret = '';

    protected string $shared = '';

    public string $open = '';

    private function hidden(): void {}
}

class Outsider
{
    public function run(Base $base): void
    {
        echo $base->open;
        echo $base->shared;
        echo $base->secret;
        $base->hidden();
    }
}
"#;

#[test]
fn a_private_or_protected_member_reached_from_outside_is_reported() {
    check(
        HIERARCHY,
        expect![[r#"
            18:21-18:27 error [inaccessible-member] Cannot access protected property Base::$shared
            19:21-19:27 error [inaccessible-member] Cannot access private property Base::$secret
            20:16-20:22 error [inaccessible-member] Cannot access private method Base::hidden()
        "#]],
    );
}

/// PHP's actual rule: a class reaches the private members of *any* instance of itself, not only of
/// `$this`. Reporting `$sibling->secret` inside the class that declares it is the classic mistake.
#[test]
fn a_sibling_instance_of_the_same_class_reaches_its_private_members() {
    check(
        r#"
//- /app.php
<?php
class Base
{
    private string $secret = '';

    private function hidden(): void {}

    public function run(Base $sibling): void
    {
        echo $sibling->secret;
        $sibling->hidden();
    }
}
"#,
        expect![""],
    );
}

/// A subclass reaches protected, both ways: `Base` reaches a `Child` instance's protected members too.
#[test]
fn a_subclass_reaches_protected() {
    check(
        r#"
//- /app.php
<?php
class Base
{
    protected string $shared = '';
}

class Child extends Base
{
    public function run(): void
    {
        echo $this->shared;
    }
}
"#,
        expect![""],
    );
}

/// The exemption that keeps this usable on framework code: a class with `__get` answers a name it
/// cannot reach directly, so an inaccessible member there is a call to the magic method.
#[test]
fn a_class_that_answers_unknown_names_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
class Magic
{
    private string $secret = '';

    public function __get(string $name): mixed
    {
        return NULL;
    }
}

class Outsider
{
    public function run(Magic $magic): void
    {
        echo $magic->secret;
    }
}
"#,
        expect![""],
    );
}
