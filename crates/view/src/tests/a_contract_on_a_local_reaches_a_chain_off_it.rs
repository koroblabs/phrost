//! What a call did to a local reaches a value read off that local, not only the local itself.
//!
//! `held_after_steps` asks the contract question about the *whole* path at the end, and no call
//! before `$thing->name()` was handed `$thing->name()` -- so the bare `$thing` (what goto, hover and
//! every member check resolve as the receiver) answered with the assertion while the chain off it,
//! which is what an assignment holds, answered without. One rule read two ways. The root of a
//! chain is a value of its own now, asked the same question first.

use expect_test::expect;

use super::{Query, check_view};

const ASSERTS: &str = r#"
//- /src/Asserts.php
<?php
namespace App;

class Asserts
{
    /**
     * @template T of object
     * @param class-string<T> $class
     * @phpstan-assert T $actual
     */
    public static function isA(string $class, mixed $actual): void {}
}
"#;

#[test]
fn an_assertion_on_a_local_reaches_a_value_read_off_it() {
    check_view(
        &[
            ASSERTS,
            r#"
//- /src/run.php
<?php
namespace App;

class User { public function name(): string { return ''; } }

function run(mixed $thing): void
{
    Asserts::isA(User::class, $thing);
    $name = $thing->name();
    $n@here@ame;
}
"#,
        ]
        .concat(),
        &[Query::variable_type("read off the asserted local", "here")],
        expect![[r#"
            read off the asserted local
            - string
        "#]],
    );
}

/// An assertion naming the class the value already is says nothing new, and taking it as the answer
/// took away what the value was written with -- `assertInstanceOf(Builder::class, $results)` on a
/// `Builder<User>` made `$results->first()` a `?Model`. Asked of the chain, this would now reach every
/// value read off the local, so it is asked of the local too.
#[test]
fn an_assertion_naming_the_class_the_value_already_is_keeps_its_arguments() {
    check_view(
        &[
            ASSERTS,
            r#"
//- /src/run.php
<?php
namespace App;

class User {}

/** @template T */
class Box
{
    /** @return T */
    public function get() {}
}

/** @param Box<User> $box */
function run(Box $box): void
{
    Asserts::isA(Box::class, $box);
    $b@whole@ox;
    $got = $box->get();
    $g@got@ot;
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("the local", "whole"),
            Query::variable_type("read off it", "got"),
        ],
        expect![[r#"
            the local
            - Box<User>
            read off it
            - User
        "#]],
    );
}
