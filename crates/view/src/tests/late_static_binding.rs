//! What `static` means for a member reached off `parent::`.
//!
//! PHP calls `parent::`, `self::` and `static::` *forwarding* calls: they leave late static binding
//! alone, so a `: static` return travels through them as the class the code is written in. Only an
//! explicit `Foo::m()` resets it.
//!
//! Its own file because it is one rule with one exception, and because getting it wrong is invisible
//! until a whole framework's factory idiom reports as broken: every Drupal form, plugin and
//! controller is written as `$instance = parent::create($container); $instance->thing = ...`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_static_return_through_parent_is_the_class_that_called_it() {
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public static function create(): static
    {
        return new static();
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public static function create(): static
    {
        $instance = parent::create();
        $inst@here@ance;
        return $instance;
    }
}
"#,
        &[Query::variable_class("through parent", "here", "instance")],
        expect![[r#"
            through parent
            - App\Child
        "#]],
    );
}

/// `self::` needs no rule of its own -- the class it looks up on is already the enclosing one -- and
/// this pins that it stays that way.
#[test]
fn a_static_return_through_self_is_the_enclosing_class() {
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public static function make(): static
    {
        return new static();
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public static function create(): static
    {
        $instance = self::make();
        $inst@here@ance;
        return $instance;
    }
}
"#,
        &[Query::variable_class("through self", "here", "instance")],
        expect![[r#"
            through self
            - App\Child
        "#]],
    );
}

/// The exception: an explicit class name is not a forwarding call, so `static` is that class.
///
/// The half that keeps the rule honest. `Base::create()` written out returns a `Base`, and answering
/// `Child` there would offer members the value does not have.
#[test]
fn a_static_return_through_a_named_class_is_that_class() {
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public static function create(): static
    {
        return new static();
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public static function build(): void
    {
        $instance = Base::create();
        $inst@here@ance;
    }
}
"#,
        &[Query::variable_class("named outright", "here", "instance")],
        expect![[r#"
            named outright
            - App\Base
        "#]],
    );
}

/// The forwarding belongs to the call written on `parent::` and not to what it returned.
///
/// `parent::repo()->create()` is two calls: the first forwards, the second is made on whatever the
/// first gave back. Carrying the forwarding into the second would name the wrong class for every
/// fluent chain that starts at a parent.
#[test]
fn the_forwarding_does_not_travel_past_the_first_call() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public static function spawn(): static
    {
        return new static();
    }
}

//- /src/Base.php
<?php
namespace App;

class Base
{
    public static function repo(): Repo
    {
        return new Repo();
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public static function build(): void
    {
        $spawned = parent::repo()->spawn();
        $spaw@here@ned;
    }
}
"#,
        &[Query::variable_class("second call", "here", "spawned")],
        expect![[r#"
            second call
            - App\Repo
        "#]],
    );
}
