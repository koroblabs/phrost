//! `array_key_exists($key, $map)` and `isset($map[$key])`: on the side that held, the key is one the
//! container holds.
//!
//! Which says something about the key's own type only where the container is keyed by strings. PHP
//! stores a numeric string as the integer it spells, so `'5'` finds an `array<int, V>`'s key 5 and
//! stays a string -- while an `int` never finds a string-keyed array's key, nor does a numeric
//! string. So an `int|string` key is a `string` past a string-keyed container's test and unchanged
//! past an int-keyed one's, and an absent key says nothing either way.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_string_keyed_container_leaves_a_string_key() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

/**
 * @param array<string, int> $map
 * @param list<int> $list
 */
function run(int|string $key, array $map, array $list): void
{
    if (array_key_exists($key, $map)) {
        $e@exists@xists = $key;
    }
    if (isset($map[$key])) {
        $i@isset@sset = $key;
    }
    if (!array_key_exists($key, $map)) {
        $a@absent@bsent = $key;
    }
    if (array_key_exists($key, $list)) {
        $l@list@ist = $key;
    }
}
"#,
        &[
            Query::variable_type("array_key_exists on string keys", "exists"),
            Query::variable_type("isset on string keys", "isset"),
            Query::variable_type("the key is absent", "absent"),
            Query::variable_type("an int-keyed list", "list"),
        ],
        expect![[r#"
            array_key_exists on string keys
            - string
            isset on string keys
            - string
            the key is absent
            - int|string
            an int-keyed list
            - int|string
        "#]],
    );
}

/// The alternative `is_null` names, on a value that cannot be null, is one that never held: the
/// path around `if (!is_null($name) && !isset($config[$name])) { return; }` is the key's, and a null
/// beside it was a value nothing can hold.
#[test]
fn an_alternative_naming_a_null_the_value_cannot_be_never_held() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

/** @param array<string, string> $config */
function run(string $name, array $config): void
{
    if (!is_null($name) && !isset($config[$name])) {
        return;
    }
    $n@name@ame = $name;
}
"#,
        &[Query::variable_type("past the guard", "name")],
        expect![[r#"
            past the guard
            - string
        "#]],
    );
}
