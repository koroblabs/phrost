//! What a template bound by a parameter stands for at a call that leaves the argument out.
//!
//! The parameter's default value, which is what PHP passes -- unless the template declares a default
//! of its own, and then that. `@template TDefault = never` says the operand is not there at all, and
//! reading `= null` off the parameter beside it answered `?TValue` for a call that cannot return
//! null. Held at the index because the answer is a stored field, and the dump is what `cold == warm`
//! and `incremental == from-scratch` compare.

use expect_test::expect;

use super::check_index;

#[test]
fn a_template_default_wins_over_the_parameter_default() {
    check_index(
        r#"
//- /src/Store.php
<?php
namespace App;

class Store
{
    /**
     * @template TDefault = never
     * @param TDefault $default
     * @return string|TDefault
     */
    public function get(string $key, mixed $default = null): mixed {}

    /**
     * @template TDefault
     * @param TDefault $default
     * @return string|TDefault
     */
    public function plain(string $key, mixed $default = null): mixed {}
}
"#,
        expect![[r#"
            state ready
            file src/Store.php [User]
              class App\Store @ 4:7-4:12
              App\Store is class
              App\Store::method get(string $key, mixed $default = null): mixed  public return names its own template returns one of string | whatever argument 1 is, or never when left out  @ 11:21-11:24 (full 11:5-11:70)
              App\Store::method plain(string $key, mixed $default = null): mixed  public return names its own template returns one of string | whatever argument 1 is, or null when left out  @ 18:21-18:26 (full 18:5-18:72)
            catalog
              [class] app\store -> src/Store.php (App\Store)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}
