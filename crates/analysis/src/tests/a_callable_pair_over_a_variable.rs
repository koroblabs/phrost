//! `[$formatter, 'format']` -- a callable written as a pair whose first element is a variable.
//!
//! Goto from the string reached the method, and nothing went the other way: a search from the method
//! listed its calls and not the pair, so a rename moved the method and left the pair naming one that
//! no longer exists. `[$this, 'm']` never had the gap, because the lowering reads that form as a
//! member reference; a variable beside a string is data as often as it is a callable, so the pair is
//! *presumed* -- kept only where the variable's class has the method.

use std::fmt::Write as _;

use expect_test::expect;

const FIXTURE: &str = r#"
//- /src/Formatter.php
<?php
namespace App;

class Formatter
{
    public function form@here@at(): string
    {
        return '';
    }

    public function asc(): void {}
}

class Client
{
    public function go(Formatter $formatter, int $id): void
    {
        $formatter->format();
        $callback = [$formatter, 'format'];
        $sorted = [$id, 'asc'];
        $other = [$formatter, 'missing'];
    }
}
"#;

#[test]
fn a_search_and_a_rename_from_the_method_reach_the_pair() {
    check(
        FIXTURE,
        expect![[r#"
            search 6:21-6:27 format
            search 18:21-18:27 format
            search 19:35-19:41 format
            rename 6:21-6:27 format
            rename 18:21-18:27 format
            rename 19:35-19:41 format
        "#]],
    );
}

/// `[$id, 'asc']` holds an int, which has no methods: the pair is data and stays out of the list for
/// a method of that name.
#[test]
fn a_pair_over_a_value_without_the_method_is_data() {
    check(
        &FIXTURE
            .replace("form@here@at", "format")
            .replace("function asc()", "function a@here@sc()"),
        expect![[r#"
            search 11:21-11:24 asc
            rename 11:21-11:24 asc
        "#]],
    );
}

fn check(fixture: &str, expect: expect_test::Expect) {
    let spec = phrost_testkit::FixtureSpec::parse(fixture);
    let world = phrost_harness::World::build(&spec);
    let position = spec.markers().position("here");
    let file = world.file_id(&position.path);
    let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
    let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);

    let mut out = String::new();
    for (asked, target) in [
        ("search", crate::reference_target(&view, position.offset)),
        ("rename", crate::rename_target(&view, position.offset)),
    ] {
        let target = target.expect("a method");
        for range in crate::references_in(&view, &target) {
            let _ = writeln!(
                out,
                "{asked} {} {}",
                world.render_range(file, range),
                &world.text_of(file)[range]
            );
        }
    }
    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

/// A pair over a value that may be null builds an array and calls nothing, so it is not a null
/// receiver -- the call it names is where null would matter, and that is not here.
#[test]
fn a_pair_over_a_nullable_value_is_not_a_null_receiver() {
    crate::tests::diagnostics::check(
        r#"
//- /src/Formatter.php
<?php
namespace App;

class Formatter
{
    public function format(): string
    {
        return '';
    }
}

class Client
{
    public function go(?Formatter $formatter): array
    {
        return [$formatter, 'format'];
    }
}
"#,
        expect![""],
    );
}

/// The dialogue opens on the pair's string, which the rename edits.
#[test]
fn the_rename_dialogue_opens_on_the_pair() {
    expect![[r#"
        19:35-19:41 format
    "#]]
    .assert_eq(&span(
        &FIXTURE
            .replace("form@here@at", "format")
            .replace("[$formatter, 'format']", "[$formatter, 'for@here@mat']"),
        "here",
    ));
}

/// Where the rename dialogue opens for the marked cursor -- which must be where the rename edits.
fn span(fixture: &str, marker: &str) -> String {
    let spec = phrost_testkit::FixtureSpec::parse(fixture);
    let world = phrost_harness::World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);
    let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
    let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
    spec.markers().assert_all_used();
    match crate::rename_span(&view, position.offset) {
        Some(range) => {
            let range = crate::rename_range(world.text_of(file), range);
            format!(
                "{} {}\n",
                world.render_range(file, range),
                &world.text_of(file)[range]
            )
        }
        None => "the dialogue does not open\n".to_owned(),
    }
}
