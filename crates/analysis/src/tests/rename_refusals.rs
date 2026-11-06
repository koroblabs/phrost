//! The renames that must not go ahead, and what they say instead.
//!
//! Rename is the one request where a wrong answer destroys code, so a decline is a feature and not
//! a gap. What is checked here is the word PHP reserves, where four edits in four files produced a
//! project that no longer parses -- and, beside it, the renames that must *not* be declined.
//!
//! A member another class in the hierarchy declares used to be the other refusal. It is now the
//! family rename, `rename_family.rs`; what remains of it here is that the hierarchy is allowed from
//! either side, which is the half that used to be wrong.
//!
//! The messages are snapshotted whole. A refusal a person cannot act on is barely better than a
//! silent one, so what it says is the behaviour.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{rename_refusal, rename_target};

/// Renders, for each marked cursor, whether renaming to `new_name` is refused and why.
fn check(fixture: &str, markers: &[(&str, &str)], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();

    for (marker, new_name) in markers {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let view = IndexView::in_file(world.skeleton(), &open, cursor);

        let Some(target) = rename_target(&view, position.offset) else {
            let _ = writeln!(out, "@{marker} -> nothing to rename");
            continue;
        };
        let refusal = rename_refusal(&view, &target, Some(new_name));
        let _ = writeln!(
            out,
            "@{marker} -> {new_name}: {}",
            refusal.as_deref().unwrap_or("allowed")
        );
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

const HIERARCHY: &str = r#"
//- /src/Processor.php
<?php
namespace App;

interface ProcessorInterface
{
    public function proc@interface@ess(string $input): string;
}

//- /src/Impl.php
<?php
namespace App;

class Processor implements ProcessorInterface
{
    public function proc@implementation@ess(string $input): string
    {
        return $this->tid@private@y($input);
    }

    private function tidy(string $input): string
    {
        return trim($input);
    }
}

//- /src/Alone.php
<?php
namespace App;

class Alone
{
    public function on@alone@ly(): void {}
}
"#;

#[test]
fn a_member_the_hierarchy_declares_twice_is_allowed_from_either_side() {
    // The whole family moves, so both ends of the contract are a rename rather than a refusal --
    // and a private helper and a method on a class nobody extends, which are the commonest renames
    // there are and must not pay for any of it.
    check(
        HIERARCHY,
        &[
            ("interface", "run"),
            ("implementation", "run"),
            ("private", "clean"),
            ("alone", "run"),
        ],
        expect![[r#"
            @interface -> run: allowed
            @implementation -> run: allowed
            @private -> clean: allowed
            @alone -> run: allowed
        "#]],
    );
}

#[test]
fn a_word_php_reserves_is_refused_for_a_class_and_allowed_for_a_local() {
    // PHP has let a variable be called `$list` forever, so the reserved words are asked about where
    // the language asks about them and nowhere else.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class R@class@epo
{
    public function run(): void
    {
        $ro@local@ws = [];
    }
}
"#,
        &[("class", "class"), ("local", "list")],
        expect![[r#"
            @class -> class: `class` is a word PHP reserves, so no class can be called it -- `Repo` would stop parsing in every file this touched
            @local -> list: allowed
        "#]],
    );
}

/// A method the language calls itself, in both directions.
///
/// Nothing in a project writes `__construct`: the call site is a `new`, `__toString`'s is a cast,
/// `__clone`'s is a `clone`. So a rename edits the declaration, reports success, and leaves a class
/// whose constructor no longer runs -- the one shape where "no references found" is not evidence.
#[test]
fn a_method_php_calls_itself_is_refused_either_way() {
    check(
        r#"
//- /src/Thing.php
<?php
namespace App;

class Thing
{
    public function __cons@construct@truct(public string $name) {}

    public function __toS@tostring@tring(): string
    {
        return $this->name;
    }

    public function bu@ordinary@ild(): void {}

    public function __hel@own@per(): void {}
}
"#,
        &[
            ("construct", "make"),
            ("tostring", "render"),
            ("ordinary", "__construct"),
            ("own", "helper"),
        ],
        expect![[r#"
            @construct -> make: `__construct` is a method PHP calls itself -- nothing in the project writes the name, so renaming it would edit the declaration and leave every place the language reaches it calling something that is gone
            @tostring -> render: `__toString` is a method PHP calls itself -- nothing in the project writes the name, so renaming it would edit the declaration and leave every place the language reaches it calling something that is gone
            @ordinary -> __construct: `__construct` is a method PHP calls itself, so renaming `build` to it would make every existing call reach the language's own meaning instead
            @own -> helper: allowed
        "#]],
    );
}
