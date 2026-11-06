//! What an array *literal* says about one of its slots.
//!
//! The other half of `element_written`, which until now only read the writes that follow an empty
//! literal -- so `$tools = []; $tools[] = new Pen();` said `Pen` and `$tools = [new Pen()]` said
//! nothing at all, which is the shorter and commoner spelling of the same thing.
//!
//! The rule that makes it safe is agreement. A write replaces what was there and so has a last; a
//! literal fills every slot at once, so `[new Pen(), new Pencil()]` really holds both and the honest
//! answer is silence rather than whichever element came last.

use expect_test::expect;
use phrost_model::{FileId, Interner, TextSize};

use crate::{
    PathRoot, ValuePath, element_written, local_name,
    path::{ElementKey, PathStep},
};

/// What the literal or the writes before `$0` put into one slot of `container`.
///
/// `key` is the slot being asked about, as text, and `None` asks about no particular slot -- which
/// is what iterating an array does, and what `$rows[$id]` does when nothing can name `$id`.
pub(super) fn check_element(
    source: &str,
    container: &str,
    key: Option<&str>,
    expect: expect_test::Expect,
) {
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = source.replace("$0", "");

    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let name = local_name(&lowered.hir, offset, container).expect("the container is a local here");
    let path = ValuePath::of(PathRoot::Local(name));

    expect.assert_eq(&match element_written(&lowered.hir, offset, &path, key)
        .map(|write| write.value)
    {
        Some(written) => super::render(&lowered.hir, &written),
        None => "<nothing written>".to_owned(),
    });
}

/// The same, for one slot *of* a slot: `$rows['a']` as the container rather than `$rows`.
///
/// Its own entry point because the container is what the question is about, and a container that is
/// itself a slot is the case a literal written into one produces -- the shape a reader reaches at two
/// levels rather than one.
fn check_slot_element(
    source: &str,
    container: &str,
    slot: &str,
    key: Option<&str>,
    expect: expect_test::Expect,
) {
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = source.replace("$0", "");

    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let name = local_name(&lowered.hir, offset, container).expect("the container is a local here");
    let mut path = ValuePath::of(PathRoot::Local(name));
    path.steps.push(PathStep::Element(Some(ElementKey::Literal(
        interner.name(slot),
    ))));

    expect.assert_eq(&match element_written(&lowered.hir, offset, &path, key)
        .map(|write| write.value)
    {
        Some(written) => super::render(&lowered.hir, &written),
        None => "<nothing written>".to_owned(),
    });
}

#[test]
fn one_element_is_what_the_array_holds() {
    // The case the roadmap called blocked: a receiver reached through `[...]` worked whenever the
    // array had an element type, and a literal carried none.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = [new Pen()];
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["new Pen"],
    );
}

#[test]
fn elements_that_disagree_say_nothing() {
    // Not the last one. Both are in the array, and offering a `Pencil`'s members on a value as
    // likely to be a `Pen` is worse than the silence this returned before.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = [new Pen(), new Pencil()];
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["<nothing written>"],
    );
}

#[test]
fn elements_that_agree_on_a_builtin_answer_once() {
    check_element(
        r#"<?php
class Repo
{
    public function run(int $count): void
    {
        $sizes = [$count, $count];
        $si$0zes;
    }
}
"#,
        "sizes",
        None,
        expect!["$count"],
    );
}

#[test]
fn one_class_written_twice_reads_as_two_claims() {
    // Conservative rather than clever, and the direction of the error is the point: a class written
    // twice is two *name references* -- each carries its own range, so they cannot be one id -- and
    // agreement is decided on the path this layer can build rather than on the type a catalog would
    // resolve them to. So this says nothing where it could say `Pen`.
    //
    // Reading them as one would take a comparison that walks every path shape and knows which parts
    // are references, and a comparison that has to be kept in step with the path enum is exactly the
    // sort of thing that goes wrong quietly. Silence costs a rare literal; being wrong costs trust.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = [new Pen(), new Pen()];
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["<nothing written>"],
    );
}

#[test]
fn a_named_key_reads_only_the_element_that_fills_it() {
    // The shape a framework project is built out of. Elements keyed by something else are not this
    // slot, so they neither answer nor disagree.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = ['pen' => new Pen(), 'pencil' => new Pencil()];
        $to$0ols;
    }
}
"#,
        "tools",
        Some("pen"),
        expect!["new Pen"],
    );
}

#[test]
fn the_same_map_asked_about_no_slot_says_nothing() {
    // Iterating that map yields either, and the union of the two is a claim this layer cannot spell.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = ['pen' => new Pen(), 'pencil' => new Pencil()];
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["<nothing written>"],
    );
}

#[test]
fn a_computed_key_could_be_the_slot_being_asked_about() {
    // A key this layer cannot name is as likely to be the slot being read as any other -- the rule
    // `fills` already applies to an append. So the computed element is a candidate for `pen` and the
    // one keyed `pencil` is not, which leaves one answer rather than a disagreement.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = [$this->slug() => new Pen(), 'pencil' => new Pencil()];
        $to$0ols;
    }
}
"#,
        "tools",
        Some("pen"),
        expect!["new Pen"],
    );
}

#[test]
fn a_computed_key_beside_a_second_candidate_says_nothing() {
    // The other side of the same rule: two elements could be this slot and they do not agree, so
    // there is no one thing it holds.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = [$this->slug() => new Pen(), 'pen' => new Pencil()];
        $to$0ols;
    }
}
"#,
        "tools",
        Some("pen"),
        expect!["<nothing written>"],
    );
}

#[test]
fn a_spread_contributes_the_elements_of_what_it_spreads() {
    // One step further in, which is the step `$others[0]` takes -- so the reader that already
    // resolves an element of a container resolves this with no new arm.
    check_element(
        r#"<?php
class Repo
{
    public function run(array $others): void
    {
        $tools = [...$others];
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["$others[element]"],
    );
}

#[test]
fn an_element_with_no_path_leaves_the_literal_saying_nothing() {
    // An interpolated string is an expression this layer cannot name, and it is still in the array.
    check_element(
        r#"<?php
class Repo
{
    public function run(string $name): void
    {
        $tools = [new Pen(), "a $name b"];
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["<nothing written>"],
    );
}

#[test]
fn an_empty_literal_says_nothing_and_the_writes_after_it_still_do() {
    // The shape this function was written for, unchanged: nothing is claimed from `[]`, and the
    // append that follows is the whole answer.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = [];
        $tools[] = new Pen();
        $to$0ols;
    }
}
"#,
        "tools",
        None,
        expect!["new Pen"],
    );
}

#[test]
fn a_later_write_into_another_slot_leaves_this_one_alone() {
    // Ordering is by statement and filtered by key: the write is later but fills a different slot,
    // so the literal's own element is still what this slot holds.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $tools = ['pen' => new Pen()];
        $tools['pencil'] = new Pencil();
        $to$0ols;
    }
}
"#,
        "tools",
        Some("pen"),
        expect!["new Pen"],
    );
}

#[test]
fn a_missing_element_is_not_an_element() {
    // `[1, , 3]` holds two ints and a comma. If the gap had become a node with no value, the literal
    // would have had an element it could not name and answered nothing.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [1, , 3];
        $ro$0ws;
    }
}
"#,
        "rows",
        None,
        expect!["(int)"],
    );
}

#[test]
fn a_nested_literal_answers_for_the_key_it_is_written_under() {
    // `$form['#rows']` is the array, and what *it* holds is a second question asked of the path this
    // returns rather than of this literal -- so the path names the literal, and the view reads its
    // one entry as a `Pen`. It used to name a bare `array`, which answered the first question and
    // threw away everything needed for the second.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $form = ['#rows' => [new Pen()]];
        $fo$0rm;
    }
}
"#,
        "form",
        Some("#rows"),
        expect!["list literal of 1"],
    );
}

#[test]
fn a_literal_written_into_a_slot_answers_for_its_own_keys() {
    // How every array of records in PHP is built, and it used to answer nothing at the second level.
    // `$rows['a']` answered the literal and `$rows['a']['pen']` answered nothing, because a write
    // whose target was an index was only ever read as "a write into a slot of the container" -- so
    // the reading that applies when the target *is* the container never ran.
    check_slot_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [];
        $rows['a'] = ['pen' => new Pen(), 'count' => 1];
        $ro$0ws;
    }
}
"#,
        "rows",
        "a",
        Some("pen"),
        expect!["new Pen"],
    );
}

#[test]
fn a_slot_of_a_slot_still_has_to_be_the_slot_asked_about() {
    // The half that keeps it exact: the write filled `$rows['a']`, so it says nothing about
    // `$rows['b']` -- and the two readings cannot both match, because one compares the container
    // against the write's base and the other against the whole target.
    check_slot_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [];
        $rows['a'] = ['pen' => new Pen()];
        $ro$0ws;
    }
}
"#,
        "rows",
        "b",
        Some("pen"),
        expect!["<nothing written>"],
    );
}

#[test]
fn a_key_the_literal_in_the_slot_does_not_write_says_nothing() {
    check_slot_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [];
        $rows['a'] = ['pen' => new Pen()];
        $ro$0ws;
    }
}
"#,
        "rows",
        "a",
        Some("pencil"),
        expect!["<nothing written>"],
    );
}

#[test]
fn a_write_through_the_container_hides_what_was_written_before_it() {
    // The rule as it has always been, pinned because the half below changes it. Symfony builds an
    // array of tuples with a placeholder and fills the placeholder exactly this way, so reading the
    // literal alone said the version was certainly null on a line that passes a string.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [];
        $rows['a'] = new Pen();
        $rows['a']['nib'] = new Nib();
        $ro$0ws;
    }
}
"#,
        "rows",
        Some("a"),
        expect!["<nothing written>"],
    );
}

#[test]
fn a_write_after_one_that_went_through_the_container_still_answers() {
    // The other half, and it was a flag rather than a position: a slot write *after* a
    // through-write replaces the whole slot, and whatever the through-write put one level inside
    // goes with it. `$order['Field2']['coCustomer'] = $sanctions;` is stale the moment
    // `$order['Field2'] = json_encode($order['Field2']);` runs three hundred lines later -- and the
    // flag made every write for the rest of the body invisible, so `strlen($order['Field2'])` on the
    // next line was judged against nothing at all.
    check_element(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $rows = [];
        $rows['a'] = new Pen();
        $rows['a']['nib'] = new Nib();
        $rows['a'] = new Pencil();
        $ro$0ws;
    }
}
"#,
        "rows",
        Some("a"),
        expect!["new Pencil"],
    );
}
