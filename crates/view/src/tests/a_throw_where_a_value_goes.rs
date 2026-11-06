//! `throw` as an operand, which PHP 8 made an expression so it could stand where a value goes.
//!
//! It is `never`: the operand that throws adds nothing to what the operator produces, and a `??`
//! whose fallback throws is its left side with the null taken away. Before, an operand with no path
//! made the whole expression untyped, so `$row = $repo->find($id) ?? throw new NotFound()` -- the
//! commonest reason anybody writes one -- was judged by nobody.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_operand_that_throws_leaves_the_others() {
    check_view(
        r#"
//- /src/Finder.php
<?php
namespace App;

class Row {}

class Finder
{
    public function go(?Row $maybe, Row|false|null $loose, ?array $list): void
    {
        $c@coalesce@oalesced = $maybe ?? throw new \RuntimeException();
        $t@ternary@ernary = $maybe !== null ? $maybe : throw new \RuntimeException();
        $e@elvis@lvis = $loose ?: throw new \RuntimeException();
        $a@array@rray = $list ?? throw new \RuntimeException();
    }
}
"#,
        &[
            Query::variable_type("`??`", "coalesce"),
            Query::variable_type("a ternary's else", "ternary"),
            Query::variable_type("`?:`", "elvis"),
            Query::variable_type("an array", "array"),
        ],
        expect![[r#"
            `??`
            - Row
            a ternary's else
            - Row
            `?:`
            - Row
            an array
            - array
        "#]],
    );
}
