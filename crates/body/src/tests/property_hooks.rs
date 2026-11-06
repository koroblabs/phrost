//! A property hook is a scope, and the names inside it are that scope's.
//!
//! Both searches, deliberately. `set(string $value)` and an implicit `set` both introduce a name
//! that is not an assignment, which is exactly the shape that works in one search and fails
//! silently in the other.

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use super::{check, check_names};

/// Every local a hooked file reads without binding, which is what the unbound-variable check reports.
///
/// The one finding the walk's step-around was suppressing by making hook bodies invisible: pulled
/// into the file's body, a hook's `$value` was a name nothing bound. It has to stay suppressed --
/// and now by a real binding rather than by the body being missing.
fn check_unbound(source: &str, expect: Expect) {
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

    // `true`: nobody can include this file, so its top level is private and its own variables are
    // answerable. The lenient answer would hide the finding this is about.
    let found = crate::locals_never_bound(&lowered.hir, true)
        .into_iter()
        .map(|local| format!("${}", local.name))
        .collect::<Vec<_>>();
    expect.assert_eq(&match found.is_empty() {
        true => "<nothing unbound>".to_owned(),
        false => found.join("\n"),
    });
}

#[test]
fn a_set_hook_binds_its_written_parameter() {
    check(
        r#"
<?php
class Item {}

class Holder
{
    public Item $current {
        set(Item $value) {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["Item"],
    );
}

#[test]
fn a_set_hook_written_with_no_list_still_binds_value() {
    // PHP's shorter spelling. Nothing in the text declares `$value`, so the parameter is
    // synthesized with the property's own type -- and without it every `$value` in every such hook
    // in the project is a variable nothing binds.
    check(
        r#"
<?php
class Item {}

class Holder
{
    public Item $current {
        set {
            $val$0ue;
        }
    }
}
"#,
        "value",
        expect!["Item"],
    );
}

#[test]
fn the_names_a_hook_binds_exist_as_well_as_holding_something() {
    // The second search. A hook parameter reachable only through `local_binding` would answer a
    // hover and leave completion inside the hook empty.
    check_names(
        r#"
<?php
class Item {}

class Holder
{
    public Item $current {
        set(Item $value) {
            $found = $value;
            $0
        }
    }
}
"#,
        expect![[r#"
            $value
            $found"#]],
    );
}

#[test]
fn a_local_written_inside_a_hook_belongs_to_that_hook() {
    check(
        r#"
<?php
class Item {}

class Holder
{
    public string $label {
        get {
            $one = new Item();
            return (string) $on$0e;
        }
    }
}
"#,
        "one",
        expect!["new Item"],
    );
}

#[test]
fn a_get_hooks_locals_are_not_the_set_hooks() {
    // Two hooks on one property are two scopes. Folding them together would make one hook's `$one`
    // the other's, which is the same mistake as folding a closure into the function around it.
    check(
        r#"
<?php
class Alpha {}
class Beta {}

class Holder
{
    public string $label {
        get {
            $one = new Alpha();
            return (string) $one;
        }
        set {
            $one = new Beta();
            $o$0ne;
        }
    }
}
"#,
        "one",
        expect!["new Beta"],
    );
}

#[test]
fn a_hooks_value_is_bound_and_not_reported_as_missing() {
    check_unbound(
        r#"<?php
class Item {}

class Holder
{
    private Item $raw;

    public Item $current {
        get => $this->raw;
        set(Item $value) => $this->raw = $value;
    }

    public Item $terse {
        set { $this->raw = $value; }
    }
}
"#,
        expect!["<nothing unbound>"],
    );
}

#[test]
fn a_typo_inside_a_hook_is_now_reachable() {
    // The other side of the same change: a hook body that was invisible reported nothing at all,
    // however wrong it was. This is the finding that modelling the scope buys.
    check_unbound(
        r#"<?php
class Holder
{
    public string $label {
        get {
            $value = 'x';
            return $vlaue;
        }
    }
}
"#,
        expect!["$vlaue"],
    );
}

#[test]
fn an_expression_bodied_hook_is_a_scope_too() {
    check(
        r#"
<?php
class Item {}

class Holder
{
    private Item $raw;

    public Item $current {
        set(Item $value) => $this->raw = $val$0ue;
    }
}
"#,
        "value",
        expect!["Item"],
    );
}
