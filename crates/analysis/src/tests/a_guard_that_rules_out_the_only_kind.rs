//! `if (!is_array($rows))` where `$rows` is an `array` -- a branch the value cannot enter.
//!
//! The narrowing kept what the value was when no alternative survived the guard, which says the
//! opposite of what the guard proved: every check then judged an unreachable line against a type
//! the guard had ruled out. Yaml's parser writes exactly that shape -- `if (!\is_array($item)) {
//! throw new ParseException(..., $item, ...); }` -- and the `?string` parameter was reported as
//! being handed an `array`.
//!
//! `mixed` is the honest answer, being the one type every check reads as *no claim*. Deciding which
//! branches are reachable is a different question, and not this layer's.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_value_of_only_that_kind_says_nothing_in_the_branch() {
    check(
        r#"
//- /src/stubs.php
<?php

function is_array(mixed $value): bool {}

//- /src/app.php
<?php
namespace App;

function takes(?string $text): void {}

class Host
{
    /** @return array */
    public function rows(): array
    {
        return [];
    }

    public function run(): void
    {
        $rows = $this->rows();

        if (!is_array($rows)) {
            takes($rows);
        }
    }
}
"#,
        expect![""],
    );
}

/// And a guard that rules out *one* of several alternatives still leaves the others, which is the
/// case this must not break.
#[test]
fn one_alternative_of_several_still_narrows() {
    check(
        r#"
//- /src/stubs.php
<?php

function is_array(mixed $value): bool {}

//- /src/app.php
<?php
namespace App;

function takes(?string $text): void {}

function wants_array(array $rows): void {}

class Host
{
    /** @return array|int */
    public function either(): array|int
    {
        return [];
    }

    public function run(): void
    {
        $value = $this->either();

        if (!is_array($value)) {
            wants_array($value);
        }
    }
}
"#,
        expect![[r#"
            21:25-21:31 error [incompatible-argument] $rows takes array, and this is int
        "#]],
    );
}
