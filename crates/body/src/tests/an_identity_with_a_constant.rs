//! A strict identity that held against a value with no null in it proves the subject has none.
//!
//! Two subjects. The value itself -- `$status === Status::Draft` -- and, through `?->`, the receiver
//! a chain was read off: `$order?->status === Status::Draft` cannot hold where `$order` was null,
//! because the chain is null there. A literal settles that here; a class constant names an enum case
//! or a `const NONE = null`, and only the declaration knows which, so that claim is named and left
//! to the reader.

use expect_test::expect;

use super::check;

#[test]
fn an_identity_with_a_constant_names_it_for_the_reader() {
    check(
        r#"<?php
class Repo
{
    public function run(?Status $status): void
    {
        if ($status === Status::Draft) {
            $sta$0tus;
        }
    }
}
"#,
        "status",
        expect!["$status without what Status::Draft cannot be"],
    );
}

#[test]
fn the_early_exit_of_a_negated_identity_says_the_same() {
    check(
        r#"<?php
class Repo
{
    public function run(?Status $status): void
    {
        if (Status::Draft !== $status) {
            return;
        }
        $sta$0tus;
    }
}
"#,
        "status",
        expect!["$status without what Status::Draft cannot be"],
    );
}

/// The else of an identity holds every value the constant is not, null among them.
#[test]
fn the_branch_an_identity_failed_on_says_nothing() {
    check(
        r#"<?php
class Repo
{
    public function run(?Status $status): void
    {
        if ($status === Status::Draft) {
            return;
        }
        $sta$0tus;
    }
}
"#,
        "status",
        expect!["?Status"],
    );
}

#[test]
fn a_receiver_whose_chain_matched_a_constant_is_named_for_the_reader() {
    check(
        r#"<?php
class Repo
{
    public function run(?Order $order): void
    {
        if ($order?->status === Status::Draft) {
            $ord$0er;
        }
    }
}
"#,
        "order",
        expect!["$order without what Status::Draft cannot be"],
    );
}

/// The shape the reference project writes: the right of an `||` runs where the left failed.
#[test]
fn the_right_of_an_or_after_a_failed_negated_identity_reads_the_receiver_as_there() {
    check(
        r#"<?php
class Repo
{
    public function run(?Order $order): void
    {
        if ($order?->status !== Status::Draft || $ord$0er->base === null) {
            return;
        }
    }
}
"#,
        "order",
        expect!["$order without what Status::Draft cannot be"],
    );
}

#[test]
fn a_receiver_whose_chain_matched_a_literal_is_not_null() {
    check(
        r#"<?php
class Repo
{
    public function run(?Order $order): void
    {
        if ($order?->name === 'paid') {
            $ord$0er;
        }
    }
}
"#,
        "order",
        expect!["not null $order"],
    );
}

#[test]
fn a_receiver_whose_chain_a_strict_search_found_is_not_null() {
    check(
        r#"<?php
class Repo
{
    public function run(?Order $order): void
    {
        if (in_array($order?->name, ['paid', 'sent'], true)) {
            $ord$0er;
        }
    }
}
"#,
        "order",
        expect!["not null $order"],
    );
}

/// A null among the values searched is a value the cut-short chain matches.
#[test]
fn a_search_that_includes_null_says_nothing_about_the_receiver() {
    check(
        r#"<?php
class Repo
{
    public function run(?Order $order): void
    {
        if (in_array($order?->name, ['paid', null], true)) {
            $ord$0er;
        }
    }
}
"#,
        "order",
        expect!["?Order"],
    );
}
