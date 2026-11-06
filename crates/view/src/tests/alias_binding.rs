//! The name a file gave an imported class, as a fact of its own.
//!
//! Neither a declaration nor a reference to one: `use App\Value\UUIDv7 as QuoteId;` creates a name
//! that means the class in this file and nothing anywhere else, and replacing it with whatever the
//! class is renamed to leaves the binding still saying `as QuoteId`. Anything that has to edit the
//! name -- rather than the class -- needs this projection, and re-deriving it from the imports is
//! how a rename came to promise `"QuoteId"` and write `UUIDv7`.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// Renders, for each marked cursor, the alias found there and every range in the file that writes
/// it.
fn check(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();

    for marker in markers {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let open = world.open(OpenScope::CursorFileOnly, Some(file));
        let view = IndexView::in_file(world.skeleton(), &open, file);

        let Some(alias) = view.alias_at(position.offset) else {
            let _ = writeln!(out, "@{marker} -> not an alias");
            continue;
        };
        let _ = writeln!(
            out,
            "@{marker} -> {} = {}, bound at {}",
            alias.name,
            alias.path,
            world.render_range(file, alias.range)
        );
        for range in view.alias_occurrences(alias.name.as_str()) {
            let _ = writeln!(out, "    {}", world.render_range(file, range));
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn an_alias_is_found_from_its_binding_and_from_every_use_of_it() {
    check(
        r#"
//- /src/Uses.php
<?php
namespace App\Other;

use App\Value\UUIDv7 as Quo@binding@teId;
use App\Value\Money;

class Uses
{
    public function make(Quo@use@teId $id, Mo@plain@ney $paid): QuoteId
    {
        return $id;
    }
}
"#,
        &["binding", "use", "plain"],
        expect![[r#"
            @binding -> QuoteId = App\Value\UUIDv7, bound at 4:25-4:32
                4:25-4:32
                9:26-9:33
                9:53-9:60
            @use -> QuoteId = App\Value\UUIDv7, bound at 4:25-4:32
                4:25-4:32
                9:26-9:33
                9:53-9:60
            @plain -> not an alias
        "#]],
    );
}

#[test]
fn a_namespace_alias_is_written_in_the_head_segment_and_nowhere_else() {
    // `N\Repo` writes the alias and a class name in one range. A cursor on the head is on the
    // alias; a cursor on `Repo` is on the class, and the occurrence that comes back covers only
    // the head -- because a range covering both would have a rename replace `Repo`.
    check(
        r#"
//- /src/Uses.php
<?php
namespace App\Other;

use App\Nested as @binding@N;

class Uses
{
    public function make(@head@N\Repo $repo, N\R@tail@epo $other): void {}
}
"#,
        &["binding", "head", "tail"],
        expect![[r#"
            @binding -> N = App\Nested, bound at 4:19-4:20
                4:19-4:20
                8:26-8:27
                8:40-8:41
            @head -> N = App\Nested, bound at 4:19-4:20
                4:19-4:20
                8:26-8:27
                8:40-8:41
            @tail -> not an alias
        "#]],
    );
}
