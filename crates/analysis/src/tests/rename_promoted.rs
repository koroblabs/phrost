//! A promoted constructor parameter and the property it declares are one rename.
//!
//! `__construct(public int $across)` is one declaration PHP reads as two: a parameter the body can
//! read as `$across`, and a property every other file reads as `->across`. Each half is reached by
//! a different search -- the body's scope chain, and the member tables -- so a rename that took
//! only the one under the cursor was short by the other, in both directions.
//!
//! Measured on a real checkout before the fix: from the parameter, four edits in one file, leaving
//! every `$this->id` in the project spelled the old way; from the property, three edits leaving the
//! constructor body reading a variable nothing binds. Neither is a subset of the other and neither
//! leaves a project that runs. That checkout writes 1 508 promoted properties in 431 files.
//!
//! Each case renders what the rename decided it was about *and* every range, because agreement
//! between those two is the property being pinned: the same set from all four positions.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{ReferenceTarget, references_in, rename_target};

/// Renders, for each marked cursor, what a rename decided it was about and every range it edits.
fn check(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();

    for marker in markers {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let target = rename_target(
            &IndexView::in_file(world.skeleton(), &open, cursor),
            position.offset,
        );
        let Some(target) = target else {
            let _ = writeln!(out, "@{marker} -> nothing to rename");
            continue;
        };
        let _ = writeln!(out, "@{marker} -> {}", label(&target));

        // A target with no search name never leaves its file, which is what a workspace does too.
        let searched: Vec<&str> = match target.search_name() {
            Some(_) => spec.files().iter().map(|file| file.path()).collect(),
            None => vec![position.path.as_str()],
        };
        for path in searched {
            let id = world.file_id(path);
            let view = IndexView::in_file(world.skeleton(), &open, id);
            for range in references_in(&view, &target) {
                let _ = writeln!(out, "  {path} {}", world.render_range(id, range));
            }
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

fn label(target: &ReferenceTarget) -> String {
    match target {
        ReferenceTarget::Function(fqn) => format!("function {fqn}"),
        ReferenceTarget::Constant(fqn) => format!("constant {fqn}"),
        ReferenceTarget::Class(fqn) => format!("class {fqn} (every spelling)"),
        ReferenceTarget::ClassUnderItsOwnName(fqn) => format!("class {fqn}"),
        ReferenceTarget::Member {
            declared_by, name, ..
        } => format!("member {declared_by}::{name}"),
        ReferenceTarget::MemberFamily {
            declarers, name, ..
        } => format!(
            "member {}::{name}",
            declarers
                .iter()
                .map(|fqn| fqn.as_str())
                .collect::<Vec<_>>()
                .join(" + ")
        ),
        ReferenceTarget::Local { name, .. } => format!("local ${name}"),
        ReferenceTarget::Alias { name } => format!("alias {name}"),
        ReferenceTarget::CatalogKey { catalog, key } => format!("{catalog} key {key}"),
        ReferenceTarget::ElementKey { key, .. } => format!("element key {key}"),
        ReferenceTarget::DeclaredInput { name, .. } => format!("declared input {name}"),
        ReferenceTarget::TraitAlias(alias) => {
            format!("trait alias {}::{}", alias.class, alias.alias)
        }
        ReferenceTarget::Parameter(parameter) => {
            format!("parameter ${} of {}", parameter.name, parameter.callable)
        }
        ReferenceTarget::MagicMember {
            answered_by, name, ..
        } => format!("magic {answered_by}::{name}"),
    }
}

const PROMOTED: &str = r#"
//- /src/Point.php
<?php
namespace App;

class Point
{
    public function __construct(
        public int $ac@declaration@ross,
    ) {
        if ($ac@in_body@ross < 0) {
            throw new \InvalidArgumentException('negative');
        }
    }

    public function describe(): string
    {
        return (string) $this->ac@through_this@ross;
    }
}

//- /src/Reader.php
<?php
namespace App;

class Reader
{
    public function read(Point $point): int
    {
        return $point->ac@elsewhere@ross;
    }
}
"#;

#[test]
fn a_promoted_parameter_and_its_property_are_renamed_from_either_side() {
    // All four positions in one snapshot, because the property is that they agree. Two of them are
    // written as a variable and two as a member access, and before this the two pairs answered two
    // different sets -- each one exactly the half its own search could see.
    check(
        PROMOTED,
        &["declaration", "in_body", "through_this", "elsewhere"],
        expect![[r#"
            @declaration -> member App\Point::across
              src/Point.php 7:20-7:27
              src/Point.php 9:13-9:20
              src/Point.php 16:32-16:38
              src/Reader.php 8:24-8:30
            @in_body -> member App\Point::across
              src/Point.php 7:20-7:27
              src/Point.php 9:13-9:20
              src/Point.php 16:32-16:38
              src/Reader.php 8:24-8:30
            @through_this -> member App\Point::across
              src/Point.php 7:20-7:27
              src/Point.php 9:13-9:20
              src/Point.php 16:32-16:38
              src/Reader.php 8:24-8:30
            @elsewhere -> member App\Point::across
              src/Point.php 7:20-7:27
              src/Point.php 9:13-9:20
              src/Point.php 16:32-16:38
              src/Reader.php 8:24-8:30
        "#]],
    );
}

const SHADOWED: &str = r#"
//- /src/Ledger.php
<?php
namespace App;

class Ledger
{
    public function __construct(private int $to@promoted@tal) {}

    public function sum(array $rows): int
    {
        $to@local@tal = 0;
        foreach ($rows as $row) {
            $total = $total + $row;
        }
        return $total;
    }
}
"#;

#[test]
fn a_local_spelled_like_the_promoted_parameter_stays_a_different_variable() {
    // Both directions of the same spelling, in one snapshot because the contrast is the property.
    // A promoted parameter is in scope in the constructor and nowhere else, so `$total` in `sum`
    // binds to that method's own assignment; and the promoted one is a property, whose rename
    // sweeps the project by name -- which is the direction that destroys code rather than merely
    // under-editing, if the scope is not asked.
    check(
        SHADOWED,
        &["local", "promoted"],
        expect![[r#"
            @local -> local $total
              src/Ledger.php 10:9-10:15
              src/Ledger.php 12:13-12:19
              src/Ledger.php 12:22-12:28
              src/Ledger.php 14:16-14:22
            @promoted -> member App\Ledger::total
              src/Ledger.php 6:45-6:51
        "#]],
    );
}

const ORDINARY: &str = r#"
//- /src/Meter.php
<?php
namespace App;

class Meter
{
    public int $reading = 0;

    public function record(int $reading): void
    {
        $this->re@property@ading = $reading;
    }
}
"#;

#[test]
fn an_ordinary_property_does_not_reach_a_parameter_of_the_same_name() {
    // A promoted property's declaration range *is* its parameter's, which is what tells the two
    // apart with no flag stored anywhere -- so the rule has to decline for a property declared in
    // the ordinary way, even where some method happens to take a parameter spelled the same. Here
    // `record`'s `$reading` is a different declaration and keeps its name.
    check(
        ORDINARY,
        &["property"],
        expect![[r#"
            @property -> member App\Meter::reading
              src/Meter.php 6:16-6:24
              src/Meter.php 10:16-10:23
        "#]],
    );
}
