//! `-1`, `-$x`, `~$x` and the two increments, read as the values they are.
//!
//! PHP has no negative literal -- `-1` is the minus operator over `1` -- so a local assigned one had
//! no type at all, and neither did anything written through `-`, `+`, `~`, `$i++` or `++$i`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn each_operator_gives_what_php_computes() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

function run(int $i, ?int $n, float $r, string $s): void
{
    $l@literal@it = -1;
    $f@float@loat = -1.5;
    $n@negated@eg = -$i;
    $p@plus@lus = +$r;
    $w@numeric@ord = -$s;
    $b@bits@its = ~$i;
    $o@old@ld = $n++;
    $e@new@ew = ++$i;
}
"#,
        &[
            Query::variable_type("a negative literal", "literal"),
            Query::variable_type("a negative float", "float"),
            Query::variable_type("an int negated", "negated"),
            Query::variable_type("a float with a plus", "plus"),
            Query::variable_type("a string negated is the number it spells", "numeric"),
            Query::variable_type("bits flipped", "bits"),
            Query::variable_type("a postfix increment is the value before", "old"),
            Query::variable_type("a prefix increment is the value after", "new"),
        ],
        expect![[r#"
            a negative literal
            - int
            a negative float
            - float
            an int negated
            - int
            a float with a plus
            - float
            a string negated is the number it spells
            - int|float
            bits flipped
            - int
            a postfix increment is the value before
            - ?int
            a prefix increment is the value after
            - int
        "#]],
    );
}
