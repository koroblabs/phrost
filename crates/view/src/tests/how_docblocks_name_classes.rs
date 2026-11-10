//! Whether a file's docblocks name classes the way its code does or from the root -- see
//! [`crate::docblock_names`]. One file per shape, each asked on its own.

use expect_test::expect;
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

const PROJECT: &str = r#"
//- /src/Data/Node.php
<?php
namespace App\Data;

class Node {}

class Edge {}

//- /src/short.php
<?php
namespace App;

use App\Data\Edge;
use App\Data\Node;

/**
 * @param Node $node
 * @return Edge
 */
function short($node) {}

//- /src/rooted.php
<?php
namespace App;

/**
 * @param \App\Data\Node $node
 * @return \App\Data\Edge
 */
function rooted($node) {}

//- /src/mostly_rooted.php
<?php
namespace App;

use App\Data\Node;

/**
 * @param \App\Data\Node $node
 * @param \App\Data\Edge $edge
 * @return Node
 */
function mostly_rooted($node, $edge) {}

//- /src/even.php
<?php
namespace App;

use App\Data\Node;

/**
 * @param \App\Data\Edge $edge
 * @return Node
 */
function even($edge) {}

//- /src/templates_only.php
<?php
namespace App;

/**
 * @template TValue
 * @param TValue $value
 * @param Missing $nothing
 * @return TValue
 */
function templates_only($value, $nothing) {}

//- /src/silent.php
<?php
namespace App;

function silent(): void {}

//- /src/trait_tag.php
<?php
namespace App;

trait Shaped {}

class Carrier
{
    /** @use Shaped<int> */
    use Shaped;
}

//- /global.php
<?php

class WP_Post {}

/** @param WP_Post $post */
function global_names($post) {}
"#;

#[test]
fn a_file_says_how_its_docblocks_name_a_class() {
    let world = World::parse(PROJECT);
    let rendered = [
        "src/short.php",
        "src/rooted.php",
        "src/mostly_rooted.php",
        "src/even.php",
        "src/templates_only.php",
        "src/silent.php",
        "src/trait_tag.php",
        "global.php",
    ]
    .map(|path| {
        let file = world.file_id(path);
        with_open_scopes(&world, Some(file), |open| {
            let view = IndexView::in_file(world.skeleton(), open, file);
            match view.docblocks_name_classes_short() {
                true => format!("{path}: as the code does\n"),
                false => format!("{path}: from the root\n"),
            }
        })
    })
    .concat();

    // A tie, a file of template parameters and unresolved names, and a file with no docblock at
    // all say nothing -- and nothing is the rooted spelling, which is right wherever it lands. A
    // trait named by `@use` is no evidence either way: it is not a type.
    expect![[r#"
        src/short.php: as the code does
        src/rooted.php: from the root
        src/mostly_rooted.php: from the root
        src/even.php: from the root
        src/templates_only.php: from the root
        src/silent.php: from the root
        src/trait_tag.php: from the root
        global.php: as the code does
    "#]]
    .assert_eq(&rendered);
}
