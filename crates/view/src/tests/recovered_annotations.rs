//! A `/** @var T $x */` whose statement error recovery dropped is still a local here.
//!
//! The lowering tests say the annotation reaches the HIR; these say a feature can reach it, which
//! is a different question. An annotation is ranked against the other bindings by its own `range`
//! rather than by where it sits in the statement list, so one appended after the statements that
//! outlived it has to answer exactly as one written in place -- and the file where recovery dropped
//! the statement is the only place the two orders differ.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_annotation_types_a_local_whose_statement_was_dropped() {
    check_view(
        r#"
//- /src/run.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

/** @var Repo $repo */
$repo@here@->
"#,
        &[Query::variable_class("dropped", "here", "repo")],
        expect![[r#"
            dropped
            - App\Repo
        "#]],
    );
}

/// Ranked by where it is written, both ways: it beats the assignment above it and says nothing
/// about the code before it. A recovered annotation is appended after the statements that survived,
/// so if anything ranked by list position instead of by range this is where it would show -- the
/// annotation would win at both markers.
#[test]
fn a_recovered_annotation_applies_from_where_it_is_written() {
    check_view(
        r#"
//- /src/run.php
<?php
namespace App;

class Repo {}
class Other {}

$repo = new Other();
$repo@before@;
/** @var Repo $repo */
$repo@after@->
"#,
        &[
            Query::variable_class("above the docblock", "before", "repo"),
            Query::variable_class("below it", "after", "repo"),
        ],
        expect![[r#"
            above the docblock
            - App\Other
            below it
            - App\Repo
        "#]],
    );
}
