//! A conditional branch with a template inside it that no argument binds.
//!
//! `substr_replace` is `($string is string ? string : array<K, string>)` beside `@template K as
//! array-key`, and a branch mentioning a template was refused whole -- so the conditional went, and a
//! call on a string read as the declared `array|string`. The template stands for its bound now. A
//! bare template and an unbounded one are still refused: the first is a value of the template's type,
//! which the bound does not describe, and the second has nothing to stand for. So is one a
//! `class-string<T>` binds, because refusing is what hands it to the call-site substitution, which
//! answers the class the call names rather than the bound.

use expect_test::expect;

use super::check_index;

#[test]
fn a_template_inside_a_branch_stands_for_its_bound() {
    check_index(
        r#"
//- /src/Strings.php
<?php
namespace App;

class Strings
{
    /**
     * @template K as array-key
     * @param string|array<K, string> $string
     * @return ($string is string ? string : array<K, string>)
     */
    public static function replace(array|string $string): array|string {}

    /**
     * @template T of object
     * @param mixed $value
     * @return ($value is string ? string : T)
     */
    public static function bare(mixed $value): mixed {}

    /**
     * @template T of object
     * @param class-string<T> $class
     * @return ($class is null ? list<object> : list<T>)
     */
    public static function named(?string $class): array {}

    /**
     * @template K
     * @param string|array<K, string> $string
     * @return ($string is string ? string : array<K, string>)
     */
    public static function unbounded(array|string $string): array|string {}
}
"#,
        expect![[r#"
            state ready
            file src/Strings.php [User]
              class App\Strings @ 4:7-4:14
              App\Strings is class
              App\Strings::method replace(array|string $string): string|array<mixed, string>  public static returns, if argument 0 is a string, string and otherwise array<int|string, string>  @ 11:28-11:35 (full 11:5-11:74)
              App\Strings::method bare(mixed $value): mixed  public static  @ 18:28-18:32 (full 18:5-18:56)
              App\Strings::method named(?string $class): array<int, object>|array<int, mixed>  public static  @ 25:28-25:33 (full 25:5-25:59)
              App\Strings::method unbounded(array|string $string): string|array<mixed, string>  public static returns, if argument 0 is a string, string and otherwise array  @ 32:28-32:37 (full 32:5-32:76)
            catalog
              [class] app\strings -> src/Strings.php (App\Strings)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}
