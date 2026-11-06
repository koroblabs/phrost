//! Renaming the name a file gave an imported class, rather than the class.
//!
//! `use App\Value\UUIDv7 as QuoteId;` puts two names on one declaration, and a rename asked at each
//! of them is a different rename. Asked at `QuoteId` it is about a name that exists in one file;
//! asked at `UUIDv7` it is about the class, and the file that renamed it keeps calling it
//! `QuoteId` because the `as` line still says so.
//!
//! Getting that wrong on a real checkout put the new name over `UUIDv7` at thirty sites in six
//! files -- the class's own declaration included -- while the editor's dialogue promised to edit
//! `"QuoteId"`. So each case renders the target *and* every range, because agreement between the
//! two is the whole property.

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

const ALIASED: &str = r#"
//- /src/UUIDv7.php
<?php
namespace App\Value;

class UUIDv7
{
    public static function generate(): self
    {
        return new UU@declaration@IDv7();
    }
}

//- /src/Port.php
<?php
namespace App\Ports;

use App\Value\UUIDv7 as Quo@binding@teId;

interface Port
{
    public function find(Quo@use@teId $id): QuoteId;
}

//- /src/Other.php
<?php
namespace App\Other;

use App\Value\UUIDv7;

class Other
{
    public function make(): UUIDv7
    {
        return UU@elsewhere@IDv7::generate();
    }
}
"#;

#[test]
fn the_alias_and_the_class_are_two_renames_and_neither_reaches_the_other() {
    // All four positions in one snapshot, because the property is the contrast: the two on
    // `QuoteId` edit one file's own name for the class, and the two on `UUIDv7` edit the class --
    // including the import *path* in the `as` line, which is written under the real name -- and
    // leave every name the alias binds alone. Rewriting those would break exactly the sites the
    // binding was keeping correct, since the binding itself is not being renamed.
    check(
        ALIASED,
        &["use", "binding", "declaration", "elsewhere"],
        expect![[r#"
            @use -> alias QuoteId
              src/Port.php 4:25-4:32
              src/Port.php 8:26-8:33
              src/Port.php 8:40-8:47
            @binding -> alias QuoteId
              src/Port.php 4:25-4:32
              src/Port.php 8:26-8:33
              src/Port.php 8:40-8:47
            @declaration -> class App\Value\UUIDv7
              src/UUIDv7.php 4:7-4:13
              src/UUIDv7.php 8:20-8:26
              src/Port.php 4:5-4:21
              src/Other.php 4:5-4:21
              src/Other.php 8:29-8:35
              src/Other.php 10:16-10:22
            @elsewhere -> class App\Value\UUIDv7
              src/UUIDv7.php 4:7-4:13
              src/UUIDv7.php 8:20-8:26
              src/Port.php 4:5-4:21
              src/Other.php 4:5-4:21
              src/Other.php 8:29-8:35
              src/Other.php 10:16-10:22
        "#]],
    );
}

#[test]
fn an_import_without_an_as_binds_a_name_the_author_never_typed_and_is_not_an_alias() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/Service.php
<?php
namespace App\Other;

use App\Repo;

class Service
{
    public function run(R@here@epo $repo): void {}
}
"#,
        &["here"],
        expect![[r#"
            @here -> class App\Repo
              src/Repo.php 4:7-4:11
              src/Service.php 4:5-4:13
              src/Service.php 8:25-8:29
        "#]],
    );
}

#[test]
fn a_class_reached_through_a_namespace_alias_is_renamed_and_the_alias_head_is_not() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App\Nested;

class Repo {}

//- /src/Service.php
<?php
namespace App\Other;

use App\Nested as N;

class Service
{
    public function run(N\R@here@epo $repo): void {}
}
"#,
        &["here"],
        expect![[r#"
            @here -> class App\Nested\Repo
              src/Repo.php 4:7-4:11
              src/Service.php 8:25-8:31
        "#]],
    );
}
