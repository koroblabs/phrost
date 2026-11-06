//! What flattening does when the hierarchy is broken.
//!
//! Real projects contain classes extending classes that are not installed, and code mid-edit
//! contains cycles. Both have to produce a usable partial answer, and neither may hang.

use expect_test::expect;
use phrost_model::Visibility::Public;

use super::{Source, check};

#[test]
fn a_self_referential_class_does_not_hang() {
    let source = Source::new().class("A", |c| c.extends("A").method("own", Public));

    check(
        &source,
        "A",
        expect![[r#"
        method own()  public  [own]
        ?? incomplete: a parent, interface, or trait was missing or cyclic
    "#]],
    );
}

#[test]
fn a_cycle_between_two_classes_does_not_hang() {
    let source = Source::new()
        .class("A", |c| c.extends("B").method("a", Public))
        .class("B", |c| c.extends("A").method("b", Public));

    check(
        &source,
        "A",
        expect![[r#"
        method a()  public  [own]
        method b()  public  [inherited B]
        ?? incomplete: a parent, interface, or trait was missing or cyclic
    "#]],
    );
}

#[test]
fn a_missing_parent_leaves_the_table_incomplete_but_usable() {
    // The class's own members are still worth showing. Refusing to answer would make completion go
    // silent whenever a dependency is not installed.
    let source = Source::new().class("C", |c| c.extends("NotInstalled").method("own", Public));

    check(
        &source,
        "C",
        expect![[r#"
        method own()  public  [own]
        ?? incomplete: a parent, interface, or trait was missing or cyclic
    "#]],
    );
}

#[test]
fn a_missing_trait_is_reported_the_same_way() {
    let source = Source::new().class("C", |c| c.uses("MissingTrait").method("own", Public));

    check(
        &source,
        "C",
        expect![[r#"
        method own()  public  [own]
        ?? incomplete: a parent, interface, or trait was missing or cyclic
    "#]],
    );
}

#[test]
fn asking_about_a_class_that_does_not_exist_is_not_an_error() {
    let source = Source::new();

    check(
        &source,
        "Nothing",
        expect![[r#"
        ?? incomplete: a parent, interface, or trait was missing or cyclic
    "#]],
    );
}

#[test]
fn a_class_name_is_matched_case_insensitively() {
    // PHP folds class names, so the hierarchy walk has to as well -- otherwise `extends base`
    // silently finds nothing and every member of the parent disappears.
    let source = Source::new()
        .class("Base", |c| c.method("run", Public))
        .class("C", |c| c.extends("BASE"));

    check(
        &source,
        "c",
        expect![[r#"
        method run()  public  [inherited BASE]
    "#]],
    );
}

#[test]
fn a_diamond_of_interfaces_contributes_each_member_once() {
    let source = Source::new()
        .interface("Root", |c| c.abstract_method("root", Public))
        .interface("Left", |c| c.extends("Root"))
        .interface("Right", |c| c.extends("Root"))
        .class("C", |c| c.implements("Left").implements("Right"));

    check(
        &source,
        "C",
        expect![[r#"
        method root()  public abstract  [interface Root]
    "#]],
    );
}
