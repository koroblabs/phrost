//! A return type the call decides.
//!
//! `@return ($p is null ? A : B)` is how a declaration says that what comes back depends on what was
//! passed. PHP's own array functions are written this way, and so is every framework helper that
//! doubles as a factory -- so a reader that cannot settle the test has to offer the intersection of
//! two unrelated classes, which is nothing.
//!
//! Only the tests an argument settles are recorded. A conditional on what an array *contains* is
//! about the argument's own type rather than about what was written at the call, and there the
//! declaration keeps whatever its branches agree on.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_null_test_is_settled_by_the_keyword_the_call_wrote() {
    check(
        r#"
//- /src/response.php
<?php
namespace App;

class ResponseFactory
{
    public function json(): string {}
}

class HttpResponse
{
    public function send(): void {}
}

/**
 * @param string|array|null $content
 * @return ($content is null ? ResponseFactory : HttpResponse)
 */
function response($content = null) {}

class Controller
{
    public function run(): void
    {
        $factory = response(null);
        $factory->@factory@
    }
}
"#,
        &[Query::completion("passed null", "factory")],
        expect![[r#"
            passed null
            - json [method]  : string  writes `();`
            - accepting the first leaves `$factory->json`
        "#]],
    );
}

#[test]
fn a_null_test_is_settled_by_a_default_when_the_call_writes_nothing() {
    check(
        r#"
//- /src/silent.php
<?php
namespace App;

class ResponseFactory
{
    public function json(): string {}
}

class HttpResponse
{
    public function send(): void {}
}

/**
 * @param string|array|null $content
 * @return ($content is null ? ResponseFactory : HttpResponse)
 */
function response($content = null) {}

class Controller
{
    public function run(): void
    {
        $factory = response();
        $factory->@defaulted@
    }
}
"#,
        &[Query::completion("nothing passed", "defaulted")],
        expect![[r#"
            nothing passed
            - json [method]  : string  writes `();`
            - accepting the first leaves `$factory->json`
        "#]],
    );
}

#[test]
fn a_string_argument_is_not_null_and_takes_the_other_branch() {
    check(
        r#"
//- /src/body.php
<?php
namespace App;

class ResponseFactory
{
    public function json(): string {}
}

class HttpResponse
{
    public function send(): void {}
}

/**
 * @param string|array|null $content
 * @return ($content is null ? ResponseFactory : HttpResponse)
 */
function response($content = null) {}

class Controller
{
    public function run(): void
    {
        $sent = response('body');
        $sent->@sent@
    }
}
"#,
        &[Query::completion("passed a body", "sent")],
        expect![[r#"
            passed a body
            - send [method]  : void  writes `();`
            - accepting the first leaves `$sent->send`
        "#]],
    );
}

#[test]
fn a_string_the_call_wrote_settles_a_literal_test() {
    check(
        r#"
//- /src/literal.php
<?php
namespace App;

class StringResult
{
    public function strMethod(): void {}
}

class IntResult
{
    public function intMethod(): void {}
}

interface Mapper
{
    /**
     * @return ($signature is "foo" ? StringResult : IntResult)
     */
    public function map(string $signature);
}

class Runner
{
    public function run(Mapper $mapper): void
    {
        $mapped = $mapper->map('foo');
        $mapped->@matched@
    }
}
"#,
        &[Query::completion("the literal matched", "matched")],
        expect![[r#"
            the literal matched
            - strMethod [method]  : void  writes `();`
            - accepting the first leaves `$mapped->strMethod`
        "#]],
    );
}

#[test]
fn a_class_string_test_reads_the_class_the_call_named() {
    check(
        r#"
//- /src/mapper.php
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
        $mapped = $mapper->map(Foobar::class, 'bar');
        $mapped->@mapped@
    }
}
"#,
        &[Query::completion("mapped to the named class", "mapped")],
        expect![[r#"
            mapped to the named class
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$mapped->fooMethod`
        "#]],
    );
}

#[test]
fn a_string_that_names_no_class_takes_the_other_branch() {
    check(
        r#"
//- /src/service.php
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
     * @return ($signature is class-string<T> ? T : Foobar)
     */
    public function map(string $signature, mixed $source);
}

class Runner
{
    public function run(TreeMapper $mapper): void
    {
        $mapped = $mapper->map('a.service.id', 'bar');
        $mapped->@service@
    }
}
"#,
        &[Query::completion("a service id names no class", "service")],
        expect![[r#"
            a service id names no class
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$mapped->fooMethod`
        "#]],
    );
}

/// A variable settles neither test, and the value is still one of the two branches -- so completion
/// offers both, each marked with the class that declares it.
#[test]
fn an_argument_this_cannot_read_leaves_both_branches() {
    check(
        r#"
//- /src/unknown.php
<?php
namespace App;

class ResponseFactory
{
    public function json(): string {}
}

class HttpResponse
{
    public function send(): void {}
}

/**
 * @param string|array|null $content
 * @return ($content is null ? ResponseFactory : HttpResponse)
 */
function response($content = null) {}

class Controller
{
    public function run(mixed $body): void
    {
        $either = response($body);
        $either->@either@
    }
}
"#,
        &[Query::completion(
            "a variable settles neither branch",
            "either",
        )],
        expect![[r#"
            a variable settles neither branch
            - json [method]  : string — ResponseFactory only  writes `();`
            - send [method]  : void — HttpResponse only  writes `();`
            - accepting the first leaves `$either->json`
        "#]],
    );
}

#[test]
fn a_negated_test_swaps_which_branch_is_which() {
    check(
        r#"
//- /src/negated.php
<?php
namespace App;

class ResponseFactory
{
    public function json(): string {}
}

class HttpResponse
{
    public function send(): void {}
}

/**
 * @param string|array|null $content
 * @return ($content is not null ? HttpResponse : ResponseFactory)
 */
function response($content = null) {}

class Controller
{
    public function run(): void
    {
        $factory = response(null);
        $factory->@negated@
    }
}
"#,
        &[Query::completion(
            "null takes the else of a negated test",
            "negated",
        )],
        expect![[r#"
            null takes the else of a negated test
            - json [method]  : string  writes `();`
            - accepting the first leaves `$factory->json`
        "#]],
    );
}

#[test]
fn a_branch_bound_to_what_a_parameter_holds_reads_the_argument() {
    check(
        r#"
//- /src/pop.php
<?php
namespace App;

class Item
{
    public function label(): string {}
}

/**
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return ($array is non-empty-array ? V : V|null)
 */
function take(array &$array): mixed {}

class Runner
{
    public function run(): void
    {
        /** @var Item[] $items */
        $taken = take($items);
        $taken->@taken@
    }
}
"#,
        &[Query::completion("what the array held", "taken")],
        expect![[r#"
            what the array held
            - label [method]  : string  writes `();`
            - accepting the first leaves `$taken->label`
        "#]],
    );
}

#[test]
fn a_conditional_whose_branches_disagree_is_either_of_them() {
    check(
        r#"
//- /src/disagree.php
<?php
namespace App;

class Left
{
    public function leftMethod(): void {}
}

class Right
{
    public function rightMethod(): void {}
}

/**
 * @return ($array is non-empty-array ? Left : Right)
 */
function pick(array $array) {}

class Runner
{
    public function run(array $rows): void
    {
        $picked = pick($rows);
        $picked->@picked@
    }
}
"#,
        &[Query::completion("two unrelated branches", "picked")],
        expect![[r#"
            two unrelated branches
            - leftMethod [method]  : void — Left only  writes `();`
            - rightMethod [method]  : void — Right only  writes `();`
            - accepting the first leaves `$picked->leftMethod`
        "#]],
    );
}

#[test]
fn a_mixed_argument_is_either_branch_rather_than_the_else() {
    // `mixed` is the absence of a kind, not a kind that fails the test. Comparing it took the else
    // branch -- this used to answer `?array<string>`, confidently, about a value nothing is known
    // about. `array<string, mixed>` read out of a config array is how a real call reaches here.
    //
    // Both branches is the answer, which is what PHPStan gives too: whatever the subject is, what
    // comes back is one of them.
    check(
        r#"
//- /src/replace.php
<?php
namespace App;

/**
 * @param string|array<string> $subject
 *
 * @return ($subject is string ? string|null : array<string>|null)
 */
function replace(string $pattern, string|array $subject) {}

class Runner
{
    /**
     * @param array<string, mixed> $config
     */
    public function run(array $config): void
    {
        $either = replace('/x/', $config['subject']);
        $eith@either@er;
    }
}
"#,
        &[Query::hover("a subject nothing is known about", "either")],
        expect![[r#"
            a subject nothing is known about
            - code: string|string[]|null $either
            - note: assigned
        "#]],
    );
}

#[test]
fn a_test_about_the_arguments_kind_reads_its_type() {
    check(
        r#"
//- /src/replace.php
<?php
namespace App;

/**
 * @param string|array<string> $subject
 *
 * @return ($subject is string ? string|null : array<string>|null)
 */
function replace(string $pattern, string|array $subject) {}

class Runner
{
    public function run(string $text, array $lines): void
    {
        $one = replace('/x/', $text);
        $many = replace('/x/', $lines);
        $on@one@e;
        $ma@many@ny;
    }
}
"#,
        &[
            Query::hover("a string subject", "one"),
            Query::hover("an array subject", "many"),
        ],
        expect![[r#"
            a string subject
            - code: ?string $one
            - note: assigned
            an array subject
            - code: ?string[] $many
            - note: assigned
        "#]],
    );
}

/// A test against a *number*, written at the call as the constant that names it -- in either
/// spelling.
///
/// `json_encode` returns `string|false` and never `false` when `JSON_THROW_ON_ERROR` is in the
/// flags: it throws instead, which was run against PHP 8.5.10. So the declaration is a conditional
/// on the flag, and settling it needs the constant's *value* -- which lives in whatever file
/// declares it, so the name travels and the reader looks it up.
///
/// Both spellings, because the reader took half of `NameAnchor::of`'s pair and resolved a
/// fully-qualified `\JSON_FLAG` as a relative name with the separator still on it: the bare form
/// settled and the qualified one did not. Fourth reader to have done that.
///
/// The third row is a call that leaves the flag out, which the declaration's own default answers.
/// It was written down for a while as a miss: lowering read a `null`, a `true` and a `false` default
/// and returned before it reached a number.
#[test]
fn a_number_test_is_settled_by_the_constant_in_either_spelling() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

const JSON_FLAG = 4194304;

class Text
{
    public function upper(): string {}
}

class Failure
{
    public function why(): string {}
}

/**
 * @return ($flags is 4194304 ? Text : Failure)
 */
function encode(mixed $value, int $flags = 0) {}

class Runner
{
    public function run(array $rows): void
    {
        encode($rows, JSON_FLAG)->up@bare@per();
        encode($rows, \App\JSON_FLAG)->up@qualified@per();
        encode($rows)->w@absent@hy();
    }
}
"#,
        &[
            Query::goto("the bare constant", "bare"),
            Query::goto("the qualified constant", "qualified"),
            Query::goto("no flag at all", "absent"),
        ],
        expect![[r#"
            the bare constant
            - method App\Text::upper @ src/app.php 8:21-8:26 (full 8:5-8:39)
            the qualified constant
            - method App\Text::upper @ src/app.php 8:21-8:26 (full 8:5-8:39)
            no flag at all
            - method App\Failure::why @ src/app.php 13:21-13:24 (full 13:5-13:37)
        "#]],
    );
}
