//! Calling a closure held in a variable.
//!
//! The layer that owns the answer: the body says a closure is a value and that a call applied an
//! `Invoked` step to it, and this is where "what does invoking it give back" is settled -- against
//! a catalog, because the class the closure returns is usually written in another file.
//!
//! The pair that matters most is a closure and an object with `__invoke`. Both are called the same
//! way and they resolve through opposite routes: one carries its own answer and the other has to be
//! looked up on a class. A test that only covered the new route would not notice the old one going
//! dark.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn calling_a_variable_holding_a_closure_gives_its_declared_return() {
    check_view(
        r#"
//- /src/Query.php
<?php
namespace App;

class Query
{
    public function run(): void {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $make = fn (): Query => new Query();
        $q = $make();
        $ma@closure@ke;
        $@called@q;
    }
}
"#,
        &[
            Query::variable_class("the closure itself", "closure", "make"),
            Query::variable_class("what calling it gives", "called", "q"),
        ],
        expect![[r#"
            the closure itself
            - (type not known here)
            what calling it gives
            - App\Query
        "#]],
    );
}

#[test]
fn an_arrow_function_that_declares_nothing_answers_from_its_body() {
    // The commoner spelling by far: nobody writes `: Query` on a one-line factory. The body is a
    // single expression, already lowered, so there is no branch to choose between.
    check_view(
        r#"
//- /src/Query.php
<?php
namespace App;

class Query
{
    public function run(): void {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $make = fn () => new Query();
        $q = $make();
        $@called@q;
    }
}
"#,
        &[Query::variable_class("inferred", "called", "q")],
        expect![[r#"
            inferred
            - App\Query
        "#]],
    );
}

#[test]
fn a_long_form_closure_is_read_the_same_way() {
    // Written with a declaration, which is the whole answer whatever the body does -- see
    // `super::inferred_closure_return` for the body being read when there is none.
    check_view(
        r#"
//- /src/Query.php
<?php
namespace App;

class Query
{
    public function run(): void {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $make = function (): Query {
            return new Query();
        };
        $q = $make();
        $@called@q;
    }
}
"#,
        &[Query::variable_class("declared", "called", "q")],
        expect![[r#"
            declared
            - App\Query
        "#]],
    );
}

#[test]
fn a_closure_declaring_nothing_names_no_class_when_its_body_names_none() {
    // A block closure with no declaration is read from its body -- see
    // `super::inferred_closure_return` -- and here the body hands back an `int`, which names no
    // class. So the answer to *this* question is still nothing, and for a different reason than it
    // used to be: not "unknown", but "known and not a class".
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $make = function () {
            return 1;
        };
        $q = $make();
        $@called@q;
        $ma@value@ke;
    }
}
"#,
        &[
            Query::variable_class("names no class", "called", "q"),
            Query::variable_type("and says so", "value"),
        ],
        expect![[r#"
            names no class
            - (type not known here)
            and says so
            - Closure(): int
        "#]],
    );
}

#[test]
fn an_object_with_invoke_still_resolves_through_its_class() {
    // The route the closure arm must not have taken over. `$maker()` is written exactly like
    // `$make()` and means something else entirely: PHP looks `__invoke` up on the class.
    check_view(
        r#"
//- /src/Query.php
<?php
namespace App;

class Query
{
    public function run(): void {}
}

//- /src/Maker.php
<?php
namespace App;

class Maker
{
    public function __invoke(): Query
    {
        return new Query();
    }
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Maker $maker): void
    {
        $q = $maker();
        $@called@q;
    }
}
"#,
        &[Query::variable_class("through __invoke", "called", "q")],
        expect![[r#"
            through __invoke
            - App\Query
        "#]],
    );
}

#[test]
fn the_closure_may_return_a_class_from_another_file() {
    // What makes this a view test rather than a body one: the return type is a written name, and
    // resolving it needs the imports of the file the closure is in and a catalog that knows where
    // the class lives.
    check_view(
        r#"
//- /src/Entity/User.php
<?php
namespace App\Entity;

class User
{
    public function name(): string {}
}

//- /src/Runner.php
<?php
namespace App;

use App\Entity\User as Person;

class Runner
{
    public function go(): void
    {
        $make = fn (): Person => new Person();
        $who = $make();
        $@called@who;
    }
}
"#,
        &[Query::variable_class("imported", "called", "who")],
        expect![[r#"
            imported
            - App\Entity\User
        "#]],
    );
}
