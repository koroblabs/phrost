//! A call into a method whose signature names no return, and whose body does.
//!
//! The value of reading it is cross-file: a factory in one file and a caller in another, where the
//! caller has nothing but the declaration. On the reference project the shape that matters most is
//! the fluent one -- `return $this;` -- because a query builder is a chain of them and one
//! unreadable link makes every step after it unresolvable.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_factory_in_another_file_answers_with_what_it_builds() {
    check_view(
        r#"
//- /src/Bar.php
<?php
namespace App;

class Bar
{
    public function label(): string
    {
    }
}

//- /src/Factory.php
<?php
namespace App;

class Factory
{
    public function create()
    {
        return new Bar();
    }

    public function rows()
    {
        return [new Bar()];
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Factory $factory): void
    {
        $b@built@ar = $factory->create();
        $r@rows@ows = $factory->rows();
        foreach ($factory->rows() as $o@one@ne) {
        }
    }
}
"#,
        &[
            Query::variable_type("what it builds", "built"),
            Query::variable_type("a list of them", "rows"),
            Query::variable_type("one of the list", "one"),
        ],
        expect![[r#"
            what it builds
            - Bar
            a list of them
            - array<int, Bar>
            one of the list
            - Bar
        "#]],
    );
}

/// The fluent chain, which is the shape a query builder is made of.
#[test]
fn a_chain_of_fluent_setters_stays_on_the_class() {
    check_view(
        r#"
//- /src/Query.php
<?php
namespace App;

class Query
{
    public function where(string $field)
    {
        return $this;
    }

    public function limit(int $rows)
    {
        return $this;
    }

    public function execute(): Result
    {
    }
}

class Result
{
    public function rows(): array
    {
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Query $query): void
    {
        $r@result@esult = $query->where('id')->limit(10)->execute();
    }
}
"#,
        &[Query::variable_type("through two fluent steps", "result")],
        expect![[r#"
            through two fluent steps
            - Result
        "#]],
    );
}

/// And the half that keeps it honest: a body whose returns this cannot read whole says nothing,
/// rather than saying what the readable ones happen to be.
#[test]
fn a_body_with_one_unreadable_return_says_nothing() {
    check_view(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Bar {}

class Factory
{
    public function make()
    {
    }

    public function create(bool $flag)
    {
        if ($flag) {
            return new Bar();
        }
        return $this->make();
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Factory $factory): void
    {
        $b@built@ar = $factory->create(TRUE);
    }
}
"#,
        &[Query::variable_type("half of it unreadable", "built")],
        expect![[r#"
            half of it unreadable
            - (no type)
        "#]],
    );
}
