//! `$obj::CONST` and `$obj::method()` — a class named by an instance rather than by a name.
//!
//! Legal since PHP 8.3 and written wherever a constant belongs to a service rather than to the class
//! reading it. PHP reads a class written as a value two ways — a class-*string* somebody wrote as
//! `Foo::class`, or an instance whose class it takes — and only the value says which.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_constant_is_reached_through_the_instance_holding_it() {
    check(
        r#"
//- /app.php
<?php
class Resource
{
    public const string STATUS_OK = 'ok';

    public static function make(): Resource {}
}

class Action
{
    private Resource $resource;

    public function run(): void
    {
        $status = $this->resource::STATUS_OK;
        $@one@status;
        $made = $this->resource::make();
        $@two@made;
        $mine = $this::STATUS_OK;
        $@three@mine;
    }

    public const string STATUS_OK = 'own';
}
"#,
        &[
            Query::hover("a constant through an instance", "one"),
            Query::hover("a static method through an instance", "two"),
            Query::hover("through $this written as a class", "three"),
        ],
        expect![[r#"
            a constant through an instance
            - code: string $status
            - note: assigned
            a static method through an instance
            - code: Resource $made
            - note: assigned
            through $this written as a class
            - code: string $mine
            - note: assigned
        "#]],
    );
}

#[test]
fn a_class_string_still_answers_first() {
    // Read from the path and needing no type at all, so it answers in a chain where nothing has been
    // resolved yet.
    check(
        r#"
//- /app.php
<?php
class Rule
{
    public function setName(string $name): void {}
}

function run(): void
{
    $class = Rule::class;
    $rule = new $class();
    $rule->$0
}
"#,
        &[Query::completion("through a class-string", "0")],
        expect![[r#"
            through a class-string
            - setName [method]  : void  writes `(|);`
            - accepting the first leaves `$rule->setName`
        "#]],
    );
}
