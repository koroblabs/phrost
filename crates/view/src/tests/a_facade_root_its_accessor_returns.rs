//! A facade whose accessor returns a class: its static calls are that class's methods.
//!
//! An application's own facade rarely writes `@method` tags -- `getFacadeAccessor()` returning
//! `ThemeService::class` is the only statement of what `Theme::dispatch()` reaches. Read as the
//! `@mixin ThemeService` it amounts to; an accessor returning a string names a container binding and
//! is left alone.

use expect_test::expect;

use phrost_harness::World;
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

const FIXTURE: &str = r#"
//- /vendor/Facade.php
<?php
namespace Orm;

abstract class Facade
{
    public static function __callStatic(string $method, array $args): mixed {}
}

//- /src/Theme.php
<?php
namespace App;

class Event {}

class ThemeService
{
    public function dispatch(string $name): Event {}
}

class Theme extends \Orm\Facade
{
    protected static function getFacadeAccessor()
    {
        return ThemeService::class;
    }
}

class Bound extends \Orm\Facade
{
    protected static function getFacadeAccessor()
    {
        return 'theme';
    }
}

function run(): void
{
    $d@dispatched@ispatched = Theme::dispatch('login');
    $b@bound@ound = Bound::dispatch('login');
    Theme::dis@goto@patch('login');
}
"#;

#[test]
fn a_facade_call_reaches_the_class_its_accessor_returns() {
    let spec = FixtureSpec::parse(FIXTURE);
    // Before the files, because lowering reads the accessor only when told to.
    let world = World::build_with(&spec, |world| {
        world.set_mixin_methods(&["getFacadeAccessor"])
    });
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("what the root's method returns", "dispatched"),
            Query::variable_type("an accessor naming a binding, not read", "bound"),
            Query::goto("goto, the root's method", "goto"),
        ],
        expect![[r#"
            what the root's method returns
            - Event
            an accessor naming a binding, not read
            - mixed
            goto, the root's method
            - method App\ThemeService::dispatch @ src/Theme.php 8:21-8:29
        "#]],
    );
}
