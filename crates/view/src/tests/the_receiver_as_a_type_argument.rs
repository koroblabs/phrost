//! `static` or `$this` as a type argument is the receiver itself, with what it was given.
//!
//! `@return NodeBuilder<static>` on a method of a generic class: the builder's `TParent` is the very
//! object the call was made on, and when that object carries arguments of its own they come along.
//! The whole-return `@return $this` already handed the receiver back whole; one level in, the word
//! was resolved to the bare class and the arguments were dropped -- so the `end()` that climbed back
//! out fell to a bound, and Symfony's configuration trees were a missing method past their first
//! level.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_receiver_as_a_type_argument_keeps_its_arguments() {
    check_view(
        r#"
//- /src/Tree.php
<?php
namespace App;

/** @template TParent */
class Arr
{
    /** @return Nb<static> */
    public function children(): Nb {}

    /** @return TParent */
    public function end(): mixed {}
}

/** @template TParent */
class Nb
{
    /** @return TParent */
    public function end(): mixed {}

    /** @return Arr<$this> */
    public function arr(): Arr {}
}

class Runner
{
    public function go(Nb $nb, Arr $bare): void
    {
        $c@children@hildren = $nb->arr()->children();
        $b@back@ack = $nb->arr()->children()->end()->end();
        $p@plain@lain = $bare->children()->end();
    }
}
"#,
        &[
            Query::variable_type("a builder of the receiver", "children"),
            Query::variable_type("climbed back out twice", "back"),
            Query::variable_type("a receiver with no arguments", "plain"),
        ],
        expect![[r#"
            a builder of the receiver
            - Nb<Arr<Nb>>
            climbed back out twice
            - Nb
            a receiver with no arguments
            - Arr
        "#]],
    );
}
