//! The classes a short name could mean, which is what an import is offered from.
//!
//! Exactly the name, and only what a `use` can import as a class. The symbol search this used to
//! be read out of matches a prefix, matches loosely, and walks the project's members -- so every
//! snapshot here is of the whole answer, in order, beside a fixture built to hold each of the
//! things that must not be in it.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

/// A name declared as a class twice over, once by a dependency; the same name as a function and
/// as a method; and two longer names that begin with it.
const NODES: &str = r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node {}

//- /src/Tree/Node.php
<?php
namespace App\Tree;

interface Node {}

//- /src/NodeType.php
<?php
namespace App;

class NodeType
{
    public function node(): void {}
}

//- /src/functions.php
<?php
namespace App;

function node(): void {}

//- /vendor/acme/graph/src/Node.php
<?php
namespace Acme\Graph;

class Node {}

class NodeVisitor {}
"#;

fn check(fixture: &str, short: &str, expect: Expect) {
    let world = World::parse(fixture);

    let rendered = with_open_scopes(&world, None, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let found = view
            .class_likes_named(short)
            .iter()
            .map(|hit| {
                format!(
                    "{} {} {}\n",
                    hit.kind.as_str(),
                    hit.fqn.as_str(),
                    world.path_of(hit.file)
                )
            })
            .collect::<String>();

        if found.is_empty() {
            "<nothing found>\n".to_owned()
        } else {
            found
        }
    });

    expect.assert_eq(&rendered);
}

/// The project's own before the dependency's, and neither `NodeType` nor `NodeVisitor`, which
/// begin with the name; nor the function and the method, which are it and are not classes.
#[test]
fn only_the_classes_of_exactly_that_name() {
    check(
        NODES,
        "Node",
        expect![[r#"
        class App\Entity\Node src/Entity/Node.php
        interface App\Tree\Node src/Tree/Node.php
        class Acme\Graph\Node vendor/acme/graph/src/Node.php
    "#]],
    );
}

/// A class name is case-insensitive in PHP, so the name as somebody typed it finds the class as it
/// was declared.
#[test]
fn the_name_is_matched_whatever_its_case() {
    check(
        NODES,
        "nodetype",
        expect![[r#"
        class App\NodeType src/NodeType.php
    "#]],
    );
}

/// A name nothing declares as a class, though a function and a method have it.
#[test]
fn a_name_only_a_function_has_is_not_a_class() {
    check(
        r#"
//- /src/functions.php
<?php
namespace App;

function helper(): void {}

class Tools
{
    public function helper(): void {}
}
"#,
        "helper",
        expect![[r#"
            <nothing found>
        "#]],
    );
}
