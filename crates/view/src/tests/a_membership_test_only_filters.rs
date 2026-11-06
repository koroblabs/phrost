//! `in_array($x, [<scalars>], true)` rules alternatives out and names nothing.
//!
//! The claim it makes on the branch where the needle was *not* in the haystack is spelled as
//! "an object", because that is the one thing the syntax settles about a haystack of literals and
//! class constants -- and a value that is neither of two strings is not thereby an object. It works
//! as a *filter* over what a declaration listed, which is all it was ever written to do.
//!
//! So the claim carries whether the guard **named** the kind or only ruled some of it out, and only
//! a named one may be the whole answer. Without that, the first predicate that started answering for
//! a value nothing else described made an object of every value a membership test had been written
//! about: 12 of the 16 findings one measurement added on Symfony, all of them `mixed` values in
//! `elseif` chains.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /stubs/standard.php  stub
<?php
function in_array(mixed $needle, array $haystack, bool $strict = false): bool {}

//- /src/Membership.php
<?php
namespace App;

class Bag
{
    public function count(): int {}
}

class Membership
{
    public function nothingDeclared(mixed $value): void
    {
        if (in_array($value, ['a', 'b'], true)) {
            return;
        }
        $v@loose@alue;
    }

    public function aUnionIsFiltered(string|Bag|null $value): void
    {
        if (in_array($value, ['a', 'b'], true)) {
            return;
        }
        $v@filtered@alue;
    }
}
"#;

#[test]
fn a_membership_test_narrows_a_union_and_says_nothing_about_a_value_nothing_described() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("nothing was listed, so nothing is ruled out", "loose"),
            Query::variable_type("the alternatives that are not objects go", "filtered"),
        ],
        expect![[r#"
            nothing was listed, so nothing is ruled out
            - mixed
            the alternatives that are not objects go
            - Bag
        "#]],
    );
}
