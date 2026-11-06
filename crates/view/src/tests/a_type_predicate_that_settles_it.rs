//! What a positive type predicate leaves, when the value was not of that kind to begin with.
//!
//! `of_kind` was written as a filter over the alternatives a declaration listed, which is right for
//! `Foo|string` and answers nothing for every other shape -- the same asymmetry between "filter the
//! declaration" and "the guard is the whole answer" that `of_one_of_classes` and `also_of_class`
//! each had to close for a class. Two halves of it, and both reported working code:
//!
//! * **Null passes no predicate.** `is_string($s)` on a `?string` is as much a null check as
//!   `!== null` is, and a `?string` is not a union to `union_operands` while `kind_of_held` reads
//!   through the `?` and finds the kind it wanted -- so the guard filtered nothing and the null it
//!   had ruled out was still there on the next line.
//! * **Nothing of the kind is a contradiction, not an answer.** A `$v` believed to be null, inside
//!   `if (is_array($v))`, was passed on as null -- so the call the guard exists to protect was
//!   reported as being handed one.
//!
//! A kind that says something *beside* what the value is keeps it: `callable` and `iterable` name no
//! family of their own, so `is_callable($handler)` on a class is not a contradiction and answering
//! `callable` would take the members away.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /stubs/standard.php  stub
<?php
function is_string(mixed $value): bool {}
function is_array(mixed $value): bool {}
function is_callable(mixed $value): bool {}

//- /src/Guards.php
<?php
namespace App;

class Handler
{
    public function __invoke(): void {}
}

class Guards
{
    public function nullableOfTheKind(?string $text): void
    {
        if (is_string($text)) {
            $t@kept@ext;
        }
    }

    public function nothingOfTheKind(bool $when): void
    {
        $value = NULL;
        if (is_array($value)) {
            $v@whole@alue;
        }
    }

    public function aUnionKeepsFiltering(int|string|null $mixedUp): void
    {
        if (is_string($mixedUp)) {
            $m@filtered@ixedUp;
        }
    }

    public function beside(?Handler $handler): void
    {
        if (is_callable($handler)) {
            $h@beside@andler;
        }
    }
}
"#;

/// The four shapes together, because the bug is that they answered differently: a predicate that
/// filters a union worked and the same predicate over every other declaration did not.
#[test]
fn a_positive_predicate_takes_the_null_off_and_is_the_answer_where_nothing_was_of_the_kind() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type(
                "a nullable of the kind keeps the kind, without the null",
                "kept",
            ),
            Query::variable_type(
                "nothing of the kind: the guard is the whole answer",
                "whole",
            ),
            Query::variable_type("a union is still filtered to the operand", "filtered"),
            Query::variable_type("a kind beside the class keeps the class", "beside"),
        ],
        expect![[r#"
            a nullable of the kind keeps the kind, without the null
            - string
            nothing of the kind: the guard is the whole answer
            - array
            a union is still filtered to the operand
            - string
            a kind beside the class keeps the class
            - Handler
        "#]],
    );
}
