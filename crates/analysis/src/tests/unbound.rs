//! A name nothing in its scope binds.
//!
//! Every test here except the first is about the *silence*, and that is not a stylistic choice: PHP
//! creates variables in half a dozen places that do not look like writing, and each one this walk
//! cannot see is a report on correct code. The first measurement on a real project found 70, every
//! one of them a destructuring target.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_name_nothing_binds_is_reported() {
    // What the check is for: two names one letter apart, which is the commonest way to get an
    // undefined variable and the one nothing else here can see.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(int $applicationId): int
    {
        return $aplicationId + 1;
    }
}
"#,
        expect![[r#"
            6:16-6:29 warning [unbound-variable] Nothing gives $aplicationId a value
        "#]],
    );
}

#[test]
fn an_argument_a_callee_might_fill_by_reference_is_bound() {
    // `preg_match` creates `$matches`, and nothing at the call site says so -- whether a parameter
    // is by reference is the callee's business and the callee may be anywhere. So any name passed
    // as an argument counts as bound, which gives up reporting `f($typo)` and keeps every
    // out-parameter in the standard library from being a finding.
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function preg_match(string $pattern, string $subject, array &$matches = null): int|false {}

//- /app.php
<?php
class Service
{
    public function run(string $text): array
    {
        preg_match('/x/', $text, $matches);
        return $matches;
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_destructuring_target_binds_every_name_in_it() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(array $rows): array
    {
        $out = [];
        foreach ($rows as [$key, $value]) {
            $out[$key] = $value;
        }
        ['first' => $first, 'second' => $second] = $out;
        return [$first, $second];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn global_and_static_bind_without_writing() {
    // The two shapes that introduce a name and give it no value here: `global` takes the outer one,
    // `static` keeps whatever the last call left.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): mixed
    {
        global $databases;
        static $seen;
        return [$databases, $seen];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_name_bound_below_where_it_is_read_is_the_positional_checks_finding() {
    // Not this check's: it asks whether anything in the scope binds the name, and something does.
    // The order is `read-before-assignment`'s question, and the two never speak about one name.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): string
    {
        $name;
        $name = 'x';
        return $name;
    }
}
"#,
        expect![[r#"
            6:9-6:14 warning [read-before-assignment] Nothing has given $name a value yet
        "#]],
    );
}

#[test]
fn a_closure_reading_a_captured_name_is_the_outer_scopes_business() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(int $count): callable
    {
        return function () use ($count): int {
            return $count + 1;
        };
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_scope_that_can_reach_a_name_this_cannot_see_is_left_alone() {
    // `extract()` puts an array's keys into the scope as variables, so no answer about any name in
    // it is safe -- the same refusal the unused check makes, for the same reason.
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function extract(array &$array): int {}

//- /app.php
<?php
class Service
{
    public function run(array $values): string
    {
        extract($values);
        return $whatever;
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_property_hooks_parameter_is_not_an_unbound_name() {
    // `set(QuoteId $value) => $this->id = $value` is function-like: the parameter is the hook's, and
    // pulling the body into the file's put a `$value` nothing binds where this check could see it.
    check(
        r#"
//- /src/Offer.php
<?php
namespace App;

class QuoteId {}

class Offer
{
    public function __construct(
        public QuoteId $id {
            get => $this->id;
            set(QuoteId $value) => $this->id = $value;
        },
    ) {}
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_name_compact_spells_and_nothing_binds() {
    // `compact('totl')` reads `$totl` by name, and PHP 8.5.10 warns "compact(): Undefined variable
    // $totl" exactly as it would for writing it. The string is where the typo is.
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function compact(array|string $var_name, array|string ...$var_names): array {}

//- /app.php
<?php
class Service
{
    public function run(): array
    {
        $total = 1;
        return compact('total', 'totl');
    }
}
"#,
        expect![[r#"
            7:34-7:38 warning [unbound-variable] Nothing gives $totl a value
        "#]],
    );
}
