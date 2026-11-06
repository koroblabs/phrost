//! Which property defaults that are one string reach the HIR.
//!
//! Only under the names the caller hands down -- Laravel's `signature` -- and only a literal: a
//! string built from a constant or a concatenation says nothing a framework can read without
//! evaluating it, and the property is still declared.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use crate::KeyedAttributes;

/// Every kept string, one per line, with the class and property it was written on.
fn check(source: &str, kept: &[&str], expect: Expect) {
    let mut interner = Interner::new();
    let lowered = crate::lower_text(
        &mut interner,
        FileId::from_raw(0),
        source,
        &KeyedAttributes::default().keeping_defaults_of(kept.iter().copied()),
    );
    let hir = &lowered.hir;
    let out = hir
        .kept_texts()
        .iter()
        .map(|kept| {
            format!(
                "{}::${} = {:?}\n",
                hir.class(kept.class).name.as_str(),
                kept.property.as_str(),
                kept.text
            )
        })
        .collect::<String>();
    expect.assert_eq(&out);
}

#[test]
fn a_string_default_is_kept_under_a_name_handed_down() {
    check(
        r#"<?php
class SendMail
{
    protected $signature = 'mail:send
        {user} {--queue=}';
    protected $description = 'Send a mail';
}

class Built
{
    protected $signature = self::NAME . ' {user}';
}
"#,
        &["signature"],
        expect![[r#"
            SendMail::$signature = "mail:send\n        {user} {--queue=}"
        "#]],
    );
}
