//! The cursor inside `compact('title')`, which names `$title` as surely as writing it does.
//!
//! The string is the variable's name, so the lookup that answers at `$title` answers here too --
//! and it is the one place a person stands to ask what the call is about to collect.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_string_is_a_use_of_the_variable() {
    check_view(
        r#"
//- /src/Report.php
<?php
namespace App;

class Row {}

function render(): array
{
    $row = new Row();
    $count = 3;
    return compact('r@row@ow', 'cou@count@nt', 'miss@missing@ing');
}
"#,
        &[
            Query::variable_type("a class", "row"),
            Query::variable_type("a scalar", "count"),
            Query::variable_type("a name nothing binds", "missing"),
        ],
        expect![[r#"
            a class
            - Row
            a scalar
            - int
            a name nothing binds
            - (no type)
        "#]],
    );
}
