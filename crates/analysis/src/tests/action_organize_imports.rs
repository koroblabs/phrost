//! Removing every unused `use` in one edit.
//!
//! Asserted by *applying* the edits and snapshotting the file that comes out, because that is the
//! only thing that says whether it still parses and whether it kept the imports it needed. Reading
//! a list of ranges says neither.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextRange;
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{ActionKind, ActionRequest, code_actions};

/// The file as the source action leaves it, or what it declined to offer.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let file = world.file_id("app.php");
    let text = world.text_of(file);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = ActionRequest {
            view: &view,
            at: TextRange::empty(phrost_model::TextSize::from(0)),
            only: &[ActionKind::OrganizeImports],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request).into_iter().next() else {
            return "- (nothing on offer)\n".to_owned();
        };

        // Applied back to front, so an earlier edit's range still means what it said.
        let mut applied = text.to_owned();
        let mut edits = action.edits;
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        for edit in edits {
            applied.replace_range(
                usize::from(edit.range.start())..usize::from(edit.range.end()),
                &edit.text,
            );
        }
        format!("- {}\n{applied}", action.title)
    });

    expect.assert_eq(&rendered);
}

#[test]
fn every_unused_import_goes_in_one_edit() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;
class Repo {}
class Model {}
class Gone {}
class AlsoGone {}

//- /app.php
<?php
namespace Other;

use App\Repo;
use App\Gone;
use App\Model;
use App\AlsoGone;

function run(Repo $repo, Model $model): void {}
"#,
        expect![[r#"
            - Remove 2 unused imports
            <?php
            namespace Other;

            use App\Repo;
            use App\Model;

            function run(Repo $repo, Model $model): void {}
        "#]],
    );
}

#[test]
fn a_grouped_import_is_left_whole() {
    // Removing a line here would take `Repo` with it, and `Repo` is used. Refusing is the whole
    // point: an offer that quietly broke the file would be worse than no offer.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;
class Repo {}
class Gone {}

//- /app.php
<?php
namespace Other;

use App\{Repo, Gone};

function run(Repo $repo): void {}
"#,
        expect![[r#"
            - (nothing on offer)
        "#]],
    );
}

#[test]
fn a_file_with_nothing_to_remove_offers_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;
class Repo {}

//- /app.php
<?php
namespace Other;

use App\Repo;

function run(Repo $repo): void {}
"#,
        expect![[r#"
            - (nothing on offer)
        "#]],
    );
}

#[test]
fn an_alias_and_the_other_two_symbol_tables_go_too() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;
class Repo {}
class Gone {}
function helper(): void {}
const THING = 1;

//- /app.php
<?php
namespace Other;

use App\Repo;
use App\Gone as Missing;
use function App\helper;
use const App\THING;

function run(Repo $repo): void {}
"#,
        expect![[r#"
            - Remove 3 unused imports
            <?php
            namespace Other;

            use App\Repo;

            function run(Repo $repo): void {}
        "#]],
    );
}
