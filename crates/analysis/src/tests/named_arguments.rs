//! The parameters of the call being written, offered by name.
//!
//! A bare identifier at an argument position is genuinely two questions -- `Stage` and `stage:` are
//! both things a person types there -- so both are offered, and the parameters come first because at
//! that position they are the more specific answer.

use expect_test::expect;

use super::{Query, check};

#[test]
fn the_parameters_of_the_call_being_written() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict = false, ?string $stage = null): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(1, s$0
    }
}
"#,
        &[Query::completion("at the second argument", "0")],
        expect![[r#"
            at the second argument
            - strict: [variable]  bool  insert `strict: `
            - stage: [variable]  ?string  insert `stage: `
            - Service [class]  Service
            - ... and the client must ask again
            - accepting the first leaves `$repo->find(1, strict: `
        "#]],
    );
}

#[test]
fn a_parameter_already_named_is_not_offered_twice() {
    // A second value for one parameter is a fatal error, so offering it is offering a mistake.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict = false, ?string $stage = null): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(1, strict: true, s$0
    }
}
"#,
        &[Query::completion("after a named argument", "0")],
        expect![[r#"
            after a named argument
            - stage: [variable]  ?string  insert `stage: `
            - Service [class]  Service
            - ... and the client must ask again
            - accepting the first leaves `$repo->find(1, strict: true, stage: `
        "#]],
    );
}

#[test]
fn a_parameter_a_positional_argument_covers_is_not_offered() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict = false, ?string $stage = null): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(i$0
    }
}
"#,
        &[Query::completion("at the first argument", "0")],
        expect![[r#"
            at the first argument
            - id: [variable]  int  insert `id: `
            - ... and the client must ask again
            - accepting the first leaves `$repo->find(id: `
        "#]],
    );
}

#[test]
fn a_named_argument_names_the_class_a_factory_returns() {
    check(
        r#"
//- /src/Registry.php
<?php
namespace App;

class Pen
{
    public function write(): string {}
}

class ServiceRegistry
{
    /**
     * @template T of object
     * @param class-string<T> $id
     * @return T
     */
    public function resolve(string $id, int $priority = 0) {}
}

class Runner
{
    public function run(ServiceRegistry $registry): void
    {
        $service = $registry->resolve(priority: 5, id: Pen::class);
        $service->@resolved@
    }
}
"#,
        &[Query::completion("out of order, by name", "resolved")],
        expect![[r#"
            out of order, by name
            - write [method]  : string  writes `();`
            - accepting the first leaves `$service->write`
        "#]],
    );
}

#[test]
fn a_named_argument_settles_a_conditional_return() {
    check(
        r#"
//- /src/Mapper.php
<?php
namespace App;

class Foobar
{
    public function fooMethod(): void {}
}

interface TreeMapper
{
    /**
     * @template T of object
     *
     * @param string|class-string<T> $signature
     * @return ($signature is class-string<T> ? T : mixed)
     */
    public function map(string $signature, mixed $source);
}

class Runner
{
    public function run(TreeMapper $mapper): void
    {
        $mapped = $mapper->map(source: 'bar', signature: Foobar::class);
        $mapped->@mapped@
    }
}
"#,
        &[Query::completion("named subject", "mapped")],
        expect![[r#"
            named subject
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$mapped->fooMethod`
        "#]],
    );
}

#[test]
fn a_by_reference_argument_written_by_name_seeds_its_own_parameter() {
    check(
        r#"
//- /src/out.php
<?php
namespace App;

class Baz
{
    public function doWork(): void {}
}

function seed(int $first, Baz &$bar): void
{
    $bar = new Baz();
}

class Runner
{
    public function run(): void
    {
        seed(bar: $out, first: 1);
        $out->@seeded@
    }
}
"#,
        &[Query::completion(
            "seeded through a named reference",
            "seeded",
        )],
        expect![[r#"
            seeded through a named reference
            - doWork [method]  : void  writes `();`
            - accepting the first leaves `$out->doWork`
        "#]],
    );
}

#[test]
fn an_attributes_parameters_are_offered_by_name() {
    // An attribute is written by name far more often than a call is, and until it was lowered as the
    // constructor call it is there was no call at that position to take the parameters from.
    check(
        r#"
//- /app.php
<?php
#[Attribute]
class Route
{
    public function __construct(private string $path, private string $name = '') {}
}

#[Route(p$0)]
class Home {}
"#,
        &[Query::completion("inside an attribute", "0")],
        expect![[r#"
            inside an attribute
            - path: [variable]  string  insert `path: `
            - ... and the client must ask again
            - accepting the first leaves `#[Route(path: )]`
        "#]],
    );
}
