//! An array argument whose *keys* the declaration disagrees with.
//!
//! The other half of [`super::element_arguments`], and the same rule read one argument to the left:
//! if a parameter says its keys are strings and the argument's are ints, the callee will index it
//! with something that is not there. Both halves of a container are now knowable, so both are asked.
//!
//! # Why it is not the assignability rule
//!
//! PHP does not convert an array's keys the way it converts an argument. Handing `array<int, Row>` to
//! a parameter declared `array<string, Row>` gives the callee int keys whatever `strict_types` says,
//! so the question is whether the two key sets can be the same keys -- disjointness -- and the
//! directive does not enter into it. That is the one place this parts company with every other
//! judgement in `phrost_ty::compat`.
//!
//! # What it does not touch
//!
//! The bracket spelling states a container and *not* how it is keyed, so `Row[]` and `array<Row>` say
//! nothing here on either side. PHPStan reads `Row[]` as `array<int|string, Row>` and reports it
//! against a string-keyed parameter; on framework code that is noise, and the brackets are the
//! spelling this project's own rules ask for.
//!
//! `list<Row>` is the one one-argument spelling that *does* name its key -- `int`, by definition --
//! and lowering now materialises it, which is what makes the two tell apart at all: before that they
//! interned to the same type.
//!
//! A key that is not an int or a string is unstated rather than a third kind: a template says
//! nothing (`array<K, V>` is 114 of the prelude's signatures), and neither does `array-key`, which
//! is `int|string` and so overlaps whichever the callee wanted.
//!
//! A literal states its keys, and a *string* one is read: `['a' => new Row()]` has it on every run.
//! An integer one is not, for the brackets' reason -- the literal cannot tell `[0 => $a]` from the
//! numbering PHP gives `[$a]`.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn keys_the_declaration_disagrees_with_are_reported_and_nothing_else_is() {
    check(
        r#"
//- /app.php
<?php

class Row {}

class Needs
{
    /** @param array<string, Row> $rows */
    public function stringKeys(array $rows): void {}

    /** @param array<int, Row> $rows */
    public function intKeys(array $rows): void {}

    /** @param array<array-key, Row> $rows */
    public function anyKeys(array $rows): void {}

    /** @param Row[] $rows */
    public function brackets(array $rows): void {}

    /** @param array<K, Row> $rows */
    public function templateKeys(array $rows): void {}

    /**
     * @param array<int, Row> $intKeyed
     * @param list<Row> $listed
     * @param array<string, Row> $stringKeyed
     * @param Row[] $bracketed
     * @param array<int|string, Row> $eitherKeyed
     */
    public function run(
        Needs $n,
        array $intKeyed,
        array $listed,
        array $stringKeyed,
        array $bracketed,
        array $eitherKeyed,
        array $bare
    ): void {
        $n->stringKeys($intKeyed);
        $n->stringKeys($listed);
        $n->intKeys($stringKeyed);

        $n->stringKeys($bracketed);
        $n->intKeys($bracketed);
        $n->anyKeys($stringKeyed);
        $n->brackets($stringKeyed);
        $n->stringKeys([]);
        $n->intKeys(['a' => new Row()]);
        $n->intKeys($eitherKeyed);
        $n->templateKeys($stringKeyed);
        $n->stringKeys($bare);
        $n->stringKeys($stringKeyed);
        $n->intKeys($intKeyed);
        $n->intKeys($listed);
    }
}
"#,
        expect![[r#"
            38:24-38:33 error [incompatible-argument] $rows takes array<string, \Row>, and this is array<int, Row>
            39:24-39:31 error [incompatible-argument] $rows takes array<string, \Row>, and this is array<int, Row>
            40:21-40:33 error [incompatible-argument] $rows takes array<int, \Row>, and this is array<string, Row>
            47:21-47:39 error [incompatible-argument] $rows takes array<int, \Row>, and this is array{a: Row}
        "#]],
    );
}

#[test]
fn the_key_rule_holds_whether_or_not_strict_types_is_declared() {
    // The pair of findings it must produce are `int` keys against `string` and the reverse, which
    // are the two the scalar table would *accept* in weak mode and reject only under the directive.
    // A key comparison that had reached for that table would therefore answer differently here, and
    // the whole point of its being a separate rule is that it does not.
    check(
        r#"
//- /app.php
<?php

declare(strict_types=1);

class Row {}

class Needs
{
    /** @param array<string, Row> $rows */
    public function stringKeys(array $rows): void {}

    /** @param array<int, Row> $rows */
    public function intKeys(array $rows): void {}

    /**
     * @param array<int, Row> $intKeyed
     * @param array<string, Row> $stringKeyed
     */
    public function run(Needs $n, array $intKeyed, array $stringKeyed): void
    {
        $n->stringKeys($intKeyed);
        $n->intKeys($stringKeyed);
    }
}
"#,
        expect![[r#"
            21:24-21:33 error [incompatible-argument] $rows takes array<string, \Row>, and this is array<int, Row>
            22:21-22:33 error [incompatible-argument] $rows takes array<int, \Row>, and this is array<string, Row>
        "#]],
    );
}

#[test]
fn a_key_stated_by_a_return_or_an_iterable_is_read_the_same_way() {
    // The interned spelling of the same question. A parameter's `array<K, V>` lives in the file's own
    // arena; a return's is in the project's type table, and `Held::ListOf` is a third shape carrying
    // neither -- so a rule taught to one of the three governs only the one it was tested with.
    check(
        r#"
//- /app.php
<?php

class Row {}

class Store
{
    /** @return array<string, Row> */
    public function byName(): array {}

    /** @return list<Row> */
    public function all(): array {}
}

class Reader
{
    /** @param array<int, Row> $rows */
    public function intKeys(array $rows): void {}

    /** @param iterable<string, Row> $rows */
    public function stringKeyedIterable(iterable $rows): void {}

    public function run(Store $store): void
    {
        $this->intKeys($store->byName());
        $this->stringKeyedIterable($store->all());

        $this->intKeys($store->all());
        $this->stringKeyedIterable($store->byName());
    }
}
"#,
        expect![[r#"
            24:24-24:40 error [incompatible-argument] $rows takes array<int, \Row>, and this is array<string, Row>
            25:36-25:49 error [incompatible-argument] $rows takes iterable<string, \Row>, and this is array<int, Row>
        "#]],
    );
}
