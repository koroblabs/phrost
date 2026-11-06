//! What a file gets wrong about where it lives, and the one edit that fixes it.
//!
//! The mistake nothing running the code reports: a class whose namespace does not match its
//! directory is never autoloaded, `composer dump-autoload -o` hides it behind a classmap, and no
//! test that does not touch the class has anything to say. Measured on one real checkout -- 1 651
//! files under a source root, **two** of them wrong, and both were classes moved to another
//! directory with their namespace left behind.
//!
//! The rules are stated rather than read, because a fixture has no `vendor/composer/
//! autoload_psr4.php` -- composer's map comes from what it *dumped*, and a project without a vendor
//! directory has no PSR-4 rules at all. That is also why the first version of this check reported
//! nothing in a scratch project and two findings on the real one.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{code_actions, diagnostics};

/// Every finding in the fixture's last file, and the file the offered fix would leave behind.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build_with(&spec, |world| {
        // The third overlaps the first on purpose: PSR-4 roots may nest, and composer resolves a
        // *class name* against the longest registered prefix rather than choosing between
        // directories -- so a file under `src/Inner` is reachable through either rule. Symfony
        // registers exactly this shape. Inert for every fixture that stays out of `src/Inner`.
        world.set_namespace_rules(&[
            ("App\\", "src"),
            ("App\\Deep\\", "other"),
            ("Legacy\\Inner\\", "src/Inner"),
        ]);
    });
    let path = spec
        .files()
        .last()
        .expect("the fixture declares a file")
        .path()
        .to_owned();
    let file = world.file_id(&path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let found = diagnostics(&view);
        if found.is_empty() {
            return "(nothing reported)\n".to_owned();
        }

        let mut out = String::new();
        for one in &found {
            out.push_str(&format!("{} {}\n", one.code, one.message));
        }
        let request = crate::ActionRequest {
            view: &view,
            at: found[0].range,
            only: &[],
            asked: Default::default(),
        };
        match code_actions(&request)
            .into_iter()
            .find(|action| action.title.starts_with("Declare "))
        {
            None => out.push_str("(no fix offered)\n"),
            Some(action) => {
                let mut text = world.text_of(file).to_owned();
                for edit in &action.edits {
                    let range = usize::from(edit.range.start())..usize::from(edit.range.end());
                    text.replace_range(range, &edit.text);
                }
                out.push_str(&format!("----\n{text}"));
            }
        }
        out
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_namespace_that_does_not_match_the_directory_is_reported_and_fixed() {
    check(
        r#"
//- /src/Deep/Thing.php
<?php

namespace App\Wrong;

class Thing {}
"#,
        expect![[r#"
            psr4-mismatch The autoloader looks for this file under `App\Deep`, not `App\Wrong`
            ----
            <?php

            namespace App\Deep;

            class Thing {}
        "#]],
    );
}

#[test]
fn the_longest_matching_rule_wins() {
    // Composer's own rule for an overlap: `App\Deep\ => other` beside `App\ => src` means a file in
    // `other` is an `App\Deep\`, and a directory under it extends that rather than the shorter one.
    check(
        r#"
//- /other/Nested/Thing.php
<?php

namespace App\Nested;

class Thing {}
"#,
        expect![[r#"
            psr4-mismatch The autoloader looks for this file under `App\Deep\Nested`, not `App\Nested`
            ----
            <?php

            namespace App\Deep\Nested;

            class Thing {}
        "#]],
    );
}

#[test]
fn a_file_under_no_source_root_is_not_the_autoloaders_business() {
    // A script, a template, something reached by a `require`. Nothing looks for it by name.
    check(
        r#"
//- /bin/console.php
<?php

namespace Whatever;

class Console {}
"#,
        expect!["(nothing reported)\n"],
    );
}

#[test]
fn a_file_declaring_several_class_likes_says_which_one_it_is_named_after_itself() {
    // PSR-4 asks for one per file and the rest do not autoload -- but which of them the file is
    // named for is the author's business, and a fixture declaring five is not a mistake anybody
    // wants reported five times.
    check(
        r#"
//- /src/Pair.php
<?php

namespace App\Wrong;

class One {}

class Two {}
"#,
        expect!["(nothing reported)\n"],
    );
}

#[test]
fn a_class_whose_file_is_named_something_else_is_reported_without_a_fix() {
    // The same defect and a different repair: the file has to be renamed, which is a workspace edit
    // of another shape and a decision about a path somebody may have history for.
    check(
        r#"
//- /src/Other.php
<?php

namespace App;

class Named {}
"#,
        expect![[r#"
            psr4-mismatch The autoloader looks for `Named` in `Named.php`, and this file is `Other.php`
            (no fix offered)
        "#]],
    );
}

/// A file under two nested PSR-4 roots satisfies either of them.
///
/// Reading only the most specific directory reported four of Symfony's `Runtime/Internal/` files as
/// unfindable -- it registers `Symfony\Runtime\Symfony\Component\ =>
/// src/Symfony/Component/Runtime/Internal/` *inside* `Symfony\Component\ => src/Symfony/Component/`,
/// and those files declare what the outer rule wants. That was a claim about composer rather than
/// about the code: the autoloader takes a class name, finds the longest prefix registered for it,
/// and looks under that rule's directory, so any rule that can produce the path will do.
#[test]
fn a_namespace_the_outer_rule_accepts_is_not_a_mismatch() {
    check(
        r#"
//- /src/Inner/Thing.php
<?php
namespace App\Inner;

class Thing {}
"#,
        expect![[r#"
            (nothing reported)
        "#]],
    );
}

/// The inner rule accepts its own spelling just as well, which is what makes this "either" rather
/// than "the outer one wins".
#[test]
fn a_namespace_the_inner_rule_accepts_is_not_a_mismatch_either() {
    check(
        r#"
//- /src/Inner/Thing.php
<?php
namespace Legacy\Inner;

class Thing {}
"#,
        expect![[r#"
            (nothing reported)
        "#]],
    );
}

/// And a namespace neither rule can produce is still reported, named by the most specific.
#[test]
fn a_namespace_no_rule_accepts_is_still_reported() {
    check(
        r#"
//- /src/Inner/Thing.php
<?php
namespace Somewhere\Else1;

class Thing {}
"#,
        expect![[r#"
            psr4-mismatch The autoloader looks for this file under `Legacy\Inner`, not `Somewhere\Else1`
            ----
            <?php
            namespace Legacy\Inner;

            class Thing {}
        "#]],
    );
}
