//! A null passed where a template the receiver bound is expected, and what the message names.
//!
//! The judgement was made against the substitution and the sentence rendered the declaration, so
//! Laravel's paginator read `$key takes \Illuminate\Support\TKey, and this may be null` -- a class
//! nobody can go and look at. `incompatible-argument` already named the substitution; both now go
//! through one rendering.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_null_against_a_bound_template_names_what_it_was_bound_to() {
    check(
        r#"
//- /app.php
<?php
/**
 * @template TKey of array-key
 * @template TValue
 */
class Coll
{
    /** @param TKey $key */
    public function put($key): void {}
}

class Holder
{
    /** @var Coll<string, int> */
    private Coll $items;

    public function run(?string $key): void
    {
        $this->items->put($key);
    }
}
"#,
        expect![[r#"
            19:27-19:31 warning [possibly-null-argument] $key takes string, and this may be null
        "#]],
    );
}

/// A template inside a union is one no substitution can put back as a single type, and the hint
/// beside it is what is judged then: `ReflectionClass` declares `@param class-string<T>|T` beside
/// `object|string`, and `new ReflectionClass($maybeNull)` was silent once a docblock naming a class
/// beside `object` began to win. Bound or not, the null is still the runtime's `TypeError`.
#[test]
fn a_null_against_a_template_inside_a_union_is_judged_by_the_hint() {
    check(
        r#"
//- /app.php
<?php
class Foo {}

/** @template T of object */
class Box
{
    /** @param class-string<T>|T $x */
    public function __construct(object|string $x) {}

    /** @param class-string<T>|T $x */
    public function put(object|string $x): void {}
}

/** @param Box<Foo> $box */
function run(?string $name, ?Foo $foo, Box $box): void
{
    new Box($name);
    $box->put($foo);
}
"#,
        expect![[r#"
            17:13-17:18 warning [possibly-null-argument] $x takes object|string, and this may be null
            18:15-18:19 warning [possibly-null-argument] $x takes object|string, and this may be null
        "#]],
    );
}
