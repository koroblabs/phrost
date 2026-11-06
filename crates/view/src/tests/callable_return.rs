//! `@param callable(Item): string $fn` followed by `$fn($item)`.
//!
//! The one shape of callable that answered nothing. A closure literal and a first-class callable
//! both arrive as a resolved callable carrying their return; a docblock's arrives as an ordinary type
//! node, and the whole callable family was folded to the bare `callable` builtin -- parameters and
//! return discarded together.
//!
//! A `\Closure` hint counts too, and that one took a second rule in a second place. `: callable`
//! beside `@param callable(Item): string` refines because the docblock's base *is* the hint; `Closure`
//! is PHP's class spelling of a callable and the base then differs, so the shape people actually
//! write in modern code was the one that lost. What is given up is `Closure`'s own three methods --
//! `$fn->bindTo()` stops resolving, and names no class, so nothing is reported about it.
//!
//! Kept as a generic over the `callable` builtin, exactly as `class-string<T>` is kept over `string`:
//! every reader that peels a generic to its base still sees a plain `callable`, so a `callable`
//! parameter is checked as it was. The **return only** -- the parameters are what a call's own
//! arguments would be checked against, which nothing asks for, and a one-argument generic says
//! nothing ambiguous.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/classes.php
<?php
namespace App;

class Item
{
    public function label(): string
    {
        return '';
    }
}

class Uses
{
    /** @param callable(Item): string $fn */
    public function apply(callable $fn, Item $i): void {}

    /** @param callable(Item): Item $make */
    public function build(callable $make, Item $i): void {}

    /** @param callable(Item) $bare */
    public function silent(callable $bare, Item $i): void {}
}
"#;

/// A `Closure` hint keeps the docblock's return, in all three spellings of the tag.
///
/// The rule lives in two places, because a type beside a hint is decided by the *written* refiner
/// when the declaration is in the file being read and by the interned one when it is not. Only the
/// interned one had it, so a probe written across files passed and the same code in one file did not.
#[test]
fn a_closure_hint_keeps_what_the_docblock_says_calling_it_gives() {
    check_view(
        r#"
//- /src/Local.php
<?php
namespace App;

class Row
{
    public function label(): string
    {
        return '';
    }
}

class Local
{
    /**
     * @param \Closure(Row): Row $qualified
     * @param Closure(Row): Row $bare
     * @param callable(Row): Row $plain
     */
    public function run(
        \Closure $qualified,
        \Closure $bare,
        callable $plain,
        Row $row
    ): void {
        $one = $qualified($row);
        $o@one@ne;
        $two = $bare($row);
        $t@two@wo;
        $three = $plain($row);
        $th@three@ree;
    }
}
"#,
        &[
            Query::variable_type("a qualified Closure", "one"),
            Query::variable_type("a bare Closure", "two"),
            Query::variable_type("and the callable beside them", "three"),
        ],
        expect![[r#"
            a qualified Closure
            - Row
            a bare Closure
            - Row
            and the callable beside them
            - Row
        "#]],
    );
}

#[test]
fn a_docblock_callable_says_what_invoking_it_gives_back() {
    check_view(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    /**
     * @param callable(Item): string $fn
     * @param callable(Item): Item $make
     * @param callable(Item) $bare
     * @param callable $plain
     */
    public function run(
        callable $fn,
        callable $make,
        callable $bare,
        callable $plain,
        Item $i
    ): void {{
        $said = $fn($i);
        $sa@said@id;
        $built = $make($i);
        $bu@built@ilt;
        $chained = $make($i)->label();
        $ch@chained@ained;
        $nothing = $bare($i);
        $no@nothing@thing;
        $unsaid = $plain();
        $un@unsaid@said;
    }}
}}
"#
        ),
        &[
            Query::variable_type("a declared return", "said"),
            Query::variable_type("a class return", "built"),
            Query::variable_type("and the chain past it", "chained"),
            Query::variable_type("no return written", "nothing"),
            Query::variable_type("a bare callable", "unsaid"),
        ],
        expect![[r#"
            a declared return
            - string
            a class return
            - Item
            and the chain past it
            - string
            no return written
            - (no type)
            a bare callable
            - (no type)
        "#]],
    );
}
