//! A call into a generator declared in another file, iterated here.
//!
//! PHP has no syntax for saying in the signature what a generator produces -- `: \Generator` is the
//! whole of what a declaration can state -- so the `yield` is where it is written down. Cross-file
//! is where that matters: the caller has the declaration and nothing else.
//!
//! And the correction that travels with it: a `return` inside a generator goes to
//! `Generator::getReturn()`, so reading one as what the declaration hands back types the call as a
//! class the value can never be.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_generator_in_another_file_answers_with_what_it_yields() {
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

//- /src/Source.php
<?php
namespace App;

class Source
{
    public function rows(): iterable
    {
        yield new Bar();
    }

    public function undeclared()
    {
        yield new Bar();
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Source $source): void
    {
        foreach ($source->rows() as $o@one@ne) {
        }
        foreach ($source->undeclared() as $u@undeclared@ndeclared) {
        }
    }
}
"#,
        &[
            Query::variable_type("one of what it yields", "one"),
            Query::variable_type("the same with no signature at all", "undeclared"),
        ],
        expect![[r#"
            one of what it yields
            - Bar
            the same with no signature at all
            - Bar
        "#]],
    );
}

#[test]
fn a_return_inside_a_generator_is_not_what_the_call_hands_back() {
    check_view(
        r#"
//- /src/Source.php
<?php
namespace App;

class Bar {}

class Source
{
    public function rows()
    {
        yield 1;
        return new Bar();
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Source $source): void
    {
        $r@rows@ows = $source->rows();
    }
}
"#,
        &[Query::variable_type(
            "a generator and not its return",
            "rows",
        )],
        expect![[r#"
            a generator and not its return
            - (no type)
        "#]],
    );
}
