//! `$x['k'] = $v` on a parameter declared as a class, which is `offsetSet` and not a new array.
//!
//! An element write with nothing else assigning the name is what *creates* an array in PHP, and a
//! fallback read that as the variable's type -- including where a parameter declared it first. So
//! `$request['id'] = 5;` on a `WP_REST_Request $request` made every later use of `$request` an
//! array: 19 WordPress findings, a `WeakMap` in Symfony's `ObjectMapper`, and a `string` whose
//! character was written by offset.
//!
//! A parameter that may be an array keeps the rule, and so does an untyped one -- `?array $options
//! = null` is an array after `$options['x'] = 1`, and a Drupal hook's `&$form` says so in no other
//! way.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_declaration_stands_unless_it_admits_an_array() {
    check(
        r#"
//- /src/app.php
<?php

// Declared here because a fixture does not index the stubs.
interface ArrayAccess
{
}

class Bag implements ArrayAccess
{
    public function offsetExists(mixed $offset): bool
    {
        return true;
    }

    public function offsetGet(mixed $offset): mixed
    {
        return null;
    }

    public function offsetSet(mixed $offset, mixed $value): void
    {
    }

    public function offsetUnset(mixed $offset): void
    {
    }

    public function count(): int
    {
        return 0;
    }
}

function takes_array(array $a): void
{
}

function run(Bag $bag, ?array $options = null): int
{
    $bag['k'] = 1;
    $options['x'] = 1;
    takes_array($options);
    return $bag->count();
}
"#,
        expect![""],
    );
}
