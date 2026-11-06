//! A method declared `object` whose argument names the class it returns.
//!
//! The claim a `@return T` bound to a `class-string<T>` parameter makes, contributed for a
//! declaration that does not make it. The reader is the same one, so what is owned here is the half
//! that is different: a docblock is the method's author speaking about their own method, and a
//! contributed rule is a list elsewhere speaking about somebody else's.
//!
//! That difference is why this cannot be safe the way the other contributed contracts are. A
//! defaulted or chained return *selects* an operand the declaration already wrote, and the reader
//! checks that it did -- so a wrong entry answers nothing. `object` writes no operands, so there is
//! nothing to select from and nothing to check. What is checked instead, and what these tests are
//! for:
//!
//! - the answer is the class the **caller** wrote, never one the rule names -- so the rule can read a
//!   name somebody wrote and cannot produce one nobody did;
//! - the declaration has to say `object` and nothing narrower, so the narrowing happens strictly
//!   inside the declared type and can never contradict it;
//! - the written name has to be a class the index holds.
//!
//! The third and fourth tests are the ones that prove the last two.

use expect_test::expect;

use super::{Query, check_view_with};

/// A resolver whose one method says nothing about what it hands back, a sibling that does, and two
/// classes to hand back -- so a wrong answer is a *different* class rather than nothing.
const FIXTURE: &str = r#"
//- /src/Container.php
<?php
namespace Dep;

interface ResolverInterface
{
    /**
     * Takes a class name and hands back an instance of it.
     *
     * @return object
     */
    public function build($definition);

    /**
     * The sibling the fourth test is about: this one has an opinion.
     *
     * @return \Dep\Widget
     */
    public function typed($definition);

    /**
     * The container's spelling: the same statement with a null on it, which arrives only when a
     * caller opts into it with a second argument.
     */
    public function maybe(string $definition): ?object;
}

class Resolver implements ResolverInterface
{
    /**
     * {@inheritdoc}
     */
    public function build($definition) {}

    /**
     * {@inheritdoc}
     */
    public function typed($definition) {}

    /**
     * {@inheritdoc}
     */
    public function maybe(string $definition): ?object {}
}

class Widget
{
    public function spin(): void {}
}

class Gadget
{
    public function whirr(): void {}
}
"#;

fn the_first_argument_names_what_build_returns(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .add_argument_class("Dep\\ResolverInterface", "build", 0);
}

/// The whole point: the class comes out of the call, and it is reachable as a receiver.
#[test]
fn a_call_that_names_a_class_gets_an_instance_of_it() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ResolverInterface;
use Dep\Gadget;
use Dep\Widget;

class Caller
{{
    public function run(ResolverInterface $resolver): void
    {{
        $made = $resolver->build(Widget::class);
        $ma@widget@de;
        $other = $resolver->build(Gadget::class);
        $oth@gadget@er;
        $resolver->build(Widget::class)->sp@member@in();
    }}
}}
"#
        ),
        &[
            Query::variable_class("widget", "widget", "made"),
            // Two different classes through one call, which is the property that makes this a
            // reading of the call rather than a claim about the method.
            Query::variable_class("gadget", "gadget", "other"),
            Query::goto("member", "member"),
        ],
        the_first_argument_names_what_build_returns,
        expect![[r#"
            widget
            - Dep\Widget
            gadget
            - Dep\Gadget
            member
            - method Dep\Widget::spin @ src/Container.php 47:21-47:25
        "#]],
    );
}

/// Nothing a call can write except a class name is an answer.
///
/// Four shapes of the same silence, in one snapshot because they are one behaviour: a variable is
/// not a literal and is never resolved for this; a string naming nothing resolves to nothing; a
/// `::class` on a class the project does not declare is a name the index cannot place; and a call
/// that writes no argument at all leaves the declaration standing, which is `object`.
///
/// Each of them has to fail *quietly*. A guess here would be a class whose every member a report
/// then calls missing, which is worse than the untyped receiver it replaced.
#[test]
fn nothing_a_call_can_write_but_a_class_name_is_an_answer() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Unwritten.php
<?php
namespace App;

use Dep\ResolverInterface;

class Unwritten
{{
    public function run(ResolverInterface $resolver, string $name): void
    {{
        $chosen = $resolver->build($name);
        $cho@variable@sen;
        $service = $resolver->build('app.thing');
        $ser@string@vice;
        $missing = $resolver->build(Nowhere::class);
        $mis@unknown@sing;
        $bare = $resolver->build();
        $ba@absent@re;
    }}
}}
"#
        ),
        &[
            Query::variable_class("variable", "variable", "chosen"),
            Query::variable_class("string", "string", "service"),
            Query::variable_class("unknown", "unknown", "missing"),
            Query::variable_class("absent", "absent", "bare"),
        ],
        the_first_argument_names_what_build_returns,
        expect![[r#"
            variable
            - (type not known here)
            string
            - (type not known here)
            unknown
            - (type not known here)
            absent
            - (type not known here)
        "#]],
    );
}

/// A string that *does* name a declared class is the same answer, and that is deliberate.
///
/// `$definition::create()` and `new $definition()` cannot tell `Widget::class` from `'Dep\Widget'`;
/// both are the same string by the time the method has them. So the collapse is PHP's rather than a
/// compromise, and it is the same one the declared half of this reader already makes.
///
/// Written in the global namespace, because that is where a class-string resolves to the class it
/// names: a relative name inside a namespace is resolved against that namespace, which is what PHP
/// would do to it if it were written as a name rather than as text.
#[test]
fn a_string_that_names_a_declared_class_answers_like_the_constant_does() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/spelled.php
<?php

use Dep\ResolverInterface;

function spelled(ResolverInterface $resolver): void
{{
    $made = $resolver->build('Dep\Widget');
    $ma@text@de;
}}
"#
        ),
        &[Query::variable_class("text", "text", "made")],
        the_first_argument_names_what_build_returns,
        expect![[r#"
            text
            - Dep\Widget
        "#]],
    );
}

/// The safety property, stated as a test: a declaration with an opinion is not overruled.
///
/// This is what stands in for the operand check the selecting contracts have. There is no branch to
/// find `Gadget` among, so the question asked instead is whether the declaration says anything at
/// all -- and `typed()` says `\Dep\Widget`, so the contributed rule contributes silence and the
/// declaration answers. A rule that could speak here would be claiming a type against a signature,
/// which is the one thing nothing at the framework seam is allowed to do.
///
/// It is also what disarms the rule if the framework ever types the method it was written for: the
/// contract stops matching, silently, in the direction that says nothing rather than something false.
#[test]
fn a_declaration_that_says_more_than_object_is_not_overruled() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Opinionated.php
<?php
namespace App;

use Dep\ResolverInterface;
use Dep\Gadget;

class Opinionated
{{
    public function run(ResolverInterface $resolver): void
    {{
        $made = $resolver->typed(Gadget::class);
        $ma@declared@de;
    }}
}}
"#
        ),
        &[Query::variable_class("declared", "declared", "made")],
        |world| {
            // `Dep\Gadget` is a real class the index holds and the call really names it. What
            // disqualifies it is the declaration, which already answered.
            world
                .contracts_mut()
                .add_argument_class("Dep\\ResolverInterface", "typed", 0);
            the_first_argument_names_what_build_returns(world);
        },
        expect![[r#"
            declared
            - Dep\Widget
        "#]],
    );
}

/// An implementation inherits the contract with the signature, `{@inheritdoc}` and all.
///
/// The concrete class writes neither a return type nor an `@return`, so the declaration the gate has
/// to read is the interface's -- one level up, where the `object` is written. Reading only the row
/// that was found would make the rule fire for a receiver typed as the interface and go quiet for one
/// typed as the class, which is the same value reached two ways.
#[test]
fn an_implementation_that_declares_nothing_is_covered_by_the_interface_it_inherits_from() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Concrete.php
<?php
namespace App;

use Dep\Resolver;
use Dep\Widget;

class Concrete
{{
    public function run(Resolver $resolver): void
    {{
        $made = $resolver->build(Widget::class);
        $ma@inherited@de;
    }}
}}
"#
        ),
        &[Query::variable_class("inherited", "inherited", "made")],
        the_first_argument_names_what_build_returns,
        expect![[r#"
            inherited
            - Dep\Widget
        "#]],
    );
}

fn the_first_argument_names_what_maybe_returns(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .add_argument_class("Dep\\ResolverInterface", "maybe", 0);
}

/// A declaration that says `?object` is the same statement, and the null is not part of the answer.
///
/// Symfony's container is written
/// `get(string $id, int $invalidBehavior = self::EXCEPTION_ON_INVALID_REFERENCE): ?object`, so the
/// null is what the *second* argument opts into: 295 of the 296 `->get(X::class)` calls on one real
/// checkout write one argument and cannot be null. Requiring a bare `object` left every one of them
/// with no class at all; keeping the null instead would put a `possibly-null-receiver` on all 295 to
/// be right about the one, which is noise on calls that cannot be null rather than a null hidden.
#[test]
fn a_factory_declared_nullable_object_still_answers_with_the_class_the_call_named() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ResolverInterface;
use Dep\Widget;

class Caller
{{
    public function run(ResolverInterface $resolver): void
    {{
        $made = $resolver->maybe(Widget::class);
        $ma@widget@de;
        $resolver->maybe(Widget::class)->sp@member@in();
    }}
}}
"#
        ),
        &[
            Query::variable_class("widget", "widget", "made"),
            Query::goto("member", "member"),
        ],
        the_first_argument_names_what_maybe_returns,
        expect![[r#"
            widget
            - Dep\Widget
            member
            - method Dep\Widget::spin @ src/Container.php 47:21-47:25
        "#]],
    );
}
