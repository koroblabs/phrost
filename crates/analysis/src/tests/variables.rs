//! Completion after a `$`.
//!
//! The one completion whose candidates come from inside a function rather than from the project, and
//! the one where PHP's case sensitivity matters: `$Name` and `$name` are two variables.

use expect_test::expect;

use super::{Query, check};

#[test]
fn parameters_assignments_and_foreach_targets_are_all_offered() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(int $count, $items): void
    {
        $total = 0;
        foreach ($items as $key => $item) {
            $@here@
        }
    }
}
"#,
        &[Query::completion("locals", "here")],
        expect![[r#"
            locals
            - $this [variable]
            - $count [variable]
            - $items [variable]
            - $total [variable]
            - $key [variable]
            - $item [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - ... and the client must ask again
            - accepting the first leaves `$this`
        "#]],
    );
}

#[test]
fn a_typed_prefix_filters_case_sensitively() {
    // PHP variable names are case-sensitive, so offering `$Name` for `$na` would suggest a variable
    // that does not exist.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(): void
    {
        $name = 1;
        $Name = 2;
        $na@here@
    }
}
"#,
        &[Query::completion("filtered", "here")],
        expect![[r#"
            filtered
            - $name [variable]
            - ... and the client must ask again
            - accepting the first leaves `$name`
        "#]],
    );
}

#[test]
fn a_superglobal_prefix_reaches_the_ones_the_language_provides() {
    check(
        r#"
//- /src/script.php
<?php
$_@here@
"#,
        &[Query::completion("superglobals", "here")],
        expect![[r#"
            superglobals
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - ... and the client must ask again
            - accepting the first leaves `$_SERVER`
        "#]],
    );
}

#[test]
fn a_closure_offers_its_own_parameters_and_what_it_captured() {
    // Not the enclosing scope's other locals: they are not in scope inside the closure, and
    // offering one produces code that runs with an undefined variable.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run($outer, $unrelated): void
    {
        $callback = function ($inner) use ($outer) {
            $@here@
        };
    }
}
"#,
        &[Query::completion("in a closure", "here")],
        expect![[r#"
            in a closure
            - $this [variable]
            - $inner [variable]
            - $outer [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - ... and the client must ask again
            - accepting the first leaves `$this`
        "#]],
    );
}

#[test]
fn an_arrow_function_offers_the_enclosing_scope_too() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run($outer): void
    {
        $arrow = fn ($inner) => strlen($@here@);
    }
}
"#,
        &[Query::completion("in an arrow function", "here")],
        expect![[r#"
            in an arrow function
            - $this [variable]
            - $inner [variable]
            - $outer [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - ... and the client must ask again
            - accepting the first leaves `$arrow = fn ($inner) => strlen($this);`
        "#]],
    );
}

#[test]
fn a_variable_assigned_after_the_cursor_is_not_offered() {
    // It does not exist yet at that point. Offering it would suggest reading a variable before it
    // has a value, which is a warning in PHP and a bug in the code.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(): void
    {
        $@here@
        $later = 1;
    }
}
"#,
        &[Query::completion("before the assignment", "here")],
        expect![[r#"
            before the assignment
            - $this [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - ... and the client must ask again
            - accepting the first leaves `$this`
        "#]],
    );
}

#[test]
fn an_arrow_function_with_nothing_after_the_arrow_falls_back_to_the_enclosing_scope() {
    // Pinned, because it is a consequence of where the candidates come from. `fn ($inner) => $` has
    // no expression for the arrow function's body, so the body has no range containing the cursor
    // and the walk stops at the method around it. What comes back is the enclosing scope's names --
    // correct as far as it goes, since an arrow function captures all of them, and missing only its
    // own parameter.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run($outer): void
    {
        $arrow = fn ($inner) => $@here@
    }
}
"#,
        &[Query::completion("nothing after the arrow", "here")],
        expect![[r#"
            nothing after the arrow
            - $this [variable]
            - $outer [variable]
            - $GLOBALS [variable]
            - $_SERVER [variable]
            - $_GET [variable]
            - $_POST [variable]
            - $_FILES [variable]
            - $_COOKIE [variable]
            - $_SESSION [variable]
            - $_REQUEST [variable]
            - $_ENV [variable]
            - ... and the client must ask again
            - accepting the first leaves `$arrow = fn ($inner) => $this`
        "#]],
    );
}

#[test]
fn a_variable_reassigned_from_itself_takes_the_new_type() {
    // `$node = $node->getTranslation($lang)` is written everywhere. Reading the receiver at the
    // asking position found this same assignment and went round until the budget ran out, so the
    // variable had no type at all after it — see `LocalBinding::written_at`.
    check(
        r#"
//- /app.php
<?php
class Bar {
    public function doBar(): void {}
}

class Foo {
    public function doFoo(): void {}
    public function withFoo(): Bar {}
}

function run(): void
{
    $foo = new Foo();
    $foo = $foo->withFoo();
    $@here@foo;
    $foo->$0
}
"#,
        &[
            Query::hover("after reassigning through itself", "here"),
            Query::completion("what it holds now", "0"),
        ],
        expect![[r#"
            after reassigning through itself
            - code: Bar $foo
            - note: assigned
            what it holds now
            - doBar [method]  : void  writes `();`
            - accepting the first leaves `$foo->doBar`
        "#]],
    );
}

#[test]
fn a_chain_of_reassignments_is_followed_to_the_last_one() {
    check(
        r#"
//- /app.php
<?php
class Third {
    public function third(): void {}
}
class Second {
    public function toThird(): Third {}
}
class First {
    public function toSecond(): Second {}
}

function run(): void
{
    $it = new First();
    $it = $it->toSecond();
    $it = $it->toThird();
    $it->$0
}
"#,
        &[Query::completion("three reassignments deep", "0")],
        expect![[r#"
            three reassignments deep
            - third [method]  : void  writes `();`
            - accepting the first leaves `$it->third`
        "#]],
    );
}

/// The most-typed variable in PHP, and the one that was never offered.
///
/// `$this` is bound by the call rather than by a statement, so it is in no scope and the list that
/// reads the scope never had it. Measured before this: typing `$t` inside a method answered
/// nothing at all.
#[test]
fn this_is_offered_inside_a_method_and_not_in_a_static_one() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    private int $count = 0;

    public function run(): void
    {
        $t@here@
    }
}
"#,
        &[Query::completion("inside a method", "here")],
        expect![[r#"
            inside a method
            - $this [variable]
            - ... and the client must ask again
            - accepting the first leaves `$this`
        "#]],
    );
}

#[test]
fn a_static_method_offers_no_this() {
    check(
        r#"
//- /src/Maker.php
<?php
namespace App;

class Maker
{
    public static function make(int $total): void
    {
        $t@here@
    }
}
"#,
        &[Query::completion("inside a static method", "here")],
        expect![[r#"
            inside a static method
            - $total [variable]
            - ... and the client must ask again
            - accepting the first leaves `$total`
        "#]],
    );
}
