//! What a `@return T` answers when a native type is written beside it.
//!
//! The tag and the hint are not two claims about the type. A `@template T of X` is bounded by exactly
//! what the hint declares, so the tag says *which* of the declared possibilities this call gets and
//! can only ever be narrower. `TypeTable::refined` cannot see that -- a template name is a `Named`
//! and looks like a class to it -- so the choice is made where the owner's template list is in hand.
//!
//! It is not a corner. **124 of the 145 templated returns** in one real vendor tree write a native
//! type beside the tag, which is what modern PHP encourages: the hint is what the engine enforces and
//! the docblock adds the half PHP has no syntax for.
//!
//! # The hint is kept, not discarded
//!
//! Nothing may supply the template -- a value of a `@template` class written without arguments is
//! ordinary, and Symfony's own `Request` declares `@var InputBag<string> $query` beside a bare
//! `InputBag $request`. What the declaration wrote is then a real type and the better answer, so it
//! stands in. With no hint the template's own bound answers, which is the rule this module already
//! followed; only where there is neither is the honest answer nothing rather than a class called `T`.
//!
//! # A union
//!
//! `@return null|TNode` answers nothing at all without this: a union is not a template name, so every
//! substitution declines and what is left is a union of names resolving to no class. It is 38 of that
//! vendor tree's templated returns.
//!
//! **Every operand has to answer**, for the reason a list literal's every entry does: a union with an
//! unsubstituted operand dropped is *narrower than the truth*, and a type narrower than the truth
//! reports correct code. What decides is whether an operand names something real rather than whether
//! it is a class template -- a `@template` written on the *method* is bound by the contract reader
//! and not by this one, so testing for a class template would keep an operand nothing resolved.

use expect_test::expect;

use super::{Query, check_view};

const PROJECT: &str = r#"
//- /src/Nodes.php
<?php
namespace App;

class Node
{
    public function line(): int {}
}

class Call extends Node
{
    public function callee(): string {}
}

/** @template TNode of Node */
class Finder
{
    /** @return TNode */
    public function only(): Node {}

    /** @return null|TNode */
    public function firstOf(): ?Node {}

    /** @return TNode|Node */
    public function firstOrAny(): Node {}

    /** @return TNode */
    public function untyped() {}
}

/** @template T */
class Unbounded
{
    /** @return T */
    public function only() {}
}

class Holder
{
    /** @var Finder<Call> */
    public Finder $calls;

    public Finder $any;

    public Unbounded $loose;
}

class Reader
{
    public function run(Holder $holder): void
    {
        $bound = $holder->calls->only();
        $maybe = $holder->calls->firstOf();
        $either = $holder->calls->firstOrAny();
        $unbound = $holder->any->only();
        $absent = $holder->any->untyped();
        $nothing = $holder->loose->only();

        $bo@bound@und;
        $ma@maybe@ybe;
        $ei@either@ther;
        $unb@unbound@ound;
        $abs@absent@ent;
        $not@nothing@hing;
    }
}
"#;

#[test]
fn the_tag_wins_over_the_hint_and_the_hint_stands_in_when_nothing_supplies_it() {
    check_view(
        PROJECT,
        &[
            Query::variable_type("the argument the receiver carries", "bound"),
            Query::variable_type("a union, with the null kept", "maybe"),
            Query::variable_type("a union of a template and a class", "either"),
            Query::variable_type("nothing supplied it, so the hint stands", "unbound"),
            Query::variable_type("no hint, so the template's own bound stands", "absent"),
            Query::variable_type("no hint and no bound, which is the one nothing", "nothing"),
        ],
        expect![[r#"
            the argument the receiver carries
            - Call
            a union, with the null kept
            - ?Call
            a union of a template and a class
            - Call|Node
            nothing supplied it, so the hint stands
            - Node
            no hint, so the template's own bound stands
            - Node
            no hint and no bound, which is the one nothing
            - (no type)
        "#]],
    );
}
