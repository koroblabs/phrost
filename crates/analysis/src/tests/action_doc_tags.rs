//! `actions::doc_tags`: adding the one tag a docblock left out.
//!
//! The *docblock the edit produces* is snapshotted rather than the text of the edit. The layout is
//! the feature -- for the Drupal house style it is two lines, so showing one of them would pass
//! whatever the other said -- and so is the placement: a `@param` written after the `@return` is
//! valid PHPDoc that every tool renders in the wrong order, and a snapshot of the inserted text
//! alone cannot see it. That is exactly how it shipped.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// The docblock the action titled `title` leaves behind, with the file's own indentation shown.
fn check(fixture: &str, marker: &str, style: Option<&str>, title: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    if let Some(style) = style {
        world.set_house_style(style);
    }
    let world = world;
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let at = phrost_model::TextRange::empty(position.offset);
        let request = crate::ActionRequest {
            view: &view,
            at,
            only: &[],
            asked: Default::default(),
        };
        let Some(action) = code_actions(&request)
            .into_iter()
            .find(|action| action.title == title)
        else {
            return "(nothing offered)\n".to_owned();
        };
        docblock_after(world.text_of(file), &action.edits).replace("    ", "····")
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// The file with every edit applied, cut down to the docblock they land in.
///
/// Applied back to front so an earlier offset is still the offset it was measured at.
fn docblock_after(text: &str, edits: &[crate::TextEdit]) -> String {
    let mut text = text.to_owned();
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
    for edit in edits {
        let range = usize::from(edit.range.start())..usize::from(edit.range.end());
        text.replace_range(range, &edit.text);
    }

    let Some(open) = text.find("/**") else {
        return text;
    };
    let Some(close) = text[open..].find("*/") else {
        return text;
    };
    let mut block = text[open..open + close + 2].to_owned();
    block.push('\n');
    block
}

#[test]
fn a_parameter_the_docblock_left_out_gets_its_own_param_tag() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     */
    public function find(int $@here@id): void
    {
    }
}
"#,
        "here",
        None,
        "Add @param $id",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· * @param int $id The id.
            ···· */
        "#]],
    );
}

#[test]
fn a_return_the_docblock_left_out_gets_its_own_return_tag() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing {}

class Repo
{
    /**
     * Finds one.
     *
     * @param int $id The id.
     */
    public function fi@here@nd(int $id): ?Thing
    {
        return NULL;
    }
}
"#,
        "here",
        None,
        "Add @return",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id The id.
            ···· * @return ?\App\Thing The thing.
            ···· */
        "#]],
    );
}

#[test]
fn drupal_puts_the_description_on_its_own_indented_line() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     */
    public function find(int $@here@id): void
    {
    }
}
"#,
        "here",
        Some("drupal"),
        "Add @param $id",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· */
        "#]],
    );
}

#[test]
fn a_parameter_already_documented_offers_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     *
     * @param int $id The id.
     */
    public function find(int $@here@id): void
    {
    }
}
"#,
        "here",
        None,
        "Add @param $id",
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_declaration_with_no_docblock_at_all_is_left_to_write_a_docblock() {
    // `WriteDocblock` owns this one -- offering a single tag onto a docblock that does not exist
    // would be offering to write half of one.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $@here@id): void
    {
    }
}
"#,
        "here",
        None,
        "Add @param $id",
        expect!["(nothing offered)\n"],
    );
}

/// The reported shape: a docblock that already documents one parameter and the return, and a
/// second parameter to document.
///
/// A `@param` after the `@return` is what this used to write. It parses, and every tool that
/// renders a docblock reads the tags in the order PHPDoc puts them in, so the card comes out with
/// the return in the middle of the parameters.
#[test]
fn a_param_goes_with_the_other_params_and_not_after_the_return() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Item {}

class Repo
{
    /**
     * Finds one.
     *
     * @param string|null $action
     *   What to do.
     *
     * @return \App\Item|null
     *   The item.
     *
     * @throws \RuntimeException
     */
    public function find(?string $action, ?Item $i@here@tem): ?Item
    {
    }
}
"#,
        "here",
        None,
        "Add @param $item",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param string|null $action
            ···· *   What to do.
            ···· * @param ?\App\Item $item The item.
            ···· *
            ···· * @return \App\Item|null
            ···· *   The item.
            ···· *
            ···· * @throws \RuntimeException
            ···· */
        "#]],
    );
}

/// The same in the house style the report came from, and with no `@param` group to join: the tag
/// starts one, so it takes the blank line that keeps it apart from the return below.
#[test]
fn drupal_separates_a_group_a_new_tag_starts() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Item {}

class Repo
{
    /**
     * Finds one.
     *
     * @return \App\Item|null
     *   The item.
     */
    public function find(?Item $i@here@tem): ?Item
    {
    }
}
"#,
        "here",
        Some("drupal"),
        "Add @param $item",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param ?\App\Item $item
            ···· *   The item.
            ···· *
            ···· * @return \App\Item|null
            ···· *   The item.
            ···· */
        "#]],
    );
}

/// A `@return` written under the parameters is a group of its own, and Drupal's standard puts a
/// blank line between two groups -- phpcs fails the docblock without it.
///
/// The distinction the placement alone cannot make: this lands in the same spot a second `@param`
/// would, and needs the separator a second `@param` must not have.
#[test]
fn drupal_separates_a_return_from_the_parameters_above_it() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing {}

class Repo
{
    /**
     * Finds one.
     *
     * @param int $id
     *   The id.
     */
    public function fin@here@d(int $id): ?Thing
    {
    }
}
"#,
        "here",
        Some("drupal"),
        "Add @return",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return ?\App\Thing
            ···· *   The thing.
            ···· */
        "#]],
    );
}

/// A promoted constructor is seven parameters and one line of prose, and documenting it a tag at a
/// time is seven trips through the same dialogue.
///
/// One edit rather than seven: they all go to the same place, and an edit per tag would leave the
/// order to whoever applies them -- where the order is the signature's.
#[test]
fn every_missing_parameter_can_be_documented_at_once() {
    check(
        r#"
//- /src/Service.php
<?php
namespace App;

class Repo {}

class Clock {}

class Service
{
    /**
     * Constructs the service.
     */
    public function __cons@here@truct(
        private Repo $repo,
        private Clock $clock,
        private string $name,
    ) {
    }
}
"#,
        "here",
        Some("drupal"),
        "Update the docblock",
        expect![[r#"
            /**
            ···· * Constructs the service.
            ···· *
            ···· * @param \App\Repo $repo
            ···· *   The repo.
            ···· * @param \App\Clock $clock
            ···· *   The clock.
            ···· * @param string $name
            ···· *   The name.
            ···· */
        "#]],
    );
}

/// One missing tag offers only the single action, which is what keeps the list short for the
/// commonest case: a signature that grew one parameter.
#[test]
fn one_missing_parameter_is_not_offered_twice() {
    const FIXTURE: &str = r#"
//- /src/Service.php
<?php
namespace App;

class Repo {}

class Service
{
    /**
     * Constructs the service.
     *
     * @param \App\Repo $repo
     *   The repository.
     */
    public function __construct(
        private Repo $repo,
        private string $na@here@me,
    ) {
    }
}
"#;

    check(
        FIXTURE,
        "here",
        Some("drupal"),
        "Add @param $name",
        expect![[r#"
            /**
            ···· * Constructs the service.
            ···· *
            ···· * @param \App\Repo $repo
            ···· *   The repository.
            ···· * @param string $name
            ···· *   The name.
            ···· */
        "#]],
    );
    // And the whole-docblock one is not there beside it: one thing missing, one action.
    check(
        FIXTURE,
        "here",
        Some("drupal"),
        "Update the docblock",
        expect!["(nothing offered)\n"],
    );
}

/// Every action on offer at the marker, with the one a keystroke would take marked.
fn offered(fixture: &str, marker: &str, style: Option<&str>) -> String {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    if let Some(style) = style {
        world.set_house_style(style);
    }
    let world = world;
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        // The quick fixes alone: the whole-file `source.fixAll` is on the same menu whenever
        // anything here is, and these tests are about which fix is offered where.
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[crate::ActionKind::QuickFix],
            asked: Default::default(),
        };
        let mut out = String::new();
        for action in code_actions(&request) {
            out.push_str(&format!(
                "- {}{}\n",
                action.title,
                if action.preferred {
                    "  [preferred]"
                } else {
                    ""
                }
            ));
        }
        if out.is_empty() {
            out.push_str("(nothing offered)\n");
        }
        out
    });
    spec.markers().assert_all_used();
    rendered
}

/// A signature that grew a parameter and a return type since its docblock was written, with the
/// marker on the name or on the new parameter.
fn grown(marker_on: &str) -> String {
    let (name, fresh) = match marker_on {
        "name" => ("fi@name@nd", "fresh"),
        _ => ("find", "fr@fresh@esh"),
    };
    format!(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing {{}}

class Repo
{{
    /**
     * Finds one.
     *
     * @param int $id
     *   The id.
     */
    public function {name}(int $id, bool ${fresh}): ?Thing
    {{
        return NULL;
    }}
}}
"#
    )
}

/// Two things missing, so the docblock is brought up to date in one action: the parameter joins
/// the `@param` group and the return starts its own below it, one separator between them -- both
/// land at the same offset and are one edit, or an applier working back to front would have put
/// the return first.
#[test]
fn a_grown_signature_is_documented_in_one_action() {
    check(
        &grown("name"),
        "name",
        Some("drupal"),
        "Update the docblock",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· * @param bool $fresh
            ···· *   The fresh.
            ···· *
            ···· * @return ?\App\Thing
            ···· *   The thing.
            ···· */
        "#]],
    );
    check(
        &grown("name"),
        "name",
        None,
        "Update the docblock",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· * @param bool $fresh The fresh.
            ···· * @return ?\App\Thing The thing.
            ···· */
        "#]],
    );
}

/// On the name the whole-docblock action is the one a keystroke takes, and the single `@return`
/// is still there for whoever wants only that; on the parameter its own tag is what a keystroke
/// takes, with the whole docblock beside it.
#[test]
fn what_is_offered_where_when_two_things_are_missing() {
    let at_name = offered(&grown("name"), "name", Some("drupal"));
    let at_parameter = offered(&grown("fresh"), "fresh", Some("drupal"));
    expect![[r#"
        at the name:
        - Add @return
        - Update the docblock  [preferred]
        at the parameter:
        - Add @param $fresh
        - Update the docblock
    "#]]
    .assert_eq(&format!(
        "at the name:\n{at_name}at the parameter:\n{at_parameter}"
    ));
}

/// Both groups above a tag that reads after them, still one edit and one separator each side.
#[test]
fn both_groups_go_above_a_throws_together() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Thing {}

class Repo
{
    /**
     * Finds one.
     *
     * @throws \RuntimeException
     *   When it cannot.
     */
    public function fi@here@nd(int $id): ?Thing
    {
        return NULL;
    }
}
"#,
        "here",
        Some("drupal"),
        "Update the docblock",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return ?\App\Thing
            ···· *   The thing.
            ···· *
            ···· * @throws \RuntimeException
            ···· *   When it cannot.
            ···· */
        "#]],
    );
}

/// One missing parameter is offered from the name too, which it was not: the only way to document
/// it used to be to stand on the parameter itself.
#[test]
fn one_missing_parameter_is_offered_from_the_name() {
    const FIXTURE: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     */
    public function fi@here@nd(int $id): void
    {
    }
}
"#;
    expect![[r#"
        - Add @param $id  [preferred]
    "#]]
    .assert_eq(&offered(FIXTURE, "here", None));
}

/// A method whose parameters and return are documented and whose body throws something its
/// docblock does not admit to.
const THROWING: &str = r#"
//- /src/spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}

//- /src/Repo.php
<?php
namespace App;

class Missing extends \RuntimeException {}

class Repo
{
    /**
     * Finds one.
     *
     * @param int $id
     *   The id.
     *
     * @return int
     *   The count.
     */
    public function fi@name@nd(int $id): int
    {
        if ($id < 0) {
            th@site@row new Missing('none');
        }
        return $id;
    }
}
"#;

/// The one undocumented throw gets its own tag, from the name and from the `throw` itself -- the
/// place a report about it underlines.
#[test]
fn an_undocumented_throw_gets_its_own_throws_tag() {
    check(
        THROWING.replace("th@site@row", "throw").as_str(),
        "name",
        Some("drupal"),
        "Add @throws \\App\\Missing",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return int
            ···· *   The count.
            ···· *
            ···· * @throws \App\Missing
            ···· *   Thrown when the operation fails.
            ···· */
        "#]],
    );
    let at_the_throw = offered(
        &THROWING.replace("fi@name@nd", "find"),
        "site",
        Some("drupal"),
    );
    expect![[r#"
        - Add @throws \App\Missing  [preferred]
    "#]]
    .assert_eq(&at_the_throw);
}

/// A `@throws` naming an ancestor covers it: `\Exception` is how a docblock is allowed to summarise
/// what a body throws, and demanding the leaf class would be demanding a rewrite of every one.
#[test]
fn a_throws_naming_an_ancestor_covers_the_throw() {
    let fixture = THROWING.replace("th@site@row", "throw").replace(
        " * @return int\n",
        " * @throws \\Exception\n *   When it cannot.\n *\n * @return int\n",
    );
    expect![[r#"
        (nothing offered)
    "#]]
    .assert_eq(&offered(&fixture, "name", Some("drupal")));
}

/// The return and the throws missing together are one action, the throws below the return.
#[test]
fn a_missing_return_and_a_missing_throws_are_one_update() {
    let fixture = THROWING
        .replace("th@site@row", "throw")
        .replace("     *\n     * @return int\n     *   The count.\n", "");
    check(
        &fixture,
        "name",
        Some("drupal"),
        "Update the docblock",
        expect![[r#"
            /**
            ···· * Finds one.
            ···· *
            ···· * @param int $id
            ···· *   The id.
            ···· *
            ···· * @return int
            ···· *   The int.
            ···· *
            ···· * @throws \App\Missing
            ···· *   Thrown when the operation fails.
            ···· */
        "#]],
    );
}

/// A free function's docblock is brought up to date the way a method's is -- the shape a Drupal
/// `.module` file is made of, and the one that had the report and no fix.
#[test]
fn a_free_functions_docblock_is_updated_too() {
    check(
        r#"
//- /src/mymodule.php
<?php

/**
 * Builds the thing.
 *
 * @param array $items
 *   The items.
 */
function mymodule_bu@here@ild(array $items, int $limit): array {
  return array_slice($items, 0, $limit);
}
"#,
        "here",
        Some("drupal"),
        "Update the docblock",
        expect![[r#"
            /**
             * Builds the thing.
             *
             * @param array $items
             *   The items.
             * @param int $limit
             *   The limit.
             *
             * @return array
             *   The array.
             */
        "#]],
    );
}

/// A method whose docblock says nothing, calling one that declares what it throws.
///
/// The marker is on the call, because that is where the report is: a propagated throw is raised at
/// the call site, and a quick fix offered only from the declaration's name would leave a reader
/// with a squiggle and nothing on it.
const THROWN_BY_A_CALL: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Missing extends \Exception {}

class Store
{
    /**
     * Fetches one.
     *
     * @throws \App\Missing
     *   When there is none.
     */
    public function fetch(int $id): int
    {
        throw new Missing('none');
    }
}

class Repo
{
    /**
     * Finds one.
     *
     * @param \App\Store $store
     *   The store.
     * @param int $id
     *   The id.
     *
     * @return int
     *   The count.
     */
    public function find(Store $store, int $id): int
    {
        return $store->fe@site@tch($id);
    }
}
"#;

/// The fix a call's declared throws earns, offered where the report is.
///
/// It had none: the action read what the body *writes*, so every finding raised by a call arrived
/// with nothing to apply. Both now come off one seam -- see `IndexView::undocumented_throws`.
#[test]
fn a_throw_a_call_declares_gets_the_same_tag_offered_at_the_call() {
    // The declaration's name first: if nothing is offered there either, the fixture is the problem
    // and not the placement.
    let at_the_name = offered(
        &THROWN_BY_A_CALL
            .replace("$store->fe@site@tch($id)", "$store->fetch($id)")
            .replace("public function find(", "public function fi@site@nd("),
        "site",
        Some("drupal"),
    );
    expect![[r#"
        - Add @throws \App\Missing  [preferred]
    "#]]
    .assert_eq(&at_the_name);

    let at_the_call = offered(THROWN_BY_A_CALL, "site", Some("drupal"));
    expect![[r#"
        - Add @throws \App\Missing  [preferred]
    "#]]
    .assert_eq(&at_the_call);
}
