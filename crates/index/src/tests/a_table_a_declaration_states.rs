//! The table a declaration's own body states, and the four statements that leave it partial.
//!
//! One fixture and one table, because what decides this is the *refusals*: a statement the walk
//! cannot read must leave the keys it did read and say it did not read them all, and each refusal
//! is an absence in a test of its own.
//!
//! The rule is the one `configured` installs for every test here -- `#[Element('x')]` on a class
//! whose `shapeOf()` writes `$slots['k'] = Slot::create('text')`. Neutral names: what an element
//! is, is a framework's business.

use expect_test::{Expect, expect};

use super::{Order, build, fixture_files};

/// Every table the fixture's declarations state, with what each property holds.
fn check(fixture: &str, expect: Expect) {
    let built = build(&fixture_files(fixture), Order::ByPath);
    let mut found = built
        .ids
        .all()
        .flat_map(|file| built.skeleton.declared_tables_of(file))
        .map(|table| {
            let names = table
                .properties
                .iter()
                .map(|(name, holds)| match holds {
                    Some(holds) => format!("{name}: {}", holds.as_str()),
                    None => format!("{name}: ?"),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let complete = if table.complete { "" } else { " (partial)" };
            format!("{}{complete} -- {names}\n", table.kind)
        })
        .collect::<Vec<_>>();
    found.sort();
    expect.assert_eq(&found.concat());
}

#[test]
fn every_builder_call_is_a_property_and_anything_else_leaves_it_partial() {
    check(
        r#"
//- /src/Plain.php
<?php
namespace App;

#[Element('plain')]
class PlainElement
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text');
        // Whatever is chained on afterwards hands its own receiver back, so what says the type is
        // still the call at the bottom.
        $slots['weight'] = Slot::create('number')->required(TRUE)->weight(-10);
        // A name the rule does not list leaves the property in the table saying nothing: the key
        // is there whatever is in it.
        $slots['body'] = Slot::create('markup');
        // The later write of a key is the one PHP keeps.
        $slots['title'] = Slot::create('number');
        return $slots;
    }
}

//- /src/Partial.php
<?php
namespace App;

#[Element('partial')]
class PartialElement
{
    public static function shapeOf($definition): array
    {
        $slots = parent::shapeOf($definition);
        $slots['title'] = Slot::create('text');
        return $slots;
    }
}

//- /src/Removed.php
<?php
namespace App;

#[Element('removed')]
class RemovedElement
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text');
        // Not read, and not readable: an `unset` is one of the statements phrost does not model,
        // so what it takes away is invisible and the honest answer is that these are not all.
        unset($slots['title']);
        return $slots;
    }
}

//- /src/Branching.php
<?php
namespace App;

#[Element('branching')]
class BranchingElement
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text');
        if ($definition) {
            $slots['extra'] = Slot::create('text');
        }
        return $slots;
    }
}

//- /src/Elsewhere.php
<?php
namespace App;

// No attribute, so no kind to file a table under -- and the body is the same shape.
class NotAnElement
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text');
        return $slots;
    }
}

//- /src/OtherMethod.php
<?php
namespace App;

#[Element('other')]
class OtherElement
{
    // The rule names one method. Another one on the same class is not the table.
    public static function somethingElse($definition): array
    {
        $slots['title'] = Slot::create('text');
        return $slots;
    }
}
"#,
        expect![[r#"
            branching (partial) -- title: string
            partial (partial) -- title: string
            plain -- weight: int, body: ?, title: int
            removed (partial) -- title: string
        "#]],
    );
}
