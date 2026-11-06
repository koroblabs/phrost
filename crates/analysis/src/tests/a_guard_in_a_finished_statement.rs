//! What a guard proves once the statement holding it has finished.
//!
//! The rule the whole file turns on: a finished statement's guards narrow the code after it exactly
//! when *arriving there proves those guards ran*. For an `if` they do not -- the branch may not have
//! been taken -- and that is why the walk stopped at a finished statement in the first place. For a
//! `try` and a bare block they do, and the omission read as a `possibly-null-receiver` on code that
//! plainly checks for null.
//!
//! Two findings on the reference project were this, on consecutive lines, and a third was the same
//! rule with `string|false` instead of null. Nothing was added.

use expect_test::expect;

use super::diagnostics::check;

/// The reported shape, and the two that draw the boundary around it.
///
/// `catchReturns` is the code that was reported: the guard is inside the `try`, the use is after the
/// `catch`, and the `catch` returns -- so the only way to reach the use is for the `try` body to have
/// completed, which means the guard ran. What made the gap read as a resolution bug rather than a
/// flow one is that the **assignment** in the same block was visible all along; only the guard was
/// lost, because the walk that finds writes descends into blocks and the walk that finds guards did
/// not.
///
/// `catchSwallows` is why the fix cannot be unconditional. Its `catch` falls through, so arriving at
/// the use proves nothing about the guard, and reporting there is correct. `oneCatchSwallows` is the
/// same point with two clauses: one leaving is not enough, every one has to.
#[test]
fn a_guard_in_a_try_survives_the_catch_only_when_no_catch_can_fall_through() {
    check(
        r#"
//- /src/app.php
<?php

class Product
{
    public function amount(): int
    {
        return 1;
    }
}

class Supplier
{
    public function product(): ?Product
    {
        return NULL;
    }
}

class NoProduct extends Exception {}

class Cases
{
    public function catchReturns(Supplier $i): int
    {
        try {
            $product = $i->product();
            if ($product === NULL) {
                throw new NoProduct('none');
            }
        }
        catch (NoProduct) {
            return 0;
        }
        return $product->amount();
    }

    public function catchSwallows(Supplier $i): int
    {
        try {
            $product = $i->product();
            if ($product === NULL) {
                throw new NoProduct('none');
            }
        }
        catch (NoProduct) {
        }
        return $product->amount();
    }

    public function oneCatchSwallows(Supplier $i): int
    {
        try {
            $product = $i->product();
            if ($product === NULL) {
                throw new NoProduct('none');
            }
        }
        catch (NoProduct) {
            return 0;
        }
        catch (RuntimeException) {
        }
        return $product->amount();
    }
}
"#,
        expect![[r#"
            19:25-19:34 error [undefined-type] Undefined type Exception
            47:16-47:24 warning [possibly-null-receiver] Cannot call method amount() on ?Product
            61:16-61:32 error [undefined-type] Undefined type RuntimeException
            63:16-63:24 warning [possibly-null-receiver] Cannot call method amount() on ?Product
        "#]],
    );
}

/// The three other kinds of finished statement, and why they answer differently.
///
/// A `finally` runs on every path that reaches the `try` at all, so if it completed, it completed --
/// no condition needed, and the third finding this fixed on the reference project was exactly this
/// shape, a `try`/`finally` with no `catch` clause at all. A bare block is the same argument in its
/// simplest form: nothing can skip it. A loop is the one that looks similar and is not, because it
/// may run zero times, and a guard that may not have run must not narrow anything.
#[test]
fn a_finally_and_a_bare_block_prove_their_guards_ran_and_a_loop_body_does_not() {
    check(
        r#"
//- /src/app.php
<?php

class Product
{
    public function amount(): int
    {
        return 1;
    }
}

class Supplier
{
    public function product(): ?Product
    {
        return NULL;
    }
}

class NoProduct extends Exception {}

class Kinds
{
    public function inFinally(Supplier $i): int
    {
        $product = $i->product();
        try {
        }
        finally {
            if ($product === NULL) {
                throw new NoProduct('none');
            }
        }
        return $product->amount();
    }

    public function bareBlock(Supplier $i): int
    {
        $product = $i->product();
        {
            if ($product === NULL) {
                throw new NoProduct('none');
            }
        }
        return $product->amount();
    }

    public function loopBody(Supplier $i, array $rows): int
    {
        $product = $i->product();
        foreach ($rows as $row) {
            if ($product === NULL) {
                throw new NoProduct('none');
            }
        }
        return $product->amount();
    }
}
"#,
        expect![[r#"
            19:25-19:34 error [undefined-type] Undefined type Exception
            55:16-55:24 warning [possibly-null-receiver] Cannot call method amount() on ?Product
        "#]],
    );
}
