//! What is on offer at a place in the file, and what accepting it leaves behind.
//!
//! The snapshot renders the *result* of applying each offer, not the edit. An edit is a range and a
//! string, and reading one tells you nothing about whether the file still parses -- which is the
//! only thing that matters about a fix.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// Every offer at the marker, each shown as the line it would leave behind.
pub(super) fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);
    let text = world.text_of(file);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let at = phrost_model::TextRange::empty(position.offset);
        let request = crate::ActionRequest {
            view: &view,
            at,
            only: &[],
            asked: Default::default(),
        };
        let offered = code_actions(&request);
        if offered.is_empty() {
            return "- (nothing on offer)\n".to_owned();
        }

        let mut out = String::new();
        for action in offered {
            let _ = writeln!(
                out,
                "- {}{}",
                action.title,
                if action.preferred {
                    "  [preferred]"
                } else {
                    ""
                }
            );
            for edit in &action.edits {
                let start = usize::from(edit.range.start());
                let end = usize::from(edit.range.end());
                let applied = format!("{}{}{}", &text[..start], edit.text, &text[end..]);
                // The line the edit lands on, in the file as it would then be: enough to see that
                // the result parses and lands where it should, without reprinting the file.
                let at = applied[..start + edit.text.len()]
                    .rfind('\n')
                    .map_or(0, |at| at + 1);
                let to = applied[at..].find('\n').map_or(applied.len(), |to| at + to);
                let _ = writeln!(out, "    leaves `{}`", applied[at..to].trim());
            }
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_name_that_resolves_to_nothing_offers_the_class_it_could_have_meant() {
    check(
        r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $thing = new No@here@de();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $node
                leaves `$thing = new Node();`
                leaves `$thing = $node;`
            - Import App\Entity\Node  [preferred]
                leaves `use App\Entity\Node;`
            - Fix the 1 problem phrost can fix
                leaves `use App\Entity\Node;`
        "#]],
    );
}

#[test]
fn two_classes_of_the_same_name_are_both_offered_and_neither_is_preferred() {
    // Which one is meant is not something the index knows, and a keystroke that applied the first of
    // them would be picking a class at random.
    //
    // The order is the shallower namespace first, which is the whole reason it is pinned here:
    // alphabetical order put a contrib module's plugin above the entity everybody means when four
    // classes in a real project were all called `NodeType`.
    check(
        r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node {}

//- /src/Tree/Node.php
<?php
namespace App\Tree;

class Node {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $thing = new No@here@de();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $node
                leaves `$thing = new Node();`
                leaves `$thing = $node;`
            - Import App\Tree\Node
                leaves `use App\Tree\Node;`
            - Import App\Entity\Node
                leaves `use App\Entity\Node;`
        "#]],
    );
}

#[test]
fn a_name_that_already_resolves_offers_nothing() {
    check(
        r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node {}

//- /src/Caller.php
<?php
namespace App;

use App\Entity\Node;

class Caller
{
    public function run(): void
    {
        $thing = new No@here@de();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $node
                leaves `$thing = new Node();`
                leaves `$thing = $node;`
        "#]],
    );
}

/// A name a `use` line already holds cannot be imported from somewhere else, even when the index
/// has never seen what that line names -- a dependency not installed. The fix for that is to the
/// line, and a second `use` for the same name is a fatal error.
#[test]
fn a_name_held_by_an_import_of_something_undeclared_offers_no_second_import() {
    check(
        r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node {}

//- /src/Caller.php
<?php
namespace App;

use Vendor\NotInstalled\Node;

class Caller
{
    public function run(): void
    {
        $thing = new No@here@de();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $node
                leaves `$thing = new Node();`
                leaves `$thing = $node;`
        "#]],
    );
}

#[test]
fn a_receiver_that_may_be_null_offers_the_nullsafe_operator() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing
{
    public function name(): string
    {
        return '';
    }
}

class Repo
{
    public function find(): ?Thing
    {
        return NULL;
    }

    public function run(): void
    {
        $thing = $this->find();
        $thing->na@here@me();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $name
                leaves `$thing->name();`
                leaves `$name;`
            - Call name() only when not null  [preferred]
                leaves `$thing?->name();`
        "#]],
    );
}

#[test]
fn a_receiver_a_guard_has_already_settled_offers_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing
{
    public function name(): string
    {
        return '';
    }
}

class Repo
{
    public function find(): ?Thing
    {
        return NULL;
    }

    public function run(): void
    {
        $thing = $this->find();
        if ($thing === NULL) {
            return;
        }
        $thing->na@here@me();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $name
                leaves `$thing->name();`
                leaves `$name;`
        "#]],
    );
}

#[test]
fn an_access_already_written_nullsafe_offers_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing
{
    public function name(): string
    {
        return '';
    }
}

class Repo
{
    public function find(): ?Thing
    {
        return NULL;
    }

    public function run(): void
    {
        $thing = $this->find();
        $thing?->na@here@me();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $name
                leaves `$thing?->name();`
                leaves `$name;`
        "#]],
    );
}

#[test]
fn the_operator_is_found_where_it_is_written_and_not_where_it_would_be() {
    // `$thing  ->  name()` is legal PHP, so the edit cannot be derived by arithmetic from either the
    // receiver's range or the member's.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing
{
    public function name(): string
    {
        return '';
    }
}

class Repo
{
    public function find(): ?Thing
    {
        return NULL;
    }

    public function run(): void
    {
        $thing = $this->find();
        $thing   ->   na@here@me();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $name
                leaves `$thing   ->   name();`
                leaves `$name;`
            - Call name() only when not null  [preferred]
                leaves `$thing   ?->   name();`
        "#]],
    );
}

#[test]
fn a_break_above_the_name_withdraws_the_import_offer() {
    // Recovery may have dropped the `use` block, in which case every import in the file looks
    // missing -- and adding one that is already written is a fatal error, not a fix.
    check(
        r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node {}

//- /src/Caller.php
<?php
namespace App;

use App\Entity\Node;

class Caller
{
    public function run(): void
    {
        $oops = 1
        $thing = new No@here@de();
    }
}
"#,
        "here",
        expect![[r#"
            - Extract $node
                leaves `$thing = new Node();`
                leaves `$thing = $node;`
        "#]],
    );
}
