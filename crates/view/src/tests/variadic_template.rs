//! A template bound to a variadic parameter is one of every argument that filled it.
//!
//! `@param class-string<T> ...$names` beside `@return T`: the declaration binds one template to a
//! *list* of arguments, and what comes back is one of them. Reading only the argument at the
//! parameter's own position answered from the first and dropped the rest, so
//! `of(A::class, B::class)` said `A` -- narrower than the truth, which is the direction that turns
//! into a false positive downstream.
//!
//! Repeats are the other half, and they are what a union built from a list produces naturally:
//! `of(A::class, A::class)` is an `A` and not an `A|A`. The interned constructor has dropped repeats
//! since it existed and the resolved one could not compare its operands until `Held` derived
//! equality -- the same "two constructors of one concept" shape the boolean fold had.

use expect_test::expect;

use super::{Query, check_view};

/// A factory whose class name is variadic, beside one that is not, so the ordinary parameter is
/// visibly unchanged.
const FIXTURE: &str = r#"
//- /src/classes.php
<?php
namespace App;

class Alpha
{
    public function onlyAlpha(): void {}
}

class Beta
{
    public function onlyBeta(): void {}
}

class Factory
{
    /**
     * @template T
     * @param class-string<T> ...$names
     * @return T
     */
    public function of(string ...$names) {}

    /**
     * @template T
     * @param class-string<T> $name
     * @return T
     */
    public function one(string $name) {}
}
"#;

#[test]
fn every_argument_filling_a_variadic_is_one_of_the_answers() {
    check_view(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    public function run(Factory $f): void
    {{
        $two = $f->of(Alpha::class, Beta::class);
        $t@two@wo;
        $three = $f->of(Alpha::class, Beta::class, Alpha::class);
        $th@three@ree;
        $same = $f->of(Alpha::class, Alpha::class);
        $sa@same@me;
        $lone = $f->of(Alpha::class);
        $lo@lone@ne;
        $plain = $f->one(Beta::class);
        $pl@plain@ain;
    }}
}}
"#
        ),
        &[
            Query::variable_type("two names", "two"),
            Query::variable_type("three names, one repeated", "three"),
            Query::variable_type("the same name twice", "same"),
            Query::variable_type("one name", "lone"),
            Query::variable_type("a parameter that is not variadic", "plain"),
        ],
        expect![[r#"
            two names
            - Alpha|Beta
            three names, one repeated
            - Alpha|Beta
            the same name twice
            - Alpha
            one name
            - Alpha
            a parameter that is not variadic
            - Beta
        "#]],
    );
}
