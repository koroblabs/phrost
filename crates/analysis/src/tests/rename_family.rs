//! Renaming a member renames every declaration of it PHP makes one contract with.
//!
//! `ProcessorInterface::process` is not one declaration: an implementation left answering the old
//! name no longer satisfies the interface, every call written against the interface still reaches
//! it, and the project stops running. Renaming only the implementations is the same mistake upside
//! down. So a member rename is the family or nothing -- and it used to be nothing, refused with a
//! sentence telling the person to do it by hand.
//!
//! The walk goes both ways for a reason each. Upwards, because a cursor is more often on an
//! override than on the declaration it overrides -- and because a class can implement two
//! interfaces declaring the same name, where renaming the first without the second leaves it
//! satisfying neither. Downwards, and *through* a subtype that does not redeclare, because
//! `C extends B extends A` overriding at `C` is invisible from `A` if the walk stops at `B`.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{references_in, rename_target};

/// Every range a rename from each marked cursor would edit, across the fixture's files.
///
/// Every marker in one snapshot, because the property being checked is that they *agree*: a rename
/// from the interface, from an implementation and from a grandchild are one answer or the feature
/// is not there.
fn check(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();

    for marker in markers {
        let _ = writeln!(out, "@{marker}");
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let target = rename_target(
            &IndexView::in_file(world.skeleton(), &open, cursor),
            position.offset,
        );
        match target {
            None => out.push_str("  nothing to rename\n"),
            Some(target) => {
                for file in spec.files() {
                    let id = world.file_id(file.path());
                    let view = IndexView::in_file(world.skeleton(), &open, id);
                    for range in references_in(&view, &target) {
                        let _ =
                            writeln!(out, "  {} {}", file.path(), world.render_range(id, range));
                    }
                }
            }
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

/// The interface, its two implementations, and every call written against any of them.
const HIERARCHY: &str = r#"
//- /src/ProcessorInterface.php
<?php
namespace App;

interface ProcessorInterface
{
    public function proc@interface@ess(string $input): string;
}

//- /src/Processor.php
<?php
namespace App;

class Processor implements ProcessorInterface
{
    public function proc@implementation@ess(string $input): string
    {
        return $input;
    }
}

//- /src/Loud.php
<?php
namespace App;

class Loud extends Processor
{
    public function proc@grandchild@ess(string $input): string
    {
        return parent::process($input);
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(ProcessorInterface $one, Processor $two, Loud $three): void
    {
        $one->process('a');
        $two->process('b');
        $three->process('c');
    }
}
"#;

#[test]
fn the_interface_an_implementation_and_a_grandchild_all_rename_the_same_set() {
    check(
        HIERARCHY,
        &["interface", "implementation", "grandchild"],
        expect![[r#"
            @interface
              src/ProcessorInterface.php 6:21-6:28
              src/Processor.php 6:21-6:28
              src/Loud.php 6:21-6:28
              src/Loud.php 8:24-8:31
              src/Runner.php 8:15-8:22
              src/Runner.php 9:15-9:22
              src/Runner.php 10:17-10:24
            @implementation
              src/ProcessorInterface.php 6:21-6:28
              src/Processor.php 6:21-6:28
              src/Loud.php 6:21-6:28
              src/Loud.php 8:24-8:31
              src/Runner.php 8:15-8:22
              src/Runner.php 9:15-9:22
              src/Runner.php 10:17-10:24
            @grandchild
              src/ProcessorInterface.php 6:21-6:28
              src/Processor.php 6:21-6:28
              src/Loud.php 6:21-6:28
              src/Loud.php 8:24-8:31
              src/Runner.php 8:15-8:22
              src/Runner.php 9:15-9:22
              src/Runner.php 10:17-10:24
        "#]],
    );
}

/// A class satisfying two interfaces with one method: renaming either one has to move the other,
/// or the class ends up satisfying neither.
#[test]
fn a_class_implementing_two_interfaces_pulls_both_in() {
    check(
        r#"
//- /src/Readable.php
<?php
namespace App;

interface Readable
{
    public function na@readable@me(): string;
}

//- /src/Nameable.php
<?php
namespace App;

interface Nameable
{
    public function name(): string;
}

//- /src/Thing.php
<?php
namespace App;

class Thing implements Readable, Nameable
{
    public function name(): string
    {
        return 'thing';
    }
}
"#,
        &["readable"],
        expect![[r#"
            @readable
              src/Readable.php 6:21-6:25
              src/Nameable.php 6:21-6:25
              src/Thing.php 6:21-6:25
        "#]],
    );
}

/// A private member is not inherited, so a subclass writing the same name declares a different one
/// and the family is exactly the class in hand. The commonest rename there is, and it must not pay
/// for any of the walk above.
#[test]
fn a_private_member_is_a_family_of_one() {
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    private function ti@private@dy(string $input): string
    {
        return trim($input);
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    private function tidy(string $input): string
    {
        return $input;
    }
}
"#,
        &["private"],
        expect![[r#"
            @private
              src/Base.php 6:22-6:26
        "#]],
    );
}

/// The half of a family a rename can be refused on before the half below is walked: the member's
/// own declaration and the ones above it. PHPUnit's `setUp()` is declared by a class outside the
/// project and overridden by every test, and finding that out walked them all.
#[test]
fn the_declarations_above_are_the_cursor_and_its_ancestors_only() {
    let fixture = r#"
//- /src/Base.php
<?php
namespace App;

abstract class Base
{
    protected function setUp(): void {}
}

//- /src/Middle.php
<?php
namespace App;

class Middle extends Base
{
    protected function set@middle@Up(): void {}
}

//- /src/Leaf.php
<?php
namespace App;

class Leaf extends Middle
{
    protected function setUp(): void {}
}
"#;
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position("middle");
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let view = IndexView::in_file(world.skeleton(), &open, cursor);
    let above = match crate::rename_target_above(&view, position.offset) {
        Some(crate::ReferenceTarget::MemberFamily { declarers, .. }) => declarers
            .iter()
            .map(|class| class.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        other => format!("{other:?}"),
    };
    spec.markers().assert_all_used();
    expect![[r#"App\Middle, App\Base"#]].assert_eq(&above);
}
