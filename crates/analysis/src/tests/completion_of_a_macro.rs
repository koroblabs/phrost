//! A macro is offered beside the methods its class declares, with what it returns.
//!
//! See the view's `a_method_a_macro_registration_gives` for which registrations count.

use expect_test::expect;

use super::{Query, check_with};

const FIXTURE: &str = r#"
//- /vendor/Support.php
<?php
namespace Support;

trait Macroable
{
    public static function macro(string $name, object|callable $macro): void {}
    public function __call($method, $parameters): mixed {}
}

class Collection
{
    use Macroable;

    public function sortBy(callable $by): static {}
}

//- /src/Provider.php
<?php
namespace App;

use Support\Collection;

class Provider
{
    public function boot(): void
    {
        Collection::macro('sortByName', function () {
            return $this->sortBy(fn ($row) => $row->name);
        });
        Collection::macro('sortByAge', function (): int {
            return 0;
        });
    }
}

//- /src/use.php
<?php
namespace App;

function run(\Support\Collection $rows): void
{
    $rows->sortB@here@
}
"#;

#[test]
fn a_macro_is_offered_with_what_it_returns() {
    check_with(
        FIXTURE,
        &[Query::completion("beside the declared method", "here")],
        |world| {
            world
                .contracts_mut()
                .bind_macro_registrar("Support\\Macroable", "macro");
        },
        expect![[r#"
            beside the declared method
            - sortBy [method]  : static  writes `(|);`
            - sortByName [method]  : Collection  writes `(|);`
            - sortByAge [method]  : int  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$rows->sortBy`
        "#]],
    );
}
