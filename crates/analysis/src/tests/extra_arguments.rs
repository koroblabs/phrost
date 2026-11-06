//! A call with more arguments than its callee has parameters for.
//!
//! The upper-bound half of [`super::arguments`]: `ArgumentCountError` from the other direction, and
//! reported only where the count can be proven rather than guessed. Every decline below matches one
//! `calls_missing_arguments` already makes, for the same reason -- see `phrost_view::arguments`.

use expect_test::expect;

use super::diagnostics::check_asked_for;

/// This check is opt-in -- see `ExtraArgument::asked_for` -- so every fixture here turns it on
/// explicitly rather than relying on the default set `check` would exercise.
fn check(fixture: &str, expect: expect_test::Expect) {
    check_asked_for(fixture, &["extra-argument"], expect);
}

/// Opt-in means silent until a project asks for it by name. A rule with real value, measured to be
/// a real risk in at least one environment, still starts off -- see the doc on `ExtraArgument` for
/// why: on a real corpus every finding traced back to one symbol-table collision this check cannot
/// see through, not to a call that was actually wrong.
#[test]
fn the_check_is_on_without_a_project_asking() {
    // It was opt-in while a dependency's test fixture could shadow `count()`. That is fixed, so
    // the default set reports this without a project naming the rule -- which is what the
    // lower-bound check has always done, and the two are one question asked from both ends.
    super::diagnostics::check(
        r#"
//- /app.php
<?php
function needs_two(string $a, string $b): void {}

function run(): void
{
    needs_two('a', 'b', 'c');
}
"#,
        expect![[r#"
            6:25-6:28 error [extra-argument] needs_two() takes at most 2 arguments, 3 given
        "#]],
    );
}

#[test]
fn a_call_with_more_arguments_than_the_callee_takes_is_reported() {
    check(
        r#"
//- /app.php
<?php
function needs_two(string $a, string $b): void {}

function run(): void
{
    needs_two('a', 'b', 'c');
}
"#,
        expect![[r#"
            6:25-6:28 error [extra-argument] needs_two() takes at most 2 arguments, 3 given
        "#]],
    );
}

/// The range names the extra arguments, not the call: everything up to them is fine.
#[test]
fn the_range_covers_only_the_extra_arguments() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->find(1, 2, 3, 4);
    }
}
"#,
        expect![[r#"
            11:24-11:31 error [extra-argument] find() takes at most 1 argument, 4 given
        "#]],
    );
}

/// A parameter with a default still occupies a position; only the positions past the last
/// parameter are extra.
#[test]
fn a_call_that_fits_including_defaults_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict = false): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->find(1, true);
    }
}
"#,
        expect![""],
    );
}

/// A variadic takes any number of arguments from its own position on, so there is no upper bound to
/// check a call against.
#[test]
fn a_variadic_has_no_upper_bound() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function many(int $id, string ...$rest): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->many(1, 'a', 'b', 'c', 'd', 'e');
    }
}
"#,
        expect![""],
    );
}

/// A spread stands for however many values are in the array. That count is not knowable here, so
/// declining is the whole difference between this check and a guess -- same reason the lower bound
/// declines it.
#[test]
fn a_spread_makes_the_count_unknowable_and_is_declined() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function two(string $a, string $b): void {}
}

class Caller
{
    public function run(Repo $repo, array $manyArgs): void
    {
        $repo->two(...$manyArgs);
    }
}
"#,
        expect![""],
    );
}

/// Past a named argument, position has stopped corresponding to a parameter at all -- the same
/// place `active_parameter` gives up, and for the same reason. A named argument that does not even
/// match a declared parameter is a different, real error, but not one this check invents a second
/// rule to catch.
#[test]
fn a_named_argument_stops_the_count_entirely() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function two(string $first, string $second): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->two(first: 'x', second: 'y', third: 'z');
    }
}
"#,
        expect![[r#"
            11:45-11:50 error [unknown-named-parameter] Unknown named parameter $third
        "#]],
    );
}

/// A first-class callable builds a `Closure` and calls nothing -- it is not a call with one
/// argument, or with any number of arguments. It is not lowered as a call at all, so it cannot reach
/// either bound.
#[test]
fn a_first_class_callable_is_not_a_call() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $fn = $repo->find(...);
        $fn(1);
    }
}
"#,
        expect![""],
    );
}

/// A class reached through `__call` for a name it never declares is not one this check can count
/// against -- there is no declaration to read a parameter list from. The same silence the
/// undefined-method check gives that name.
#[test]
fn a_method_only_reachable_through_call_is_not_counted() {
    check(
        r#"
//- /app.php
<?php
class Proxy
{
    public function __call(string $name, array $arguments): mixed
    {
        return null;
    }
}

class Caller
{
    public function run(Proxy $proxy): void
    {
        $proxy->whatever(1, 2, 3, 4, 5);
    }
}
"#,
        expect![""],
    );
}

/// `__call` being present does not excuse a method the class *does* declare: that one is reached
/// directly, never through the magic method, so a real declaration still has a real upper bound.
#[test]
fn a_declared_method_is_still_counted_even_with_call_present() {
    check(
        r#"
//- /app.php
<?php
class Proxy
{
    public function known(string $a): void {}

    public function __call(string $name, array $arguments): mixed
    {
        return null;
    }
}

class Caller
{
    public function run(Proxy $proxy): void
    {
        $proxy->known('a', 'b');
    }
}
"#,
        expect![[r#"
            16:28-16:31 error [extra-argument] known() takes at most 1 argument, 2 given
        "#]],
    );
}

/// The declaration that applies is the one the resolved receiver's class actually has, not an
/// interface or parent it happens to satisfy. A receiver typed as the override accepts what the
/// override declares, including a parameter the interface never promised.
#[test]
fn an_override_with_more_parameters_is_counted_against_the_override() {
    check(
        r#"
//- /app.php
<?php
interface Greeter
{
    public function greet(string $name): void;
}

class LoudGreeter implements Greeter
{
    public function greet(string $name, bool $shout = false): void {}
}

class Caller
{
    public function run(LoudGreeter $greeter): void
    {
        $greeter->greet('a', true);
        $greeter->greet('a', true, 'extra');
    }
}
"#,
        expect![[r#"
            17:36-17:43 error [extra-argument] greet() takes at most 2 arguments, 3 given
        "#]],
    );
}

/// A constructor is counted the same way anything else is, promoted parameters included.
#[test]
fn a_constructor_is_counted_too() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function __construct(private int $id) {}
}

class Caller
{
    public function run(): void
    {
        new Repo(1, 2, 3);
    }
}
"#,
        expect![[r#"
            11:21-11:25 error [extra-argument] the constructor takes at most 1 argument, 3 given
        "#]],
    );
}

/// A callee nothing declares is not a call with too many arguments either. Reporting a count
/// against a signature that was never found is exactly the noise `calls_missing_arguments` already
/// refuses to make.
#[test]
fn a_call_whose_callee_is_unknown_says_nothing() {
    check(
        r#"
//- /app.php
<?php
class Caller
{
    public function run($whatever): void
    {
        $whatever->find(1, 2, 3, 4, 5);
    }
}
"#,
        expect![""],
    );
}

/// A body reading `func_get_args()` treats its declared parameters as a lower bound, not a
/// contract: PHP raises nothing when a caller supplies more, and the extra values come back through
/// the call rather than through a name. So nothing is reported here.
///
/// This test used to pin the opposite, with a comment saying the fix needed a fact recorded at
/// lowering time and that a failure here would be progress. The fact exists now --
/// `phrost_sema::MemberRow::reads_own_arguments` -- and what turned the prose's "theoretical risk"
/// into a measured one was typing a closure's parameter: `$pool->throwIf($a, $b)` in Laravel's own
/// tests calls a method declaring one parameter and reading `func_get_args()[1]`, and it was
/// reported the moment `$pool` had a type at all.
#[test]
fn a_body_reading_its_own_arguments_takes_more_than_it_names() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function func_get_args(): array {}

//- /app.php
<?php
function log_it(string $message): void
{
    $all = func_get_args();
    if ($all) {
    }
}

function run(): void
{
    log_it('a', 'b', 'c');
}
"#,
        expect![""],
    );
}

/// A method reached only through a `@mixin` is answered by `__call`, which takes whatever it is
/// given. Laravel's higher-order proxy is `@mixin Enumerable` for the type `->map->` hands back and
/// forwards the call to every element, so `Enumerable::toArray()` taking none says nothing about
/// `->map->toArray($request)`.
#[test]
fn a_method_a_mixin_forwards_takes_any_count() {
    check(
        r#"
//- /app.php
<?php
class Listing
{
    public function toArray(): array { return []; }
}

/** @mixin Listing */
class Proxy
{
    public function __call(string $name, array $arguments): mixed { return null; }
}

function run(Proxy $proxy, Listing $listing): void
{
    $proxy->toArray('request');
    $listing->toArray('request');
}
"#,
        expect![[r#"
            16:23-16:32 error [extra-argument] toArray() takes at most 0 arguments, 1 given
        "#]],
    );
}

/// A `@mixin` forwards calls on an instance through `__call`, and `new` never goes there: a class
/// declaring no constructor takes nothing, whatever its mixin's constructor asks for. PHP runs both
/// `new Pager` lines; the `new Items()` beside them is the control that the check is running.
#[test]
fn a_mixins_constructor_is_not_the_one_new_runs() {
    check(
        r#"
//- /app.php
<?php
class Items
{
    public function __construct(array $items, string $label) {}
}

/** @mixin Items */
class Pager
{
    public function __call(string $name, array $arguments): mixed { return null; }
}

function page(): array
{
    return [new Pager(), new Pager(1, 2, 3), new Items(['a'], 'b', 'c')];
}
"#,
        expect![[r#"
            15:68-15:71 error [extra-argument] the constructor takes at most 2 arguments, 3 given
        "#]],
    );
}
