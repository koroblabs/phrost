//! `[new Row(), new Row()]` -- what a list literal holds.
//!
//! The literal used to resolve to a bare `array`, so `foreach ([new Row()] as $row)` gave `$row`
//! nothing while the same array in a typed variable gave `Row`. The entries were thrown away at the
//! body layer, which is the wrong place to decide: each entry is itself a path, and following one
//! needs the index.
//!
//! A literal carries its **slots**, keyed the way PHP keys them: `[new Row(), new Row()]` is an
//! `array{0: Row, 1: Row}` and `['a' => 1]` is an `array{a: int}`, which are the same statement
//! about two spellings of one thing. That is what makes `[$handler, $stage, $rest]` in a `foreach`
//! destructure to three types rather than to the union of all three -- the shape a test's data
//! provider is written in.
//!
//! So the all-or-nothing rule a *union* needed is gone with the union: `[new Row(), $unknown]` is
//! an `array{0: Row, 1: mixed}`, which is exact about the slot it knows and says `mixed` about the
//! one it does not. A union built from the entries that answered would have been narrower than the
//! truth; a slot is not.
//!
//! A `...$rest` contributes another array's entries in place. When what it spreads is itself a
//! record its arity is known, so the slots are spliced in the way PHP splices them -- integer keys
//! renumbered from where the counter stands, string keys kept -- and `[...$pair, 'x']` is an
//! `array{0: Row, 1: Row, 2: string}`. When it is not, the literal falls back to the element rule,
//! where `[...$rows]` on a `Row[]` is a `Row[]`.
//!
//! The one thing that rule will not do is flatten a record whose slots **disagree**. That union is
//! what every entry of the outer literal would have to be, with the positions that said otherwise
//! gone -- the same approximation the all-or-nothing rule used to guard against, arriving one level
//! in. Laravel spreads exactly such a pair and destructures the result, and reading the union put a
//! `bool` among the operands of a slot holding an event name.
//!

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_list_literal_holds_the_union_of_its_entries() {
    check_view(
        r#"
//- /src/classes.php
<?php
namespace App;

class Row
{
    public function label(): string
    {
        return '';
    }
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(mixed $unknown): void
    {
        $objects = [new Row(), new Row()];
        $ob@objects@jects;
        $scalars = ['a', 'b'];
        $sc@scalars@alars;
        $several = [1, 'x'];
        $se@several@veral;
        $partly = [new Row(), $unknown];
        $pa@partly@rtly;
        $keyed = ['a' => 1];
        $ke@keyed@yed;
        $empty = [];
        $em@empty@pty;
        $spread = [...$objects];
        $sp@spread@read;
        $both = [...$objects, new Row()];
        $bo@both@th;
        $widened = [...$objects, 'x'];
        $wi@widened@dened;
        foreach ($objects as $one) {
            $o@one@ne;
        }
    }
}
"#,
        &[
            Query::variable_type("all one class", "objects"),
            Query::variable_type("all one scalar", "scalars"),
            Query::variable_type("two kinds", "several"),
            Query::variable_type("one entry says nothing", "partly"),
            Query::variable_type("keys of its own", "keyed"),
            Query::variable_type("nothing in it", "empty"),
            Query::variable_type("a spread of a typed array", "spread"),
            // The same class reached two ways -- an interned `Row` beside a built one -- which is
            // what the two folds a join already uses are for.
            Query::variable_type("a spread beside an entry", "both"),
            Query::variable_type("a spread beside a different kind", "widened"),
            Query::variable_class("and iterating it", "one", "one"),
        ],
        expect![[r#"
            all one class
            - array{0: Row, 1: Row}
            all one scalar
            - array{0: non-falsy-string, 1: non-falsy-string}
            two kinds
            - array{0: int, 1: non-falsy-string}
            one entry says nothing
            - array{0: Row, 1: mixed}
            keys of its own
            - array{a: int}
            nothing in it
            - array
            a spread of a typed array
            - array{0: Row, 1: Row}
            a spread beside an entry
            - array{0: Row, 1: Row, 2: Row}
            a spread beside a different kind
            - array{0: Row, 1: Row, 2: non-falsy-string}
            and iterating it
            - App\Row
        "#]],
    );
}
