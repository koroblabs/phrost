//! The cursor on a name inside `[$a, $b] = ...`, which is where the name is bound.
//!
//! A use of `$a` further down already read the slot it took; the target itself was searched as a
//! plain use, backwards from its own write, and found nothing -- so the card said "no declared type"
//! on the one line that gives it one. The same bug §8 names for a `catch` variable: a feature that
//! works at a use and not at the declaration of the same thing.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn each_target_is_the_slot_it_takes() {
    check_view(
        r#"
//- /src/Pairs.php
<?php
namespace App;

class Duo {}

class Pairs
{
    /** @return array{Duo, int} */
    private function pair(): array {}

    /** @return array{name: Duo} */
    private function named(): array {}

    public function go(): void
    {
        [$f@first@irst, $s@second@econd] = $this->pair();
        list($l@list@isted) = $this->pair();
        ['name' => $k@keyed@eyed] = $this->named();
    }
}
"#,
        &[
            Query::variable_type("the first slot", "first"),
            Query::variable_type("the second slot", "second"),
            Query::variable_type("`list()`", "list"),
            Query::variable_type("a keyed target", "keyed"),
        ],
        expect![[r#"
            the first slot
            - Duo
            the second slot
            - int
            `list()`
            - Duo
            a keyed target
            - Duo
        "#]],
    );
}
