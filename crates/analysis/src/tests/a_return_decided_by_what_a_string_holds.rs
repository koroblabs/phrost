//! A conditional return keyed on what a string *holds*: `non-falsy-string`, `non-empty-string`.
//!
//! WordPress's `wpdb::get_row()` and `get_results()` are the shape -- `($query is non-falsy-string ?
//! ($output is 'OBJECT' ? ...) : null)` -- and reading neither test left every call untyped, because
//! a conditional return replaces the plain `@return` and nothing settled it. No type phrost keeps
//! says a string is non-falsy; what the call *wrote* does: a quoted query, an interpolated one, or
//! one joined from a quoted head.
//!
//! A query nothing can read -- `$wpdb->prepare(...)` returns `?string` -- settles nothing, and the
//! branch that test would have picked is only the absence. So the call is the rest of the chain or
//! nothing, which is the rest's answer with a null beside it.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn the_query_a_call_wrote_settles_the_test() {
    check(
        r#"
//- /src/constants.php
<?php

define( 'OBJECT', 'OBJECT' );
define( 'ARRAY_A', 'ARRAY_A' );

//- /src/Db.php
<?php

class Row {}

class Db
{
    /**
     * @phpstan-return (
     *     $query is non-falsy-string
     *         ? ( $output is 'OBJECT' ? Row : ( $output is 'ARRAY_A' ? array : null ) )
     *         : null
     * )
     */
    public function row($query = null, $output = OBJECT) {}

    /** @return string|null */
    public function prepare($query, ...$args) {}
}

//- /src/app.php
<?php

function run(Db $db, string $where): void
{
    $quoted = $db->row('SELECT 1');
    $quo@quoted@ted;
    $interpolated = $db->row("SELECT * FROM {$where}");
    $interpo@interpolated@lated;
    $joined = $db->row('SELECT * FROM t WHERE ' . $where);
    $joi@joined@ned;
    $keyed = $db->row('SELECT 1', ARRAY_A);
    $key@keyed@ed;
    $prepared = $db->row($db->prepare('SELECT %d', 1));
    $prepa@prepared@red;
    $nothing = $db->row();
    $noth@nothing@ing;
    $zero = $db->row('0');
    $ze@zero@ro;
}
"#,
        &[
            Query::hover("a quoted query", "quoted"),
            Query::hover("an interpolated query", "interpolated"),
            Query::hover("a query joined from a quoted head", "joined"),
            Query::hover("a quoted query and a constant output", "keyed"),
            Query::hover("a query nothing can read", "prepared"),
            Query::hover("no query at all", "nothing"),
            Query::hover("the one falsy string beside the empty one", "zero"),
        ],
        expect![[r#"
            a quoted query
            - code: Row $quoted
            - note: assigned
            an interpolated query
            - code: Row $interpolated
            - note: assigned
            a query joined from a quoted head
            - code: Row $joined
            - note: assigned
            a quoted query and a constant output
            - code: array $keyed
            - note: assigned
            a query nothing can read
            - code: ?Row $prepared
            - note: assigned
            no query at all
            - code: null $nothing
            - note: assigned
            the one falsy string beside the empty one
            - code: null $zero
            - note: assigned
        "#]],
    );
}

/// One test rather than a chain, read the same way: a branch that is only the absence makes an
/// unsettled call the other branch or nothing. `is not` swaps the branches and changes nothing else.
#[test]
fn a_single_test_beside_the_absence_reads_the_same_way() {
    check(
        r#"
//- /src/Parser.php
<?php

class Node {}

class Parser
{
    /** @return ($text is not non-empty-string ? null : Node) */
    public function parse(string $text = '') {}
}

//- /src/app.php
<?php

function run(Parser $parser, string $text): void
{
    $read = $parser->parse($text);
    $re@read@ad;
    $written = $parser->parse('<p>');
    $writ@written@ten;
    $empty = $parser->parse();
    $emp@empty@ty;
}
"#,
        &[
            Query::hover("a string nothing can read", "read"),
            Query::hover("a string the call wrote", "written"),
            Query::hover("the default", "empty"),
        ],
        expect![[r#"
            a string nothing can read
            - code: ?Node $read
            - note: assigned
            a string the call wrote
            - code: Node $written
            - note: assigned
            the default
            - code: null $empty
            - note: assigned
        "#]],
    );
}
