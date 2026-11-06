//! `$grade?->getMaxDiscount()` out of a method declared `: float`.
//!
//! `?->` makes the *whole expression* null when the receiver is, and nothing carried that: the
//! chain resolved to whatever the last call declared, so a nullsafe result handed to a `return`, an
//! argument or a `foreach` was judged as if the null could not arrive. Three real ones on the
//! reference project, including `return User::load($id)?->hasPermission($p);` out of a `: bool`.
//!
//! What it must not do is report the chain itself. PHP short-circuits the rest of a chain rather
//! than reaching through the null -- `$a?->b()->c()` never calls `c()`, and `$a?->b()['k']` never
//! indexes -- so a receiver and a subscript base are resolved with the flag cleared, and only
//! whoever *receives* the expression sees the null.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_nullsafe_result_is_judged_where_the_expression_lands() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Grade
{
    public function maxLtv(): float
    {
        return 1.0;
    }
}

class Grid
{
    public function grade(): ?Grade
    {
        return null;
    }

    public function best(): float
    {
        return $this->grade()?->maxLtv();
    }
}
"#,
        expect![[r#"
            21:16-21:41 warning [possibly-null-return] ?float returned where float is declared
        "#]],
    );
}

/// The chain itself is not at risk, because PHP stops at the `?->` rather than calling through it.
#[test]
fn the_rest_of_the_chain_is_not_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Leaf
{
    public function name(): string
    {
        return 'n';
    }

    public function rows(): array
    {
        return [];
    }
}

class Mid
{
    public function leaf(): Leaf
    {
        return new Leaf();
    }
}

class Root
{
    public function mid(): ?Mid
    {
        return null;
    }

    public function read(): mixed
    {
        return $this->mid()?->leaf()->rows()['k'];
    }
}
"#,
        expect![""],
    );
}

/// `mixed` already admits null, and a `?mixed` reported a `return $options ?? [];` that handles it.
#[test]
fn a_nullsafe_call_returning_mixed_stays_mixed() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Storage
{
    public function setting(string $key): mixed
    {
        return null;
    }
}

class Host
{
    public function storage(): ?Storage
    {
        return null;
    }

    public function options(): array
    {
        $options = $this->storage()?->setting('allowed');

        return $options ?? [];
    }
}
"#,
        expect![""],
    );
}
