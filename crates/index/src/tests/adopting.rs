//! What survives an index being replaced.
//!
//! A skeleton is thrown away twice in a session's life -- when a cache load supersedes it, and when
//! configuration changes and the project is re-read -- and both used to be a whole-struct
//! assignment. Anything installed from above went with it, silently: the project was then served as
//! if no framework were present, on exactly the runs where the cache worked. The previous attempt
//! shipped that bug twice, in two different tables.

use phrost_model::{FileId, SourceOrigin};

use crate::ProjectSkeleton;

/// Builds a skeleton over one file, so `adopt` has something real to take on.
fn indexed(text: &str) -> ProjectSkeleton {
    let mut skeleton = ProjectSkeleton::new();
    skeleton.set_root("/project");
    skeleton.ingest(
        FileId::from_raw(0),
        std::path::Path::new("/project/app.php"),
        SourceOrigin::User,
        text,
    );
    skeleton.mark_ready();
    skeleton
}

#[test]
fn adopting_takes_the_files_and_keeps_the_contracts() {
    let mut live = ProjectSkeleton::new();
    live.returns_mut().add_unproduced(
        "Db\\Connection",
        "query",
        phrost_model::BuiltinTy::Null,
        None,
        None,
    );

    live.adopt(indexed("<?php class Thing {}"));

    assert_eq!(live.file_count(), 1, "the loaded files arrived");
    assert!(
        live.state().is_ready(),
        "and so did what it knew about itself"
    );
    assert_eq!(
        live.returns()
            .unproduced_declarers("query", phrost_model::BuiltinTy::Null)
            .collect::<Vec<_>>(),
        ["Db\\Connection"],
        "and the contract installed from above stayed"
    );
}

#[test]
fn adopting_does_not_take_the_contracts_of_the_index_it_replaces() {
    let mut loaded = indexed("<?php class Thing {}");
    loaded.returns_mut().add_unproduced(
        "Db\\Connection",
        "query",
        phrost_model::BuiltinTy::Null,
        None,
        None,
    );

    let mut live = ProjectSkeleton::new();
    live.adopt(loaded);

    // The one that matters for a cache: a contract is a fact about the framework installed *now*,
    // and one that came back from disk would speak for a framework the project may have dropped.
    assert!(live.returns().is_empty());
}

/// The two by-name gates a guard reader asks before it resolves anything survive being adopted.
///
/// They are derived from the files rather than installed from above, so they travel with them --
/// and that is the half worth pinning, because the failure is a *warm start*: the first server of
/// the day answers and every one after it does not. §8 records that shape from the hook catalog,
/// where a claim absent from both sides of an equality was invisible to the round-trip test that
/// existed. Neither list had a test of its own until this one.
#[test]
fn adopting_keeps_the_names_the_guard_reader_gates_on() {
    let mut live = ProjectSkeleton::new();
    live.adopt(indexed(
        r#"<?php
/**
 * @phpstan-assert-if-true \Err $thing
 */
function is_err(mixed $thing): bool {}

function abort_now(): never {}
"#,
    ));

    assert!(
        live.declares_argument_assertion("is_err"),
        "a guard calling is_err() would otherwise resolve nothing on every warm start"
    );
    assert!(
        live.declares_never_returning("abort_now"),
        "and a branch ending in abort_now() would read as falling through"
    );
    // Case-insensitively, because PHP compares a callable's name that way and a call writes
    // whatever the author typed.
    assert!(live.declares_argument_assertion("IS_ERR"));
    assert!(!live.declares_argument_assertion("is_error"));
}

/// A table a file's own body stated has to be said again into the adopting skeleton's contracts.
///
/// The trap this module exists for, arriving a fourth time. A loaded index puts what its files said
/// into *its* `returns`, and `returns` is the one thing adopting does not carry -- rightly, because
/// the rule saying which declaration states a table is configuration. What the files said is not,
/// so it has to be re-stated, exactly as the catalog claims are.
///
/// It could only be seen through a driven server: `phrost query` and `phrost check` are both cold
/// starts, so both answered correctly while every warm session answered `mixed`.
#[test]
fn adopting_re_states_the_tables_its_files_declared() {
    let mut live = ProjectSkeleton::new();
    live.returns_mut()
        .add_declared_table(crate::DeclaredTableRule {
            attribute: "Element".into(),
            argument: crate::AttributeArgument::At(0),
            method: "shapeOf".into(),
            builders: vec!["Slot".into()],
            kinds: vec![("text".into(), phrost_model::BuiltinTy::String)],
            catalog: "element".into(),
        });
    live.returns_mut()
        .add_catalog_kind("element", "card", "cardish");

    let mut loaded = ProjectSkeleton::new();
    loaded
        .returns_mut()
        .add_declared_table(crate::DeclaredTableRule {
            attribute: "Element".into(),
            argument: crate::AttributeArgument::At(0),
            method: "shapeOf".into(),
            builders: vec!["Slot".into()],
            kinds: vec![("text".into(), phrost_model::BuiltinTy::String)],
            catalog: "element".into(),
        });
    loaded.set_keyed_attributes(crate::KeyedAttributes::of(["Element"]));
    loaded.set_root("/project");
    loaded.ingest(
        FileId::from_raw(0),
        std::path::Path::new("/project/app.php"),
        SourceOrigin::User,
        r#"<?php
#[Element('cardish')]
class CardElement
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text');
        return $slots;
    }
}
"#,
    );
    loaded.mark_ready();

    live.adopt(loaded);

    let rows = live
        .returns()
        .property_table("element", "card")
        .expect("the table its files stated, through the kind the key is of");
    assert_eq!(rows.properties.len(), 1);
    assert_eq!(&*rows.properties[0].0, "title");
    assert_eq!(rows.properties[0].1, Some(phrost_model::BuiltinTy::String));
    assert!(rows.complete);
    assert!(
        live.property_shape("element", "card").is_some(),
        "and the shape, interned against the table adopting just installed"
    );
}
