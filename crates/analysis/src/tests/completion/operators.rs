//! Which members an access operator can reach.
//!
//! `->` and `::` reach different sets, and what a receiver has *earned* decides whether private
//! members are among them. Offering one that PHP would refuse suggests code that does not run.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn this_offers_the_classes_own_members_including_private_ones() {
    // The one receiver whose type is free: `$this` is the enclosing class, no inference needed.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    private array $cache = [];

    public function find(int $id): void {}

    private function normalise(string $raw): string {}

    public function run(): void
    {
        $this->@here@
    }
}
"#,
        &[Query::completion("members", "here")],
        expect![[r#"
            members
            - find [method]  : void  writes `(|);`
            - normalise [method]  : string  writes `(|);`
            - run [method]  : void  writes `();`
            - cache [property]  array
            - accepting the first leaves `$this->find`
        "#]],
    );
}

#[test]
fn an_arrow_offers_a_static_method_but_not_a_static_property_or_a_constant() {
    // The asymmetry is PHP's. `$obj->staticMethod()` resolves the method on the class and runs it,
    // and Drupal code reaches `create()` that way constantly. `$obj->CONST` is not PHP at all, and
    // `$obj->staticProperty` reads an instance property that does not exist -- offering either
    // would suggest code that does not run.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public const string VERSION = '1';
    public static array $registry = [];
    public static function make(): static {}
    public function find(): void {}

    public function run(): void
    {
        $this->@here@
    }
}
"#,
        &[Query::completion("instance only", "here")],
        expect![[r#"
            instance only
            - find [method]  : void  writes `();`
            - make [method]  : static  writes `();`
            - run [method]  : void  writes `();`
            - accepting the first leaves `$this->find`
        "#]],
    );
}

#[test]
fn a_double_colon_offers_statics_constants_and_methods() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public const string VERSION = '1';
    public static array $registry = [];
    public function find(): void {}

    public function run(): void
    {
        self::@here@
    }
}
"#,
        &[Query::completion("static side", "here")],
        expect![[r#"
            static side
            - find [method]  : void  writes `();`
            - run [method]  : void  writes `();`
            - $registry [property]  array
            - VERSION [const]  string
            - class [const]  App\Repo
            - accepting the first leaves `self::find`
        "#]],
    );
}

#[test]
fn parent_offers_the_parents_members_but_not_its_private_ones() {
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    protected function shared(): void {}
    private function hidden(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    public function run(): void
    {
        parent::@here@
    }
}
"#,
        &[Query::completion("parent", "here")],
        expect![[r#"
            parent
            - shared [method]  : void  writes `();`
            - class [const]  App\Base
            - accepting the first leaves `parent::shared`
        "#]],
    );
}

#[test]
fn private_members_are_reachable_through_this() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    private Repo $twin;

    public function open(): void {}

    private function hidden(): void {}

    public function run(): void
    {
        $this->@here@
    }
}
"#,
        &[Query::completion("through this", "here")],
        expect![[r#"
            through this
            - hidden [method]  : void  writes `();`
            - open [method]  : void  writes `();`
            - run [method]  : void  writes `();`
            - twin [property]  Repo
            - accepting the first leaves `$this->hidden`
        "#]],
    );
}

#[test]
fn private_members_are_not_reachable_through_a_property_of_the_same_class() {
    // A different object of the same class. PHP does allow reaching its privates, but knowing that
    // means comparing the receiver's class to the enclosing one, which this does not do yet -- and
    // offering too few is the direction that does not put an illegal call in the file.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    private Repo $twin;

    public function open(): void {}

    private function hidden(): void {}

    public function run(): void
    {
        $this->twin->@here@
    }
}
"#,
        &[Query::completion("through a property", "here")],
        expect![[r#"
            through a property
            - hidden [method]  : void  writes `();`
            - open [method]  : void  writes `();`
            - run [method]  : void  writes `();`
            - twin [property]  Repo
            - accepting the first leaves `$this->twin->hidden`
        "#]],
    );
}

#[test]
fn a_static_property_is_offered_with_the_sigil_it_is_written_with() {
    // `Repo::$registry` is how PHP reaches it, and a list showing `registry` offers something that
    // does not parse where it would be accepted.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public static array $registry = [];

    public function run(Repo $repo): void
    {
        $repo::$re@here@;
    }
}
"#,
        &[Query::completion("the sigil already typed", "here")],
        expect![[r#"
            the sigil already typed
            - $registry [property]  array
            - ... and the client must ask again
            - accepting the first leaves `$repo::$registry;`
        "#]],
    );
}

#[test]
fn an_enum_case_reached_through_the_double_colon_is_an_instance_of_its_enum() {
    // `Suit::Hearts` is stored as an enum case and not as a property, and the text gives no way to
    // tell it from `Config::DEFAULT`. Both tables are asked, so both spellings resolve.
    check(
        r#"
//- /src/Suit.php
<?php
namespace App;

enum Suit: string
{
    case Hearts = 'hearts';

    public function colour(): string { return ''; }
}

class Deck
{
    public function run(): void
    {
        Suit::Hearts->@here@
    }
}
"#,
        &[Query::completion("a case is an instance", "here")],
        expect![[r#"
            a case is an instance
            - colour [method]  : string  writes `();`
            - accepting the first leaves `Suit::Hearts->colour`
        "#]],
    );
}

#[test]
fn a_static_method_on_an_unrelated_instance_is_offered_last() {
    // The same argument this file already answered about magic methods, a second time. Through
    // `->` the engine looks a static method up on the class and calls it, with no error and no
    // deprecation -- so it is PHP, and the complaint against suggesting it is about crowding.
    // Ranking answers that; leaving it out answered a question nobody asked, and two fixtures in a
    // borrowed corpus said so.
    check(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Factory
{
    public function run(): void {}

    public static function make(): static
    {
        return new static();
    }
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function use(Factory $factory): void
    {
        $factory->@here@
    }
}
"#,
        &[Query::completion("on an unrelated instance", "here")],
        expect![[r#"
            on an unrelated instance
            - make [method]  : static  shown last  writes `();`
            - run [method]  : void  writes `();`
            - accepting the first leaves `$factory->run`
        "#]],
    );
}
