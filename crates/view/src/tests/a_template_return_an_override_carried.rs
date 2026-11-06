//! An override that writes a native `: mixed` and no docblock, under an interface's `@return T`.
//!
//! The override carries the docblock from above, and it carried the type without saying where it
//! was written -- so `T` was read as a template of the implementing class, which declares none. It
//! rendered as a class called `T`, and an `@implements Getter<Item>` between the two never reached
//! it: the same shape with *no* native hint answered `Item`, because that path climbs to the
//! declaration and keeps its owner. Laravel's container is `ArrayAccess` with nothing supplied, and
//! `$this[Kernel::class]` read as a class called `V` 110 times.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_carried_template_return_is_the_interfaces_template() {
    check_view(
        r#"
//- /src/Getter.php
<?php
namespace App;

class Item {}

/** @template T */
interface Getter
{
    /** @return T */
    public function get(): mixed;
}

class Bare implements Getter
{
    public function get(): mixed {}
}

/** @implements Getter<Item> */
class Bound implements Getter
{
    public function get(): mixed {}
}

class Deeper extends Bound
{
    public function get(): mixed {}
}

class Runner
{
    public function go(Bare $bare, Bound $bound, Deeper $deeper): void
    {
        $n@nothing@othing = $bare->get();
        $b@bound@ound = $bound->get();
        $d@deeper@eeper = $deeper->get();
    }
}
"#,
        &[
            Query::variable_type("nothing supplied it", "nothing"),
            Query::variable_type("`@implements Getter<Item>`", "bound"),
            Query::variable_type("carried twice", "deeper"),
        ],
        expect![[r#"
            nothing supplied it
            - mixed
            `@implements Getter<Item>`
            - Item
            carried twice
            - Item
        "#]],
    );
}
