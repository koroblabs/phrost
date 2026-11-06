//! `$app['config']->get(...)` -- a subscript whose key is a catalog key.
//!
//! Some classes answer a subscript by looking the key up: Laravel's container implements
//! `offsetGet($offset) { return $this->make($offset); }`, so `$app['config']` *is*
//! `make('config')` and the class it hands back depends on the string. Nothing in the declarations
//! says so -- both methods return `mixed` -- so the receiver of `->get` had no type at all, on 575
//! sites of one framework's own source against 80 written as `make`.
//!
//! One rule and three spellings, which is why the generic layer needed one arm rather than a
//! mechanism: a call with a literal first argument already reaches `returned_from_argument`, and
//! the subscript is the spelling that has no call to read. A framework states the binding once and
//! all three answer.
//!
//! The binding list is asked *before* the receiver is resolved, and the last test here is what
//! pins that: in a project whose framework binds nothing, a subscript must cost one failed
//! comparison rather than a receiver resolution.

use expect_test::expect;

use super::diagnostics::check_framework;

const CONTAINER: &str = r#"
//- /app.php
<?php
class Repository
{
    public function get(string $key): mixed
    {
        return NULL;
    }
}

class Container
{
    public function make(string $id): mixed
    {
        return NULL;
    }

    public function offsetGet(string $id): mixed
    {
        return $this->make($id);
    }
}
"#;

/// The subscript spelling, which is the one nothing could read before.
#[test]
fn a_literal_subscript_resolves_through_the_catalog() {
    check_framework(
        &[
            CONTAINER,
            r#"
//- /uses.php
<?php
function read(Container $app): void
{
    $app['config']->get('a.b');
    $app['config']->nope();
}
"#,
        ]
        .concat(),
        |world| {
            world
                .contracts_mut()
                .bind_argument("Container", "offsetGet", 0, "", "container");
            world
                .contracts_mut()
                .catalog_entry("container", "config", Some("Repository"));
        },
        expect![[r#"
            5:21-5:25 error [undefined-method] Undefined method Repository::nope()
        "#]],
    );
}

/// The call spelling, on the same binding, so the pair says the rule is one rule.
#[test]
fn the_same_binding_answers_a_call() {
    check_framework(
        &[
            CONTAINER,
            r#"
//- /called.php
<?php
function read(Container $app): void
{
    $app->make('config')->get('a.b');
    $app->make('config')->nope();
}
"#,
        ]
        .concat(),
        |world| {
            world
                .contracts_mut()
                .bind_argument("Container", "make", 0, "", "container");
            world
                .contracts_mut()
                .catalog_entry("container", "config", Some("Repository"));
        },
        expect![[r#"
            5:27-5:31 error [undefined-method] Undefined method Repository::nope()
        "#]],
    );
}

/// A key the catalog does not hold answers nothing, rather than a plausible class.
///
/// The catalog is open on purpose -- an application binds its own ids constantly, so a closed one
/// would report every `$app[Repo::class]` as an unknown key, which is the mistake §8 records for
/// `getInstanceFromDefinition`. So an unknown key is silence, and silence is what this pins: the
/// call on it reports nothing at all rather than reporting against the wrong class.
#[test]
fn a_key_the_catalog_does_not_hold_says_nothing() {
    check_framework(
        &[
            CONTAINER,
            r#"
//- /unknown.php
<?php
function read(Container $app): void
{
    $app['not.registered']->nope();
}
"#,
        ]
        .concat(),
        |world| {
            world
                .contracts_mut()
                .bind_argument("Container", "offsetGet", 0, "", "container");
            world
                .contracts_mut()
                .catalog_entry("container", "config", Some("Repository"));
        },
        expect![[r#""#]],
    );
}

/// A free function reaching the same container -- Laravel's `app('config')` -- on a binding of its
/// own, since a function has no class to bind a method on. A function of the same name with no
/// binding, called in a namespace that declares one, answers nothing.
#[test]
fn a_bound_function_answers_through_the_same_catalog() {
    check_framework(
        &[
            CONTAINER,
            r#"
//- /helpers.php
<?php
function app(?string $id = NULL): mixed
{
    return NULL;
}

//- /local.php
<?php
namespace Local;

function app(?string $id = NULL): mixed
{
    return NULL;
}

//- /helped.php
<?php
function read(): void
{
    app('config')->get('a.b');
    app('config')->nope();
    app('not.registered')->nope();
    \Local\app('config')->nope();
}
"#,
        ]
        .concat(),
        |world| {
            world
                .contracts_mut()
                .bind_function_argument("app", 0, "container");
            world
                .contracts_mut()
                .catalog_entry("container", "config", Some("Repository"));
        },
        expect![[r#"
            5:20-5:24 error [undefined-method] Undefined method Repository::nope()
        "#]],
    );
}

/// The chain being typed, which has no expression to read the argument off: `app('config')->` is
/// characters, and the text reader has to settle the binding the parsed path does.
#[test]
fn a_bound_function_completes_while_its_chain_is_typed() {
    super::check_with(
        &[
            CONTAINER,
            r#"
//- /helpers.php
<?php
function app(?string $id = NULL): mixed
{
    return NULL;
}

//- /typing.php
<?php
function read(): void
{
    app('config')->$0
}
"#,
        ]
        .concat(),
        &[super::Query::completion("app('config')->", "0")],
        |world| {
            world
                .contracts_mut()
                .bind_function_argument("app", 0, "container");
            world
                .contracts_mut()
                .catalog_entry("container", "config", Some("Repository"));
        },
        expect![[r#"
            app('config')->
            - get [method]  : mixed  writes `(|);`
            - accepting the first leaves `app('config')->get`
        "#]],
    );
}
