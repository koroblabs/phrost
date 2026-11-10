//! Which members a receiver can reach, which is a question about the *class* the cursor is in.
//!
//! PHP scopes visibility per class, not per object: inside `Repo`, another `Repo` instance's private
//! members are reachable, and the parent's private ones are not even through `$this`. Getting that
//! wrong in either direction is visible immediately -- a list that offers what will not compile, or
//! one that hides what the author is reaching for.
//!
//! One marker per method throughout. A completion marker leaves a dangling `->`, which does not
//! parse; mago recovers, but statements after the break are dropped, so two markers in one method
//! silently test only the first. This file was written wrong that way once and reported two defects
//! that were not there.

use expect_test::expect;

use super::{Query, check};

/// The class pair every test here asks about, with one marker filled in per test.
fn classes(inside: &str, outside: &str) -> String {
    format!(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{{
    public int $openBase = 0;
    protected int $guardedBase = 0;
    private int $ownBase = 0;

    public function openOnBase(): void {{}}
    protected function guardedOnBase(): void {{}}
    private function ownOnBase(): void {{}}
}}

class Repo extends Base
{{
    public int $open = 0;
    protected int $guarded = 0;
    private int $own = 0;

    public function inside(Repo $sibling, Base $parentTyped): void
    {{
        {inside}
    }}
}}

class Outsider
{{
    public function outside(Repo $repo): void
    {{
        {outside}
    }}
}}
"#
    )
}

#[test]
fn this_reaches_its_own_private_and_its_parents_protected() {
    // Not the parent's private: PHP scopes private to the class that wrote it, so a subclass cannot
    // see it even through `$this`.
    check(
        &classes("$this->@here@", ""),
        &[Query::completion("through $this", "here")],
        expect![[r#"
            through $this
            - guardedOnBase [method]  : void  writes `();`
            - inside [method]  : void  writes `(|);`
            - openOnBase [method]  : void  writes `();`
            - guarded [property]  int
            - guardedBase [property]  int
            - open [property]  int
            - openBase [property]  int
            - own [property]  int
            - accepting the first leaves `$this->guardedOnBase`
        "#]],
    );
}

#[test]
fn another_instance_of_the_same_class_reaches_its_private_members() {
    // PHP's rule, and the one this is most likely to get wrong: visibility is per class, not per
    // object. `$sibling->own` inside `Repo` is legal.
    check(
        &classes("$sibling->@here@", ""),
        &[Query::completion("a sibling instance", "here")],
        expect![[r#"
            a sibling instance
            - guardedOnBase [method]  : void  writes `();`
            - inside [method]  : void  writes `(|);`
            - openOnBase [method]  : void  writes `();`
            - guarded [property]  int
            - guardedBase [property]  int
            - open [property]  int
            - openBase [property]  int
            - own [property]  int
            - accepting the first leaves `$sibling->guardedOnBase`
        "#]],
    );
}

#[test]
fn an_instance_typed_as_the_parent_reaches_its_protected_members() {
    check(
        &classes("$parentTyped->@here@", ""),
        &[Query::completion("typed as the parent", "here")],
        expect![[r#"
            typed as the parent
            - guardedOnBase [method]  : void  writes `();`
            - openOnBase [method]  : void  writes `();`
            - guardedBase [property]  int
            - openBase [property]  int
            - accepting the first leaves `$parentTyped->guardedOnBase`
        "#]],
    );
}

#[test]
fn a_receiver_in_another_class_reaches_only_public_members() {
    check(
        &classes("", "$repo->@here@"),
        &[Query::completion("from another class entirely", "here")],
        expect![[r#"
            from another class entirely
            - inside [method]  : void  writes `(|);`
            - openOnBase [method]  : void  writes `();`
            - open [property]  int
            - openBase [property]  int
            - accepting the first leaves `$repo->inside`
        "#]],
    );
}

/// A class with most of PHP's magic methods on it, and one ordinary member to prove the list works.
fn magical(call: &str) -> String {
    format!(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{{
    public function __construct(private int $id) {{}}
}}

class Repo extends Base
{{
    public function __get(string $name): mixed
    {{
        return null;
    }}

    public function __set(string $name, mixed $value): void {{}}

    public function __toString(): string
    {{
        return '';
    }}

    public function __clone(): void {{}}

    public function __call(string $name, array $arguments): mixed
    {{
        return null;
    }}

    public static function __callStatic(string $name, array $arguments): mixed
    {{
        return null;
    }}

    public function find(): void {{}}

    public function build(): void
    {{
        {call}
    }}
}}
"#
    )
}

/// A magic method is offered, and shown after the rest.
///
/// It used to be left out. The argument was that PHP calls these rather than a person, and it is
/// wrong for four of them: `->__toString()` is written by hand 51 times in Drupal core and 59 in
/// Symfony, `->__invoke()` 33 and 18, `->__construct()` and `->__serialize()` seventeen and fifteen.
/// They are members of the language and a person reaching for one is entitled to find it.
///
/// The complaint that produced the old rule was about **crowding** -- a class with a handful of them
/// pushing the wanted members off the visible part of the list -- and that is what the ranking is
/// for, and `shown last` in the rendering below is where it is asserted -- the list prints
/// alphabetically, so order here would say nothing.
#[test]
fn a_magic_method_is_offered_last_rather_than_left_out() {
    check(
        &magical("$this->@here@"),
        &[Query::completion("through $this", "here")],
        expect![[r#"
            through $this
            - __call [method]  : mixed  shown last  writes `(|);`
            - __callStatic [method]  : mixed  shown last  writes `(|);`
            - __clone [method]  : void  shown last  writes `();`
            - __construct [method]  shown last  writes `(|);`
            - __get [method]  : mixed  shown last  writes `(|);`
            - __set [method]  : void  shown last  writes `(|);`
            - __toString [method]  : string  shown last  writes `();`
            - build [method]  : void  writes `();`
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->build`
        "#]],
    );
}

/// `parent::__construct(...)` is ordinary PHP and is written by hand constantly.
///
/// It used to be the *one* magic method that stayed, and the only place it stayed. Now every one of
/// them is offered and this position is no longer special -- what it still shows is that a
/// constructor reached relatively is reachable at all, where `$obj->__construct()` is filtered by
/// `reachable` rather than demoted.
#[test]
fn a_constructor_stays_when_it_is_reached_relatively() {
    check(
        &magical("parent::@here@"),
        &[Query::completion("through parent", "here")],
        expect![[r#"
            through parent
            - __construct [method]  shown last  writes `(|);`
            - class [const]  App\Base
            - accepting the first leaves `parent::class`
        "#]],
    );
}

/// The same through the class's own name, written inside it -- which is `self` spelled out, so
/// every method is reachable and the magic ones are still last. Through the name of a class the
/// code does not run as, only the static ones are reachable at all: see
/// `completion::the_enclosing_class_by_name`.
#[test]
fn a_magic_method_reached_statically_is_offered_last_too() {
    check(
        &magical("Repo::@here@"),
        &[Query::completion("through the class name", "here")],
        expect![[r#"
            through the class name
            - __call [method]  : mixed  shown last  writes `(|);`
            - __callStatic [method]  : mixed  shown last  writes `(|);`
            - __clone [method]  : void  shown last  writes `();`
            - __construct [method]  shown last  writes `(|);`
            - __get [method]  : mixed  shown last  writes `(|);`
            - __set [method]  : void  shown last  writes `(|);`
            - __toString [method]  : string  shown last  writes `();`
            - build [method]  : void  writes `();`
            - find [method]  : void  writes `();`
            - class [const]  App\Repo
            - accepting the first leaves `Repo::build`
        "#]],
    );
}

/// A class with one member of each staticness, and one marker filled in per test.
fn mixed(inside: &str, outside: &str) -> String {
    format!(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{{
    public int $count = 0;
    public static int $seen = 0;
    public const int LIMIT = 10;

    public function find(): void {{}}
    public static function fromNode(): void {{}}

    public function inside(): void
    {{
        {inside}
    }}
}}

class Service
{{
    public function outside(Repo $repo): void
    {{
        {outside}
    }}
}}
"#
    )
}

#[test]
fn an_arrow_on_another_object_does_not_offer_a_static_method() {
    // Legal PHP -- the engine looks it up on the class and calls it -- and a style error every
    // linter flags. Reading `$repo->fromNode()` in a completion list was surprising enough to be
    // reported as a bug, which is the only evidence that matters about a suggestion.
    check(
        &mixed("", "$repo->@here@"),
        &[Query::completion("through another object", "here")],
        expect![[r#"
            through another object
            - find [method]  : void  writes `();`
            - fromNode [method]  : void  shown last  writes `();`
            - inside [method]  : void  writes `();`
            - count [property]  int
            - accepting the first leaves `$repo->find`
        "#]],
    );
}

#[test]
fn an_arrow_on_this_still_offers_one() {
    // `$this->create(...)` is written by hand -- six times in the project this was measured on and
    // five in Drupal core. Marginal, and somebody's code.
    check(
        &mixed("$this->@here@", ""),
        &[Query::completion("through $this", "here")],
        expect![[r#"
            through $this
            - find [method]  : void  writes `();`
            - fromNode [method]  : void  writes `();`
            - inside [method]  : void  writes `();`
            - count [property]  int
            - accepting the first leaves `$this->find`
        "#]],
    );
}

#[test]
fn a_named_class_does_not_offer_an_instance_method_through_the_double_colon() {
    // The mirror image, and a fatal error since PHP 8.0 rather than a style opinion.
    check(
        &mixed("", "Repo::@here@"),
        &[Query::completion("through a named class", "here")],
        expect![[r#"
            through a named class
            - fromNode [method]  : void  writes `();`
            - $seen [property]  int
            - LIMIT [const]  int
            - class [const]  App\Repo
            - accepting the first leaves `Repo::fromNode`
        "#]],
    );
}

#[test]
fn self_does_offer_one_through_the_double_colon() {
    // `self::helper()` and `parent::__construct()` are ordinary PHP.
    check(
        &mixed("self::@here@", ""),
        &[Query::completion("through self", "here")],
        expect![[r#"
            through self
            - find [method]  : void  writes `();`
            - fromNode [method]  : void  writes `();`
            - inside [method]  : void  writes `();`
            - $seen [property]  int
            - LIMIT [const]  int
            - class [const]  App\Repo
            - accepting the first leaves `self::find`
        "#]],
    );
}
