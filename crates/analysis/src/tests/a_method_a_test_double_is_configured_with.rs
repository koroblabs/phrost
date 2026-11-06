//! `$repo->method('find')`: a test double configured by naming a method of what it stands in for.
//!
//! The double's type says what that is -- PHPUnit's `createMock(Repo::class)` is a `MockObject&Repo`
//! -- so the string is the method `->find()` would reach on the same value. Read as nothing, a rename
//! of `Repo::find` left every test configuring it naming the old method, which PHPUnit reports only
//! when the test runs. Both spellings: on the double itself, and after a builder call made on it.
//!
//! Fixture names stand in for PHPUnit's: the rule is stated, not detected.

use expect_test::expect;

use crate::tests::{Query, check_with};

const FIXTURE: &str = r#"
//- /src/Repo.php
<?php
namespace App;

interface Double
{
    public function expects(int $count): object;

    public function method(string $name): object;
}

class Repo
{
    public function find(): void {}

    protected function stamp(): int
    {
        return 0;
    }
}

class RepoTest
{
    public function go(Double&Repo $repo): void
    {
        $repo->method('fi@here@nd');
        $repo->expects(1)->method('find');
        $repo->method('stamp');
        $repo->method('missing');
    }
}
"#;

fn rule(world: &mut phrost_harness::World) {
    world.set_stand_in_method("App\\Double", "method", 0, &["expects"]);
}

#[test]
fn the_string_reaches_the_method_it_configures() {
    check_with(
        FIXTURE,
        &[Query::goto("the configured method", "here")],
        rule,
        expect![[r#"
            the configured method
            - method App\Repo::find @ src/Repo.php 13:21-13:25 (full 13:5-13:36)
        "#]],
    );
}

/// What the string can name: the subject's methods, protected ones included -- a double replaces
/// both -- and none of the double's own.
#[test]
fn completion_offers_the_methods_of_what_the_double_stands_in_for() {
    check_with(
        &FIXTURE
            .replace("fi@here@nd", "find")
            .replace("$repo->method('missing');", "$repo->method('@typed@');"),
        &[Query::completion("in an empty string", "typed")],
        rule,
        expect![[r#"
            in an empty string
            - find [method]  : void
            - stamp [method]  : int
            - accepting the first leaves `$repo->method('find');`
        "#]],
    );
}

/// A rename edits both spellings with the declaration, and leaves a string naming nothing alone.
#[test]
fn a_rename_edits_every_test_that_configures_the_method() {
    use std::fmt::Write as _;

    let spec = phrost_testkit::FixtureSpec::parse(FIXTURE);
    let world = phrost_harness::World::build_with(&spec, rule);
    let position = spec.markers().position("here");
    let file = world.file_id(&position.path);
    let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
    let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
    let target = crate::rename_target(&view, position.offset).expect("a method to rename");
    let mut out = String::new();
    for range in crate::references_in(&view, &target) {
        let _ = writeln!(out, "{}", world.render_range(file, range));
    }
    expect![[r#"
        13:21-13:25
        25:24-25:28
        26:36-26:40
    "#]]
    .assert_eq(&out);
}

/// Naming a protected method is how a double configures one, and nothing is called from the test --
/// so no visibility is reported for the string, and the real call beside it is the control that the
/// check ran. A string naming nothing is not a missing method either: the double rejects it at run
/// time, and a presumed access with no member is not an access.
#[test]
fn a_protected_method_a_double_names_is_not_an_access() {
    let with_a_call = FIXTURE.replace(
        "        $repo->method('missing');\n",
        "        $repo->method('missing');\n        $repo->stamp();\n",
    );
    crate::tests::diagnostics::check_framework(
        &with_a_call,
        rule,
        expect![[r#"
            29:16-29:21 error [inaccessible-member] Cannot access protected method App\Repo::stamp()
        "#]],
    );
}
