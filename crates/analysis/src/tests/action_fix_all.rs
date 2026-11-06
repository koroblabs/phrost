//! `source.fixAll`: every fix in the file nobody has to choose, as one edit.
//!
//! The whole file after the edit is snapshotted, because the feature is what the file looks like
//! afterwards: two docblocks brought up to date, a `use` written for the one class that could have
//! been meant, and the nullable receiver left exactly as it was -- that fix changes what the code
//! does, and a save must not.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{ActionKind, code_actions};

/// The cursor file with the fix-all action applied, or the titles offered when none is, in a
/// project that switched on the checks `asked` names.
fn check(fixture: &str, style: Option<&str>, asked: &[&str], expect: Expect) {
    let asked: Vec<String> = asked.iter().map(|code| (*code).to_owned()).collect();
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    if let Some(style) = style {
        world.set_house_style(style);
    }
    let world = world;
    let position = spec.markers().position("here");
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[ActionKind::FixAll],
            asked: crate::AskedFor {
                codes: &asked,
                off: &[],
            },
        };
        let Some(action) = code_actions(&request).into_iter().next() else {
            return "(nothing to fix)\n".to_owned();
        };
        let mut text = world.text_of(file).to_owned();
        let mut edits = action.edits.clone();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        for edit in edits {
            let range = usize::from(edit.range.start())..usize::from(edit.range.end());
            text.replace_range(range, &edit.text);
        }
        format!("{}\n---\n{}", action.title, text.replace("    ", "····"))
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// The checks a project documenting its code switches on, which is what the tag fixes repair.
const DOCUMENTED: &[&str] = &["doc-missing-tags", "doc-missing-throws"];

/// Two docblocks missing tags, a class used without its `use`, and a nullable receiver.
const REPO: &str = r#"
//- /src/Entity/Node.php
<?php
namespace App\Entity;

class Node
{
    public function id(): int { return 1; }
}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     */
    public function find(int $id, bool $fresh): ?Repo
    {
        return NULL;
    }

    /**
     * Counts them.
     *
     * @return int
     *   The count.
     */
    public function count(string $kind): int
    {
        return 0;
    }

    /**
     * Reads the id of what it found, which may be nothing.
     */
    public function idOf(int $id): void
    {
        $node = new Node();
        $this->find($id, TRUE)->count('x');
    }
}
@here@
"#;

#[test]
fn every_unattended_fix_in_the_file_is_one_edit() {
    check(
        REPO,
        Some("drupal"),
        DOCUMENTED,
        expect![[r#"
            Fix the 4 problems phrost can fix
            ---
            <?php
            namespace App;
            use App\Entity\Node;

            class Repo
            {
            ····/**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· * @param bool $fresh
            ···· *   The fresh.
            ···· *
            ···· * @return ?\App\Repo
            ···· *   The repo.
            ···· */
            ····public function find(int $id, bool $fresh): ?Repo
            ····{
            ········return NULL;
            ····}

            ····/**
            ···· * Counts them.
            ···· *
            ···· * @param string $kind
            ···· *   The kind.
            ···· *
            ···· * @return int
            ···· *   The count.
            ···· */
            ····public function count(string $kind): int
            ····{
            ········return 0;
            ····}

            ····/**
            ···· * Reads the id of what it found, which may be nothing.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· */
            ····public function idOf(int $id): void
            ····{
            ········$node = new Node();
            ········$this->find($id, TRUE)->count('x');
            ····}
            }

        "#]],
    );
}

#[test]
fn a_file_with_nothing_to_fix_offers_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Counts them.
     *
     * @param string $kind
     *   The kind.
     *
     * @return int
     *   The count.
     */
    public function count(string $kind): int
    {
        return 0;
    }
}
@here@
"#,
        Some("drupal"),
        DOCUMENTED,
        expect![[r#"
            (nothing to fix)
        "#]],
    );
}

/// The same file in a project that never switched the docblock checks on: the `use` is written and
/// the docblocks are left as they are.
///
/// A missing `@param` is offered in the menu either way. Written on every save in a project that
/// does not document that way, it is an edit nobody asked for in every file a person touches --
/// over Laravel's own source, 2 321 boilerplate tag lines across 419 files.
#[test]
fn a_fix_for_a_check_the_project_never_switched_on_is_not_applied() {
    check(
        REPO,
        Some("drupal"),
        &[],
        expect![[r#"
            Fix the 1 problem phrost can fix
            ---
            <?php
            namespace App;
            use App\Entity\Node;

            class Repo
            {
            ····/**
            ···· * Finds one.
            ···· */
            ····public function find(int $id, bool $fresh): ?Repo
            ····{
            ········return NULL;
            ····}

            ····/**
            ···· * Counts them.
            ···· *
            ···· * @return int
            ···· *   The count.
            ···· */
            ····public function count(string $kind): int
            ····{
            ········return 0;
            ····}

            ····/**
            ···· * Reads the id of what it found, which may be nothing.
            ···· */
            ····public function idOf(int $id): void
            ····{
            ········$node = new Node();
            ········$this->find($id, TRUE)->count('x');
            ····}
            }

        "#]],
    );
}

/// One save documents a throw all the way up the file, so a second save finds nothing to add.
///
/// `find()` calls `load()`, which throws and does not say so. What a caller can throw was read off
/// what its callees *declare*, so the first save added `@throws` to `load()` alone and the second
/// added it to `find()` -- 32 of the reference project's 117 fixable files changed again on the
/// next save, and a third could reach the callers of `find()`. A callee in the same file is read
/// for what it throws, which is what its docblock says once this save is done.
#[test]
fn a_throw_is_documented_up_the_file_in_one_save() {
    check(
        r#"
//- /src/Failed.php
<?php
namespace App;

class Failed extends \Exception {}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     */
    public function find(int $id): void
    {
        $this->load($id);
    }

    /**
     * Loads one.
     */
    public function load(int $id): void
    {
        throw new Failed();
    }
}
@here@
"#,
        Some("drupal"),
        DOCUMENTED,
        expect![[r#"
            Fix the 2 problems phrost can fix
            ---
            <?php
            namespace App;

            class Repo
            {
            ····/**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @throws \App\Failed
            ···· *   Thrown when the operation fails.
            ···· */
            ····public function find(int $id): void
            ····{
            ········$this->load($id);
            ····}

            ····/**
            ···· * Loads one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @throws \App\Failed
            ···· *   Thrown when the operation fails.
            ···· */
            ····public function load(int $id): void
            ····{
            ········throw new Failed();
            ····}
            }

        "#]],
    );
}
