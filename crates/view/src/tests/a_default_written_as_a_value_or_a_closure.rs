//! A template bound by a parameter that takes the value *or a closure producing it*.
//!
//! Laravel writes every lazy default this way -- `first($callback, TFirstDefault|(\Closure():
//! TFirstDefault) $default = null)`, `value(TValue|\Closure(TArgs): TValue $value)` -- and the
//! callee calls a closure it is handed. Matched only as a bare `T`, the parameter bound nothing, so
//! every `->first()` on a collection answered nothing at all. What arrives is the value, the
//! closure's result, or the parameter's own default when the call leaves it out; and a value that
//! may be a closure whose result nothing names leaves the branch unanswered rather than claiming
//! the closure itself.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /stubs/Closure.php  stub
<?php
final class Closure {}

//- /src/Coll.php
<?php
namespace App;

final class Post
{
    /** @return Coll<int, static> */
    public static function all() {}
}

/**
 * @template TKey
 * @template TValue
 */
class Coll
{
    /**
     * @template TFirstDefault
     * @param (callable(TValue, TKey): bool)|null $callback
     * @param TFirstDefault|(\Closure(): TFirstDefault) $default
     * @return TValue|TFirstDefault
     */
    public function first(?callable $callback = null, $default = null) {}
}

/**
 * @template TValue
 * @param TValue|\Closure(): TValue $value
 * @return TValue
 */
function value($value) {}

/**
 * @param Coll<int, Post> $posts
 * @param \Closure|string $either
 * @param \Closure(): Post $makes
 */
function run(Coll $posts, $either, $makes): void
{
    $l@left@eft = $posts->first();
    $w@written@ritten = $posts->first(null, 'none');
    $l@lazy@azy = $posts->first(null, fn (): string => 'none');
    $s@statically@tatically = Post::all()->first();
    $e@either@ither = value($either);
    $m@makes@akes = value($makes);
}
"#;

#[test]
fn the_value_the_closure_or_the_default_is_what_arrives() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("left out, so the default's null", "left"),
            Query::variable_type("a value written", "written"),
            Query::variable_type("a closure, so what it returns", "lazy"),
            Query::variable_type("a receiver whose argument is `static`", "statically"),
            Query::variable_type("a value that may be a closure", "either"),
            Query::variable_type("a closure whose result is declared", "makes"),
        ],
        expect![[r#"
            left out, so the default's null
            - ?Post
            a value written
            - Post|non-falsy-string
            a closure, so what it returns
            - Post|string
            a receiver whose argument is `static`
            - ?Post
            a value that may be a closure
            - (no type)
            a closure whose result is declared
            - Post
        "#]],
    );
}
