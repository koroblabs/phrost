//! A conditional return whose then-branch is a template bound by an argument's *keys*.
//!
//! PHP 8.5's `array_key_first` is written `($array is non-empty-array|non-empty-list ? K : null)`
//! beside `@param array<K, mixed> $array`. A branch was reduced to an argument only when its template
//! named the elements, so this one was refused, the conditional went with it, and what was stored
//! was the agreed `K` or null -- which no guard above a call and no literal holding something could
//! take the null out of. `array_first`, the same declaration spelled with a `V`, kept its test.

use expect_test::expect;

use super::check_index;

#[test]
fn a_template_bound_by_the_keys_keeps_the_test() {
    check_index(
        r#"
//- /src/Arrays.php
<?php
namespace App;

class Arrays
{
    /**
     * @template K as array-key
     * @param array<K, mixed> $array
     * @return ($array is non-empty-array|non-empty-list ? K : null)
     */
    public static function keyFirst(array $array): string|int|null {}

    /**
     * @template V
     * @param array<array-key, V> $array
     * @return ($array is non-empty-array|non-empty-list ? V : null)
     */
    public static function valueFirst(array $array): mixed {}
}
"#,
        expect![[r#"
            state ready
            file src/Arrays.php [User]
              class App\Arrays @ 4:7-4:13
              App\Arrays is class
              App\Arrays::method keyFirst(array $array): string|int|null  public static returns, if argument 0 is not empty, what names a slot of argument 0 and otherwise null  @ 11:28-11:36 (full 11:5-11:70)
              App\Arrays::method valueFirst(array $array): mixed  public static returns, if argument 0 is not empty, what argument 0 holds and otherwise null  @ 18:28-18:38 (full 18:5-18:62)
            catalog
              [class] app\arrays -> src/Arrays.php (App\Arrays)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}
