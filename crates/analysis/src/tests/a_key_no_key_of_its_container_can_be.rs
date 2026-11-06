//! `$map[0]` on an `array<string, int>`, `[$a, $b] = $map`, and `$list['name']`.
//!
//! PHP stores every numeric string key as an integer, so an array keyed by strings has no `0` on any
//! run, and one keyed by integers has no `'name'`. Only a key kind the declaration states outright
//! counts: a `V[]` says nothing about its keys, and a shape lists the ones it has, which is
//! `misspelt-key`'s business.

use expect_test::expect;

use super::diagnostics::check;

const MAPS: &str = r#"
//- /src/Maps.php
<?php

class Maps
{
    /**
     * @return array<string, int>
     */
    public function byName(): array
    {
        return [];
    }

    /**
     * @return list<int>
     */
    public function inOrder(): array
    {
        return [];
    }

    /**
     * @return int[]
     */
    public function loose(): array
    {
        return [];
    }
}
"#;

#[test]
fn a_string_keyed_array_has_no_integer_key() {
    // `'0'` is the same key as `0`: PHP turns the string into the integer before storing it. `'007'`
    // has a leading zero, so PHP keeps it a string and it may well be there.
    check(
        &format!(
            r#"{MAPS}
//- /src/app.php
<?php

function run(Maps $maps): void
{{
    $byName = $maps->byName();

    echo $byName['0'];
    echo $byName['007'];
    echo $byName[1];
}}
"#
        ),
        expect![[r#"
            7:10-7:22 warning [impossible-key] array<string, int> has no key 0
            9:10-9:20 warning [impossible-key] array<string, int> has no key 1
        "#]],
    );
}

#[test]
fn a_destructuring_reads_the_keys_it_numbers() {
    // `[$a, $b]` reads `0` and `1`, so each element is the read; the explicit key beside them is
    // judged as written.
    check(
        &format!(
            r#"{MAPS}
//- /src/app.php
<?php

function run(Maps $maps): void
{{
    [$first, $second] = $maps->byName();
    ['total' => $total] = $maps->byName();

    echo $first, $second, $total;
}}
"#
        ),
        expect![[r#"
            5:6-5:12 warning [impossible-key] array<string, int> has no key 0
            5:14-5:21 warning [impossible-key] array<string, int> has no key 1
        "#]],
    );
}

#[test]
fn a_list_has_no_string_key() {
    // Rendered as `array<int, int>`: a `list` is stored as the array its keys make it, which is the
    // weaker claim and the true one.
    check(
        &format!(
            r#"{MAPS}
//- /src/app.php
<?php

function run(Maps $maps): void
{{
    $inOrder = $maps->inOrder();

    echo $inOrder['name'];
    echo $inOrder[0];
    echo $inOrder['1'];
    [$head] = $inOrder;

    echo $head;
}}
"#
        ),
        expect![[r#"
            7:10-7:26 warning [impossible-key] array<int, int> has no key 'name'
        "#]],
    );
}

#[test]
fn an_array_that_states_no_key_kind_answers_nothing() {
    check(
        &format!(
            r#"{MAPS}
//- /src/app.php
<?php

function run(Maps $maps): void
{{
    $loose = $maps->loose();

    echo $loose['name'];
    echo $loose[0];
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn an_array_a_guard_found_empty_has_no_key_at_all() {
    // `[]` is the one falsy array. A read is reported only where the guard was about the local
    // being read: a call is a slot, and its guard is not proof about a second call.
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return list<int> */
    public function load(): array { return []; }

    /** @param list<int> $rows */
    public function first(array $rows): int
    {
        if ($rows === []) {
            return $rows[0];
        }
        if (empty($rows)) {
            return $rows['total'];
        }
        $loaded = $this->load();
        if (!$loaded) {
            $again = $this->load();
            return $again[0];
        }
        return $rows[0];
    }
}
"#,
        expect![[r#"
            13:20-13:28 warning [impossible-key] array{} has no key 0
            16:20-16:34 warning [impossible-key] array{} has no key 'total'
        "#]],
    );
}
