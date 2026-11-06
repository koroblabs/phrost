use super::{Query, check_with};
use expect_test::expect;

const FIXTURE: &str = r#"
//- /src/helpers.php
<?php
namespace App;

class Application
{
    public function terminating(callable $callback): static {}
}

class Signup
{
    public function isEnabled(): bool {}
}

/**
 * @template TClass of object
 * @param string|class-string<TClass>|null $abstract
 * @param array $parameters
 * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? \App\Application : mixed))
 */
function app($abstract = null, array $parameters = []) {}

function make(): Signup {}

/**
 * @template TClass of object
 * @param class-string<TClass> $abstract
 * @return TClass
 */
function build($abstract) {}

//- /src/a.php
<?php
namespace App;

function a(): void
{
    app(Signup::class)->isEna@named@
}

//- /src/c.php
<?php
namespace App;

function c(): void
{
    make()->isEna@plain@
}

//- /src/d.php
<?php
namespace App;

function d(): void
{
    build(Signup::class)->isEna@templ@
}

//- /src/b.php
<?php
namespace App;

function b(): void
{
    app()->termin@bare@
}
"#;

#[test]
fn a_contract_the_call_text_settles_is_read_for_completion() {
    check_with(
        FIXTURE,
        &[
            Query::completion("the class an argument names", "named"),
            Query::completion("an argument left out", "bare"),
            Query::completion("a declared return, for comparison", "plain"),
            Query::completion("a template a class-string binds", "templ"),
        ],
        |world| {
            world
                .contracts_mut()
                .bind_function_argument("app", 0, "container");
        },
        expect![[r#"
            the class an argument names
            - isEnabled [method]  : bool  writes `();`
            - ... and the client must ask again
            - accepting the first leaves `app(Signup::class)->isEnabled`
            an argument left out
            - terminating [method]  : static  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `app()->terminating`
            a declared return, for comparison
            - isEnabled [method]  : bool  writes `();`
            - ... and the client must ask again
            - accepting the first leaves `make()->isEnabled`
            a template a class-string binds
            - isEnabled [method]  : bool  writes `();`
            - ... and the client must ask again
            - accepting the first leaves `build(Signup::class)->isEnabled`
        "#]],
    );
}
