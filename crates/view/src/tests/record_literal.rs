//! `['id' => 1, 'when' => new DateTimeImmutable()]` -- what a record literal holds.
//!
//! A field is a *slot*, so an entry that answers nothing is a key that is certainly there holding
//! something unstated -- which is what `mixed` says, and what `returned_shape` already writes into
//! a docblock for the same entry. All or nothing on the keys, never on the values.
//!
//! The keys are PHP's own, which is why `list_literal` is not a separate rule any more but the same
//! one: a bare entry takes the next index, and an integer key written out moves the counter past
//! it, so `['k' => 1, 2]` is an `array{k: int, 0: int}` exactly as PHP builds it. What still gives
//! the whole literal up is a key nothing here can name -- a computed one, or a `...$rest`, which
//! contributes however many entries another array has and makes every index after it unknown.
//!
//! Until this existed the literal collapsed to a bare `array` at the body layer, below the only
//! layer that can resolve what a key holds -- so a value read back out of one had no type at all,
//! and through that a call on such a value was invisible to every check that asks what a receiver
//! is. On a real Drupal checkout that is the root of most of what `coverage` calls an untyped
//! receiver.
//!
//! A key the literal does not name stays a miss. It must not be answered from what the *other* keys
//! hold: `$row['nope']` reading as `int|DateTimeImmutable` is a type for a slot that is not there,
//! and it is the same answer a docblock `array{...}` gives -- nothing.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_record_literal_holds_what_each_of_its_keys_was_given() {
    check_view(
        r#"
//- /src/classes.php
<?php
namespace App;

class Row
{
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(mixed $unknown): void
    {
        $row = ['id' => 1, 'name' => 'x', 'row' => new Row()];
        $r@row@ow;
        $id = $row['id'];
        $i@id@d;
        $held = $row['row'];
        $he@held@ld;
        $missing = $row['nope'];
        $mi@missing@ssing;
        $unstated = ['there' => $unknown];
        $un@unstated@stated;
        $nested = ['db' => ['host' => 'x']];
        $host = $nested['db']['host'];
        $ho@host@st;
        $repeated = ['k' => 1, 'k' => 'last'];
        $re@repeated@peated;
        $mixed = ['k' => 1, 2];
        $mi@mixed@xed;
    }
}
"#,
        &[
            Query::variable_type("a record names its keys", "row"),
            Query::variable_type("a key holds what it was given", "id"),
            Query::variable_type("and an object stays one", "held"),
            Query::variable_type("a key it does not name is a miss", "missing"),
            Query::variable_type(
                "a slot whose value says nothing is still a slot",
                "unstated",
            ),
            Query::variable_type("a record inside a record", "host"),
            Query::variable_type("PHP keeps the last of a repeated key", "repeated"),
            Query::variable_type("a keyed entry beside a bare one", "mixed"),
        ],
        expect![[r#"
            a record names its keys
            - array{id: int, name: non-falsy-string, row: Row}
            a key holds what it was given
            - int
            and an object stays one
            - Row
            a key it does not name is a miss
            - (no type)
            a slot whose value says nothing is still a slot
            - array{there: mixed}
            a record inside a record
            - non-falsy-string
            PHP keeps the last of a repeated key
            - array{k: non-falsy-string}
            a keyed entry beside a bare one
            - array{k: int, 0: int}
        "#]],
    );
}

#[test]
fn a_record_does_not_survive_being_passed_to_a_function() {
    check_view(
        r#"
//- /src/merge.php
<?php
namespace App;

/**
 * `array_merge`'s own shape, declared here because a fixture has no standard library.
 *
 * @template K
 * @template V
 * @param array<K, V> ...$arrays
 * @return array<K, V>
 */
function merge(array ...$arrays): array
{
    return [];
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(array $other): void
    {
        $merged = merge(['a' => '', 'b' => ''], $other);
        $me@merged@rged;
        $alone = merge(['a' => '', 'b' => '']);
        $al@alone@one;
    }
}
"#,
        &[
            Query::variable_type("what a call made of it is not what went in", "merged"),
            Query::variable_type("and one argument says no more than several", "alone"),
        ],
        expect![[r#"
            what a call made of it is not what went in
            - array
            and one argument says no more than several
            - array
        "#]],
    );
}
