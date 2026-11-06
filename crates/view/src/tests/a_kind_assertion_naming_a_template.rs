//! `@phpstan-assert iterable<T> $haystack` -- a kind assertion whose argument is a template.
//!
//! PHPUnit's `assertContainsOnlyInstancesOf` is written this way, with `T` bound by the class-string
//! the call names. Nothing substitutes that here, so the tag's `T` is a class nobody declares, and
//! taken as written every element read off the value was one. The kind still holds, so what the
//! value already had of it stands.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn what_the_value_held_stands_beside_a_template_the_tag_cannot_fill() {
    check_view(
        r#"
//- /src/run.php
<?php
namespace App;

class File {}

class Asserts
{
    /**
     * @template T
     * @param class-string<T> $class
     * @param iterable<mixed> $haystack
     * @phpstan-assert iterable<T> $haystack
     */
    public static function allOf(string $class, iterable $haystack): void {}
}

/** @param list<File> $files */
function run(array $files): void
{
    Asserts::allOf(File::class, $files);
    $f@here@iles;
}
"#,
        &[Query::variable_type("after the assertion", "here")],
        expect![[r#"
            after the assertion
            - array<int, File>
        "#]],
    );
}
