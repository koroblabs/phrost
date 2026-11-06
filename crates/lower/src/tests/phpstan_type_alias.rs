//! `@phpstan-type` / `@psalm-type`: a class's own alias, substituted where it is used.
//!
//! The behaviour lives in `crates/lower/src/docblock/type_alias.rs` and the `aliases` parameter it
//! adds to `crates/lower/src/docblock/type_lowering.rs::lower_doc_type`; this file is the snapshot
//! that proves it landed. [`check_hir`](super::check_hir) cannot: its dump renders a member's
//! *native* `type_ref`, never `doc_type_ref`, so a fixture whose native declaration is a bare
//! `array` (the common case an alias refines) shows the same text whether or not the substitution
//! happened. [`member_types`] and [`param_doc_type`] read `doc_type_ref` directly instead.

use phrost_hir::render::render_type_ref;
use phrost_model::FileId;
use phrost_syntax::ParseSession;

/// The native and docblock-derived type of one class's member, each rendered as text.
///
/// `None` for whichever side the member did not declare: a bare `@return` with no native hint
/// yields no first element, and a native hint with nothing beside it yields no second.
fn member_types(source: &str, class: &str, member: &str) -> (Option<String>, Option<String>) {
    let mut interner = phrost_model::Interner::new();
    let session = ParseSession::new();
    let parsed = session.parse(source);
    let lowered = crate::lower_file(
        &mut interner,
        FileId::from_raw(0),
        &parsed,
        &crate::KeyedAttributes::default(),
    );
    let hir = &lowered.hir;

    let (_, class_data) = hir
        .classes()
        .find(|(_, data)| data.name.as_str() == class)
        .unwrap_or_else(|| panic!("fixture declares no class named {class}"));
    let member_data = hir
        .member_list(class_data.members)
        .iter()
        .map(|&id| hir.member(id))
        .find(|data| data.name.as_str() == member)
        .unwrap_or_else(|| panic!("class {class} declares no member named {member}"));

    (
        member_data.type_ref.map(|id| render_type_ref(hir, id)),
        member_data.doc_type_ref.map(|id| render_type_ref(hir, id)),
    )
}

/// The docblock-derived type of one parameter of one method, rendered as text.
///
/// [`member_types`] answers about the method itself -- its return type -- which is a different
/// declaration from one of its parameters; a `@param` alias needs this one instead.
fn param_doc_type(source: &str, class: &str, method: &str, param: &str) -> Option<String> {
    let mut interner = phrost_model::Interner::new();
    let session = ParseSession::new();
    let parsed = session.parse(source);
    let lowered = crate::lower_file(
        &mut interner,
        FileId::from_raw(0),
        &parsed,
        &crate::KeyedAttributes::default(),
    );
    let hir = &lowered.hir;

    let (_, class_data) = hir
        .classes()
        .find(|(_, data)| data.name.as_str() == class)
        .unwrap_or_else(|| panic!("fixture declares no class named {class}"));
    let method_data = hir
        .member_list(class_data.members)
        .iter()
        .map(|&id| hir.member(id))
        .find(|data| data.name.as_str() == method)
        .unwrap_or_else(|| panic!("class {class} declares no member named {method}"));
    let param_data = hir
        .param_list(method_data.params)
        .iter()
        .map(|&id| hir.param(id))
        .find(|data| data.name.as_str() == param)
        .unwrap_or_else(|| panic!("method {method} declares no parameter named {param}"));

    param_data.doc_type_ref.map(|id| render_type_ref(hir, id))
}

#[test]
fn an_alias_expands_where_a_bare_array_return_leaves_room_for_it() {
    // The exact shape a bare `array` leaves room for an element type -- see
    // `refined_type_ref` in `crates/hir/src/type_ref.rs`, which is what actually decides which of
    // the two sides below wins at the view layer. Both are recorded here because that decision is
    // deliberately not this file's: this only proves the alias reached `doc_type_ref` at all.
    let (native, doc) = member_types(
        r#"<?php
class User {}

/**
 * @phpstan-type UserList array<int, User>
 */
class Foo
{
    /**
     * @return UserList
     */
    public function getUsers(): array
    {
    }
}
"#,
        "Foo",
        "getUsers",
    );

    assert_eq!(native.as_deref(), Some("array"));
    assert_eq!(doc.as_deref(), Some("array<int, User>"));
}

#[test]
fn the_psalm_prefix_names_the_same_tag_and_reaches_a_parameter_too() {
    // mago folds both vendor spellings into one `TagValue::TypeAlias` before this ever sees the
    // tag name -- see the module doc on `TypeAliasTable` -- so `@psalm-type` needs no code of its
    // own here. Exercised on a `@param` rather than `@return` to cover the other tag
    // `doc_type_beside_with_aliases` is called for -- and threaded through
    // `Lowerer::params_with_aliases` rather than the plain `Lowerer::params` a free function uses.
    let doc = param_doc_type(
        r#"<?php
class User {}

/**
 * @psalm-type UserMap array<string, User>
 */
class Registry
{
    /** @param UserMap $users */
    public function seed($users): void {}
}
"#,
        "Registry",
        "seed",
        "users",
    );

    assert_eq!(doc.as_deref(), Some("array<string, User>"));
}

#[test]
fn an_alias_expands_nested_inside_a_union_and_a_property_var() {
    // The shape a real, unmodified codebase used this against: a magic-cache property whose
    // `@var` names the alias, and the alias itself is an `object{...}` shape rather than an
    // `array<K, V>`. Both a plain use and a use nested inside `false|Alias` (also seen in that
    // corpus) go through the same recursive `lower_doc_type`, so one fixture covers both shapes.
    let (_, doc) = member_types(
        r#"<?php
/**
 * @phpstan-type CachedItemObject object{cid: string, data: string}
 */
class Cache
{
    /** @var false|CachedItemObject */
    public $current;
}
"#,
        "Cache",
        "current",
    );

    // The shape's properties survive: an `object{...}` is its own node, not a bare `object`.
    assert_eq!(
        doc.as_deref(),
        Some("false|object{cid: string, data: string}")
    );
}

#[test]
fn an_alias_used_as_a_generic_argument_expands_too() {
    // `array<string, FieldType>`: the alias is not the whole type, only one argument of it -- the
    // shape `docblock_type_aliases`'s own module doc calls out as needing `lower_doc_type`'s
    // recursion to thread `aliases` all the way down rather than only checking the top-level type.
    let (_, doc) = member_types(
        r#"<?php
/**
 * @phpstan-type FieldType array{title: string}
 */
class Form
{
    /** @return array<string, FieldType> */
    public function fields(): array
    {
    }
}
"#,
        "Form",
        "fields",
    );

    assert_eq!(doc.as_deref(), Some("array<string, array{title: string}>"));
}

#[test]
fn a_self_referential_alias_terminates_instead_of_looping() {
    // The termination bound `type_alias.rs` documents: an alias's own right-hand side is lowered
    // against an empty table, so `@phpstan-type A A` can never look itself up while it is still
    // being built. The test finishing at all is part of the proof; the assertion is that what
    // comes out is an ordinary, unresolved reference to a class named `A` -- the same answer a
    // member would get if `@phpstan-type` had never been declared, which is the deterministic
    // *not found* the zero-fallbacks rule asks for rather than a wrong guess.
    let (_, doc) = member_types(
        r#"<?php
/**
 * @phpstan-type A A
 */
class Loop
{
    /** @return A */
    public function value() {}
}
"#,
        "Loop",
        "value",
    );

    assert_eq!(doc.as_deref(), Some("A"));
}

#[test]
fn an_import_type_tag_settles_nothing_rather_than_naming_a_class() {
    // `@phpstan-import-type` names an alias declared on a *different* file's class -- a cross-file
    // lookup lowering cannot make, since lowering is one file at a time and the class it would read
    // may not even have been parsed yet (see the module doc on `type_alias.rs`). So it is not
    // expanded.
    //
    // What it must not do is what it used to: leaving the name unread made it an ordinary class
    // reference, so `@return Row` above a method handing back an array read as a class the project
    // does not declare and the return was reported. The same trap a `@template` parameter sets, and
    // the same answer -- the name is recorded and the type settles nothing, which leaves the native
    // hint standing and says nothing false.
    let doc = param_doc_type(
        r#"<?php
/**
 * @phpstan-import-type AddressComparisonResult from \App\AddressMatcher
 */
class Report
{
    /** @param AddressComparisonResult $result */
    public function record($result): void {}
}
"#,
        "Report",
        "record",
        "result",
    );

    assert_eq!(doc, None);
}

#[test]
fn an_imported_alias_renamed_by_an_as_clause_is_recorded_under_the_local_name() {
    // `... as Line` is the spelling this file uses, and the name it imported never appears in it --
    // so recording the far name would leave the near one reaching a class nobody declares, which is
    // the whole thing being avoided.
    let doc = param_doc_type(
        r#"<?php
/**
 * @phpstan-import-type Row from \App\Source as Line
 */
class Report
{
    /** @param Line $line */
    public function record($line): void {}
}
"#,
        "Report",
        "record",
        "line",
    );

    assert_eq!(doc, None);
}

#[test]
fn an_alias_may_be_written_in_terms_of_one_declared_above_it() {
    // Symfony's container configurator is what this is for: `ConfigType` is assembled out of
    // `ImportsConfig`, `ParametersConfig` and `ServicesConfig`, each its own `@psalm-type` on the
    // same class. Until an alias's body was lowered against the aliases already resolved, the
    // inner names stayed bare -- and a bare name in a type position is not *nothing*, it is an
    // ordinary class reference, so every array passed for `services` was reported against a class
    // nobody declares.
    let doc = param_doc_type(
        r#"<?php
/**
 * @phpstan-type Inner array{a: int}
 * @phpstan-type Outer array{b?: Inner}
 */
class Holder
{
    /** @param Outer $config */
    public function take($config): void {}
}
"#,
        "Holder",
        "take",
        "config",
    );

    assert_eq!(doc.as_deref(), Some("array{b?: array{a: int}}"));
}

#[test]
fn a_cycle_among_aliases_resolves_to_nothing_rather_than_looping() {
    // Declaration order is the whole termination bound: a name is entered only once its own body
    // is lowered, so a body that names itself -- or names a sibling that names it back -- looks
    // itself up in a table it is not in yet and misses. The same miss the old rule gave for every
    // chain, now only for the chains that cannot mean anything.
    let doc = param_doc_type(
        r#"<?php
/**
 * @phpstan-type Ping Pong
 * @phpstan-type Pong Ping
 */
class Holder
{
    /** @param Ping $value */
    public function take($value): void {}
}
"#,
        "Holder",
        "take",
        "value",
    );

    assert_eq!(doc.as_deref(), Some("Pong"));
}
