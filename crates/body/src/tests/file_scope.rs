//! Whether a file's own top level is a scope anything else can share.
//!
//! PHP shares a scope through `include`, so a top-level variable is ordinarily the includer's --
//! `$databases = [];` in a settings file is read by the bootstrap that included it, and nothing here
//! could see the reader. Whether *this* file can be included that way is not a fact a body knows, so
//! it arrives as an answer; what this file pins is that the answer is obeyed in both directions.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use crate::{locals_never_bound, locals_never_read};

/// Every unread and unbound local, as `name` lines, under one answer about the file's scope.
fn check(source: &str, private_file_scope: bool, expect: Expect) {
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let mut out = String::new();
    for found in locals_never_read(&lowered.hir, private_file_scope) {
        out.push_str(&format!("unread ${}\n", found.name));
    }
    for found in locals_never_bound(&lowered.hir, private_file_scope) {
        out.push_str(&format!("unbound ${}\n", found.name));
    }
    expect.assert_eq(&out);
}

const SOURCE: &str = r#"<?php

$stray = $missing + 1;

function run(): void
{
    $inner = 1;
    $other = $absent;
}
"#;

#[test]
fn a_shared_top_level_says_nothing_about_its_own_variables() {
    check(
        SOURCE,
        false,
        expect![[r#"
            unread $inner
            unread $other
            unbound $absent
        "#]],
    );
}

#[test]
fn a_private_top_level_is_checked_like_any_other_scope() {
    check(
        SOURCE,
        true,
        expect![[r#"
            unread $stray
            unread $inner
            unread $other
            unbound $missing
            unbound $absent
        "#]],
    );
}
