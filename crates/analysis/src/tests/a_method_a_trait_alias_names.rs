//! `use Greets { hello as greet; }` and then `$this->greet()`.
//!
//! The alias is a method of the class, under a name the trait never declares. Goto from a call goes
//! to the trait's `hello`, which is where the code is -- but a search or a rename at `greet` is about
//! the name, and reading it as `hello` renamed the trait's method, the class's own `hello` and the
//! clause's left half while every call to `greet` kept the old name.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{reference_target, references_in, rename_refusal, rename_target};

const FIXTURE: &str = r#"
//- /src/Greets.php
<?php
namespace App;

trait Greets
{
    public function hello(): string
    {
        return 'hi';
    }
}

//- /src/Person.php
<?php
namespace App;

class Person
{
    use Greets {
        hello as protected gr@clause@eet;
    }

    public function hello(): string
    {
        return 'own ' . $this->gr@call@eet();
    }
}

class Child extends Person
{
    public function twice(): string
    {
        return $this->greet() . $this->hello();
    }
}
"#;

/// For each marker: what a search and a rename are about, and every range each lists across files.
fn check(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();
    for marker in markers {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let at = IndexView::in_file(world.skeleton(), &open, cursor);
        for (asked, target) in [
            ("search", reference_target(&at, position.offset)),
            ("rename", rename_target(&at, position.offset)),
        ] {
            let Some(target) = target else {
                let _ = writeln!(out, "@{marker} {asked}: nothing");
                continue;
            };
            if asked == "rename"
                && let Some(why) = rename_refusal(&at, &target, None)
            {
                let _ = writeln!(out, "@{marker} rename refused: {why}");
                continue;
            }
            for file in spec.files() {
                let id = world.file_id(file.path());
                let view = IndexView::in_file(world.skeleton(), &open, id);
                for range in references_in(&view, &target) {
                    let _ = writeln!(
                        out,
                        "@{marker} {asked} {} {} {}",
                        file.path(),
                        world.render_range(id, range),
                        &world.text_of(id)[range]
                    );
                }
            }
        }
    }
    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn a_search_and_a_rename_at_the_alias_are_about_the_alias() {
    check(
        FIXTURE,
        &["call", "clause"],
        expect![[r#"
            @call search src/Person.php 7:28-7:33 greet
            @call search src/Person.php 12:32-12:37 greet
            @call search src/Person.php 20:23-20:28 greet
            @call rename src/Person.php 7:28-7:33 greet
            @call rename src/Person.php 12:32-12:37 greet
            @call rename src/Person.php 20:23-20:28 greet
            @clause search src/Person.php 7:28-7:33 greet
            @clause search src/Person.php 12:32-12:37 greet
            @clause search src/Person.php 20:23-20:28 greet
            @clause rename src/Person.php 7:28-7:33 greet
            @clause rename src/Person.php 12:32-12:37 greet
            @clause rename src/Person.php 20:23-20:28 greet
        "#]],
    );
}

/// A subclass declaring a method of the alias's name overrides it, so renaming the alias alone
/// would stop the override from overriding.
#[test]
fn a_rename_is_refused_where_a_subclass_overrides_the_alias() {
    check(
        &FIXTURE.replace(
            "    public function twice(): string",
            "    public function greet(): string\n    {\n        return 'child';\n    }\n\n    public function twice(): string",
        ),
        &["call", "clause"],
        expect![[r#"
            @call search src/Person.php 7:28-7:33 greet
            @call search src/Person.php 12:32-12:37 greet
            @call rename refused: `greet` is overridden by `Child`, which declares a method of that name -- renaming the alias alone would stop it overriding
            @clause search src/Person.php 7:28-7:33 greet
            @clause search src/Person.php 12:32-12:37 greet
            @clause rename refused: `greet` is overridden by `Child`, which declares a method of that name -- renaming the alias alone would stop it overriding
        "#]],
    );
}

/// The dialogue opens on the alias in the clause as well as on a call, since both are edited.
#[test]
fn the_rename_dialogue_opens_on_the_clause() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build(&spec);
    let mut out = String::new();
    for marker in ["clause", "call"] {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(file));
        let view = IndexView::in_file(world.skeleton(), &open, file);
        let range = crate::rename_span(&view, position.offset).expect("a span");
        let _ = writeln!(out, "@{marker} {}", world.render_range(file, range));
    }
    spec.markers().assert_all_used();
    expect![[r#"
        @clause 7:28-7:33
        @call 12:32-12:37
    "#]]
    .assert_eq(&out);
}
