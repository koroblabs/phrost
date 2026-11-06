//! A `@return A|B` whose operands are bound by *different* things.
//!
//! Symfony's `InputBag::get` is the shape and the only one in a 300 MB vendor tree that writes it:
//!
//! ```text
//! @template TDefault of string|int|float|bool|null
//! @param TDefault $default
//! @return TDefault|TInput
//! ```
//!
//! `TDefault` is the type of an argument at the call; `TInput` is what the receiver was written with,
//! and Symfony writes `@var InputBag<string> $query` on `Request`. Two resolvers answer "what does
//! this template mean" -- one is handed the call and the other the receiver -- and each was complete
//! for its own kind and blind to the other's, so a union spanning both resolved in neither and fell
//! back to the declared `string|int|float|bool|null`.
//!
//! # Built for the form, not the catch
//!
//! One method in that vendor tree, and three false positives on the reference project. What earns it
//! is that "two resolvers, each complete for its own kind and blind to the other's" is a shape this
//! codebase met four times before this one -- the element half against the key half, the written
//! refiner against the interned one, two union constructors, two readers of a return tag -- and every
//! previous fix treated the instance. `CallFacts::receiver` is the seam that removes the form.
//!
//! # Every operand has to answer
//!
//! A union with an unresolved operand dropped is *narrower than the truth*, and a type narrower than
//! the truth reports correct code. So a receiver written without arguments -- `Request::$request` is a
//! plain `InputBag` where `$query` is an `InputBag<string>` -- declines the whole union and the
//! declared return stands. Answering it as written would put a variable called `TInput` in a tooltip.
//!
//! # And an argument the call left out
//!
//! What the *declaration* puts there is what arrives: `get('token')` on
//! `get($key, mixed $default = null)` is a `?string` where `get('token', '')` is a `string`. Reading
//! the written argument alone answered the smaller half -- measured on one real project, **5 calls
//! write the default and 62 leave it out**.

use expect_test::expect;

use super::{Query, check_view};

const PROJECT: &str = r#"
//- /src/Bag.php
<?php
namespace App;

/** @template TInput of string|int|float|bool|null */
class Bag
{
    /**
     * @template TDefault of string|int|float|bool|null
     *
     * @param TDefault $default
     *
     * @return TDefault|TInput
     */
    public function get(string $key, mixed $default = NULL): string|int|float|bool|null {}
}

class Holder
{
    /** @var Bag<string> */
    public Bag $query;

    public Bag $any;
}

class Reader
{
    public function run(Holder $holder): void
    {
        $agreeing = $holder->query->get('n', '');
        $differing = $holder->query->get('page', 1);
        $noDefault = $holder->query->get('n');
        $unbound = $holder->any->get('n', '');

        $agre@agreeing@eing;
        $diff@differing@ering;
        $noDef@noDefault@ault;
        $unbo@unbound@und;
    }
}
"#;

#[test]
fn each_operand_is_settled_by_whatever_binds_it_and_all_of_them_must_be() {
    check_view(
        PROJECT,
        &[
            Query::variable_type("both halves agree, so the union folds to one", "agreeing"),
            Query::variable_type("the argument decides one half", "differing"),
            Query::variable_type(
                "no argument, so the parameter's own default answers",
                "noDefault",
            ),
            Query::variable_type("a receiver written without arguments, likewise", "unbound"),
        ],
        expect![[r#"
            both halves agree, so the union folds to one
            - string
            the argument decides one half
            - int|string
            no argument, so the parameter's own default answers
            - ?string
            a receiver written without arguments, likewise
            - string|int|float|bool|null
        "#]],
    );
}
