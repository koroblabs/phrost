//! A method a class is given by a call in a body -- `Bag::macro('total', fn (): int => 0)` -- is
//! stored with the file that makes the call, withdrawn with it, and kept by the cache.
//!
//! Which registrations count is decided where one is used; the index keeps every call to a
//! registering method, whatever class it names.

use expect_test::expect;

use super::{Step, cache::check_round_trip, check_incremental};

const FIXTURE: &str = r#"
//- /src/Provider.php
<?php
namespace App;

use Db\Bag;

class Provider
{
    public function boot(): void
    {
        Bag::macro('total', fn (): int => 0);
        Bag::macro('sorted', function () {
            return $this->sortBy('name')->values();
        });
        Bag::macro('fresh', fn () => static::make());
        Bag::macro('built', fn () => \Db\Factory::make()->bag());
        Bag::macro(NAME, fn (): int => 0);
        Bag::macro('loose', $callable);
    }
}
"#;

/// A written return, a chain off `$this`, one off `static::` and one off a class; a name that is
/// not a literal and a macro that is not a closure are nothing anybody can read.
#[test]
fn each_registration_keeps_what_its_closure_returns() {
    check_incremental(
        FIXTURE,
        &[],
        expect![[r#"
            state ready
            file src/Provider.php [User]
              macro Db\Bag::total() returns int @ 10:21-10:26
              macro Db\Bag::sorted() returns static::sortBy->values @ 11:21-11:27
              macro Db\Bag::fresh() returns static::make @ 14:21-14:26
              macro Db\Bag::built() returns Db\Factory::make->bag @ 15:21-15:26
              class App\Provider @ 6:7-6:15
              use Db\Bag as Bag  [class]
              App\Provider is class
              App\Provider::method boot(): void  public  @ 8:21-8:25 (full 8:5-18:6)
            catalog
              [class] app\provider -> src/Provider.php (App\Provider)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_registration_is_kept_with_the_file_and_withdrawn_with_it() {
    check_incremental(
        FIXTURE,
        &[Step::write(
            "src/Provider.php",
            "<?php\nnamespace App;\n\\Db\\Bag::macro('total', fn (): int => 0);\n",
        )],
        expect![[r#"
            state ready
            file src/Provider.php [User]
              macro Db\Bag::total() returns int @ 3:17-3:22
            catalog
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_registration_survives_the_cache() {
    check_round_trip(FIXTURE);
}
