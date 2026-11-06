//! `@yield` where `@var` or `@return` was meant, and the plurals and typos of the typing tags.
//!
//! The reference project writes `@yield` five times, above a property and four methods, and every
//! type written beside one is invisible to every tool -- which is why its twenty-eight reads of
//! `ProductReviewer::$app` resolved to nothing. Reported where the tag is, since that one word is
//! the whole of the mistake, and fixed by replacing it.
//!
//! The negative half is most of the rule: a tag a framework or a tool invents is never one of these,
//! a docblock that already writes the meant tag is left alone, and an `@` that does not open a line
//! is prose.

use expect_test::expect;

use super::{actions, diagnostics::check};

const FIXTURE: &str = r#"
//- /src/app.php
<?php
namespace App;

class Grade {}

class Report
{
    /**
     * The grade.
     *
     * @yield \App\Grade
     */
    protected mixed $grade;

    /**
     * Every grade.
     *
     * @yi$0eld \App\Grade[]
     *   The grades.
     */
    public function grades(): array { return []; }

    /**
     * @returns int
     * @params string $x
     */
    public function plural(string $x) { return 1; }

    /**
     * @yield int
     * @return int
     */
    public function both(): int { return 1; }

    /**
     * Mentions @yield in prose, and uses {@link Grade}.
     *
     * @internal
     * @dataProvider rows
     * @FieldType(id = "x")
     */
    public function framework(): void {}
}
"#;

#[test]
fn a_tag_no_tool_reads_is_reported_where_it_is_written() {
    check(
        FIXTURE,
        expect![[r#"
            11:9-11:14 warning [doc-misspelt-tag] @yield is not a tag any tool reads, so the type beside it is lost -- this means @var
            18:9-18:14 warning [doc-misspelt-tag] @yield is not a tag any tool reads, so the type beside it is lost -- this means @return
            24:9-24:16 warning [doc-misspelt-tag] @returns is not a tag any tool reads, so the type beside it is lost -- this means @return
            25:9-25:15 warning [doc-misspelt-tag] @params is not a tag any tool reads, so the type beside it is lost -- this means @param
        "#]],
    );
}

#[test]
fn the_fix_writes_the_tag_it_meant() {
    actions::check(
        FIXTURE,
        "0",
        expect![[r#"
            - Replace @yield with @return  [preferred]
                leaves `* @return \App\Grade[]`
            - Fix the 4 problems phrost can fix
                leaves `* @var \App\Grade`
                leaves `* @return \App\Grade[]`
                leaves `* @return int`
                leaves `* @param string $x`
        "#]],
    );
}

/// The shape the reference project writes: a generated `@return array` above the `@yield` that holds
/// the real type. The meant tag is there and says less, so the finding stands and the fix takes the
/// weaker block out with its description and the blank line after it.
const BESIDE_A_WEAKER_TAG: &str = r#"
//- /src/app.php
<?php
namespace App;

class Segment {}

class Report
{
    /**
     * Returns segments.
     *
     * @param int|null $id
     *   Segment ID.
     *
     * @return array
     *   The array.
     *
     * @yi$0eld \App\Segment[]
     *   Segment objects.
     */
    public function segments(?int $id = NULL): array { return []; }
}
"#;

#[test]
fn a_weaker_meant_tag_beside_it_does_not_silence_it() {
    check(
        BESIDE_A_WEAKER_TAG,
        expect![[r#"
            17:9-17:14 warning [doc-misspelt-tag] @yield is not a tag any tool reads, so the type beside it is lost -- the @return it means says less
        "#]],
    );
}

#[test]
fn the_fix_drops_the_weaker_tag_and_keeps_the_type() {
    actions::check(
        BESIDE_A_WEAKER_TAG,
        "0",
        expect![[r#"
            - Replace @yield with @return, dropping the @return that says less  [preferred]
                leaves `* @yield \App\Segment[]`
                leaves `* @return \App\Segment[]`
            - Fix the 1 problem phrost can fix
                leaves `* @yield \App\Segment[]`
                leaves `* @return \App\Segment[]`
        "#]],
    );
}

/// Both edits at once, which is what a client applies -- the whole docblock, to see it left well
/// formed: one blank line between the tags and nothing of the weaker block.
#[test]
fn the_fix_leaves_one_well_formed_docblock() {
    use phrost_harness::{World, with_open_scopes};
    use phrost_testkit::FixtureSpec;
    use phrost_view::IndexView;

    let spec = FixtureSpec::parse(BESIDE_A_WEAKER_TAG);
    let world = World::build(&spec);
    let position = spec.markers().position("0");
    let file = world.file_id(&position.path);
    let text = world.text_of(file).to_owned();
    let applied = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[],
            asked: Default::default(),
        };
        let action = crate::code_actions(&request)
            .into_iter()
            .find(|action| action.title.starts_with("Replace @yield"))
            .expect("the fix is on offer");
        let mut edits = action.edits;
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        let mut applied = text.clone();
        for edit in edits {
            applied.replace_range(
                usize::from(edit.range.start())..usize::from(edit.range.end()),
                &edit.text,
            );
        }
        applied
    });
    let docblock = &applied[applied.find("/**").unwrap()..applied.find("*/").unwrap() + 2];
    expect![[r#"
        /**
             * Returns segments.
             *
             * @param int|null $id
             *   Segment ID.
             *
             * @return \App\Segment[]
             *   Segment objects.
             */"#]]
    .assert_eq(docblock);
}
