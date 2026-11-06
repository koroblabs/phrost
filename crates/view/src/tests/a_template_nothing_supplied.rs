//! A generic class reached with nothing supplied for its template, and one given it by a literal.
//!
//! `@return T` beside `: mixed` rendered as a variable called `T`: the fallback for "nothing
//! supplied it" was meant to be the hint written beside the tag, and was handed the *effective*
//! type, which had already preferred the docblock -- so it was the template again. `@return
//! list<T>` did the same one level in, and iterating it bound a local of type `T`. Symfony's
//! `JsonStreamer` and `Process` reported `?V` and `V|string|resource` because of it.
//!
//! And `new Coll([new Item()])` bound nothing where `new Coll($items)` did: a literal is a shape,
//! and a shape has no element -- its values are what `@param array<int, T> $items` holds.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_template_nothing_supplied_is_not_a_class_called_t() {
    check_view(
        r#"
//- /src/Coll.php
<?php
namespace App;

class Item {}

/** @template T */
class Coll
{
    /** @param array<int, T> $items */
    public function __construct(private array $items) {}

    /** @return T */
    public function first(): mixed {}

    /** @return ?T */
    public function maybe(): mixed {}

    /** @return list<T> */
    public function all(): array {}
}

class Runner
{
    public function go(Coll $bare): void
    {
        $f@first@irst = $bare->first();
        $m@maybe@aybe = $bare->maybe();
        foreach ($bare->all() as $e@each@ach) {
        }
        $literal = new Coll([new Item()]);
        $b@bound@ound = $literal->first();
    }
}
"#,
        &[
            Query::variable_type("`@return T` beside `: mixed`", "first"),
            Query::variable_type("`@return ?T`", "maybe"),
            Query::variable_type("an element of `list<T>`", "each"),
            Query::variable_type("bound by a literal", "bound"),
        ],
        expect![[r#"
            `@return T` beside `: mixed`
            - mixed
            `@return ?T`
            - mixed
            an element of `list<T>`
            - mixed
            bound by a literal
            - Item
        "#]],
    );
}
