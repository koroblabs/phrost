//! What a `foreach` binds when the thing being iterated is written as a union.
//!
//! Its own file because the rule turns on a distinction a union reached with `->` never has to
//! make. There a `string` operand and a silent one are refused alike, and both refusals are right.
//! Here they part: a `string` *positively* holds no element and rules the union out, while a class
//! declaring no `@template` has said nothing either way and is passed over.
//!
//! `@return T[]|SomeCollection` is PHPDoc's ordinary way of writing "an iterable of T, concretely
//! this class", and the class half is a container declaring no `@template` for anyone to read.
//! Refusing there discarded the half that answered in order to honour a half that had not spoken.

use expect_test::expect;

use crate::tests::{Query, check_view};

#[test]
fn a_list_beside_a_collection_class_yields_the_list_element() {
    // The pathauto shape, taken from a production Drupal checkout:
    // `@return \Drupal\Core\Condition\ConditionInterface[]|\Drupal\Core\Condition\ConditionPluginCollection`.
    // The collection extends three classes and declares no `@template` anywhere in the chain, so
    // nothing can say what iterating *it* gives -- and the array half says exactly what the caller
    // is asking about.
    check_view(
        r#"
//- /src/Conditions.php
<?php
namespace App;

interface ConditionInterface
{
    public function getPluginId(): string;
}

class ConditionCollection {}

interface Pattern
{
    /**
     * @return ConditionInterface[]|ConditionCollection
     */
    public function getSelectionConditions();
}

class Uses
{
    public function run(Pattern $pattern): void
    {
        foreach ($pattern->getSelectionConditions() as $condition) {
            $condition->getPl@member@uginId();
            $cond@variable@ition;
        }
    }
}
"#,
        &[
            Query::variable_class("the loop variable", "variable", "condition"),
            Query::goto("the member reached through it", "member"),
        ],
        expect![[r#"
            the loop variable
            - App\ConditionInterface
            the member reached through it
            - method App\ConditionInterface::getPluginId @ src/Conditions.php 6:21-6:32
        "#]],
    );
}

#[test]
fn two_operands_that_both_answer_yield_a_union_of_both() {
    // The permissive rule must not flatten a union that is genuinely one, so what both halves
    // declare is still reachable and what only one half declares is still refused -- which is the
    // member rule doing its own job on the type this one built.
    check_view(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Ok
{
    public function shared(): void {}
    public function onlyOnOk(): void {}
}

class Err
{
    public function shared(): void {}
}

interface Source
{
    /**
     * @return Ok[]|Err[]
     */
    public function rows();
}

class Uses
{
    public function run(Source $source): void
    {
        foreach ($source->rows() as $row) {
            $row->sha@shared@red();
            $row->onlyO@only@nOk();
        }
    }
}
"#,
        &[
            Query::goto("declared by both operands", "shared"),
            Query::goto("declared by one operand", "only"),
        ],
        expect![[r#"
            declared by both operands
            - method App\Ok::shared @ src/Rows.php 6:21-6:27
            declared by one operand
            - method App\Ok::onlyOnOk @ src/Rows.php 7:21-7:29
        "#]],
    );
}

#[test]
fn a_scalar_operand_rules_the_whole_union_out() {
    // The direction that keeps a miss from becoming a wrong answer. `string|Row[]` read by index
    // may well be a `string`, and answering `Row` there would offer members on a value half the
    // callers pass a string for. This shape is written all over real code -- a callback declared as
    // "a method name, or a [$object, $method] pair" is exactly it.
    check_view(
        r#"
//- /src/Callbacks.php
<?php
namespace App;

class Row
{
    public function value(): string { return ''; }
}

class Uses
{
    /**
     * @param array<int, string|Row[]> $entries
     */
    public function run(array $entries): void
    {
        foreach ($entries as $entry) {
            $one = $entry[0];
            $one->val@member@ue();
            $o@variable@ne;
        }
    }
}
"#,
        &[
            Query::variable_class("what indexing the operand gave", "variable", "one"),
            Query::goto("the member reached through it", "member"),
        ],
        expect![[r#"
            what indexing the operand gave
            - (type not known here)
            the member reached through it
            - (not found)
        "#]],
    );
}

#[test]
fn a_union_where_nothing_answers_still_says_nothing() {
    // Two containers, neither of which declares what it holds. Dropping the operands that say
    // nothing must not leave an empty union standing in for an answer.
    check_view(
        r#"
//- /src/Silent.php
<?php
namespace App;

class LeftCollection {}
class RightCollection {}

interface Source
{
    /**
     * @return LeftCollection|RightCollection
     */
    public function rows();
}

class Uses
{
    public function run(Source $source): void
    {
        foreach ($source->rows() as $row) {
            $r@row@ow;
        }
    }
}
"#,
        &[Query::variable_class("the loop variable", "row", "row")],
        expect![[r#"
            the loop variable
            - (type not known here)
        "#]],
    );
}
