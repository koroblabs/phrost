//! `$repo->find(...)` and `Repo::find(...)` on the two projections that must disagree about them.
//!
//! The whole design of [`phrost_hir::body::ExprData::CallableMember`] is which walk reaches it. It
//! must be a **member site**, so that hover, goto, find-references and rename answer on the method
//! name -- and it must *not* be a **call site**, because it calls nothing: an argument-count check
//! walks the call sites, and a callable written with no arguments would fail every one of them.
//! A single snapshot of both lists is the only assertion that can catch either half breaking.
//!
//! The value's path is in the same snapshot for the third half of the same rule. `$fn` holds a
//! `Closure` and *not* whatever `find()` returns, so the path is a callable wrapped around the call
//! that was never made -- `callable(...): $repo->find()`. The wrapper is the whole assertion: with
//! the same inner path and no wrapper, `$fn` would hold a `Repo` and `$fn(1)` would be a call on
//! one.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_model::{FileId, Interner};

use super::render;
use crate::{CallTarget, SiteKind, call_sites, member_sites};

/// Every member site, every call site, and what each assignment's value paths to.
fn check(source: &str, expect: Expect) {
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
    let hir = &lowered.hir;

    let mut out = String::new();

    let _ = writeln!(out, "member sites");
    for site in member_sites(hir) {
        let _ = writeln!(
            out,
            "  {} [{:?}] on {}",
            site.name,
            site.kind,
            render(hir, &site.receiver)
        );
    }

    let _ = writeln!(out, "call sites");
    for site in call_sites(hir) {
        let _ = writeln!(
            out,
            "  {}",
            match &site.target {
                CallTarget::Member { receiver, name } =>
                    format!("{}->{name}()", render(hir, receiver)),
                CallTarget::Function(reference) => format!("{}()", hir.name_ref(*reference).path),
                CallTarget::Constructor(class) =>
                    format!("new {}", super::class_target(hir, *class)),
                CallTarget::Invoked(value) => format!("{}()", render(hir, value)),
            }
        );
    }

    // Each assignment's right-hand side, which is where a wrongly modelled callable would show up
    // as a type nobody wrote.
    let _ = writeln!(out, "values");
    for site in crate::assignment_sites(hir) {
        let _ = writeln!(
            out,
            "  {} = {}",
            match &site.target {
                crate::AssignmentTarget::Variable(name) => format!("${name}"),
                crate::AssignmentTarget::Property { name, .. } => format!("<value>->{name}"),
                crate::AssignmentTarget::StaticProperty { name, .. } => format!("<class>::${name}"),
            },
            match &site.value {
                Some(path) => render(hir, path),
                None => "<no path>".to_owned(),
            }
        );
    }

    expect.assert_eq(&out);
}

#[test]
fn a_static_callable_is_a_member_site_and_not_a_call_site() {
    check(
        r#"<?php

class Repo
{
    public static function find(int $id): Repo
    {
        return new Repo();
    }
}

function go(): void
{
    $fn = Repo::find(...);
}
"#,
        expect![[r#"
            member sites
              find [Method] on Repo
            call sites
              new Repo
            values
              $fn = callable(...): Repo->find()
        "#]],
    );
}

#[test]
fn a_method_callable_on_a_variable_is_a_member_site_on_that_variable() {
    check(
        r#"<?php

function go(Repo $repo): void
{
    $fn = $repo->find(...);
}
"#,
        expect![[r#"
            member sites
              find [Method] on $repo
            call sites
            values
              $fn = callable(...): $repo->find()
        "#]],
    );
}

#[test]
fn the_call_beside_it_is_still_a_call() {
    // The pair that says the split is about the shape and not about the name: one `find` is entered
    // and the other is not, and both name the same method on the same receiver.
    check(
        r#"<?php

function go(Repo $repo): void
{
    $fn = $repo->find(...);
    $row = $repo->find(1);
}
"#,
        expect![[r#"
            member sites
              find [Method] on $repo
              find [Method] on $repo
            call sites
              $repo->find()
            values
              $fn = callable(...): $repo->find()
              $row = $repo->find(@0 '1')
        "#]],
    );
}

#[test]
fn a_relative_receiver_keeps_its_own_meaning() {
    // `self::` is the declaring class and `static::` is the class the call was made through, which
    // is the one a guard on `$this` proves -- so it renders as `$this`. The same three roots a
    // static call produces, because the receiver goes through the same `class_path_root`.
    check(
        r#"<?php

class Repo
{
    public function make(): void
    {
        $one = self::find(...);
        $two = static::find(...);
        $three = parent::find(...);
    }
}
"#,
        expect![[r#"
            member sites
              find [Method] on self
              find [Method] on $this
              find [Method] on <parent>
            call sites
            values
              $one = callable(...): self->find()
              $two = callable(...): $this->find()
              $three = callable(...): <parent>->find()
        "#]],
    );
}

#[test]
fn a_computed_name_names_no_member() {
    // `$repo->$name(...)` has no name written, so there is nothing to hover and nothing to rename --
    // and the variable that computes it is still reachable, which is what the walk has to keep.
    check(
        r#"<?php

function go(Repo $repo, string $name): void
{
    $fn = $repo->$name(...);
}
"#,
        expect![[r#"
            member sites
            call sites
            values
              $fn = <no path>
        "#]],
    );
}

#[test]
fn a_callable_inside_a_call_is_reached_through_the_argument() {
    // The shape real code writes it in. `array_map` is a call site and the callable inside it is a
    // member site, and neither walk may swallow the other.
    check(
        r#"<?php

function go(Repo $repo, array $ids): array
{
    return array_map($repo->find(...), $ids);
}
"#,
        expect![[r#"
            member sites
              find [Method] on $repo
            call sites
              array_map()
            values
        "#]],
    );
}

#[test]
fn a_function_callable_names_no_member() {
    // `strlen(...)` has no receiver, so there is no member site to make. Its name is a name
    // reference in the function table, which is where goto on a function already looks.
    check(
        r#"<?php

function go(): void
{
    $fn = strlen(...);
}
"#,
        expect![[r#"
            member sites
            call sites
            values
              $fn = callable(...): strlen()
        "#]],
    );
}

#[test]
fn a_partial_application_php_does_not_have_keeps_what_it_wrote() {
    // mago parses the partial-application syntax an RFC proposed and no PHP release accepts.
    // Nothing models it, but the arguments it binds are written expressions and stay reachable --
    // the call inside `$repo->find($this->id(), ?)` resolves like a call anywhere else.
    check(
        r#"<?php

class Caller
{
    public function id(): int
    {
        return 1;
    }

    public function go(Repo $repo): void
    {
        $fn = $repo->find($this->id(), ?);
    }
}
"#,
        expect![[r#"
            member sites
              id [Method] on $this
            call sites
              $this->id()
            values
              $fn = <no path>
        "#]],
    );
}

/// A cursor on the method name of a callable finds the site, and one beside it does not.
#[test]
fn the_cursor_lands_on_the_method_name() {
    let source = r#"<?php

function go(Repo $repo): void
{
    $fn = $repo->find(...);
}
"#;
    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        source,
        &phrost_lower::KeyedAttributes::default(),
    );
    let hir = &lowered.hir;

    let name_at = u32::try_from(source.find("find").expect("the fixture writes the name"))
        .expect("fixture fits in u32");
    let mut out = String::new();
    // Every offset of the line, so the snapshot shows exactly where the site starts and stops --
    // an off-by-one in `name_range` is invisible to a single probe.
    for offset in name_at - 2..name_at + 7 {
        let site = crate::member_site_at(hir, phrost_model::TextSize::from(offset));
        let _ = writeln!(
            out,
            "{offset}: {}",
            match site {
                Some(site) => format!("{} [{:?}]", site.name, site.kind),
                None => "<nothing>".to_owned(),
            }
        );
    }

    expect![[r#"
        54: <nothing>
        55: <nothing>
        56: find [Method]
        57: find [Method]
        58: find [Method]
        59: find [Method]
        60: find [Method]
        61: <nothing>
        62: <nothing>
    "#]]
    .assert_eq(&out);

    // And the kind is a method, which is what decides whether hover says `find()` or `$find`.
    assert!(matches!(
        crate::member_site_at(hir, phrost_model::TextSize::from(name_at)).map(|site| site.kind),
        Some(SiteKind::Method)
    ));
}
