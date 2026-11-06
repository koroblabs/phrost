//! The class a value names.
//!
//! `Foo::class` is a string at runtime, so nothing in the type layer keeps which class it named.
//! What keeps it is the path: it ends at `Foo` and a `::class` step, which is why this is read
//! syntactically and not from a type.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_variable_holding_a_class_literal_can_be_instantiated() {
    check(
        r#"
//- /src/New.php
<?php
namespace App;

class Foo
{
    public function doSomething(): void {}
}

class Runner
{
    public function run(): void
    {
        $name = Foo::class;
        $made = new $name();
        $made->@member@
    }
}
"#,
        &[Query::completion("instantiated from a variable", "member")],
        expect![[r#"
            instantiated from a variable
            - doSomething [method]  : void  writes `();`
            - accepting the first leaves `$made->doSomething`
        "#]],
    );
}

#[test]
fn a_variable_holding_a_class_literal_can_be_called_statically() {
    check(
        r#"
//- /src/Static.php
<?php
namespace App;

class Foo
{
    public static function make(): self {}

    public function doSomething(): void {}
}

class Runner
{
    public function run(): void
    {
        $name = Foo::class;
        $made = $name::make();
        $made->@member@
    }
}
"#,
        &[Query::completion("called through a variable", "member")],
        expect![[r#"
            called through a variable
            - doSomething [method]  : void  writes `();`
            - make [method]  : self  shown last  writes `();`
            - accepting the first leaves `$made->doSomething`
        "#]],
    );
}

/// A value that holds something else names no class, and saying so is the point: `new $name` where
/// `$name` came from a request is a class nothing here can know.
#[test]
fn a_variable_holding_anything_else_names_no_class() {
    check(
        r#"
//- /src/Unknown.php
<?php
namespace App;

class Foo
{
    public function doSomething(): void {}
}

class Runner
{
    public function run(string $name): void
    {
        $made = new $name();
        $made->@member@
    }
}
"#,
        &[Query::completion("a class nobody named", "member")],
        expect![[r#"
            a class nobody named
            - (nothing offered)
        "#]],
    );
}

/// The value is asked as of where it was written, not where the cursor is: `$f = new $f;` reassigns
/// the very name it reads, and asking at the cursor would answer the read with the write it is part
/// of.
#[test]
fn a_variable_reassigned_from_itself_reads_what_it_held_before() {
    check(
        r#"
//- /src/Reassigned.php
<?php
namespace App;

class Foo
{
    public function doSomething(): void {}
}

class Runner
{
    public function run(): void
    {
        $f = Foo::class;
        $f = new $f();
        $f->@member@
    }
}
"#,
        &[Query::completion("reassigned in place", "member")],
        expect![[r#"
            reassigned in place
            - doSomething [method]  : void  writes `();`
            - accepting the first leaves `$f->doSomething`
        "#]],
    );
}

/// `static::class` means whatever class the call was made on, which for `new $class` is by
/// definition not the one it was written in. Reading it as the enclosing class reported every Drupal
/// plugin's `create()` as instantiating its own abstract base.
#[test]
fn a_late_bound_class_name_names_no_class_here() {
    check(
        r#"
//- /src/Late.php
<?php
namespace App;

abstract class Base
{
    public function baseMethod(): void {}

    public static function make(): static
    {
        $class = static::class;
        return new $class();
    }
}

class Runner
{
    public function run(): void
    {
        $made = Base::make();
        $made->@member@
    }
}
"#,
        &[Query::completion("late bound, so unknown here", "member")],
        expect![[r#"
            late bound, so unknown here
            - baseMethod [method]  : void  writes `();`
            - make [method]  : static  shown last  writes `();`
            - accepting the first leaves `$made->baseMethod`
        "#]],
    );
}
