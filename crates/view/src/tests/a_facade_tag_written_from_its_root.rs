//! A facade's `@method` tag, read as the declaration it was generated from.
//!
//! Laravel's `Cache` facade tags `remember()` as returning `mixed`: the script that writes the tags
//! drops `Repository::remember()`'s `@template TCacheValue`, so what the closure returns never
//! reached the value. Where the root declares the method its declaration is read, and a tag it does
//! not declare stays as written.

use expect_test::expect;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
//- /vendor/Cache.php
<?php
namespace Orm;

abstract class Facade
{
    public static function __callStatic(string $method, array $args): mixed {}
}

/**
 * @method static mixed remember(string $key, int $ttl, \Closure $callback)
 * @method static bool flush()
 */
class Cache extends Facade {}

class Repository
{
    /**
     * @template TCacheValue
     * @param \Closure(): TCacheValue $callback
     * @return TCacheValue
     */
    public function remember(string $key, int $ttl, \Closure $callback) {}
}

//- /src/Use.php
<?php
namespace App;

class User {}

function run(): void
{
    $r@remembered@emembered = \Orm\Cache::remember('users', 60, fn () => new User());
    $f@flushed@lushed = \Orm\Cache::flush();
    \Orm\Cache::rem@goto@ember('users', 60, fn () => 1);
}
"#;

#[test]
fn a_facade_call_reads_the_method_its_tag_was_written_from() {
    check_view_with(
        FIXTURE,
        &[
            Query::variable_type(
                "what the closure returns, through the root's template",
                "remembered",
            ),
            Query::variable_type("a tag the root does not declare, as written", "flushed"),
            Query::goto("goto, the root's declaration", "goto"),
        ],
        |world| world.add_tag_source("Orm\\Cache", "Orm\\Repository"),
        expect![[r#"
            what the closure returns, through the root's template
            - User
            a tag the root does not declare, as written
            - bool
            goto, the root's declaration
            - method Orm\Repository::remember @ vendor/Cache.php 22:21-22:29
        "#]],
    );
}
