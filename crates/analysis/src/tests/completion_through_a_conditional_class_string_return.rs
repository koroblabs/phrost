//! `$container->make(Repo::class)->` -- a chain through a call whose return is conditional on a
//! `class-string<T>` argument, as Laravel's container declares it.
//!
//! The completion chain reads the call out of text, so it has no literal arguments: the condition
//! was settled from the scanned `Repo::class` and the branch it chose, `TClass`, was then filled
//! from the literals alone and came back empty. Goto, which has the call expression, answered.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn the_template_branch_is_filled_from_the_scanned_argument() {
    check(
        r#"
//- /src/Container.php
<?php
namespace App;

class Container
{
    /**
     * @template TClass of object
     * @param  string|class-string<TClass>  $abstract
     * @return ($abstract is class-string<TClass> ? TClass : mixed)
     */
    public function make($abstract)
    {
        return null;
    }
}

class Repo
{
    public function find(): void {}
}

class Client
{
    public function go(Container $container): void
    {
        $container->make(Repo::class)->@here@
    }
}
"#,
        &[Query::completion("after the call", "here")],
        expect![[r#"
            after the call
            - find [method]  : void  writes `();`
            - accepting the first leaves `$container->make(Repo::class)->find`
        "#]],
    );
}
