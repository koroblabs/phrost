//! Goto on a name a call brought into being: `preg_match($p, $s, $matches)`.
//!
//! Every other binding a local can have is a write somebody wrote -- a parameter, an assignment, a
//! `foreach`, a `catch`, a `static`. This one is not written anywhere: PHP creates `$matches` when
//! the call runs, because `preg_match`'s third parameter is by reference. So the search that asks
//! "what in this body writes this name" found nothing, and goto answered nothing on a variable
//! whose type the very same file already knew.
//!
//! Which argument is by reference is the callee's business, and no layer below the index can say.
//! What makes answering safe anyway is that this is asked **only** when nothing else bound the name
//! at all: either the parameter is by reference, or the code reads a variable nothing set -- and the
//! second is what `unbound-variable` reports, with the call still being where a reader would look.
//!
//! Found by the comparison against another server, where it was the whole of the remaining goto
//! difference on the reference project: two sites, both `$matches`.

use expect_test::expect;

use super::{Query, check};

#[test]
fn goto_on_a_name_a_by_reference_argument_created_lands_on_the_argument() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    public function run(string $subject): void
    {
        if (preg_match('/(\d+)/', $subject, $matches)) {
            echo $mat@use@ches[1];
        }
    }
}
"#,
        &[Query::goto("the name the call created", "use")],
        expect![[r#"
            the name the call created
            - variable matches @ src/Rows.php 8:45-8:53 (full 8:45-8:53)
        "#]],
    );
}

#[test]
fn an_assignment_still_wins_over_the_call_that_passed_it() {
    // The ranking that keeps this from taking goto away from anything: a name written in the body
    // is bound where it is written, and passing it to something afterwards is a use.
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    public function run(): void
    {
        $rows = [];
        array_push($rows, 1);
        echo $ro@use@ws[0];
    }
}
"#,
        &[Query::goto("the assignment above it", "use")],
        expect![[r#"
            the assignment above it
            - variable rows @ src/Rows.php 8:9-8:14 (full 8:9-8:14)
        "#]],
    );
}

#[test]
fn a_name_reached_through_an_argument_is_not_bound_by_it() {
    // `f($rows['k'])` never writes `$rows`, whatever the parameter says -- so the match is on the
    // bare name and nothing reached through it.
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    public function run(): void
    {
        preg_match('/x/', 'y', $found['inner']);
        echo $fou@use@nd;
    }
}
"#,
        &[Query::goto("nothing bound it", "use")],
        expect![[r#"
            nothing bound it
            - (nothing to go to)
        "#]],
    );
}
