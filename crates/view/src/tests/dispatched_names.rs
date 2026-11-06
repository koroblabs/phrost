//! Which calls the view will say dispatch an extension point, and which it refuses.
//!
//! The index records a candidate per call and leaves the receiver unresolved; this is where the
//! receiver is checked, so this is where the rule that keeps `alter` on an unrelated service from
//! declaring an extension point actually lives. Every candidate in the fixture is rendered with its
//! verdict rather than only the ones that pass -- an answer listing nothing would otherwise be
//! indistinguishable from a projection that read nothing at all.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::IndexView;

/// What the view makes of every name the fixture's calls could be dispatching.
fn check(fixture: &str, names: &[&str], expect: Expect) {
    let world = World::build_with(&phrost_testkit::FixtureSpec::parse(fixture), |world| {
        world
            .contracts_mut()
            .add_dispatcher("invokeAll", dispatches(0, false, ""));
        world
            .contracts_mut()
            .add_dispatcher("alter", dispatches(0, true, "_alter"));
    });

    let file = world.file_id("app.php");
    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        names
            .iter()
            .map(|name| {
                let sites = view.dispatch_sites("hook", name);
                if sites.is_empty() {
                    return format!(
                        "{name} -> {}\n",
                        if view.maybe_dispatched("hook", name) {
                            "no call this can identify, so nothing is claimed either way"
                        } else {
                            "nothing"
                        }
                    );
                }
                sites
                    .iter()
                    .map(|site| {
                        format!(
                            "{name} -> {} {} by {}\n",
                            world.path_of(site.file),
                            world.render_range(site.file, site.name_range),
                            site.method
                        )
                    })
                    .collect()
            })
            .collect::<String>()
    });

    expect.assert_eq(&rendered);
}

/// One rule, spelled once: six fields of which four are strings is not something to repeat.
fn dispatches(argument: u16, several: bool, suffix: &str) -> phrost_index::DispatchRule {
    phrost_index::DispatchRule {
        class: "Core\\ModuleHandler".into(),
        argument,
        several,
        names: "hook_".into(),
        suffix: suffix.into(),
        catalog: "hook".into(),
    }
}

/// Two services with the same two method names, and one file dispatching through each.
const PROJECT: &str = r#"
//- /app.php
<?php
namespace Core;

interface ModuleHandler {
    public function invokeAll(string $hook, array $args = []): array;
    public function alter($type, &$data): void;
}

interface Breadcrumbs {
    public function invokeAll(string $name, array $args = []): array;
    public function alter($type, &$data): void;
}

class Registry {
    public static function handler(): ModuleHandler {}
}

class Reports {
    public function __construct(
        private ModuleHandler $moduleHandler,
        private Breadcrumbs $breadcrumbs,
    ) {}

    public function run(ModuleHandler $passed): void {
        $this->moduleHandler->invokeAll('through_a_property');
        Registry::handler()->invokeAll('through_an_accessor');
        $this->moduleHandler->alter(['first', 'second']);
        $this->breadcrumbs->invokeAll('through_something_else');
        $passed->invokeAll('through_a_local');
    }
}
"#;

#[test]
fn a_call_on_the_class_the_rule_names_is_where_the_point_comes_from() {
    // The two receiver shapes a real project writes -- a service held in a property, and one handed
    // back by a static accessor -- and the list form, whose elements are separate names.
    check(
        PROJECT,
        &[
            "hook_through_a_property",
            "hook_through_an_accessor",
            "hook_first_alter",
            "hook_second_alter",
        ],
        expect![[r#"
            hook_through_a_property -> app.php 25:41-25:61 by invokeAll
            hook_through_an_accessor -> app.php 26:40-26:61 by invokeAll
            hook_first_alter -> app.php 27:38-27:45 by alter
            hook_second_alter -> app.php 27:47-27:55 by alter
        "#]],
    );
}

#[test]
fn a_method_of_the_same_name_on_another_class_dispatches_nothing() {
    // The trap the gate exists for, and it has to be silence in *both* directions: the call names
    // no place a reader could go, and it is not evidence that the name exists either -- the
    // receiver is typed and it is provably the wrong type.
    check(
        PROJECT,
        &["hook_through_something_else"],
        expect![[r#"
            hook_through_something_else -> nothing
        "#]],
    );
}

#[test]
fn a_receiver_this_cannot_identify_answers_neither_way() {
    // A bare local. Nothing is claimed about where the point comes from, because the index recorded
    // no receiver to check -- and nothing is claimed about the point *not* existing either, because
    // absent evidence is not evidence. The two readers get opposite answers on purpose.
    check(
        PROJECT,
        &["hook_through_a_local"],
        expect![[r#"
            hook_through_a_local -> no call this can identify, so nothing is claimed either way
        "#]],
    );
}

#[test]
fn a_name_nothing_dispatches_is_a_deterministic_miss() {
    check(
        PROJECT,
        &["hook_nobody_writes_this"],
        expect![[r#"
            hook_nobody_writes_this -> nothing
        "#]],
    );
}
