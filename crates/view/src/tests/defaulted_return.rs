//! A declared union one of whose branches the call selects by writing no argument.
//!
//! The shape a chain contract cannot express and an argument binding has nothing to read. One method
//! is two: `resolver()` hands back the resolver and `resolver(Widget::class)` hands back a widget, and
//! the union that has to cover both says nothing about either call.
//!
//! Owned at this layer because this is where a value's class is decided, and because the property
//! that makes the contract safe is enforced here rather than in the list that declares it: the branch
//! a framework names is checked against the operands of the method's *own* declared return type, so a
//! contract can select what the declaration wrote and can never invent something it did not. The
//! third test is the one that proves it.

use expect_test::expect;

use super::{Query, check_view_with};

/// A resolver, a widget, and a class nobody's declaration mentions -- so a wrong answer is a
/// *different* class rather than nothing.
const FIXTURE: &str = r#"
//- /src/Container.php
<?php
namespace Dep;

interface ResolverInterface
{
    public function build(string $definition): object;
}

class Widget {}

/// Declared nowhere in the union below, which is what the third test is about.
class Unlisted {}

class Container
{
    /**
     * @return \Dep\ResolverInterface|object
     *   The resolver, or if $class is provided, an instance of it.
     */
    public static function resolver($class = NULL) {}
}
"#;

fn resolver_defaults_to_the_interface(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .add_defaulted("Dep\\Container", "resolver", 0, "Dep\\ResolverInterface");
}

/// The whole point: a bare call gets the branch, and a call that writes the argument does not.
///
/// Both halves in one fixture, because a contract that fired for every call would pass the first on
/// its own -- and firing for every call is exactly the wrong answer this shape can produce.
#[test]
fn a_call_that_writes_no_argument_gets_the_branch_its_default_selects() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Container;
use Dep\Widget;

class Caller
{{
    public function run(): void
    {{
        $resolver = Container::resolver();
        $reso@bare@lver;
        $widget = Container::resolver(Widget::class);
        $wid@given@get;
    }}
}}
"#
        ),
        &[
            Query::variable_class("bare", "bare", "resolver"),
            Query::variable_class("given", "given", "widget"),
        ],
        resolver_defaults_to_the_interface,
        expect![[r#"
            bare
            - Dep\ResolverInterface
            given
            - (type not known here)
        "#]],
    );
}

/// And the branch is reachable as a receiver, which is the answer anybody actually asked for.
#[test]
fn the_selected_branch_answers_a_member_reached_through_it() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Reader.php
<?php
namespace App;

use Dep\Container;

class Reader
{{
    public function run(): void
    {{
        $made = Container::resolver()->bu@call@ild('app.thing');
    }}
}}
"#
        ),
        &[Query::goto("build", "call")],
        resolver_defaults_to_the_interface,
        expect![[r#"
            build
            - method Dep\ResolverInterface::build @ src/Container.php 6:21-6:26
        "#]],
    );
}

/// The safety property, stated as a test: a branch the declaration does not list answers nothing.
///
/// This is what keeps the contract a *selector* rather than a claim. Every other kind at the
/// framework seam can only narrow -- a non-null return removes a null the declaration allowed, a
/// chained return picks between branches it already wrote -- and this one is only safe because the
/// same restraint is enforced here instead of trusted. A provider naming a class that is not an
/// operand of the method's own declared return type contributes silence, not a wrong type.
#[test]
fn a_branch_the_declaration_does_not_list_is_not_an_answer() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Wrong.php
<?php
namespace App;

use Dep\Container;

class Wrong
{{
    public function run(): void
    {{
        $resolver = Container::resolver();
        $reso@wrong@lver;
    }}
}}
"#
        ),
        &[Query::variable_class("unlisted", "wrong", "resolver")],
        |world| {
            // `Dep\Unlisted` is a real class the index holds, and it is still not an answer: what
            // disqualifies it is that `resolver()` does not declare it, not that it is unknown.
            world
                .contracts_mut()
                .add_defaulted("Dep\\Container", "resolver", 0, "Dep\\Unlisted");
        },
        expect![[r#"
            unlisted
            - (type not known here)
        "#]],
    );
}
