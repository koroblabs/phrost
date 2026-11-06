//! `FieldItemListInterface<T of FieldItemInterface> extends ListInterface<T>`, and `first(): ?T`.
//!
//! The substitution chases `T` up the hierarchy and lands on *another template* -- the composing
//! interface passes its own along -- so the answer was a name nothing declares, the filter that
//! catches such a name turned it into nothing, and the bound was never asked for. Every chain after
//! `->first()` was a dead end: `$entity->get('field_x')->first()?->getValue()` answered no goto, no
//! hover and no completion, which is one of the commonest single-value-field idioms there is.
//!
//! The bound to use is the *landing* template's own, which is the narrower of the two -- an
//! interface that re-declares `@template T of FieldItemInterface` over a parent's
//! `@template T of TypedDataInterface` is saying something more precise, and saying it about this
//! very slot.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_bound_of_the_template_it_lands_on_answers() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

interface Data {}

interface Item extends Data
{
    public function value(): string;
}

/**
 * @template T of Data
 */
interface Listed
{
    /** @phpstan-return ?T */
    public function first();
}

/**
 * @template T of Item
 * @extends Listed<T>
 */
interface Items extends Listed {}

class Host
{
    public function read(Items $items): void
    {
        $first = $items->first();
        $f@bounded@irst;
    }
}
"#,
        &[Query::variable_type("the narrower bound", "bounded")],
        expect![[r#"
            the narrower bound
            - ?Item
        "#]],
    );
}
