//! What an operator produces.
//!
//! Two kinds, and the difference is who decides. `.` builds a string and a comparison answers a
//! bool whatever they were given, so the operator settles it alone. `+` and its neighbours are
//! settled by the operands, so they say nothing until both are known.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_operator_that_decides_alone_states_its_result() {
    check(
        r#"
//- /src/Ops.php
<?php
namespace App;

class Ops
{
    public function run(mixed $a, mixed $b): void
    {
        $text = $a . $b;
        $same = $a === $b;
        $bits = $a | $b;
        $rest = $a % $b;
        $both = $a && $b;
        $kind = $a instanceof Ops;
        $te@one@xt;
        $sa@two@me;
        $bi@three@ts;
        $re@four@st;
        $bo@five@th;
        $ki@six@nd;
    }
}
"#,
        &[
            Query::hover("concatenation", "one"),
            Query::hover("identity", "two"),
            Query::hover("bitwise or", "three"),
            Query::hover("modulo", "four"),
            Query::hover("logical and", "five"),
            Query::hover("instanceof", "six"),
        ],
        expect![[r#"
            concatenation
            - code: string $text
            - note: assigned
            identity
            - code: bool $same
            - note: assigned
            bitwise or
            - code: int $bits
            - note: assigned
            modulo
            - code: int $rest
            - note: assigned
            logical and
            - code: bool $both
            - note: assigned
            instanceof
            - code: bool $kind
            - note: assigned
        "#]],
    );
}

#[test]
fn arithmetic_is_settled_by_the_operands() {
    check(
        r#"
//- /src/Sums.php
<?php
namespace App;

class Sums
{
    public function run(mixed $unknown): void
    {
        $whole = 1 + 2;
        $mixed = 1 + 1.5;
        $power = 2 ** 3;
        $merge = [] + ['a' => 1];
        $guess = $unknown * 2;
        $wh@one@ole;
        $mi@two@xed;
        $po@three@wer;
        $me@four@rge;
        $gu@five@ess;
    }
}
"#,
        &[
            Query::hover("two ints", "one"),
            Query::hover("an int and a float", "two"),
            Query::hover("exponentiation", "three"),
            Query::hover("two arrays", "four"),
            Query::hover("an operand with no type", "five"),
        ],
        expect![[r#"
            two ints
            - code: int $whole
            - note: assigned
            an int and a float
            - code: float $mixed
            - note: assigned
            exponentiation
            - code: int $power
            - note: assigned
            two arrays
            - code: array $merge
            - note: assigned
            an operand with no type
            - code: $guess
            - note: assigned
            - note: no declared type
        "#]],
    );
}

#[test]
fn division_is_the_union_php_leaves_it_as() {
    // `4 / 2` is an int and `5 / 2` is a float, so the type is `int|float`. Naming either half
    // alone would be right about half the code and confidently wrong about the rest; the union is
    // the whole answer, and it is the one every other PHP tool gives.
    check(
        r#"
//- /src/Ratio.php
<?php
namespace App;

class Ratio
{
    public function run(): void
    {
        $part = 4 / 2;
        $pa@here@rt;
    }
}
"#,
        &[Query::hover("a division", "here")],
        expect![[r#"
            a division
            - code: int|float $part
            - note: assigned
        "#]],
    );
}

#[test]
fn an_operand_that_may_be_several_kinds_is_read_kind_by_kind() {
    // Each pair of kinds the two sides may be makes its own result, and PHP's coercions are part of
    // the arithmetic: `false` and `null` are `0`, a bool is `0` or `1`, and a string is whichever
    // number it spells. An array beside anything else is a `TypeError` on one path, so it answers
    // nothing rather than the half that works.
    check(
        r#"
//- /src/Sums.php
<?php
namespace App;

class Sums
{
    public function run(?int $count, int|float $rate, int|false $found, bool $flag, string $digits, array|int $either, int $i): void
    {
        $next = $found + 1;
        $total = $count + $i;
        $scaled = $i * $rate;
        $flagged = $flag + 1;
        $parsed = $digits + 1;
        $broken = $either + 1;
        $ne@one@xt;
        $to@two@tal;
        $sc@three@aled;
        $fl@four@agged;
        $pa@five@rsed;
        $br@six@oken;
    }
}
"#,
        &[
            Query::hover("an int or false", "one"),
            Query::hover("an int or null", "two"),
            Query::hover("an int or a float", "three"),
            Query::hover("a bool", "four"),
            Query::hover("a string", "five"),
            Query::hover("an array or an int", "six"),
        ],
        expect![[r#"
            an int or false
            - code: int $next
            - note: assigned
            an int or null
            - code: int $total
            - note: assigned
            an int or a float
            - code: int|float $scaled
            - note: assigned
            a bool
            - code: int $flagged
            - note: assigned
            a string
            - code: int|float $parsed
            - note: assigned
            an array or an int
            - code: $broken
            - note: assigned
            - note: no declared type
        "#]],
    );
}

#[test]
fn a_step_is_not_an_addition() {
    // `++$flag` leaves a bool as it was and `++$name` counts letters, so neither is `+ 1` -- only a
    // number stepped is the number it was.
    check(
        r#"
//- /src/Steps.php
<?php
namespace App;

class Steps
{
    public function run(bool $flag, string $name, int $i): void
    {
        $up = ++$i;
        $same = ++$flag;
        $letters = ++$name;
        $u@one@p;
        $sa@two@me;
        $le@three@tters;
    }
}
"#,
        &[
            Query::hover("an int", "one"),
            Query::hover("a bool", "two"),
            Query::hover("a string", "three"),
        ],
        expect![[r#"
            an int
            - code: int $up
            - note: assigned
            a bool
            - code: $same
            - note: assigned
            - note: no declared type
            a string
            - code: $letters
            - note: assigned
            - note: no declared type
        "#]],
    );
}
