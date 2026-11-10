//! The template parameters and type aliases a docblock can name where it stands: its own and the
//! ones of the class around it, and not another declaration's in the same file.

use expect_test::expect;
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::IndexView;

const FILE: &str = r#"
//- /src/Bag.php
<?php
namespace App;

/**
 * @template TKey of array-key
 * @template-covariant TValue
 * @phpstan-type Row array{id: int}
 * @extends Base<@class@TValue>
 */
class Bag extends Base
{
    /**
     * @template TMapped
     * @return @method@Bag<TKey, TMapped>
     */
    public function map(callable $map) {}

    /** @return @sibling@TValue */
    public function first() {}
}

/**
 * @template TElsewhere
 * @return @function@TElsewhere
 */
function elsewhere() {}

/** @var @loose@int */
const LOOSE = 1;
"#;

#[test]
fn a_docblock_names_its_own_and_its_classes() {
    let spec = FixtureSpec::parse(FILE);
    let world = World::build(&spec);
    let file = world.file_id("src/Bag.php");

    let rendered = ["class", "method", "sibling", "function", "loose"]
        .map(|marker| {
            let offset = spec.markers().position(marker).offset;
            with_open_scopes(&world, Some(file), |open| {
                let view = IndexView::in_file(world.skeleton(), open, file);
                let names = view
                    .templates_in_scope(offset)
                    .iter()
                    .map(|(name, kind)| format!("{name} ({kind:?})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{marker}: {names}\n")
            })
        })
        .concat();
    spec.markers().assert_all_used();

    expect![[r#"
        class: TKey (Template), TValue (Template), Row (TypeAlias)
        method: TMapped (Template), TKey (Template), TValue (Template), Row (TypeAlias)
        sibling: TKey (Template), TValue (Template), Row (TypeAlias)
        function: TElsewhere (Template)
        loose: 
    "#]]
    .assert_eq(&rendered);
}
