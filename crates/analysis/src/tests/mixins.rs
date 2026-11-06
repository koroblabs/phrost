//! `@mixin` — members reachable through a class that PHP says nothing about.
//!
//! The tag is the only statement anywhere that they exist: at runtime the calls go through `__call`,
//! and no declaration in the class mentions them. Which is why they lose to everything real — a
//! class's own member, an inherited one, a trait's — and why goto still lands in the class that
//! declared them.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_mixin_adds_its_members() {
    check(
        r#"
//- /src/Both.php
<?php
namespace App;

class Helper
{
    public function help(): void {}
}

/** @mixin Helper */
class Service
{
    public function serve(): void {}
}

class Runner
{
    public function run(Service $service): void
    {
        $service->@here@
    }
}
"#,
        &[Query::completion("its own and the mixin's", "here")],
        expect![[r#"
            its own and the mixin's
            - help [method]  : void  writes `();`
            - serve [method]  : void  writes `();`
            - accepting the first leaves `$service->help`
        "#]],
    );
}

#[test]
fn a_mixin_loses_to_the_classs_own_declaration() {
    check(
        r#"
//- /src/Shadow.php
<?php
namespace App;

class Helper
{
    public function run(): string { return ''; }
}

/** @mixin Helper */
class Service
{
    public function run(): int { return 0; }
}

class Runner
{
    public function go(Service $service): void
    {
        $service->run()->@here@
    }
}
"#,
        &[Query::completion("the class's own wins", "here")],
        expect![[r#"
            the class's own wins
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_parameterised_mixin_is_both_a_supertype_and_an_argument_for_one() {
    check(
        r#"
//- /src/Boxed.php
<?php
namespace App;

class Item
{
    public function label(): string { return ''; }
}

/** @template T */
class Bag
{
    /** @return T */
    public function first() {}
}

/** @mixin Bag<Item> */
class Shelf {}

class Runner
{
    public function run(Shelf $shelf): void
    {
        $shelf->first()->@here@
    }
}
"#,
        &[Query::completion("through a parameterised mixin", "here")],
        expect![[r#"
            through a parameterised mixin
            - label [method]  : string  writes `();`
            - accepting the first leaves `$shelf->first()->label`
        "#]],
    );
}

#[test]
fn two_classes_mixing_each_other_in_do_not_hang() {
    // Broken code writes it, and a flattener with no cycle guard hangs the server rather than
    // failing. The answer is an incomplete table, not an empty one.
    check(
        r#"
//- /src/Loop.php
<?php
namespace App;

/** @mixin Second */
class First
{
    public function one(): void {}
}

/** @mixin First */
class Second
{
    public function two(): void {}
}

class Runner
{
    public function run(First $first): void
    {
        $first->@here@
    }
}
"#,
        &[Query::completion("a cycle of mixins", "here")],
        expect![[r#"
            a cycle of mixins
            - one [method]  : void  writes `();`
            - two [method]  : void  writes `();`
            - accepting the first leaves `$first->one`
        "#]],
    );
}

#[test]
fn a_mixin_on_a_trait_is_not_applied() {
    // A trait is not something anyone holds. What is held is the class that uses it, and there the
    // tag is a claim about *that* class's hierarchy — `ProphecyTrait` is `@mixin TestCase` and every
    // class using it already extends TestCase. Applying it here put PHPUnit's largest member list
    // inside the trait, and every composing class copied it to change nothing.
    check(
        r#"
//- /src/Composed.php
<?php
namespace App;

class Helper
{
    public function help(): void {}
}

/** @mixin Helper */
trait Borrows
{
    public function borrowed(): void {}
}

class Service
{
    use Borrows;
}

class Runner
{
    public function run(Service $service): void
    {
        $service->@here@
    }
}
"#,
        &[Query::completion("the trait's tag is not followed", "here")],
        expect![[r#"
            the trait's tag is not followed
            - borrowed [method]  : void  writes `();`
            - accepting the first leaves `$service->borrowed`
        "#]],
    );
}
