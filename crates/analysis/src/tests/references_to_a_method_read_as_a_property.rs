//! A method a framework reads as a property of the same name: Eloquent's `$post->author` loads
//! `author()`.
//!
//! Goto from the property already lands on the method, so the references of the method include the
//! property spelling -- otherwise a rename rewrote the declaration and the calls and left every
//! `$post->author` reading a property nothing answers any more. Asked from either end, the answer
//! is the same list.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{reference_target, references_in};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public function __get(string $key): mixed {}
}

/** @template TRelated */
abstract class Relation
{
    /** @return ?TRelated */
    public function getResults() {}
}

/** @template TRelated @extends Relation<TRelated> */
class BelongsTo extends Relation {}

//- /src/Post.php
<?php
namespace App;

use Orm\BelongsTo;
use Orm\Model;

class User extends Model {}

class Post extends Model
{
    /** @return BelongsTo<User> */
    public function au@declared@thor(): BelongsTo {}
}

function run(Post $post): void
{
    $post->author();
    $named = $post->au@property@thor;
}
"#;

fn check(marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, |world| {
        world.contracts_mut().bind_method_property(
            "Orm\\Model",
            "Orm\\Relation",
            "getResults",
            None,
            &[],
        );
    });
    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let target = reference_target(
        &IndexView::in_file(world.skeleton(), &open, cursor),
        position.offset,
    )
    .expect("a target");

    let mut out = String::new();
    for file in spec.files() {
        let id = world.file_id(file.path());
        let view = IndexView::in_file(world.skeleton(), &open, id);
        for range in references_in(&view, &target) {
            let _ = writeln!(out, "{} {}", file.path(), world.render_range(id, range));
        }
    }
    expect.assert_eq(&out);
}

#[test]
fn the_method_counts_its_property_spelling() {
    check(
        "declared",
        expect![[r#"
            src/Post.php 12:21-12:27
            src/Post.php 17:12-17:18
            src/Post.php 18:21-18:27
        "#]],
    );
}

#[test]
fn the_property_spelling_finds_the_same_list() {
    check(
        "property",
        expect![[r#"
            src/Post.php 12:21-12:27
            src/Post.php 17:12-17:18
            src/Post.php 18:21-18:27
        "#]],
    );
}
