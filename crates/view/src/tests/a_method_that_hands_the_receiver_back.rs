//! `static`, `self` and `$this` hand back what went in, arguments and all.
//!
//! The fluent interface, which is how every PHP framework is written:
//! `$query->where(...)->orderBy(...)`, `$collection->filter(...)->first()`, PSR-7's `withHeader`.
//! A method declared to return itself was resolved to its class's *name* -- which is all
//! `class_of_relative` can do -- and the type arguments the receiver was written with went with it,
//! so one step through any of them and what the container held was gone.
//!
//! Every spelling had the same hole: `@return static`, `@return self`, `@return $this`, and the
//! native `: static` and `: self` with no tag at all. Only an explicit `@return Box<T>` survived,
//! because that one goes through the template substitution and the relative words do not.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn every_spelling_of_returning_itself_keeps_what_the_receiver_held() {
    check_view(
        r#"
//- /src/Box.php
<?php
namespace App;

class Row
{
    public function label(): string
    {
        return '';
    }
}

/** @template T of object */
class Box
{
    /** @return T|null */
    public function get(): ?object
    {
        return NULL;
    }

    /** @return static */
    public function tagged(): static
    {
        return $this;
    }

    /** @return $this */
    public function fluent(): static
    {
        return $this;
    }

    public function bare(): self
    {
        return $this;
    }
}

class Use_
{
    public function run(): void
    {
        /** @var Box<Row> $box */
        $box = new Box();
        $afterTagged = $box->tagged()->get();
        $afterFluent = $box->fluent()->get();
        $afterBare = $box->bare()->get();
        $twoSteps = $box->tagged()->bare()->get();
        $after@tagged@Tagged;
        $after@fluent@Fluent;
        $after@bare@Bare;
        $two@two@Steps;
    }
}
"#,
        &[
            Query::variable_type("after @return static", "tagged"),
            Query::variable_type("after @return $this", "fluent"),
            Query::variable_type("after a bare : self", "bare"),
            Query::variable_type("after two fluent steps", "two"),
        ],
        expect![[r#"
            after @return static
            - ?Row
            after @return $this
            - ?Row
            after a bare : self
            - ?Row
            after two fluent steps
            - ?Row
        "#]],
    );
}

/// The refusal beside it. `parent` names a different set of templates, and what the value supplied
/// was for its own -- so the arguments say nothing about the supertype's and are not carried.
#[test]
fn a_parent_return_carries_nothing_because_its_templates_are_not_these() {
    check_view(
        r#"
//- /src/Box.php
<?php
namespace App;

class Row {}

/** @template B of object */
class Base
{
    /** @return B|null */
    public function get(): ?object
    {
        return NULL;
    }
}

/**
 * @template T of object
 * @extends Base<T>
 */
class Box extends Base
{
    /** @return parent */
    public function up(): parent
    {
        return $this;
    }
}

class Use_
{
    public function run(): void
    {
        /** @var Box<Row> $box */
        $box = new Box();
        $climbed = $box->up();
        $cl@climbed@imbed;
    }
}
"#,
        &[Query::variable_type("a parent return", "climbed")],
        expect![[r#"
            a parent return
            - Base
        "#]],
    );
}
