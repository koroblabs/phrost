//! A name the project's own code declares in two places.
//!
//! `Cannot redeclare class C` the moment both files are loaded, and before that it is worse than a
//! fatal: which one wins is an accident of the autoloader, so every answer about the name is a coin
//! toss. A catalog that holds two claimants ranks them, and the loser is silently invisible -- which
//! is the shape `docs/08` records as making every probe in a scratch directory lie.
//!
//! The rules are stated rather than read, for the reason `psr4.rs` gives: composer's map comes from
//! what it *dumped*, and a fixture has no `vendor/`. Two directories under one prefix, which is an
//! ordinary composer configuration and the shape that produces a real conflict.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::diagnostics;

/// Every finding in every file of the fixture, since the claim is about a pair of them.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build_with(&spec, |world| {
        world.set_namespace_rules(&[("App\\", "src"), ("App\\", "lib")]);
    });

    let mut out = String::new();
    for declared in spec.files() {
        let file = world.file_id(declared.path());
        let rendered = with_open_scopes(&world, Some(file), |open| {
            let view = IndexView::in_file(world.skeleton(), open, file);
            diagnostics(&view)
                .iter()
                .map(|one| {
                    format!(
                        "{} {} {} {}\n",
                        declared.path(),
                        world.render_range(file, one.range),
                        one.code,
                        one.message
                    )
                })
                .collect::<String>()
        });
        out.push_str(&rendered);
    }
    expect.assert_eq(match out.is_empty() {
        true => "(nothing reported)\n",
        false => &out,
    });
}

#[test]
fn two_autoloadable_files_claiming_one_name_are_both_reported() {
    check(
        r#"
//- /src/Twin.php
<?php

namespace App;

class Twin {}

//- /lib/Twin.php
<?php

namespace App;

/**
 * The same name, declared further down its file, so the two ranges differ -- which is what makes
 * this say whether each report underlines its *own* declaration.
 */
final class Twin {}
"#,
        expect![[r#"
            src/Twin.php 5:7-5:11 twice-declared App\Twin is also declared in lib/Twin.php
            lib/Twin.php 9:13-9:17 twice-declared App\Twin is also declared in src/Twin.php
        "#]],
    );
}

/// A shim is how a project ships a class the runtime may already have, and the guard is the point
/// -- on either side, which the first spelling of this asked of only one.
#[test]
fn a_guarded_declaration_is_a_shim_and_not_a_conflict() {
    check(
        r#"
//- /stubs.php
<?php

/** Declared here because the fixture harness installs no stubs. */
function class_exists(string $class, bool $autoload = true): bool {}

//- /src/Shim.php
<?php

namespace App;

class Shim {}

//- /lib/Shim.php
<?php

namespace App;

if (!class_exists(Shim::class)) {
    class Shim {}
}
"#,
        expect![[r#"
            (nothing reported)
        "#]],
    );
}

/// A file the autoloader would never be asked for may share a name freely: it is included by path,
/// so PHP never sees a second declaration. Symfony's fixtures and psl's examples are forty-three
/// of these between them.
#[test]
fn a_file_outside_the_autoload_map_may_share_a_name() {
    check(
        r#"
//- /src/Fixture.php
<?php

namespace App;

class Fixture {}

//- /tests/data/Fixture.php
<?php

namespace App;

class Fixture {}
"#,
        expect![[r#"
            (nothing reported)
        "#]],
    );
}
