//! A name no declaration answers, on a class whose `__get` or `__call` does.
//!
//! PHP runs the accessor for exactly those names, so what it *declares* it returns is what reading
//! one gives back. Only what it declares: an accessor's body is a dispatcher, and one branch of it
//! is a claim about one name rather than about every name. And not for a name the file asks
//! `method_exists` about, which is PHP's way of saying a subclass declares it -- then that
//! declaration runs and the accessor does not.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_accessor_that_declares_its_return_types_every_name_it_answers() {
    check_view(
        r#"
//- /src/Bag.php
<?php
namespace App;

class Item {}

class Bag
{
    public function __get(string $name): Item {}
    public function __call(string $name, array $arguments): Item {}
    public static function __callStatic(string $name, array $arguments): Item {}
}

class Documented
{
    /** @return Item */
    public function __get($name) {}
}

class Runner
{
    public function go(Bag $bag, Documented $doc): void
    {
        $p@property@roperty = $bag->anything;
        $m@method@ethod = $bag->anything();
        $s@static@tatic = Bag::anything();
        $d@documented@ocumented = $doc->anything;
    }
}
"#,
        &[
            Query::variable_type("`__get`", "property"),
            Query::variable_type("`__call`", "method"),
            Query::variable_type("`__callStatic`", "static"),
            Query::variable_type("a `@return` on `__get`", "documented"),
        ],
        expect![[r#"
            `__get`
            - Item
            `__call`
            - Item
            `__callStatic`
            - Item
            a `@return` on `__get`
            - Item
        "#]],
    );
}

#[test]
fn a_declared_member_or_a_tag_answers_before_the_accessor() {
    check_view(
        r#"
//- /src/Bag.php
<?php
namespace App;

class Item {}
class Tagged {}

/** @property Tagged $tagged */
class Bag
{
    public int $count = 0;

    public function __get(string $name): Item {}
}

class Runner
{
    public function go(Bag $bag): void
    {
        $c@declared@ount = $bag->count;
        $t@tagged@agged = $bag->tagged;
    }
}
"#,
        &[
            Query::variable_type("a declared property", "declared"),
            Query::variable_type("a `@property` tag", "tagged"),
        ],
        expect![[r#"
            a declared property
            - int
            a `@property` tag
            - Tagged
        "#]],
    );
}

#[test]
fn an_accessor_whose_body_alone_says_what_it_returns_answers_nothing() {
    check_view(
        r#"
//- /src/Client.php
<?php
namespace App;

class Result {}

class Client
{
    public function __call($name, array $args)
    {
        return new Result();
    }
}

class Runner
{
    public function go(Client $client): void
    {
        $r@result@esult = $client->describeTable();
    }
}
"#,
        &[Query::variable_type("a dispatcher's body", "result")],
        expect![[r#"
            a dispatcher's body
            - (no type)
        "#]],
    );
}

#[test]
fn a_name_the_file_tests_with_method_exists_is_not_the_accessors() {
    check_view(
        r#"
//- /src/Mailable.php
<?php
namespace App;

class Mailable
{
    /** @return $this */
    public function __call($method, $parameters) {}

    public function hydrate(): void
    {
        if (! method_exists($this, 'headers')) {
            return;
        }
        $h@guarded@eaders = $this->headers();
        $o@other@ther = $this->withSubject();
    }
}
"#,
        &[
            Query::variable_type("a method `method_exists` asked about", "guarded"),
            Query::variable_type("any other name", "other"),
        ],
        expect![[r#"
            a method `method_exists` asked about
            - (no type)
            any other name
            - Mailable
        "#]],
    );
}
