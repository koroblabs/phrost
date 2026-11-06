//! Two calls writing one name through a by-reference parameter, and which of them is still there.
//!
//! `preg_match($p, $s, $m); preg_match_all($q, $s, $m);` leaves `$m` in the second one's shape --
//! a list of the lists of captures rather than one list of them. The walk hands the calls back in
//! source order and both readers took the *first* one they could use, so the older tag won
//! whichever order they were written in: reversing the fixture answered the same way round, which
//! is the tell that the order was not being read at all.

use expect_test::expect;

use super::{Query, check_view};

const PCRE: &str = r#"
//- /stubs/pcre.php  stub
<?php
/**
 * @param-out array<string> $matches
 */
function preg_match(string $pattern, string $subject, &$matches = [], int $flags = 0, int $offset = 0): int|false {}
/**
 * @param-out list<array<string>> $matches
 */
function preg_match_all(string $pattern, string $subject, &$matches = null, int $flags = 0, int $offset = 0): int|false {}
"#;

#[test]
fn the_last_call_to_write_the_name_is_the_one_that_answers() {
    check_view(
        &[
            PCRE,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function one_then_all(string $text): void
    {
        preg_match('/(a)(b)/', $text, $matches);
        preg_match_all('/(a)/', $text, $matches);
        $m@later@atches;
    }

    public function all_then_one(string $text): void
    {
        preg_match_all('/(a)/', $text, $matches);
        preg_match('/(a)(b)/', $text, $matches);
        $m@earlier@atches;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("`preg_match_all` last", "later"),
            Query::variable_type("`preg_match` last", "earlier"),
        ],
        expect![[r#"
            `preg_match_all` last
            - array<int, string[]>
            `preg_match` last
            - string[]
        "#]],
    );
}

/// Each on its own, so the fixture above is read as a statement about the *order* rather than
/// about either tag.
#[test]
fn one_call_answers_with_its_own_tag() {
    check_view(
        &[
            PCRE,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function all(string $text): void
    {
        preg_match_all('/(a)/', $text, $matches);
        $m@all@atches;
    }

    public function one(string $text): void
    {
        preg_match('/(a)(b)/', $text, $matches);
        $m@one@atches;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::variable_type("`preg_match_all`", "all"),
            Query::variable_type("`preg_match`", "one"),
        ],
        expect![[r#"
            `preg_match_all`
            - array<int, string[]>
            `preg_match`
            - string[]
        "#]],
    );
}
