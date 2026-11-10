//! How a class is written in a docblock: the way the file's own docblocks write one.
//!
//! Drupal and Laravel name every class in a comment from the root; Symfony, WordPress and what
//! php-cs-fixer has been run over name it as the code does. Completion wrote the first everywhere:
//! `@throws LockStorag` under `use ...\LockStorageException;` became the whole path, and
//! `@param WP_Po` in a file with no namespace became `\WP_Post`. Asked at every docblock name in
//! WordPress's own source cut in half, 97 of 104 came back with a `\` the file never writes.
//!
//! One marker to a fixture, and the evidence -- the docblock that already names a class -- beside
//! it in the same file.

use expect_test::expect;

use crate::tests::{Query, check};

const DECLARED: &str = r#"
//- /src/Data/Shapes.php
<?php
namespace App\Data;

class Edge {}

class Node {}

class Notch {}
"#;

fn check_beside_shapes(client: &str, label: &'static str, expect: expect_test::Expect) {
    check(
        &format!("{DECLARED}\n{client}"),
        &[Query::completion(label, "here")],
        expect,
    );
}

/// The imported class is written by its name, and the one nothing imports is written out: a
/// comment is still no reason to add a `use` line.
#[test]
fn a_file_naming_classes_as_its_code_does_is_completed_that_way() {
    check_beside_shapes(
        r#"
//- /src/Graph.php
<?php
namespace App;

use App\Data\Edge;
use App\Data\Node;

class Graph
{
    /** @param Edge $edge */
    public function add(Edge $edge): void {}

    /** @return No@here@ */
    public function first() {}
}
"#,
        "names written short",
        expect![[r#"
            names written short
            - Node [class]  App\Data\Node
            - Notch [class]  App\Data\Notch  insert `\App\Data\Notch`
            - ... and the client must ask again
            - accepting the first leaves `/** @return Node */`
        "#]],
    );
}

/// The same file shape with the names written from the root: nothing changes for it.
#[test]
fn a_file_naming_classes_from_the_root_is_completed_that_way() {
    check_beside_shapes(
        r#"
//- /src/Graph.php
<?php
namespace App;

use App\Data\Edge;
use App\Data\Node;

class Graph
{
    /** @param \App\Data\Edge $edge */
    public function add(Edge $edge): void {}

    /** @return No@here@ */
    public function first() {}
}
"#,
        "names written from the root",
        expect![[r#"
            names written from the root
            - Node [class]  App\Data\Node  insert `\App\Data\Node`
            - Notch [class]  App\Data\Notch  insert `\App\Data\Notch`
            - ... and the client must ask again
            - accepting the first leaves `/** @return \App\Data\Node */`
        "#]],
    );
}

/// A file whose docblocks name no class yet says nothing, and gets the spelling that is right
/// wherever it lands.
#[test]
fn a_file_that_has_named_no_class_is_completed_from_the_root() {
    check_beside_shapes(
        r#"
//- /src/Graph.php
<?php
namespace App;

use App\Data\Node;

class Graph
{
    /** @return No@here@ */
    public function first() {}
}
"#,
        "no evidence",
        expect![[r#"
            no evidence
            - Node [class]  App\Data\Node  insert `\App\Data\Node`
            - Notch [class]  App\Data\Notch  insert `\App\Data\Notch`
            - ... and the client must ask again
            - accepting the first leaves `/** @return \App\Data\Node */`
        "#]],
    );
}

/// WordPress's shape: no namespace anywhere, so the bare name is the whole name.
#[test]
fn a_global_class_in_a_file_with_no_namespace_has_nothing_to_be_rooted_at() {
    check(
        r#"
//- /includes/classes.php
<?php

class WP_Post {}

class WP_Query {}

//- /plugin.php
<?php

/** @param WP_Query $query */
function first($query) {}

/** @return WP_Po@here@ */
function second() {}
"#,
        &[Query::completion("no namespace", "here")],
        expect![[r#"
            no namespace
            - WP_Post [class]  WP_Post
            - ... and the client must ask again
            - accepting the first leaves `/** @return WP_Post */`
        "#]],
    );
}
