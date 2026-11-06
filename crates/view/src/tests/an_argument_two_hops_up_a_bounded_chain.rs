//! A value's template argument reaches a member declared two `@extends` up, whatever the bound.
//!
//! The chase up the hierarchy translates a parent's template into the class's own, and it went
//! through the reader that answers a value with *no* arguments -- which, landing on a template,
//! hands back its bound. So the receiver's argument was thrown away exactly when the template had a
//! bound, and a template without one got through by accident. Symfony's config builders are the
//! shape: `ScalarNodeDefinition<NodeBuilder>` reaches `NodeDefinition::end(): TParent` through
//! `VariableNodeDefinition`, answered `NodeParentInterface|null`, and every chain after `->end()`
//! was a missing method.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_argument_two_hops_up_a_bounded_chain_is_the_receivers() {
    check_view(
        r#"
//- /src/Defs.php
<?php
namespace App;

interface Parentish {}

/** @template TParent of Parentish|null = null */
abstract class Def implements Parentish
{
    /** @return TParent */
    public function end(): ?Parentish {}
}

/**
 * @template TParent of Parentish|null = null
 * @extends Def<TParent>
 */
class MidDef extends Def {}

/**
 * @template TParent of Parentish|null = null
 * @extends MidDef<TParent>
 */
class ScalarDef extends MidDef {}

class Builder implements Parentish
{
    /** @return ScalarDef<$this> */
    public function scalar(): ScalarDef {}
}

class Runner
{
    public function go(Builder $b, ScalarDef $bare): void
    {
        $e@ended@nded = $b->scalar()->end();
        $n@none@one = $bare->end();
    }
}
"#,
        &[
            Query::variable_type("the receiver's argument, two hops up", "ended"),
            Query::variable_type("no argument: the bound", "none"),
        ],
        expect![[r#"
            the receiver's argument, two hops up
            - Builder
            no argument: the bound
            - ?Parentish
        "#]],
    );
}
