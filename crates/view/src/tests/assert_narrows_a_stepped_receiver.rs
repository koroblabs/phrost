//! An `@phpstan-assert` contract narrows a receiver reached through a step exactly like it
//! narrows a bare local.
//!
//! The bug this guards: completion's chain scanner (`IndexView::chain_held`) and a real
//! expression's walk (`IndexView::held_after_steps`) used to answer "did an earlier call assert
//! something about this value" differently -- only the latter asked. A bare local always went
//! through `held_after_steps` (its own steps are empty, so the walk is trivial), which is why it
//! narrowed; `$constants['C']` has a step, took the chain scanner's other branch, and never asked
//! at all. Both now call the same `IndexView::contract_narrowed`.
//!
//! Unlike the rest of this module, these tests call [`IndexView::chain_classes`] directly instead
//! of going through [`super::Query`]: completion's own scanner hands it a [`ChainRoot`] and a
//! slice of [`ChainStep`], which `Query` has no way to build -- turning characters into those is
//! `phrost_analysis`'s concern, not this crate's.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::{ChainKey, ChainRoot, ChainStep, IndexView};

fn assert_classes(
    source: &str,
    marker: &str,
    root: ChainRoot<'_>,
    steps: &[ChainStep<'_>],
    expect: Expect,
) {
    let spec = FixtureSpec::parse(source);
    let world = World::build(&spec);
    let file = world.file_id("app.php");
    let offset = spec.markers().position(marker).offset;

    let answer = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        view.chain_classes(offset, root, steps)
            .classes()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(", ")
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&answer);
}

/// Control: this already worked before the fix, and must keep working after it -- a bare local's
/// receiver has no steps, so `chain_held` falls straight through to `chain_root`'s own
/// `resolve_path`, which always asked the contract.
#[test]
fn a_bare_local_narrows() {
    assert_classes(
        r#"
//- /app.php
<?php

class ASTNode {
    public function getImage(): string { return ''; }
}

class BaseAssert
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, object $actual): void
    {
    }
}

class MyTestCase extends BaseAssert
{
    public function testIt(object $actual): void
    {
        static::assertInstanceOf(ASTNode::class, $actual);
        $actual@here@->getImage();
    }
}
"#,
        "here",
        ChainRoot::Local("actual"),
        &[],
        expect![["ASTNode"]],
    );
}

/// The fixed case: the same assertion, on a subject reached through an array-index step. Before
/// the fix this returned nothing, because `chain_held`'s `walked_chain` fallback resolved the
/// element directly through `indexed_of_held` and never reached the contract check.
#[test]
fn a_subscript_subject_narrows_the_same_way() {
    assert_classes(
        r#"
//- /app.php
<?php

class ASTNode {
    public function getImage(): string { return ''; }
}

class BaseAssert
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, object $actual): void
    {
    }
}

class MyTestCase extends BaseAssert
{
    /**
     * @param array<string, mixed> $constants
     */
    public function testIt(array $constants): void
    {
        static::assertInstanceOf(ASTNode::class, $constants['C']);
        $constants['C']@here@->getImage();
    }
}
"#,
        "here",
        ChainRoot::Local("constants"),
        &[ChainStep::Element(Some(ChainKey::Literal("C")))],
        expect![["ASTNode"]],
    );
}
