//! A callback passed to one link of a fluent chain is checked against that link's declaration.
//!
//! Every link of `(new Coll)->map(...)->when(...)` starts at the same offset, and the declaration
//! each call reaches is remembered per call -- so a memo keyed by the offset alone handed `map`
//! the declaration of `when`, and a `map` callback's parameter read as the `$this` a `when`
//! callback receives. Only a check sees it, because only a check asks about both links through one
//! view; the control below is a real mistake in the same chain.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_callback_reads_the_declaration_of_the_link_it_is_passed_to() {
    check(
        r#"
//- /app.php
<?php
class Coll
{
    /**
     * @param callable(float): mixed $callback
     * @return static
     */
    public function map(callable $callback) { return $this; }

    /**
     * @param (\Closure($this): bool)|bool $value
     * @param (callable($this): mixed)|null $callback
     * @return $this
     */
    public function when($value, ?callable $callback = null) { return $this; }
}

function takes_float(float $number): void {}

function run(Coll $items): void
{
    $items
        ->map(fn ($value) => takes_float($value))
        ->when(true, fn ($same) => takes_float($same));
}
"#,
        expect![[r#"
            24:48-24:53 error [incompatible-argument] $number takes float, and this is Coll
        "#]],
    );
}
