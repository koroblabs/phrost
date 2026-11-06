//! An argument judged against a parameter typed with one of the class's templates.
//!
//! `@param T $item` is not a type. Judging an argument against the bare name is wrong in **both**
//! directions at once, which is what made this worth finding rather than merely improving: `T`
//! resolves to no class, so a scalar is rejected for not being one and *any* class is accepted
//! because nothing can be related to it.
//!
//! Measured on a probe before the fix: five findings, all false, and two genuinely wrong classes
//! silently accepted. The carrier was the standard library -- `Ds\Sequence::push(...$values)` is
//! `@param TValue`, so `$deque->push(1)` reported `$values takes \Ds\TValue, and this is int` on code
//! that runs.
//!
//! # What settles it
//!
//! What the receiver was written with. `Bag<Row>` says `T` is `Row` here, so `add(1)` and
//! `add(new Other())` are both real and `add($row)` is silent. A receiver written *without* arguments
//! settles nothing, and there the parameter is not judged at all -- the honest answer, and the one
//! that costs a report rather than inventing one.
//!
//! Substituted to an interned type, because that is what the judgement takes. So a container of a
//! template -- `@param list<T>` -- declines: rebuilding `array<int, Row>` needs the type table open
//! for writing, which a reading view does not have. A miss, and it replaces an answer that was
//! accidentally right for a scalar and accidentally wrong for a class.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_receiver_decides_what_the_template_takes_and_an_unbound_one_is_not_judged() {
    check(
        r#"
//- /src/Bag.php
<?php
namespace App;

class Row {}
class Other {}

/** @template T */
class Bag
{
    /** @param T $item */
    public function add($item): void {}

    /** @param list<T> $items */
    public function addAll(array $items): void {}
}

/** @template T of Row */
class Bounded
{
    /** @param T $item */
    public function add($item): void {}
}

class Holder
{
    /** @var Bag<Row> */
    public Bag $rows;

    public Bag $any;

    /** @var Bounded<Row> */
    public Bounded $bounded;
}

class Caller
{
    public function run(Holder $holder, Row $row, Other $other): void
    {
        $holder->rows->add($row);
        $holder->rows->add(1);
        $holder->rows->add($other);

        $holder->bounded->add($row);
        $holder->bounded->add($other);

        $holder->any->add(1);
        $holder->any->add($other);

        $holder->rows->addAll([1]);
    }
}
"#,
        expect![[r#"
            40:28-40:29 error [incompatible-argument] $item takes \App\Row, and this is int
            41:28-41:34 error [incompatible-argument] $item takes \App\Row, and this is Other
            44:31-44:37 error [incompatible-argument] $item takes \App\Row, and this is Other
        "#]],
    );
}

#[test]
fn the_sentinel_check_asks_the_same_question_the_judgement_does() {
    // The other half of the same rule, and it had never asked. `incompatible_arguments` has put a
    // receiver's template arguments in place since templates existed; `sentinel_arguments` read the
    // parameter as written, so `@template TValue` on an interface with `@param TValue|null $value`
    // on its method reads as "takes null and not false" -- and `transform(FALSE)` was reported as
    // passing a `false` where a class nothing declares was wanted.
    //
    // Two constructors of one rule, with `bound_expectation` as the seam that now serves both.
    // Measured: 37 findings gone across three corpora and none added, every one of them a message
    // naming `\T`, `\TValue` or `array<\K, \V>` -- a sentence no reader can act on.
    //
    // The second call is the control, and it is what keeps the silence from being the fixture's
    // rather than the rule's: an ordinary parameter beside the templated one still reports.
    check(
        r#"
//- /src/app.php
<?php

/**
 * @template TValue
 */
interface Transformer
{
    /**
     * @param TValue|null $value
     */
    public function transform($value): void;

    public function named(string $name): void;
}

class Uses
{
    public function run(Transformer $t): void
    {
        $t->transform(FALSE);
        $t->named(FALSE);
    }
}
"#,
        expect![[r#"
            21:19-21:24 warning [false-argument] Cannot pass false as $name, which takes string
        "#]],
    );
}
