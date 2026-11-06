//! A property whose only declaration is a key of the table its class's body states is not renamed.
//!
//! Goto lands on `$slots['enabled']`, which is right, and is what made the property an ordinary
//! member to the rename -- which would then rewrite the key and the accesses on the class and leave
//! the same name standing where the framework stores it and in every read through a list of that
//! kind. Neutral names, as in the view's test of the same table.

use expect_test::expect;
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{rename_refusal, rename_target};

#[test]
fn a_key_of_a_declared_table_refuses_and_a_declared_property_beside_it_does_not() {
    let spec = FixtureSpec::parse(
        r#"
//- /src/Plain.php
<?php
namespace App;

#[Element('plain')]
class Plain
{
    public string $declared = '';

    public function __get(string $name): mixed { return null; }

    public static function shapeOf($definition): array
    {
        $slots['enabled'] = Slot::create('flag');
        return $slots;
    }

    public function read(): void
    {
        $a = $this->en@tabled@abled;
        $b = $this->dec@declared@lared;
    }
}
"#,
    );
    let world = World::build_with(&spec, |world| {
        world.add_declared_table(
            "Element",
            "shapeOf",
            &["Slot"],
            &[("flag", phrost_model::BuiltinTy::Bool)],
            "element",
        );
    });

    let mut out = String::new();
    for marker in ["tabled", "declared"] {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let view = IndexView::in_file(world.skeleton(), &open, cursor);
        let answer = rename_target(&view, position.offset)
            .map(|target| rename_refusal(&view, &target, Some("renamed")))
            .map_or("nothing to rename".to_owned(), |refusal| {
                refusal.unwrap_or_else(|| "allowed".to_owned())
            });
        out.push_str(&format!("@{marker}: {answer}\n"));
    }
    spec.markers().assert_all_used();
    expect![[r#"
        @tabled: `enabled` is a key of the table `Plain` states, not a property PHP declares -- the same name is read through every value of that kind and stored under it, and a rename can reach neither
        @declared: allowed
    "#]]
    .assert_eq(&out);
}
