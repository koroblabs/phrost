//! What a chain rooted at a free function's call offers.
//!
//! `reset($rows)->`, `array_pop($queue)?->`, `make_thing()->`. A method call, a static call and a
//! `new` all worked; a bare function call offered nothing at all, and the scanner's own comment
//! said why -- it "needs a return type looked up by name rather than a class", and that lookup had
//! never been written. So it is a root of its own, and the two things that answer it are the two a
//! completed statement would have used.
//!
//! The **declaration's return** is the whole of it for a project's own `make_thing(): Row`. And for
//! `reset($rows)` it is `mixed`, because what that one gives back is decided by the array it was
//! handed -- so the written argument is read too: the contract says which parameter's *elements*
//! come back, the characters say which local was passed, and resolving that local is what the call
//! expression would have given.
//!
//! Only a plain `$local`. An argument written as anything else leaves the declaration standing,
//! which is the same refusal every other reader of scanned text makes.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function label(): string
    {
        return '';
    }
}

function make_row(): Row
{
    return new Row();
}

/**
 * @template K
 * @template V
 *
 * @param object|array<K, V> $array
 *
 * @return ($array is non-empty-array ? V : V|false)
 */
function reset(object|array &$array): mixed {}
"#;

#[test]
fn a_functions_declared_return_is_what_its_call_offers() {
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    public function run(): void
    {{
        make_row()->@here@
    }}
}}
"#
        ),
        &[Query::completion("a project's own function", "here")],
        expect![[r#"
            a project's own function
            - label [method]  : string  writes `();`
            - accepting the first leaves `make_row()->label`
        "#]],
    );
}

/// The half a declared return cannot give: `reset` says `mixed`, and the array says `Row`.
#[test]
fn an_argument_settles_what_the_declaration_leaves_open() {
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    /** @param Row[] $rows */
    public function run(array $rows): void
    {{
        reset($rows)->@here@
    }}
}}
"#
        ),
        &[Query::completion("the element of the argument", "here")],
        expect![[r#"
            the element of the argument
            - label [method]  : string  writes `();`
            - accepting the first leaves `reset($rows)->label`
        "#]],
    );
}

/// A name nothing declares, which is the refusal the lookup is built on.
#[test]
fn a_function_the_project_does_not_have_offers_nothing() {
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    public function run(): void
    {{
        no_such_helper()->@here@
    }}
}}
"#
        ),
        &[Query::completion("an undeclared function", "here")],
        expect![[r#"
            an undeclared function
            - (nothing offered)
        "#]],
    );
}

/// A return picked by which string an argument is, with the argument left at its default -- which
/// is a constant `define()` declares. `get_post($id)->` in every WordPress template.
#[test]
fn a_default_constant_picks_the_branch_of_a_chain_of_string_tests() {
    check(
        r#"
//- /src/constants.php
<?php
define( 'OBJECT', 'OBJECT' );

//- /src/post.php
<?php
class WP_Post
{
    public function title(): string
    {
        return '';
    }
}

/**
 * @phpstan-return ($output is 'ARRAY_A' ? array<string, mixed> : ($output is 'ARRAY_N' ? array<int, mixed> : WP_Post))
 */
function get_post( $post = null, $output = OBJECT ) {}

function run(int $id): void
{
    get_post( $id )->@here@
}
"#,
        &[Query::completion("the default's branch", "here")],
        expect![[r#"
            the default's branch
            - title [method]  : string  writes `();`
            - accepting the first leaves `get_post( $id )->title`
        "#]],
    );
}
