//! An explicit cast where the declaration names another type.
//!
//! The second place the type judgement's answer is not the whole answer, and unlike the literal
//! beside it this one is a position rather than a fact about the runtime. Weak mode converts between
//! the scalars, so `add(string $id)` called `add((int) $node->id())` runs -- PHP turns the int back
//! into a string at the boundary. What makes it worth reporting is that the cast is the author
//! writing the type down: one line says the value is an int and the declaration says it is a
//! string, and one of the two is a mistake whichever way it is resolved.
//!
//! Asked of the *strict* table, the same way the literal rule is asked. The strict and weak tables
//! differ only over the scalars, so a verdict that changes between them is exactly a coercion PHP
//! would have performed -- and `mixed`, `int|string`, a class and an unbound template answer
//! identically in both, which is what keeps them silent with no case of their own.
//!
//! Every fixture here is deliberately in **weak** mode. In a strict file the ordinary judgement
//! reports all of it already, and a test that declared `strict_types` would pass whether or not any
//! of this existed.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_cast_to_another_type_is_reported_wherever_it_is_written() {
    // The three sites, which is the whole of what has the written expression in hand: an argument,
    // an assignment and a return.
    check(
        r#"
//- /app.php
<?php

function needs_string(string $s): void {}

function needs_bool(bool $b): void {}

class Holder
{
    public string $label = '';

    public function get(): string
    {
        return (int) $this->label;
    }

    public function fill(int $n, string $text): void
    {
        $this->label = (float) $n;
        needs_string((int) $n);
        needs_bool((int) $n);
        needs_string((bool) $text);
    }
}
"#,
        expect![[r#"
            13:16-13:34 error [incompatible-return] Cannot return int where string is declared
            18:24-18:34 error [incompatible-assignment] Holder::$label takes string, and this is float
            19:22-19:30 error [incompatible-argument] $s takes string, and this is int
            20:20-20:28 error [incompatible-argument] $b takes bool, and this is int
            21:22-21:34 error [incompatible-argument] $s takes string, and this is bool
        "#]],
    );
}

#[test]
fn the_cast_the_declaration_asked_for_is_the_point_of_writing_one() {
    // The half that keeps this from reporting the fix. A cast to the declared type is what a reader
    // writes to make the call correct, and `int -> float` is the widening PHP keeps under
    // `strict_types` too -- so `(int)` where a `float` is declared is not a claim about anything.
    check(
        r#"
//- /app.php
<?php

function needs_string(string $s): void {}

function needs_float(float $f): void {}

class Holder
{
    public function fill(int $n): void
    {
        needs_string((string) $n);
        needs_float((int) $n);
        needs_float((float) $n);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_declaration_that_admits_the_cast_takes_it() {
    // Asked of the strict table rather than by peeling the expectation, which is what makes these
    // three fall out with no case of their own: a union naming `int` accepts an int, `mixed` accepts
    // everything, and an unbound template is not a type to judge against.
    check(
        r#"
//- /app.php
<?php

function takes_either(int|string $v): void {}

function takes_mixed(mixed $v): void {}

/**
 * @template T
 * @param T $v
 */
function takes_template($v): void {}

class Holder
{
    public function fill(string $text): void
    {
        takes_either((int) $text);
        takes_mixed((int) $text);
        takes_template((int) $text);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn only_a_cast_says_what_the_value_is() {
    // A value whose *type* is an int may be an int holding anything, and the declaration converting
    // it is what weak mode is for -- so an int reaching a `string` without a cast written over it
    // stays exactly as silent as it was before this existed. This is the whole of what the rule
    // leaves unreported, and it is deliberate: a cast has no doubt about its runtime type and a
    // variable typed `int` is not a claim its author wrote down at this call.
    check(
        r#"
//- /app.php
<?php

function needs_string(string $s): void {}

class Holder
{
    public function fill(int $n, int ...$rest): void
    {
        needs_string($n);
        needs_string($rest[0]);
        needs_string(7);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_cast_to_a_container_reaches_no_scalar_rule() {
    // `(array)` and `(object)` are not scalars, so nothing about them changes between the two
    // tables: an `(array)` where an `array` is declared was already accepted and an `(array)` where
    // a `string` is declared was already rejected, both without this.
    check(
        r#"
//- /app.php
<?php

function needs_array(array $a): void {}

function needs_object(object $o): void {}

class Holder
{
    public function fill(string $text): void
    {
        needs_array((array) $text);
        needs_object((object) $text);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_parameter_typed_only_by_its_docblock_is_the_shape_this_was_reported_on() {
    // Drupal's `FileUsageInterface::add(FileInterface $file, $module, $type, $id, $count = 1)` has
    // no native hints at all -- `@param string $id` is the whole declaration -- and the call the
    // report came from is `add($file, 'file', 'node', (int) $node->id())`. A `@param` reaches the
    // judgement through `ParamRow::rejects` like any other expectation, so the rule needs no case
    // for it; the fixture exists because the reported site had one and a rule that missed it would
    // read as working.
    check(
        r#"
//- /app.php
<?php

class Usage
{
    /**
     * @param string $module
     * @param string $id
     * @param int $count
     */
    public function add($module, $id, $count = 1) {}

    public function run(int $nid): void
    {
        $this->add('file', (int) $nid);
    }
}
"#,
        expect![[r#"
            14:28-14:38 error [incompatible-argument] $id takes string, and this is int
        "#]],
    );
}
