//! What a file declares, and finding a declaration by name.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{document_symbols, workspace_symbols};

/// Renders the outline of the fixture's last file.
fn check_outline(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let path = spec
        .files()
        .last()
        .expect("the fixture declares a file")
        .path()
        .to_owned();
    let file = world.file_id(&path);

    // Through the open-versus-closed dimension: an outline is read from the skeleton, so it must not
    // change when the file is open.
    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        for entry in document_symbols(&view) {
            let _ = writeln!(
                out,
                "{} {}{}",
                entry.kind.as_str(),
                entry.label,
                entry
                    .detail
                    .as_ref()
                    .map(|detail| format!("  {detail}"))
                    .unwrap_or_default()
            );
            for child in entry.children {
                let _ = writeln!(
                    out,
                    "  {} {}{}",
                    child.kind.as_str(),
                    child.label,
                    child
                        .detail
                        .as_ref()
                        .map(|detail| format!("  {detail}"))
                        .unwrap_or_default()
                );
            }
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// Renders a project-wide search for `query`.
fn check_search(fixture: &str, query: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let open = world.open(phrost_harness::OpenScope::EveryFile, None);
    let view = IndexView::new(world.skeleton(), &open);

    let rendered = workspace_symbols(&view, query)
        .into_iter()
        .map(|hit| format!("{} {}\n", hit.kind.as_str(), hit.label))
        .collect::<String>();

    expect.assert_eq(&rendered);
}

#[test]
fn a_file_outlines_as_declarations_holding_their_members() {
    check_outline(
        r#"
//- /src/app.php
<?php
namespace App;

const LIMIT = 10;

interface Runs
{
    public function run(): void;
}

enum Stage: string
{
    case Draft = 'draft';
}

class Service implements Runs
{
    public const VERSION = '1';

    private int $count = 0;

    public function run(): void {}

    private function helper(string $name): ?int { return null; }
}

function go(int $times = 1): void {}
"#,
        expect![[r#"
            constant LIMIT
            interface Runs
              method run  run(): void
            enum Stage
              case Draft
            class Service
              const VERSION
              property count  int $count
              method run  run(): void
              method helper  helper(string $name): ?int
            function go  go(int $times = 1): void
        "#]],
    );
}

#[test]
fn a_search_finds_declarations_by_the_start_of_their_short_name() {
    check_search(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repository {}
class RepositoryFactory {}
class Unrelated {}

function report(): void {}

//- /vendor/other/Repo.php
<?php
namespace Other;

class Repository {}
"#,
        "repo",
        expect![[r#"
            function App\report
            class App\Repository
            class App\RepositoryFactory
            class Other\Repository
        "#]],
    );
}

#[test]
fn an_empty_query_finds_nothing_rather_than_everything() {
    check_search(
        r#"
//- /src/Repo.php
<?php
class Repository {}
"#,
        "",
        expect![[r#""#]],
    );
}

/// An anonymous class, which is listed under the name PHP itself gives it.
///
/// Its **members** are the reason. `new class { public function reset(): void {} }` is how Symfony
/// and Laravel write a test double, and filtering the class out of the outline took every method of
/// one with it -- eleven in one Laravel file, invisible to "go to symbol in file" and to the
/// breadcrumb that reads the same answer. Found by `tools/outline-lists-what-is-declared.py`, which
/// asks only that a declaration a text scan of the file finds is in the outline of that file.
///
/// `class@anonymous` and not the synthesised name: the offset that makes it unique stays in the
/// index, and what a reader sees is what PHP's own errors say.
#[test]
fn an_anonymous_class_is_listed_with_the_members_that_are_the_point_of_it() {
    check_outline(
        r#"
//- /src/app.php
<?php
namespace App;

class Resetter
{
    public function double(): object
    {
        return new class {
            public int $counter = 0;

            public function reset(): void
            {
            }
        };
    }
}
"#,
        expect![[r#"
            class Resetter
              method double  double(): object
            class class@anonymous
              property counter  int $counter
              method reset  reset(): void
        "#]],
    );
}
