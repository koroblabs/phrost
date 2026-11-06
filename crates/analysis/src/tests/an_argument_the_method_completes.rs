//! An argument that is a catalog key with something taken off it.
//!
//! Nearly every method that looks a value up by a string is passed the key itself. Drupal's
//! `alter($type)` is not: it runs `hook_{$type}_alter`, so `alter('form')` names `form_alter` and
//! the six characters that make it a key are supplied by the method. The dispatching side of the
//! seam has carried that suffix since it existed; the *reading* side did not, which is the whole
//! shape of this -- one concept with two halves, and only one of them knowing the rule.
//!
//! What made the gap look harmless is that a stem is often a hook name too: both `hook_entity_view`
//! and `hook_entity_view_alter` are real, so judging `alter('entity_viw')` against `entity_view`
//! gave the right answer for the wrong reason. Where the stem is *not* a hook the check was silent,
//! and where it nearly hit an unrelated key the suggestion was a name nobody can write at that
//! position.
//!
//! Both directions have to hold, and the second is the one that costs. A suggestion is what a person
//! types, so the suffix comes off it -- and a candidate that does not carry the suffix at all is
//! declined rather than offered, because nothing written at that position can reach it.

use expect_test::expect;

use super::diagnostics::check_framework;

const FIXTURE: &str = r#"
//- /src/Handler.php
<?php
namespace Dep;

class ModuleHandler
{
    public function alter(string $type, mixed $data = NULL): void {}

    public function invokeAll(string $hook): array
    {
        return [];
    }
}

//- /src/Caller.php
<?php
namespace App;

use Dep\ModuleHandler;

class Caller
{
    public function suffixed(ModuleHandler $handler): void
    {
        $handler->alter('form', NULL);
        $handler->alter('frm', NULL);
        $handler->alter('form_alter', NULL);
        $handler->alter('presav', NULL);
    }

    public function plain(ModuleHandler $handler): void
    {
        $handler->invokeAll('form_alter');
        $handler->invokeAll('frm_alter');
        $handler->invokeAll('presave');
    }
}
"#;

fn registered(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\ModuleHandler", "alter", 0, "_alter", "hook");
    world
        .contracts_mut()
        .bind_argument("Dep\\ModuleHandler", "invokeAll", 0, "", "hook");
    // Two keys that carry the suffix and one that does not, which is the ordinary shape of a hook
    // catalog: a point something alters and a point something is notified of.
    world
        .contracts_mut()
        .catalog_entry("hook", "form_alter", None);
    world
        .contracts_mut()
        .catalog_entry("hook", "entity_view_alter", None);
    world.contracts_mut().catalog_entry("hook", "presave", None);
}

#[test]
fn the_suffix_the_method_adds_is_part_of_the_key_and_not_part_of_the_suggestion() {
    // Four suffixed calls and three plain ones over one catalog, so what the suffix changes is
    // visible against what it does not.
    //
    // `alter('form')` is silent because `form_alter` is registered -- the case that was silent
    // before too, and for the wrong reason. `alter('frm')` reports and names **`form`**, not
    // `form_alter`: the suggestion has to be typeable where it is offered. `alter('form_alter')` is
    // the suffix written twice, which asks for `hook_form_alter_alter` and is settled exactly rather
    // than by distance -- the argument as written is itself a key, so the fix is to take the suffix
    // off. And `alter('presav')` is silent although `presave` is one edit away, because `presave` is
    // not an `_alter` hook and no argument to `alter` can reach it.
    //
    // The plain half is the control: `invokeAll('frm_alter')` suggests `form_alter` whole, since
    // nothing is appended there and the registered key is what a person writes.
    check_framework(
        FIXTURE,
        registered,
        expect![[r#"
            11:25-11:30 warning [unknown-catalog-key] No hook is registered as `frm_alter`. Did you mean `form`?
            12:25-12:37 warning [unknown-catalog-key] No hook is registered as `form_alter_alter`. Did you mean `form`?
            19:29-19:40 warning [unknown-catalog-key] No hook is registered as `frm_alter`. Did you mean `form_alter`?
        "#]],
    );
}
