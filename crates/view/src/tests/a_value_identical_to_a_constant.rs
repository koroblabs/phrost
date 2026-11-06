//! A strict identity with a class constant takes off whatever sentinel the constant cannot be.
//!
//! `$status === Status::Draft` leaves a `Status`, and `$order?->status === Status::Draft` leaves the
//! order there. Which constants qualify is the declaration's question: an enum case and a `string`
//! are never null, `const NONE = null` is, and a `mixed` one says nothing -- which its sentinel
//! reach reports as "impossible" so that checks stay quiet about it, and which a guard must not
//! read as proof.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn what_a_constant_cannot_be_the_value_identical_to_it_is_not() {
    check_view(
        r#"
//- /src/Run.php
<?php
namespace App;

enum Status: string
{
    case Draft = 'd';
}

final class K
{
    public const NONE = null;
    public const mixed ANY = null;
    public const ?string MAYBE = 'x';
    public const string NAME = 'x';
}

final class Order
{
    public Status $status = Status::Draft;
}

final class Run
{
    public function run(?Status $s, ?string $a, ?string $b, ?string $c, string|false|null $e, ?Order $r): void
    {
        if ($s === Status::Draft) {
            $o@enum@ne = $s;
        }
        if ($a === K::NONE) {
            $t@none@wo = $a;
        }
        if ($b === K::ANY) {
            $t@any@hree = $b;
        }
        if ($c === K::MAYBE) {
            $f@maybe@our = $c;
        }
        if ($e === K::NAME) {
            $f@named@ive = $e;
        }
        if ($r?->status === Status::Draft) {
            $s@receiver@ix = $r;
        }
    }
}
"#,
        &[
            Query::variable_type("an enum case", "enum"),
            Query::variable_type("a constant that is null", "none"),
            Query::variable_type("a mixed constant", "any"),
            Query::variable_type("a nullable constant", "maybe"),
            Query::variable_type("a string constant takes both sentinels", "named"),
            Query::variable_type("the receiver of a chain that matched", "receiver"),
        ],
        expect![[r#"
            an enum case
            - Status
            a constant that is null
            - ?string
            a mixed constant
            - ?string
            a nullable constant
            - ?string
            a string constant takes both sentinels
            - string
            the receiver of a chain that matched
            - Order
        "#]],
    );
}
