//! The span a jump is asked *from* -- what the protocol calls `originSelectionRange`.
//!
//! Its own file and its own renderer, because every other goto snapshot in this crate shows where
//! a jump *lands*, and a field a snapshot does not show cannot be tested by it.
//!
//! What it is for: a client underlines this span for a ctrl-click and follows the click inside it.
//! The field is optional, and omitting it is not free -- the client then works the span out of the
//! buffer by its own idea of where a word ends, which for `App\Nested\Thing` is a guess about PHP
//! made by something that is not parsing PHP. Every arm that is asked from a position fills it;
//! the ones nothing points at (a call hierarchy's callers) leave it, and say so here.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{goto_definition, goto_implementation, goto_type_definition};

/// What each position-asked jump says the reader is standing on, sliced from the asking file.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let text = world.text_of(file);
        // Rendered as the text it covers, because that is what a reader sees underlined -- a pair
        // of line and column numbers would pin the same fact and say nothing about whether the
        // span is the name, one segment of it, or the whole expression around it.
        let render = |label: &str, targets: Vec<crate::NavigationTarget>| {
            if targets.is_empty() {
                return format!("{label}: (nothing to go to)\n");
            }
            targets
                .into_iter()
                .map(|target| {
                    // Beside where it lands, because the whole point of the field is that the two
                    // are different places -- and on a declaration, tellingly, the same one.
                    let landing = format!(
                        "{} {}",
                        world.path_of(target.file),
                        world.render_range(target.file, target.selection_range),
                    );
                    match target.origin {
                        None => format!("{label}: (no origin) -> {landing}\n"),
                        Some(origin) => format!(
                            "{label}: `{}` @ {} -> {landing}\n",
                            &text[usize::from(origin.start())..usize::from(origin.end())],
                            world.render_range(file, origin),
                        ),
                    }
                })
                .collect()
        };

        let mut out = render("definition", goto_definition(&view, position.offset));
        out.push_str(&render(
            "type",
            goto_type_definition(&view, position.offset),
        ));
        out.push_str(&render(
            "implementation",
            goto_implementation(&view, position.offset),
        ));
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_qualified_name_is_one_link_and_not_its_segments() {
    // The case a client cannot get right on its own: a backslash is a word character to some
    // editors and a separator to others, so a guessed span is either `Nested` or the whole path
    // depending on which editor is asking.
    check(
        r#"
//- /src/Nested/Thing.php
<?php
namespace App\Nested;

class Thing {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(\App\Nes@here@ted\Thing $given): void {}
}
"#,
        "here",
        expect![[r#"
            definition: `\App\Nested\Thing` @ 6:25-6:42 -> src/Nested/Thing.php 4:7-4:12
            type: `\App\Nested\Thing` @ 6:25-6:42 -> src/Nested/Thing.php 4:7-4:12
            implementation: (nothing to go to)
        "#]],
    );
}

#[test]
fn a_member_access_is_asked_from_the_member_name_alone() {
    // Not the receiver and not the whole chain: `$repo->find()` holds three things a reader could
    // be pointing at, and the jump is about one of them.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing {}

class Repo
{
    public function find(): Thing
    {
        return new Thing();
    }
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->fi@here@nd();
    }
}
"#,
        "here",
        expect![[r#"
            definition: `find` @ 8:16-8:20 -> src/Repo.php 8:21-8:25
            type: `find` @ 8:16-8:20 -> src/Repo.php 4:7-4:12
            implementation: (nothing to go to)
        "#]],
    );
}

#[test]
fn a_declarations_own_name_is_asked_from_itself() {
    // Goto on a declaration answers with that declaration, deliberately -- and the span it was
    // asked from is then the same name in the same file, which is the one case where a client
    // reading only the target learns nothing about where the reader was.
    check(
        r#"
//- /src/Thing.php
<?php
namespace App;

class Th@here@ing {}
"#,
        "here",
        expect![[r#"
            definition: `Thing` @ 4:7-4:12 -> src/Thing.php 4:7-4:12
            type: `Thing` @ 4:7-4:12 -> src/Thing.php 4:7-4:12
            implementation: (nothing to go to)
        "#]],
    );
}

#[test]
fn a_variable_is_asked_from_the_use_and_not_from_its_binding() {
    // The jump lands a line above; the underline belongs where the cursor is. Two ranges in one
    // file, and reading either for the other is what this pins.
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $held = 1;
        $he@here@ld;
    }
}
"#,
        "here",
        expect![[r#"
            definition: `$held` @ 9:9-9:14 -> src/Caller.php 8:9-8:14
            type: (nothing to go to)
            implementation: (nothing to go to)
        "#]],
    );
}

#[test]
fn the_inheritdoc_tag_is_asked_from_the_tag() {
    // The one arm whose origin is not a name: the cursor is inside a comment, and the span that
    // sent it anywhere is the tag itself.
    check(
        r#"
//- /src/BaseService.php
<?php
namespace App;

class BaseService
{
    /**
     * Upstream summary.
     */
    public function label(): string { return 'base'; }
}

//- /src/ChildService.php
<?php
namespace App;

class ChildService extends BaseService
{
    /** {\@inherit@here@doc} */
    public function label(): string { return 'child'; }
}
"#,
        "here",
        expect![[r#"
            definition: `{@inheritdoc}` @ 6:9-6:22 -> src/BaseService.php 9:21-9:26
            type: (nothing to go to)
            implementation: (nothing to go to)
        "#]],
    );
}

#[test]
fn every_implementer_is_linked_from_the_one_name_the_reader_is_on() {
    // Several targets, one origin: the reader is standing on a single name, and each link says so
    // rather than the first one carrying it.
    check(
        r#"
//- /src/Shape.php
<?php
namespace App;

interface Shape {}

//- /src/Circle.php
<?php
namespace App;

class Circle implements Shape {}

//- /src/Square.php
<?php
namespace App;

class Square implements Sh@here@ape {}
"#,
        "here",
        expect![[r#"
            definition: `Shape` @ 4:25-4:30 -> src/Shape.php 4:11-4:16
            type: `Shape` @ 4:25-4:30 -> src/Shape.php 4:11-4:16
            implementation: `Shape` @ 4:25-4:30 -> src/Circle.php 4:7-4:13
            implementation: `Shape` @ 4:25-4:30 -> src/Square.php 4:7-4:13
        "#]],
    );
}
