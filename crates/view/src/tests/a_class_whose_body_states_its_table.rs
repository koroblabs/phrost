//! A property read on an instance of the class whose own body states the table of its kind.
//!
//! Drupal's field item is the shape: `#[FieldType(id: 'refund')] class RefundItem` writes
//! `$properties['enabled'] = DataDefinition::create('boolean')` and declares no `$enabled`, so
//! `$item->enabled` is answered by `__get` -- and the table is the one statement anywhere of what
//! that returns. Neutral names, as in the index's own test of the table: `#[Element('x')]` whose
//! `shapeOf()` writes `$slots['k'] = Slot::create('text')`.

use expect_test::expect;
use phrost_harness::World;

use super::{Query, check_view_ingested_with};

fn element(world: &mut World) {
    world.add_declared_table(
        "Element",
        "shapeOf",
        &["Slot"],
        &[
            ("text", phrost_model::BuiltinTy::String),
            ("flag", phrost_model::BuiltinTy::Bool),
        ],
        "element",
    );
}

#[test]
fn the_table_answers_what_the_accessor_does_not_declare() {
    check_view_ingested_with(
        r#"
//- /src/Base.php
<?php
namespace App;

abstract class Base
{
    public string $declared = '';

    public function __get(string $name): mixed { return null; }
}

//- /src/Plain.php
<?php
namespace App;

#[Element('plain')]
class Plain extends Base
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text');
        $slots['enabled'] = Slot::create('flag');
        $slots['declared'] = Slot::create('flag');
        return $slots;
    }

    public function read(): void
    {
        $i@this@nside = $this->title;
    }
}

//- /src/Partial.php
<?php
namespace App;

#[Element('partial')]
class Partial extends Plain
{
    public static function shapeOf($definition): array
    {
        $slots = parent::shapeOf($definition);
        $slots['note'] = Slot::create('text');
        return $slots;
    }
}

//- /src/Narrower.php
<?php
namespace App;

class Narrower extends Plain {}

//- /src/use.php
<?php
namespace App;

function run(Plain $plain, Partial $partial, Narrower $narrower): void
{
    $e@enabled@nabled = $plain->en@goto_enabled@abled;
    $u@unlisted@nlisted = $plain->unl@goto_unlisted@isted;
    $d@declared@eclared = $plain->declared;
    $o@own@wn = $partial->note;
    $i@inherited@nherited = $partial->ti@goto_inherited@tle;
    $s@subclass@ubclass = $narrower->enabled;
}
"#,
        &[
            Query::variable_type("a property the table states", "enabled"),
            Query::variable_type("a name the table does not hold", "unlisted"),
            Query::variable_type("a declared property is the declaration's", "declared"),
            Query::variable_type("a partial table's own key", "own"),
            Query::variable_type(
                "and a parent's, which the partial table says is there",
                "inherited",
            ),
            Query::variable_type("a subclass that states no table of its own", "subclass"),
            Query::variable_type("`$this` inside the declaring class", "this"),
            Query::goto("goto lands on the key the table writes", "goto_enabled"),
            Query::cursor("and the cursor names it", "goto_enabled"),
            Query::goto("a parent's key, through a partial table", "goto_inherited"),
            Query::goto("a name no table holds goes nowhere", "goto_unlisted"),
        ],
        element,
        expect![[r#"
            a property the table states
            - bool
            a name the table does not hold
            - mixed
            a declared property is the declaration's
            - string
            a partial table's own key
            - string
            and a parent's, which the partial table says is there
            - string
            a subclass that states no table of its own
            - bool
            `$this` inside the declaring class
            - string
            goto lands on the key the table writes
            - property App\Plain::enabled @ src/Plain.php 10:17-10:24
            and the cursor names it
            - member access App\Plain::enabled [property]
            a parent's key, through a partial table
            - property App\Plain::title @ src/Plain.php 9:17-9:22
            a name no table holds goes nowhere
            - (not found)
        "#]],
    );
}
