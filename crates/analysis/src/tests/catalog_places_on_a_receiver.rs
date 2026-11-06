//! A catalog key written to an instance method, on a receiver the caller was handed.
//!
//! The shape `catalog_places` does not cover: there the binding is on a static call, where the class
//! is written at the call site and there is nothing to resolve. The commonest spelling in a real
//! project is the other one -- `$form_state->setRedirect('route.name')` in every submit handler,
//! 63 of them on the measured project -- and it only answers if the receiver's class is worked out
//! first and then matched against the binding, including down a hierarchy the binding does not name.
//!
//! Both halves have failed independently before: a binding declared on a concrete class misses every
//! caller that types its parameter as the interface, and one declared on an interface answers nothing
//! if the receiver is not resolved at all.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::{IndexView, Place};
use std::path::Path;
use std::sync::Arc;

use crate::{completions, goto_external};

/// One binding, on the interface, and two routes to find with it.
fn with_places(world: &mut World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\FormStateInterface", "setRedirect", 0, "", "route");
    for (key, line, detail) in [
        ("app.edit", 12_u32, "/app/{id}/edit"),
        ("app.delete", 20, "/app/{id}/delete"),
    ] {
        world.contracts_mut().catalog_place(
            "route",
            key,
            Place {
                path: Arc::from(Path::new("/project/app.routing.yml")),
                line,
                character: 0,
                detail: detail.into(),
            },
        );
    }
}

/// The interface the binding names, and a class under it that the binding does not.
const FIXTURE: &str = r#"
//- /src/Form.php
<?php
namespace Dep;

interface FormStateInterface
{
    public function setRedirect(string $route): static;
}

class FormState implements FormStateInterface
{
    public function setRedirect(string $route): static
    {
        return $this;
    }
}
"#;

fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    with_places(&mut world);
    let world = world;
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        let list = completions(&view, position.offset);
        if list.items.is_empty() {
            out.push_str("offered: (nothing)\n");
        }
        for item in &list.items {
            out.push_str(&format!(
                "offered: {}  {}\n",
                item.label,
                item.detail.as_deref().unwrap_or("")
            ));
        }
        match goto_external(&view, position.offset) {
            Some(target) => out.push_str(&format!(
                "goes to: {}:{}:{} as `{}`\n",
                target.path.display(),
                target.line + 1,
                target.character + 1,
                target.key
            )),
            None => out.push_str("goes to: (nowhere outside the project)\n"),
        }
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn a_parameter_typed_as_the_bound_interface_reads_the_key() {
    // What a submit handler is: the receiver's class is nowhere at the call, only on the parameter.
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\FormStateInterface;

class Caller
{{
    public function submit(FormStateInterface $form_state): void
    {{
        $form_state->setRedirect('app.e@here@dit');
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            offered: app.edit  /app/{id}/edit
            offered: app.delete  /app/{id}/delete
            goes to: /project/app.routing.yml:13:1 as `app.edit`
        "#]],
    );
}

#[test]
fn a_class_below_the_bound_interface_reads_it_too() {
    // The reason a binding is declared on the interface and not on the class that implements it:
    // Drupal has three implementations and a project's forms are handed whichever one the caller
    // built. A binding that named only one of them would answer on a third of the call sites.
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\FormState;

class Caller
{{
    public function run(): void
    {{
        $state = new FormState();
        $state->setRedirect('app.d@here@elete');
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            offered: app.delete  /app/{id}/delete
            offered: app.edit  /app/{id}/edit
            goes to: /project/app.routing.yml:21:1 as `app.delete`
        "#]],
    );
}

#[test]
fn a_receiver_with_no_type_reads_nothing() {
    // The honest answer, and the one that keeps the binding from claiming every `setRedirect` in a
    // project: a method name alone is not evidence that the string beside it is a route.
    check(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    public function run($whatever): void
    {{
        $whatever->setRedirect('app.e@here@dit');
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            offered: (nothing)
            goes to: (nowhere outside the project)
        "#]],
    );
}
