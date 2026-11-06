//! The kind filter a client sends, and what it costs an offer that was filtered out.
//!
//! Separate from the offers' own tests on purpose: this is about the generic layer, and it is what
//! keeps the menu affordable as the list of offers grows. An offer the client did not ask for must
//! not run at all rather than run and be discarded.
//!
//! Everything here goes through `with_open_scopes`, so each case is also asserting that the menu
//! does not depend on which other files happen to be open.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextRange;
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{ActionKind, ActionRequest, code_actions};

/// A call that offers something: a member reached through a value that may be null.
const FIXTURE: &str = r#"
//- /src/Repo.php
<?php

class Node
{
    public function title(): string
    {
        return '';
    }
}

class Repo
{
    public function run(?Node $node): void
    {
        $node->ti@at@tle();
    }
}
"#;

/// Every offer for one `only` filter, with the kind it is and the code it repairs.
fn check(only: &[ActionKind], expect: Expect) {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build(&spec);
    let position = spec.markers().position("at");
    let file = world.file_id(&position.path);
    let at = TextRange::empty(position.offset);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = ActionRequest {
            view: &view,
            at,
            only,
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
                "- {}  [{:?}]  fixes {}",
                action.title,
                action.kind,
                action.fixes.unwrap_or("-")
            );
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn an_unfiltered_request_offers_the_quick_fix() {
    // Also the link that lets a client show the fix on the squiggle. The code is the check's own
    // constant, so what this asserts is the wiring rather than the spelling.
    check(
        &[],
        expect![[r#"
            - Extract $title  [Refactor]  fixes -
            - Call title() only when not null  [QuickFix]  fixes possibly-null-receiver
        "#]],
    );
}

#[test]
fn asking_only_for_quick_fixes_offers_the_same() {
    check(
        &[ActionKind::QuickFix],
        expect![[r#"
            - Call title() only when not null  [QuickFix]  fixes possibly-null-receiver
        "#]],
    );
}

#[test]
fn asking_only_for_refactors_offers_nothing_here() {
    // A refactor exists (`unused_import::RemoveUnusedImport`), but nothing at this position is
    // one -- there is no unused import here, only a null receiver -- so a client asking for
    // refactors still gets an empty menu, and does not pay for the quick fixes to find that out.
    check(
        &[ActionKind::Refactor],
        expect![[r#"
            - Extract $title  [Refactor]  fixes -
        "#]],
    );
}

#[test]
fn asking_only_for_source_actions_offers_nothing_here() {
    check(
        &[ActionKind::OrganizeImports],
        expect![[r#"
        - (nothing on offer)
    "#]],
    );
}
