//! A misspelt hook name, which is the one runtime mistake that leaves no trace at all.
//!
//! `$moduleHandler->invokeAll('notifications_altr')` throws nothing, logs nothing, and returns an
//! empty array. Dispatch is by *name*: the kernel goes looking for `<extension>_notifications_altr`,
//! finds none, and that is a legitimate outcome for a hook nobody has implemented. So the typo is
//! indistinguishable from an extension point waiting for its first implementation -- at runtime.
//!
//! # Why the obvious catalog cannot answer
//!
//! There is already a catalog of hook names, filled from the project's own `invokeAll` and `alter`
//! calls. It cannot be measured against, because **the call being checked is what fills it**: a typo
//! registers itself and every dispatch site holds every name. That is why the near-miss reader was
//! silent here while it reported a misspelt route.
//!
//! So the catalog this is bound to is filled from the other side -- the *names of the functions that
//! implement hooks*, and the `function hook_x()` declarations that document them. A hook
//! implementation's name **is** the contract, which is what makes the name readable as one:
//! `app_core_notifications_alter()` implements `notifications_alter` because it is named
//! `<extension>_<hook>`. Measured on one real checkout: 1 038 names against the 7 the dispatch
//! catalog held, and a deliberate typo of each of the six hooks the project dispatches suggests the
//! right one while the four it does not hold suggest nothing.
//!
//! # Open, and unclosable
//!
//! A hook nobody has implemented is an ordinary extension point. Reporting an absence would light up
//! every new one on the day it is written, so the only claim is the near-miss.

use expect_test::expect;

use super::diagnostics::check_framework;

const FIXTURE: &str = r#"
//- /src/Handler.php
<?php
namespace Dep;

class ModuleHandler
{
    public function invokeAll(string $hook, array $args = []): array
    {
        return [];
    }

    public function invoke(string $module, string $hook, array $args = []): mixed
    {
        return NULL;
    }
}

//- /src/mine.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function mine_notifications_alter(array &$data): void {}

function hook_documented_thing(): void {}

//- /src/Caller.php
<?php
namespace App;

use Dep\ModuleHandler;

class Caller
{
    public function run(ModuleHandler $handler): void
    {
        $handler->invokeAll('notifications_alter');
        $handler->invokeAll('notifications_altr');
        $handler->invokeAll('documented_thing');
        $handler->invokeAll('nothing_has_implemented_this_yet');

        $handler->invoke('mine', 'notifications_altr');
        $handler->invoke('notifications_altr', 'notifications_alter');
    }
}
"#;

fn registered(world: &mut phrost_harness::World) {
    // The extension whose name a function's own name begins with.
    world
        .contracts_mut()
        .catalog_entry("extension", "mine", None);
    world
        .contracts_mut()
        .derive_names_with("extension", "hook_", "hook");
    world
        .contracts_mut()
        .bind_argument("Dep\\ModuleHandler", "invokeAll", 0, "", "hook");
    // The one that puts the module first and the hook second.
    world
        .contracts_mut()
        .bind_argument("Dep\\ModuleHandler", "invoke", 1, "", "hook");
}

#[test]
fn a_typo_reports_and_a_hook_nobody_has_implemented_does_not() {
    // The last two lines are the position half: `invoke($module, $hook)` puts the hook second, so the
    // *module* argument is a plain string -- `invoke('notifications_altr', ...)` must say nothing even
    // though the same text is a typo one slot to the right.
    check_framework(
        FIXTURE,
        registered,
        expect![[r#"
        11:29-11:49 warning [unknown-catalog-key] No hook is registered as `notifications_altr`. Did you mean `notifications_alter`?
        15:34-15:54 warning [unknown-catalog-key] No hook is registered as `notifications_altr`. Did you mean `notifications_alter`?
    "#]],
    );
}
