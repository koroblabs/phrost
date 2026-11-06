//! A method a class is given by a call in somebody's body: Laravel's `Collection::macro(...)`.
//!
//! The closure is the method, so what it hands back is what a call to the macro does: the return
//! it writes, with `static` the class it is called on, or else the chain its one `return` builds
//! off `$this`. The registration counts on the class it names and below it, and only where that
//! class takes registrations -- a `macro` method on a class that does not use the registering
//! trait is somebody else's method of that name.

use expect_test::expect;

use phrost_harness::World;
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

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

    /** @return static */
    public function sortBy(callable $by) {}
}

class Helper
{
    public static function sorted(Collection $items, callable|string $by): Collection {}
}

class Plain
{
    public static function macro(string $name, \Closure $build): void {}
}

//- /src/Provider.php
<?php
namespace App;

use Support\Collection;
use Support\Plain;

class Provider
{
    public function boot(): void
    {
        Collection::macro('sortByName', function () {
            return $this->sortBy(fn ($row) => $row->name);
        });
        Collection::macro('countAll', function (): int {
            return 0;
        });
        Collection::macro('again', fn (): static => $this);
        Collection::macro('helped', function (callable|string $by) {
            /** @var Collection */
            $items = $this;

            return \Support\Helper::sorted($items, $by);
        });
        Plain::macro('nope', function (): int {
            return 1;
        });
    }
}

//- /src/use.php
<?php
namespace App;

use Support\Collection;
use Support\Plain;

class Rows extends Collection {}

function run(Collection $rows, Rows $mine, Plain $plain): void
{
    $s@sorted@orted = $rows->sortByName();
    $c@counted@ounted = $rows->countAll();
    $a@again@gain = $mine->again();
    $h@helped@elped = $rows->helped('name');
    $n@nope@ope = $plain->nope();
    $rows->sortBy@goto@Name();
}
"#;

fn rules(world: &mut World) {
    world
        .contracts_mut()
        .bind_macro_registrar("Support\\Macroable", "macro");
}

#[test]
fn a_macro_answers_on_the_class_it_was_registered_on() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, rules);
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("a chain the closure returns off $this", "sorted"),
            Query::variable_type("a return the closure writes", "counted"),
            Query::variable_type("static, on a subclass", "again"),
            Query::variable_type("a chain off another class", "helped"),
            Query::variable_type("a class that does not take registrations", "nope"),
            Query::goto("goto, the name the registration writes", "goto"),
        ],
        expect![[r#"
            a chain the closure returns off $this
            - Collection
            a return the closure writes
            - int
            static, on a subclass
            - Rows
            a chain off another class
            - Collection
            a class that does not take registrations
            - (no type)
            goto, the name the registration writes
            - method Support\Collection::sortByName @ src/Provider.php 11:28-11:38
        "#]],
    );
}
