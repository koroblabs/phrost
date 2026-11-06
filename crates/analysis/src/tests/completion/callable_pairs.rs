//! What the string of a callable pair offers: `[$this, 'vali|']`.
//!
//! The pair is how PHP wrote a callable before `(...)`, and how a framework is still handed one --
//! a validator, a Twig filter, a `usort` comparator. Goto already lands on the method a pair names;
//! these are the other half, offering the name while it is written. The rules are the ones the
//! operators follow, because they are the ones PHP checks the pair against.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_pair_on_this_offers_the_methods_of_the_class() {
    // The reported shape: a named argument holding a list of pairs. Private is offered because the
    // pair is written inside the class, which is where PHP checks it from.
    check(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public string $title = '';

    public function build(): void
    {
        $this->field(validations: [[$this, 'vali@here@']]);
    }

    public function field(array $validations): void {}

    private function validateBudget(int $income): bool {}

    public static function validateAll(): bool {}
}
"#,
        &[Query::completion("pair on this", "here")],
        expect![[r#"
            pair on this
            - validateAll [method]  : bool
            - validateBudget [method]  : bool
            - ... and the client must ask again
            - accepting the first leaves `$this->field(validations: [[$this, 'validateAll']]);`
        "#]],
    );
}

#[test]
fn an_empty_string_offers_every_method_and_no_property() {
    check(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public const LIMIT = 3;
    public string $title = '';

    public function build(): array
    {
        return [$this, '@here@'];
    }

    private function check(): bool {}
}
"#,
        &[Query::completion("empty", "here")],
        expect![[r#"
            empty
            - build [method]  : array
            - check [method]  : bool
            - accepting the first leaves `return [$this, 'build'];`
        "#]],
    );
}

#[test]
fn a_pair_on_a_variable_offers_what_the_variable_can_reach() {
    // Outside the class, so only what PHP would let the pair call from here.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
    protected function load(): void {}
    private function cache(): void {}
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(Repo $repo): void
    {
        usort($rows, [$repo, '@here@']);
    }
}
"#,
        &[Query::completion("variable", "here")],
        expect![[r#"
            variable
            - find [method]  : void
            - accepting the first leaves `usort($rows, [$repo, 'find']);`
        "#]],
    );
}

#[test]
fn a_class_named_outright_offers_its_static_methods() {
    // `[Repo::class, 'find']` is not callable from outside -- PHP refuses a non-static method
    // reached through a class name -- so it is not offered.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
    public static function create(): Repo {}
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        array_map([Repo::class, '@here@'], []);
    }
}
"#,
        &[Query::completion("named class", "here")],
        expect![[r#"
            named class
            - create [method]  : Repo
            - accepting the first leaves `array_map([Repo::class, 'create'], []);`
        "#]],
    );
}

#[test]
fn self_class_offers_instance_methods_too() {
    // Inside an instance method PHP calls `[self::class, 'build']` on `$this`, which is the same
    // reach `self::build()` has.
    check(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public function build(): void
    {
        array_map([self::class, '@here@'], []);
    }

    public static function make(): Form {}
}
"#,
        &[Query::completion("self", "here")],
        expect![[r#"
            self
            - build [method]  : void
            - make [method]  : Form
            - accepting the first leaves `array_map([self::class, 'build'], []);`
        "#]],
    );
}

#[test]
fn the_legacy_array_spelling_is_the_same_pair() {
    check(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public function build(): array
    {
        return array($this, 'bu@here@');
    }
}
"#,
        &[Query::completion("legacy", "here")],
        expect![[r#"
            legacy
            - build [method]  : array
            - ... and the client must ask again
            - accepting the first leaves `return array($this, 'build');`
        "#]],
    );
}

#[test]
fn a_string_that_is_not_the_second_of_two_offers_nothing() {
    // The third element is data whatever stands before it, and so is a pair whose first element is
    // an int -- an int has no methods to offer.
    check(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public function build(int $id): array
    {
        $a = [$id, '@pair@'];
        return [$this, 'build', '@third@'];
    }
}
"#,
        &[
            Query::completion("int pair", "pair"),
            Query::completion("third element", "third"),
        ],
        expect![[r#"
            int pair
            - (nothing offered)
            third element
            - (nothing offered)
        "#]],
    );
}
