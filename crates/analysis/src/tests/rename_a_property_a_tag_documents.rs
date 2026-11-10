//! A property the class declares and a `@property` tag documents beside it.
//!
//! `@property-read string $scheme` over `protected $scheme`, served through `__get`: the ordinary
//! way to say what a magic accessor returns for a property that exists. The member table holds
//! one row for the name, the declaration's, so the tag belonged to nothing -- a search left it out
//! and a rename rewrote the declaration and the reads and left the tag naming the old property.
//! The same for a method beside an `@method` tag. A tag that is the only declaration was always
//! found, and is here as the control.

use expect_test::expect;
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{references_in, rename_target};

#[test]
fn the_tag_is_one_of_the_places_the_member_is_named() {
    let spec = FixtureSpec::parse(
        r#"
//- /src/Iri.php
<?php
namespace App;

/**
 * @property-read string $scheme
 * @property int $port
 * @method string render(bool $short)
 */
class Iri
{
    protected $scheme = '';

    public function __get(string $name): mixed
    {
        return $this->$name;
    }

    public function render(bool $short = false): string
    {
        return $this->sch@declared@eme . ':' . $this->po@tagged@rt;
    }

    public function show(): string
    {
        return $this->ren@method@der();
    }
}
"#,
    );
    let world = World::build(&spec);

    let mut out = String::new();
    for marker in ["declared", "tagged", "method"] {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let view = IndexView::in_file(world.skeleton(), &open, cursor);
        let text = spec.file(&position.path).text();
        let places = rename_target(&view, position.offset)
            .map(|target| references_in(&view, &target))
            .unwrap_or_default()
            .into_iter()
            .map(|range| {
                let line = text[..usize::from(range.start())].matches('\n').count() + 1;
                format!("line {line} `{}`", &text[range])
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("@{marker}: {places}\n"));
    }
    spec.markers().assert_all_used();
    expect![[r#"
        @declared: line 5 `$scheme`, line 11 `$scheme`, line 20 `scheme`
        @tagged: line 6 `$port`, line 20 `port`
        @method: line 7 `render`, line 18 `render`, line 25 `render`
    "#]]
    .assert_eq(&out);
}
