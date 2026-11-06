use expect_test::expect;
use phrost_model::Visibility::{Protected, Public};

use super::{Source, check};

#[test]
fn two_traits_declaring_the_same_method_is_reported_not_resolved() {
    // Fatal in PHP. Inventing a winner would make the language server disagree with the runtime,
    // and disagreeing silently is worse than saying so.
    let source = Source::new()
        .php_trait("A", |c| c.method("run", Public))
        .php_trait("B", |c| c.method("run", Public))
        .class("C", |c| c.uses("A").uses("B"));

    check(
        &source,
        "C",
        expect![[r#"
        method run()  public  [trait A]
        !! method run from both A and B
    "#]],
    );
}

#[test]
fn insteadof_resolves_the_conflict() {
    let source = Source::new()
        .php_trait("A", |c| c.method("run", Public))
        .php_trait("B", |c| c.method("run", Public).method("other", Public))
        .class("C", |c| c.uses("A").uses("B").insteadof("A", "run", &["B"]));

    check(
        &source,
        "C",
        expect![[r#"
        method other()  public  [trait B]
        method run()  public  [trait A]
    "#]],
    );
}

#[test]
fn an_alias_adds_a_name_and_leaves_the_original_reachable() {
    // `as` does not rename. Treating it as a rename would make the original method disappear from
    // completion even though calling it still works.
    let source = Source::new()
        .php_trait("T", |c| c.method("run", Public))
        .class("C", |c| c.uses("T").alias("run", Some("legacyRun"), None));

    check(
        &source,
        "C",
        expect![[r#"
        method run() (as legacyRun)  public  [trait T]
        method run()  public  [trait T]
    "#]],
    );
}

#[test]
fn an_alias_can_change_visibility_while_renaming() {
    let source = Source::new()
        .php_trait("T", |c| c.method("run", Public))
        .class("C", |c| {
            c.uses("T")
                .alias("run", Some("internalRun"), Some(Protected))
        });

    check(
        &source,
        "C",
        expect![[r#"
        method run() (as internalRun)  protected  [trait T]
        method run()  public  [trait T]
    "#]],
    );
}

#[test]
fn a_bare_visibility_change_modifies_the_member_in_place() {
    let source = Source::new()
        .php_trait("T", |c| c.method("run", Public))
        .class("C", |c| c.uses("T").alias("run", None, Some(Protected)));

    check(
        &source,
        "C",
        expect![[r#"
        method run()  protected  [trait T]
    "#]],
    );
}

#[test]
fn a_trait_using_another_trait_contributes_both() {
    let source = Source::new()
        .php_trait("Inner", |c| c.method("inner", Public))
        .php_trait("Outer", |c| c.uses("Inner").method("outer", Public))
        .class("C", |c| c.uses("Outer"));

    check(
        &source,
        "C",
        expect![[r#"
        method inner()  public  [trait Inner]
        method outer()  public  [trait Outer]
    "#]],
    );
}

#[test]
fn a_later_trait_does_not_conflict_with_an_inherited_member() {
    // Only trait-versus-trait is a conflict. Trait-over-parent is the normal precedence rule and
    // must not be reported.
    let source = Source::new()
        .class("Base", |c| c.method("run", Public))
        .php_trait("T", |c| c.method("run", Public))
        .class("C", |c| c.extends("Base").uses("T"));

    check(
        &source,
        "C",
        expect![[r#"
        method run()  public  [trait T]
    "#]],
    );
}

#[test]
fn trait_properties_and_constants_come_through_too() {
    let source = Source::new()
        .php_trait("T", |c| {
            c.property("items", Public).constant("LIMIT", Public)
        })
        .class("C", |c| c.uses("T"));

    check(
        &source,
        "C",
        expect![[r#"
        property $items  public  [trait T]
        const LIMIT  public  [trait T]
    "#]],
    );
}

#[test]
fn an_alias_naming_a_trait_takes_the_method_from_that_trait() {
    // The canonical PHP idiom: `insteadof` gives the name to one trait, and `as` re-exposes the
    // other trait's version under a second name. Resolving the alias against the merged table
    // instead would hand back the winner's method and report the wrong declaration site.
    let source = Source::new()
        .php_trait("Alpha", |c| c.method("run", Public))
        .php_trait("Beta", |c| c.method("run", Public))
        .class("C", |c| {
            c.uses("Alpha")
                .uses("Beta")
                .insteadof("Alpha", "run", &["Beta"])
                .alias_from("Beta", "run", Some("legacyRun"), Some(Protected))
        });

    check(
        &source,
        "C",
        expect![[r#"
        method run() (as legacyRun)  protected  [trait Beta]
        method run()  public  [trait Alpha]
    "#]],
    );
}

#[test]
fn an_alias_can_re_expose_a_method_insteadof_removed_entirely() {
    // Beta::run is excluded from the merged table, so the alias is the only way it stays reachable.
    let source = Source::new()
        .php_trait("Alpha", |c| c.method("run", Public))
        .php_trait("Beta", |c| c.method("run", Public).method("only", Public))
        .class("C", |c| {
            c.uses("Alpha")
                .uses("Beta")
                .insteadof("Alpha", "run", &["Beta"])
                .alias_from("Beta", "run", Some("betaRun"), None)
        });

    check(
        &source,
        "C",
        expect![[r#"
        method run() (as betaRun)  public  [trait Beta]
        method only()  public  [trait Beta]
        method run()  public  [trait Alpha]
    "#]],
    );
}
