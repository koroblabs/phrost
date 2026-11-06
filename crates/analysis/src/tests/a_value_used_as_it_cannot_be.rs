//! Invoking and cloning a value whose kind does not allow it.
//!
//! `Value of type int is not callable` and `clone(): Argument #1 ($object) must be of type object`.
//! Both stop the line.
//!
//! Which kinds was run against PHP 8.5.10 and not reasoned, and two of the answers are why this
//! cannot simply be "not an object": a **string** is callable -- `$f = 'strlen'; $f('ab')` is how
//! PHP has always written a callback -- and an **array** is callable too, because
//! `[$object, 'method']` is the other spelling. `clone` is stricter and takes an object alone.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_kind_that_cannot_be_invoked_or_cloned_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Uses
{
    public function run(int $n, float $f, bool $b, ?object $maybe): void
    {
        $n();
        $f();
        $b();
        $copy = clone $n;
        echo $copy;
    }
}
"#,
        expect![[r#"
            8:9-8:11 error [impossible-use] a value of type int is not callable
            9:9-9:11 error [impossible-use] a value of type float is not callable
            10:9-10:11 error [impossible-use] a value of type bool is not callable
            11:23-11:25 error [impossible-use] clone needs an object, and this is int
        "#]],
    );
}

/// What PHP accepts: a string and an array are callables, and an object clones.
#[test]
fn a_callable_string_or_array_and_an_object_clone_are_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Row {}

class Uses
{
    /**
     * @param callable $c
     * @param array{0: Row, 1: string} $pair
     */
    public function run(string $name, array $pair, Row $row, $c): void
    {
        $name();
        $pair();
        $c();
        $copy = clone $row;
        echo $copy === null;
    }
}
"#,
        expect![""],
    );
}

/// A class reached through a value needs an object or the class's *name*, which is why a `callable`
/// is not reported: it may be either, and Symfony reaches `$callable::class` three times.
#[test]
fn a_class_name_that_is_neither_an_object_nor_a_string() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Uses
{
    public function run(int $n, string $name, callable $c, object $o): void
    {
        echo $n::LABEL;
        $n::make();
        echo $name::LABEL;
        echo $c::class;
        echo $o::class;
    }
}
"#,
        expect![[r#"
            8:14-8:16 error [impossible-use] a class name has to be an object or a string, and this is int
            9:9-9:11 error [impossible-use] a class name has to be an object or a string, and this is int
        "#]],
    );
}

/// And a string read at an offset PHP cannot take as a position.
#[test]
fn a_string_read_at_a_word_offset() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Uses
{
    public function run(string $s, int $at): void
    {
        echo $s['key'];
        echo $s['1'];
        echo $s[0];
        echo $s[$at];
    }
}
"#,
        expect![[r#"
            8:14-8:23 error [bad-string-offset] 'key' is not a position in a string, and PHP refuses the offset
        "#]],
    );
}
