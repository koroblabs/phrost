//! The tripwire for a method the set declares twice in one class.
//!
//! An extension can overload a method, or take a different signature depending on the library it
//! was built against, and PHP has no way to write either. Upstream writes both out:
//!
//! ```text
//! public function convolveImage(ImagickKernel $kernel, int $channel = ...): bool {}
//!
//! public function convolveImage(array $kernel, int $channel = ...): bool {}
//! ```
//!
//! PHP refuses that, and the index keeps the *last* of the two -- so the first form, which is the
//! one a build against ImageMagick 7 takes, was reported on a call that runs. Probed over all nine:
//! ten false findings, among them `IntlCalendar::set(2026, 10, 10)`, which is the standard library.
//!
//! Each is folded into one signature that takes whatever either took, which makes it a **local
//! edit** and so something a re-vendor reverts in silence. This counts them, and the count is zero.
//! The second test reads the set exactly as published and lists what it still declares twice: when
//! upstream folds one, that list moves and says the edit here is no longer ours to carry.

use std::{collections::BTreeMap, fs, path::PathBuf};

use expect_test::expect;
use phrost_model::{FileId, Interner};

use crate::PRELUDE;

/// Every `Class::method` a file declares more than once, with how many times.
fn declared_twice(text: &str) -> Vec<String> {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        text,
        &phrost_lower::KeyedAttributes::default(),
    );
    let hir = &lowered.hir;

    let mut found = Vec::new();
    // Class by class and not by name: two namespaces may each declare a `Client`, and a method of
    // one is not a second declaration of the other's.
    for (_, class) in hir.classes() {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for &member in hir.member_list(class.members) {
            let member = hir.member(member);
            if member.kind.as_str() != "method" {
                continue;
            }
            // A method name is case-insensitive in PHP, so two spellings are one method.
            *counts
                .entry(member.name.as_str().to_ascii_lowercase())
                .or_default() += 1;
        }
        for (method, times) in counts {
            if times > 1 {
                found.push(format!("{}::{method} x{times}", class.name.as_str()));
            }
        }
    }
    found
}

#[test]
fn no_class_in_the_set_declares_one_method_twice() {
    let mut found: Vec<String> = Vec::new();
    for (name, contents) in PRELUDE {
        for twice in declared_twice(contents) {
            found.push(format!("{name}: {twice}"));
        }
    }

    assert!(
        found.is_empty(),
        "the index keeps the last of two declarations, so the other form is reported on code that \
         runs -- fold them into one signature that takes what either takes:\n  {}",
        found.join("\n  ")
    );
}

/// What the published set still declares twice, which is what the folds here are for.
#[test]
fn the_set_as_published_still_declares_these_twice() {
    let upstream =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/php-prelude-upstream");
    let mut names: Vec<String> = fs::read_dir(&upstream)
        .expect("the published set is beside the vendored one")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".php"))
        .collect();
    names.sort();

    let mut found = String::new();
    for name in names {
        let Ok(text) = fs::read_to_string(upstream.join(&name)) else {
            continue;
        };
        for twice in declared_twice(&text) {
            found.push_str(&format!("{name}: {twice}\n"));
        }
    }

    expect![[r#"
        imagick.php: Imagick::convolveimage x2
        imagick.php: Imagick::count x2
        imagick.php: Imagick::setimageblueprimary x2
        imagick.php: Imagick::setimagegreenprimary x2
        imagick.php: Imagick::setimageredprimary x2
        imagick.php: Imagick::setimagewhitepoint x2
        intl.php: IntlCalendar::set x2
        parle.php: RLexer::push x3
    "#]]
    .assert_eq(&found);
}

/// Every top-level function a file declares more than once, by namespace and name.
fn functions_declared_twice(text: &str) -> Vec<String> {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        text,
        &phrost_lower::KeyedAttributes::default(),
    );
    let hir = &lowered.hir;

    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (_, function) in hir.functions() {
        *counts
            .entry((
                format!("{:?}", function.namespace),
                function.name.as_str().to_ascii_lowercase(),
            ))
            .or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, times)| *times > 1)
        .map(|((_, name), times)| format!("{name} x{times}"))
        .collect()
}

/// The functions the set still declares twice, which is every one the published set does but
/// `setrawcookie`.
///
/// That one was two real forms -- the seven positional parameters, and a name, a value and an
/// array of options -- and the first is what a call is read against, so the options form was
/// reported as an array handed to an `int`. It is one declaration now, `array|int` in the third
/// position, which is how the set already writes `setcookie`.
///
/// The five left are the generator repeating itself rather than an overload: three are the same
/// signature twice, and for `pg_jit` and `ldap_exop_sync` the first copy is the one PHP documents
/// and the one that is read. They cost nothing in use, so they are listed rather than edited --
/// and a sixth appearing, or `setrawcookie` coming back, moves this.
#[test]
fn the_functions_the_set_still_declares_twice_are_these() {
    let mut found = String::new();
    for (name, contents) in PRELUDE {
        for twice in functions_declared_twice(contents) {
            found.push_str(&format!("{name}: {twice}\n"));
        }
    }

    expect![[r#"
        ldap.php: ldap_exop_sync x2
        pgsql.php: pg_change_password x2
        pgsql.php: pg_jit x2
        pgsql.php: pg_result_memory_size x2
        pgsql.php: pg_socket_poll x2
    "#]]
    .assert_eq(&found);
}
