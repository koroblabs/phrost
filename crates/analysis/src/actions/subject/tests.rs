//! Which subjects one range holds, and so which offers get to run.
//!
//! Every one of the four is asked at every marker, and the answer is the whole list rather than the
//! one an offer standing there cares about. The failure this has to catch is a subject reading
//! absent where it is present -- that removes an action from a menu, and an absent menu entry is
//! not something anybody notices -- so a case that only asked about the subject it expected would
//! be blind to exactly that.
//!
//! Every case goes through `with_open_scopes`, so this also asserts that what a range holds does
//! not depend on which other files happen to be open.
//!
//! **A position anywhere inside a body holds a declaration**, including a blank line, and that is
//! deliberate rather than sloppy: `doc-missing-throws` reports a throw a *call* raises, which is a
//! position on no name and no parameter, and the fix for it has to be reachable from the squiggle.
//! The gate only decides whether an offer gets to *look* -- four of the five declaration offers
//! iterate the narrow list and find nothing here, and the fifth's own guards decide what it may
//! offer. See `SubjectSites::declarations_or_their_bodies`.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextRange;
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use super::{ActionSubject, SubjectSites};
use crate::ActionRequest;

const EVERY_SUBJECT: [(ActionSubject, &str); 4] = [
    (ActionSubject::Declaration, "declaration"),
    (ActionSubject::WrittenName, "written name"),
    (ActionSubject::MemberAccess, "member access"),
    (ActionSubject::Import, "import"),
];

/// One fixture with a cursor at each marker, and what each cursor turns out to be on.
fn check(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();
    let width = markers.iter().map(|marker| marker.len()).max().unwrap_or(0);

    for marker in markers {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);

        let rendered = with_open_scopes(&world, Some(file), |open| {
            let view = IndexView::in_file(world.skeleton(), open, file);
            let request = ActionRequest {
                view: &view,
                at: TextRange::empty(position.offset),
                only: &[],
                asked: Default::default(),
            };
            let sites = SubjectSites::of(&request);
            let held = EVERY_SUBJECT
                .iter()
                .filter(|(subject, _)| sites.holds(*subject))
                .map(|(_, name)| *name)
                .collect::<Vec<_>>();
            if held.is_empty() {
                "(nothing)".to_owned()
            } else {
                held.join(", ")
            }
        });

        let _ = writeln!(out, "{marker:width$}  ->  {rendered}");
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

/// One file holding all four sorts at once: a method with no docblock, a name that resolves to
/// nothing, an import nothing names, and a call through a value that may be null.
const FIXTURE: &str = r#"
//- /src/Node.php
<?php
namespace App;

class Node
{
    public function title(): string
    {
        return '';
    }
}

//- /src/Repo.php
<?php
namespace App;

use App\No@used_import@de;
use App\Noth@unused_import@ing;

class Repo
{
    public function ru@method_name@n(?No@written_type@de $no@parameter@de, int $count): void
    {
        $no@receiver@de->ti@member_name@tle();
@blank_line@
        new Miss@unresolved@ing();
    }
}
"#;

#[test]
fn what_each_place_in_a_file_holds() {
    check(
        FIXTURE,
        &[
            "used_import",
            "unused_import",
            "method_name",
            "written_type",
            "parameter",
            "receiver",
            "member_name",
            "blank_line",
            "unresolved",
        ],
        expect![[r#"
            used_import    ->  written name
            unused_import  ->  written name, import
            method_name    ->  declaration
            written_type   ->  written name
            parameter      ->  declaration
            receiver       ->  member access
            member_name    ->  declaration, member access
            blank_line     ->  (nothing)
            unresolved     ->  declaration, written name
        "#]],
    );
}
