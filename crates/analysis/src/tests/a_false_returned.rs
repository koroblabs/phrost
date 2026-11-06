//! A `false` handed back where the declaration admits none.
//!
//! The return's entry in the sentinel vocabulary, beside the receiver's and the argument's. What
//! puts it there is that the standard library reports failure this way and a great deal of real code
//! returns the result straight: `strtotime()`, `NumberFormatter::format()`,
//! `DateTime::createFromFormat()` and `reset()` all hand back `false`, and a `: int` or a `: DateTime`
//! above them is a `TypeError` on the run that fails.
//!
//! A *certain* `false` is `incompatible-return`'s, which says `Cannot return false where T is
//! declared` and says it as an error. This is the branch nobody tested.
//!
//! As much about the silence as the receiver's is, and with one silence of its own: `bool` names the
//! `false` without spelling it, so `object|bool` admits one.

use expect_test::expect;

use super::diagnostics::check;

const STUBS: &str = r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return ($array is non-empty-array ? V : V|false)
 */
function reset(object|array &$array): mixed {}

/** @return int|false */
function strtotime(string $datetime): int|false {}
"#;

#[test]
fn a_false_returned_where_a_class_is_declared_is_reported() {
    check(
        &[
            STUBS,
            r#"
//- /app.php
<?php
class Row {}

class Repo
{
    /** @param Row[] $rows */
    public function first(array $rows): Row
    {
        return reset($rows);
    }

    public function when(string $written): int
    {
        return strtotime($written);
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            9:16-9:28 warning [possibly-false-return] Row|false returned where \Row is declared
            14:16-14:35 warning [possibly-false-return] int|false returned where int is declared
        "#]],
    );
}

/// Every declaration that already admits the `false`, so the check says nothing.
///
/// `bool` is the one worth pinning: it names the `false` without spelling it, and reading only the
/// literal `false` operand reported `object|bool` -- a declaration that plainly accepts one.
#[test]
fn a_declaration_that_admits_a_false_is_silent() {
    check(
        &[
            STUBS,
            r#"
//- /app.php
<?php
class Row {}

class Repo
{
    /** @param Row[] $rows */
    public function orFalse(array $rows): Row|false
    {
        return reset($rows);
    }

    /** @param Row[] $rows */
    public function orBool(array $rows): Row|bool
    {
        return reset($rows);
    }

    /** @param Row[] $rows */
    public function asMixed(array $rows): mixed
    {
        return reset($rows);
    }

    public function asBool(string $written): bool
    {
        return strtotime($written);
    }
}
"#,
        ]
        .concat(),
        expect![""],
    );
}

/// Every way of proving the array is not empty, including the one that was missing.
///
/// `$rows !== []` is the fourth spelling of "empty" -- `empty()`, `count(...) === 0` and plain
/// truthiness all settled it already -- and it is the one a person reaches for when the value is a
/// typed array property, because there is nothing to be nullable about.
#[test]
fn proving_the_array_is_not_empty_is_silent() {
    check(
        &[
            STUBS,
            r#"
//- /app.php
<?php
class Row {}

class Repo
{
    /** @var Row[] */
    private array $rows = [];

    public function viaNotEmptyLiteral(): Row
    {
        if ($this->rows !== []) {
            return reset($this->rows);
        }
        return new Row();
    }

    public function viaEmptyLiteral(): Row
    {
        if ($this->rows === []) {
            return new Row();
        }
        return reset($this->rows);
    }

    public function viaEmptyCall(): Row
    {
        if (empty($this->rows)) {
            return new Row();
        }
        return reset($this->rows);
    }

    public function viaTruthiness(): Row
    {
        if ($this->rows) {
            return reset($this->rows);
        }
        return new Row();
    }

    /** @param Row[] $rows */
    public function viaGuardedResult(array $rows): Row
    {
        $first = reset($rows);
        if ($first === FALSE) {
            return new Row();
        }
        return $first;
    }
}
"#,
        ]
        .concat(),
        expect![""],
    );
}
