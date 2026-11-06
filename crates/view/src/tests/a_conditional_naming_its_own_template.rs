//! A conditional return whose branches name the declaration's own `@template`.
//!
//! WordPress's `wp_unslash()` is `(T is string ? string : (T is array ? array<key-of<T>, ...> : T))`
//! and Laravel's `Arr::wrap()` is `($value is null ? array{} : ($value is array ? array<TKey, TValue>
//! : array{TValue}))`. A template names no class, so the tag was refused whole -- and with it the
//! contract, over one branch nothing could take apart -- and both answered nothing on every call.
//!
//! Two readings now. The contract keeps a branch it cannot take apart as the container it is
//! (`array`), so a call on a string is a string. And a call it cannot settle gets the declaration,
//! read with every template standing for `mixed`: true of every call, where the bound would be a
//! claim about this one that nobody made.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_template_in_a_branch_stands_for_anything() {
    check_view(
        r#"
//- /src/functions.php
<?php
/**
 * @return string|array
 *
 * @phpstan-template T
 * @phpstan-param T $value
 * @phpstan-return (T is string ? string : (T is array ? array<key-of<T>, mixed> : T))
 */
function wp_unslash( $value ) {}

/**
 * @template TKey of array-key
 * @template TValue
 * @param array<TKey, TValue>|TValue|null $value
 * @return ($value is null ? array{} : ($value is array ? array<TKey, TValue> : array{TValue}))
 */
function wrap( $value ) {}

/**
 * @template TValue of array<array|scalar>|scalar|null
 * @param TValue $value
 * @return (TValue is scalar ? array|scalar : array<array|scalar>)
 */
function resolve( $value ) {}

function run( $unknown, string $text, array $rows ) {
    $a = wp_unslash( $unknown );
    $@a@a;
    $b = wp_unslash( $text );
    $@b@b;
    $c = wp_unslash( $rows );
    $@c@c;
    $d = wrap( $text );
    $@d@d;
    $e = wrap( null );
    $@e@e;
    $f = resolve( $rows );
    $@f@f;
    $g = resolve( $text );
    $@g@g;
}
"#,
        &[
            Query::variable_type("nothing settles it, so anything", "a"),
            Query::variable_type("a string is the first branch", "b"),
            Query::variable_type("an array is the container the branch names", "c"),
            Query::variable_type("a string wrapped", "d"),
            Query::variable_type("null wrapped", "e"),
            Query::variable_type("an array is not a scalar", "f"),
            Query::variable_type("a string is", "g"),
        ],
        expect![[r#"
            nothing settles it, so anything
            - mixed
            a string is the first branch
            - string
            an array is the container the branch names
            - array
            a string wrapped
            - array
            null wrapped
            - array{}
            an array is not a scalar
            - array<array|int|float|string|bool>
            a string is
            - array|int|float|string|bool
        "#]],
    );
}
