use expect_test::expect;
use phrost_model::Visibility::{Private, Protected, Public};

use super::{Source, check};

#[test]
fn own_beats_trait_beats_parent() {
    // The order that makes a trait able to override what a parent defined, while still losing to
    // the class's own declaration.
    let source = Source::new()
        .class("Base", |c| {
            c.method("fromParent", Public).method("shared", Public)
        })
        .php_trait("T", |c| {
            c.method("fromTrait", Public).method("shared", Public)
        })
        .class("C", |c| {
            c.extends("Base").uses("T").method("shared", Public)
        });

    check(
        &source,
        "C",
        expect![[r#"
            method fromParent()  public  [inherited Base]
            method fromTrait()  public  [trait T]
            method shared()  public  [own]
        "#]],
    );
}

#[test]
fn a_trait_overrides_an_inherited_member() {
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
fn a_parents_private_member_is_not_inherited() {
    // PHP does not inherit private members. Treating them as inherited would make a child's own
    // declaration look like an override, and report the parent as the declaration site.
    let source = Source::new()
        .class("Base", |c| {
            c.method("secret", Private)
                .method("shared", Protected)
                .property("hidden", Private)
        })
        .class("C", |c| c.extends("Base"));

    check(
        &source,
        "C",
        expect![[r#"
        method shared()  protected  [inherited Base]
    "#]],
    );
}

#[test]
fn a_traits_private_member_is_included() {
    // Traits are copy-paste: a private trait method behaves as if written in the using class. This
    // is the exact opposite of the inheritance rule above, which is why both are pinned.
    let source = Source::new()
        .php_trait("T", |c| c.method("helper", Private).method("run", Public))
        .class("C", |c| c.uses("T"));

    check(
        &source,
        "C",
        expect![[r#"
        method helper()  private  [trait T]
        method run()  public  [trait T]
    "#]],
    );
}

#[test]
fn a_concrete_parent_implementation_beats_an_interface_requirement() {
    let source = Source::new()
        .interface("Runnable", |c| c.abstract_method("run", Public))
        .class("Base", |c| c.method("run", Public))
        .class("C", |c| c.extends("Base").implements("Runnable"));

    check(
        &source,
        "C",
        expect![[r#"
        method run()  public  [inherited Base]
    "#]],
    );
}

#[test]
fn an_unimplemented_interface_member_still_appears_as_a_requirement() {
    // Completion should offer it and hover should describe it, even though nothing implements it
    // yet -- and a later diagnostic needs to know it is missing.
    let source = Source::new()
        .interface("Runnable", |c| {
            c.abstract_method("run", Public).constant("VERSION", Public)
        })
        .class("C", |c| c.implements("Runnable").abstract_class());

    check(
        &source,
        "C",
        expect![[r#"
        method run()  public abstract  [interface Runnable]
        const VERSION  public  [interface Runnable]
    "#]],
    );
}

#[test]
fn provenance_names_the_class_that_actually_declared_the_member() {
    // Two levels up. Reporting the immediate parent would send goto to a class that only inherited
    // it too.
    let source = Source::new()
        .class("Grandparent", |c| c.method("deep", Public))
        .class("Parent", |c| c.extends("Grandparent"))
        .class("Child", |c| c.extends("Parent"));

    check(
        &source,
        "Child",
        expect![[r#"
        method deep()  public  [inherited Grandparent]
    "#]],
    );
}

#[test]
fn a_member_inherited_through_a_parents_trait_points_at_the_trait() {
    // The declaration site is the trait, and that is where navigation should land -- not at the
    // parent that happened to use it.
    let source = Source::new()
        .php_trait("T", |c| c.method("run", Public))
        .class("Base", |c| c.uses("T"))
        .class("C", |c| c.extends("Base"));

    check(
        &source,
        "C",
        expect![[r#"
        method run()  public  [trait T]
    "#]],
    );
}

#[test]
fn an_interface_extending_another_collects_both() {
    let source = Source::new()
        .interface("A", |c| c.abstract_method("a", Public))
        .interface("B", |c| c.extends("A").abstract_method("b", Public));

    check(
        &source,
        "B",
        expect![[r#"
        method a()  public abstract  [interface A]
        method b()  public abstract  [own]
    "#]],
    );
}

#[test]
fn static_and_instance_members_of_the_same_name_are_one_member() {
    // PHP has one method table per class: a static and an instance method cannot share a name.
    let source = Source::new()
        .class("Base", |c| c.method("make", Public))
        .class("C", |c| c.extends("Base").static_method("make", Public));

    check(
        &source,
        "C",
        expect![[r#"
        method make()  public static  [own]
    "#]],
    );
}
