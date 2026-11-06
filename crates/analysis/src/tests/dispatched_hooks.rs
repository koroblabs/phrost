//! An extension point the project declares by dispatching it, reached from a docblock.
//!
//! The half of `Implements hook_X()` that had no answer. A framework's own extension points are
//! documented in files the index reads, so the sentence names a declaration and hover shows its
//! prose; a project's own are documented nowhere at all, and until the call that dispatches one was
//! indexed the sentence named nothing -- goto returned empty and the checker could not tell such a
//! name apart from one that had been removed.
//!
//! What hover can show for one is deliberately less: there is no signature and no `@param` lines,
//! because a dispatched name has no declaration to read them off. What there is is where the call
//! is and what it passes, which is the whole of what the arguments of the implementation are.

use expect_test::expect;
use phrost_harness::{DispatchRule, World};

use super::{Query, check_with, diagnostics::check_framework};

/// A project that invokes an extension point of its own, and a module that implements it.
const PROJECT: &str = r#"
//- /core/module.api.php
<?php
namespace Core;
interface ModuleHandler {
    public function invokeAll(string $hook, array $args = []): array;
    public function alter($type, &$data): void;
}

//- /modules/app_core/src/Notifications.php
<?php
namespace Core;
class Notifications {
    public function __construct(private ModuleHandler $moduleHandler) {}

    public function types(): array {
        return $this->moduleHandler->invokeAll('notifications_alter');
    }
}

//- /modules/app_region/app_region.php
<?php

/**
 * Implements hook_notifications_@reference@alter().
 */
function app_region_$0notifications_alter(): array {
    return [];
}
"#;

/// The convention, and the two methods the framework dispatches with.
fn conventions(world: &mut World) {
    world.contracts_mut().add_documented_elsewhere(
        "Implements",
        "hook_",
        Some("extension"),
        Some("hook"),
    );
    world
        .contracts_mut()
        .catalog_entry("extension", "app_region", None);
    world
        .contracts_mut()
        .add_dispatcher("invokeAll", dispatches(0, false, ""));
    world
        .contracts_mut()
        .add_dispatcher("alter", dispatches(0, true, "_alter"));
}

/// One rule, spelled once: six fields of which four are strings is not something to repeat.
fn dispatches(argument: u16, several: bool, suffix: &str) -> DispatchRule {
    DispatchRule {
        class: "Core\\ModuleHandler".into(),
        argument,
        several,
        names: "hook_".into(),
        suffix: suffix.into(),
        catalog: "hook".into(),
    }
}

#[test]
fn the_call_that_dispatches_a_hook_is_where_the_hook_comes_from() {
    check_with(
        PROJECT,
        &[
            Query::goto("goto from the sentence", "reference"),
            Query::hover("hover on the sentence", "reference"),
            Query::hover("hover on the implementation", "0"),
        ],
        conventions,
        expect![[r#"
            goto from the sentence
            - catalog key hook_notifications_alter @ modules/app_core/src/Notifications.php 7:48-7:69 (full 7:16-7:70)
            hover on the sentence
            - code: hook_notifications_alter()
            - note: hook_notifications_alter() is not declared anywhere -- it is dispatched from:
            - note: modules/app_core/src/Notifications.php
            - code: $this->moduleHandler->invokeAll('notifications_alter')
            hover on the implementation
            - code: <?php
            function app_region_notifications_alter(): array
            - ---
            - doc: Implements hook_notifications_alter().
            - ---
            - note: hook_notifications_alter() is not declared anywhere -- it is dispatched from:
            - note: modules/app_core/src/Notifications.php
            - code: $this->moduleHandler->invokeAll('notifications_alter')
        "#]],
    );
}

#[test]
fn a_claim_about_a_point_the_project_dispatches_is_not_reported() {
    // The six findings that made the companion check unshippable, in one file. Without the dispatch
    // indexed this reports the implementation as dead code -- and the code runs.
    check_framework(
        r#"
//- /core/module.api.php
<?php
namespace Core;
interface ModuleHandler {
    public function invokeAll(string $hook, array $args = []): array;
}

//- /modules/app_core/src/Notifications.php
<?php
namespace Core;
class Notifications {
    public function __construct(private ModuleHandler $moduleHandler) {}

    public function types(): array {
        return $this->moduleHandler->invokeAll('notifications_alter');
    }
}

//- /modules/app_region/app_region.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function app_region_notifications_alter(): array {
    return [];
}
"#,
        conventions,
        expect![[r#""#]],
    );
}

#[test]
fn a_claim_about_a_point_that_was_removed_is_reported() {
    // The class of bug a team hits on every major upgrade: the framework drops an extension point,
    // the implementations of it stay, and nothing anywhere says so. It parses, it type checks, and
    // it is dead.
    check_framework(
        r#"
//- /core/module.api.php
<?php
namespace Core;
interface ModuleHandler {
    public function invokeAll(string $hook, array $args = []): array;
}

//- /modules/app_region/app_region.php
<?php

/**
 * Implements hook_field_widget_form_alter().
 */
function app_region_field_widget_form_alter(array &$element): void {
}
"#,
        conventions,
        expect![[r#"
            6:10-6:44 warning [undeclared-implementation] nothing declares or dispatches hook_field_widget_form_alter(), so app_region_field_widget_form_alter() implements nothing and never runs
        "#]],
    );
}

#[test]
fn a_dispatch_this_cannot_identify_the_receiver_of_reports_nothing() {
    // The blind spot, and the direction it falls in. A call on a bare local says nothing about what
    // it was made on, so nothing can be *shown* about where the point comes from -- and reporting
    // the implementation as dead on that basis would be reporting on absent evidence.
    check_framework(
        r#"
//- /core/module.api.php
<?php
namespace Core;
interface ModuleHandler {
    public function invokeAll(string $hook, array $args = []): array;
}

//- /modules/app_core/src/Notifications.php
<?php
namespace Core;
class Notifications {
    public function types(ModuleHandler $handler): array {
        return $handler->invokeAll('notifications_alter');
    }
}

//- /modules/app_region/app_region.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function app_region_notifications_alter(): array {
    return [];
}
"#,
        conventions,
        expect![[r#""#]],
    );
}

#[test]
fn a_method_of_that_name_on_something_else_dispatches_nothing() {
    // The trap the receiver gate exists for. `alter` is a method a project declares for things of
    // its own -- one real checkout calls it three times on a breadcrumb builder and a form-alter
    // service -- and a rule keyed on the method name alone would read whatever those were passed as
    // extension points, and then say a removed hook still exists.
    check_framework(
        r#"
//- /core/module.api.php
<?php
namespace Core;
interface ModuleHandler {
    public function invokeAll(string $hook, array $args = []): array;
}
interface Breadcrumbs {
    public function invokeAll(string $name, array $args = []): array;
}

//- /modules/app_core/src/Notifications.php
<?php
namespace Core;
class Notifications {
    public function __construct(private Breadcrumbs $moduleHandler) {}

    public function types(): array {
        return $this->moduleHandler->invokeAll('notifications_alter');
    }
}

//- /modules/app_region/app_region.php
<?php

/**
 * Implements hook_notifications_alter().
 */
function app_region_notifications_alter(): array {
    return [];
}
"#,
        conventions,
        expect![[r#"
            6:10-6:40 warning [undeclared-implementation] nothing declares or dispatches hook_notifications_alter(), so app_region_notifications_alter() implements nothing and never runs
        "#]],
    );
}
