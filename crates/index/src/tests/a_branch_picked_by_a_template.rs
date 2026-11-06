//! A conditional return whose subject is a `@template` a parameter is declared as.
//!
//! `(T is 'array' ? ... : ...)` beside `@param T $type` is `($type is 'array' ? ...)` written the
//! other way, and Symfony's `ArrayNodeDefinition::prototype()` -- called by every configuration tree
//! with a list in it -- is written so. Only the `$parameter` spelling was read, so the branch was
//! never picked. Held at the index because the contract is stored, and both invariants compare the
//! dump.

use expect_test::expect;

use super::check_index;

#[test]
fn a_template_a_parameter_binds_picks_the_branch() {
    check_index(
        r#"
//- /src/Def.php
<?php
namespace App;

class ArrDef {}
class ScalarDef {}

class Def
{
    /**
     * @template T of 'array'|'scalar'
     * @param T $type
     * @return (T is 'array' ? ArrDef : ScalarDef)
     */
    public function prototype(string $type): object {}

    /**
     * @template T
     * @param T $value
     * @return (T is null ? ScalarDef : ArrDef)
     */
    public function tested(mixed $value): object {}
}
"#,
        expect![[r#"
            state ready
            file src/Def.php [User]
              class App\ArrDef @ 4:7-4:13
              class App\ScalarDef @ 5:7-5:16
              class App\Def @ 7:7-7:10
              App\ArrDef is class
              App\ScalarDef is class
              App\Def is class
              App\Def::method prototype(string $type): ArrDef|ScalarDef  public returns, if argument 0 is 'array', \App\ArrDef and otherwise \App\ScalarDef  @ 14:21-14:30 (full 14:5-14:55)
              App\Def::method tested(mixed $value): ScalarDef|ArrDef  public returns, if argument 0 is null, \App\ScalarDef and otherwise \App\ArrDef  @ 21:21-21:27 (full 21:5-21:52)
            catalog
              [class] app\arrdef -> src/Def.php (App\ArrDef)
              [class] app\def -> src/Def.php (App\Def)
              [class] app\scalardef -> src/Def.php (App\ScalarDef)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
        "#]],
    );
}
