//! What a variable's declaration says it holds.
//!
//! Owned here rather than only tested through completion, because this projection is where every
//! future source of a variable's type arrives: an assignment, a docblock `@var`, a narrowing
//! `instanceof`. Pinning the answers at this layer means the features above keep their tests when
//! the way the answer is produced changes.

use expect_test::expect;

use super::{Query, check_view, check_view_with};

#[test]
fn a_parameters_type_hint_names_the_class() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(Repo $other): void
    {
        $oth@here@er;
    }
}
"#,
        &[Query::variable_class("hint", "here", "other")],
        expect![[r#"
            hint
            - App\Repo
        "#]],
    );
}

#[test]
fn a_hint_is_resolved_against_the_declaring_files_imports() {
    check_view(
        r#"
//- /src/Entity/User.php
<?php
namespace App\Entity;
class User {}

//- /src/Repo.php
<?php
namespace App;

use App\Entity\User as Person;

class Repo
{
    public function run(Person $who): void
    {
        $wh@here@o;
    }
}
"#,
        &[Query::variable_class("alias", "here", "who")],
        expect![[r#"
            alias
            - App\Entity\User
        "#]],
    );
}

#[test]
fn a_nullable_hint_still_names_its_class() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(?Repo $other): void
    {
        $oth@here@er;
    }
}
"#,
        &[Query::variable_class("nullable", "here", "other")],
        expect![[r#"
            nullable
            - App\Repo
        "#]],
    );
}

#[test]
fn self_and_parent_hints_are_read_against_the_enclosing_class() {
    // `static` is deliberately absent: PHP allows it as a return type only, so it cannot reach this
    // projection from a parameter. It will matter once a call's result has a type.
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;
class Base {}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    public function run(self $a, parent $b): void
    {
        $@self@a;
        $@parent@b;
    }
}
"#,
        &[
            Query::variable_class("self", "self", "a"),
            Query::variable_class("parent", "parent", "b"),
        ],
        expect![[r#"
            self
            - App\Repo
            parent
            - App\Base
        "#]],
    );
}

#[test]
fn a_name_declared_in_another_function_is_not_borrowed() {
    // Two functions, one parameter name. Answering from the one the cursor is not in would produce
    // a plausible, wrong class -- and a wrong member list reads as authoritative.
    check_view(
        r#"
//- /src/Two.php
<?php
namespace App;

class Alpha {}
class Beta {}

class Two
{
    public function first(Alpha $thing): void {}

    public function second(Beta $thing): void
    {
        $thi@here@ng;
    }
}
"#,
        &[Query::variable_class("second", "here", "thing")],
        expect![[r#"
            second
            - App\Beta
        "#]],
    );
}

#[test]
fn what_is_not_declared_is_not_guessed() {
    // Each of these is knowable in principle and not knowable from what is written: an untyped
    // parameter needs its docblock, a union names more than one class, and a scalar names none.
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Alpha {}
class Beta {}

class Repo
{
    public function run($untyped, Alpha|Beta $either, int $count): void
    {
        $@untyped@untyped;
        $@either@either;
        $@count@count;
        $@absent@nothing;
    }
}
"#,
        &[
            Query::variable_class("untyped", "untyped", "untyped"),
            Query::variable_class("union", "either", "either"),
            Query::variable_class("scalar", "count", "count"),
            Query::variable_class("never declared", "absent", "nothing"),
        ],
        expect![[r#"
            untyped
            - (type not known here)
            union
            - (type not known here)
            scalar
            - (type not known here)
            never declared
            - (type not known here)
        "#]],
    );
}

#[test]
fn a_closures_own_parameter_shadows_the_one_outside_it() {
    // The wrong answer here is the dangerous kind: an outer parameter of the same name has a type,
    // so the projection would confidently name a class the variable is not.
    check_view(
        r#"
//- /src/Two.php
<?php
namespace App;

class Alpha {}
class Beta {}

class Two
{
    public function run(Alpha $item): void
    {
        $callback = function (Beta $item) {
            $it@inner@em;
        };
        $it@outer@em;
    }
}
"#,
        &[
            Query::variable_class("inside the closure", "inner", "item"),
            Query::variable_class("outside it", "outer", "item"),
        ],
        expect![[r#"
            inside the closure
            - App\Beta
            outside it
            - App\Alpha
        "#]],
    );
}

#[test]
fn a_variable_a_closure_neither_declares_nor_captures_is_not_reached_for() {
    // An arrow function captures by value automatically, so `$outer` inside one really is the outer
    // one. A `function () {}` without a `use` clause does not see it at all, and answering from the
    // enclosing method would describe a variable that is undefined there.
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(Repo $outer): void
    {
        $closure = function () {
            $out@closure@er;
        };
        $arrow = fn () => $out@arrow@er;
    }
}
"#,
        &[
            Query::variable_class("in a closure", "closure", "outer"),
            Query::variable_class("in an arrow function", "arrow", "outer"),
        ],
        expect![[r#"
            in a closure
            - (type not known here)
            in an arrow function
            - App\Repo
        "#]],
    );
}

#[test]
fn a_captured_variable_keeps_the_type_it_had_outside() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(Repo $outer): void
    {
        $closure = function () use ($outer) {
            $out@here@er;
        };
    }
}
"#,
        &[Query::variable_class("captured", "here", "outer")],
        expect![[r#"
            captured
            - App\Repo
        "#]],
    );
}

#[test]
fn a_free_functions_parameter_is_read_too() {
    check_view(
        r#"
//- /src/helpers.php
<?php
namespace App;

class Repo {}

function run(Repo $repo): void
{
    $re@here@po;
}
"#,
        &[Query::variable_class("function", "here", "repo")],
        expect![[r#"
            function
            - App\Repo
        "#]],
    );
}

#[test]
fn a_property_declared_in_a_parent_still_answers() {
    // The member table is flattened, so where the property was written does not matter -- which is
    // what makes a chain through an inherited property work at all.
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;

class Repo {}

class Base
{
    protected Repo $backend;
}

//- /src/Service.php
<?php
namespace App;

class Service extends Base
{
    public function run(): void
    {
        $th@here@is;
    }
}
"#,
        &[Query::member_class(
            "inherited",
            "here",
            "App\\Service",
            "backend",
        )],
        expect![[r#"
            inherited
            - App\Repo
        "#]],
    );
}

#[test]
fn an_untyped_property_holds_nothing_knowable() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    private $backend;

    public function run(): void
    {
        $th@here@is;
    }
}
"#,
        &[Query::member_class(
            "untyped",
            "here",
            "App\\Repo",
            "backend",
        )],
        expect![[r#"
            untyped
            - (type not known here)
        "#]],
    );
}

#[test]
fn an_assignment_is_followed_through_however_many_hops_it_takes() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Alpha {}

class Repo
{
    private Alpha $backend;

    public function run(): void
    {
        $first = new Alpha();
        $second = $first;
        $third = $this->backend;
        $@first@first;
        $@second@second;
        $@third@third;
    }
}
"#,
        &[
            Query::variable_class("from new", "first", "first"),
            Query::variable_class("from another local", "second", "second"),
            Query::variable_class("from a property", "third", "third"),
        ],
        expect![[r#"
            from new
            - App\Alpha
            from another local
            - App\Alpha
            from a property
            - App\Alpha
        "#]],
    );
}

#[test]
fn a_local_assigned_from_itself_gives_up_rather_than_looping() {
    // Not valid PHP that anyone means, but text an editor sees on the way to something valid.
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(): void
    {
        $a = $b;
        $b = $a;
        $@here@a;
    }
}
"#,
        &[Query::variable_class("cycle", "here", "a")],
        expect![[r#"
            cycle
            - (type not known here)
        "#]],
    );
}

#[test]
fn a_name_written_past_the_last_statement_still_resolves_in_its_namespace() {
    // `namespace App;` with no braces governs everything after it, and mago's recovery ends the
    // namespace node at the last statement it could parse. In a file being typed into, the cursor
    // sits past that -- so a name written there resolved in the *global* namespace, which is every
    // completion at the end of a script.
    check_view(
        r#"
//- /src/script.php
<?php
namespace App;

class Beta {}

$thing = new Beta();
$thing->@here@
"#,
        &[Query::variable_class(
            "past the last statement",
            "here",
            "thing",
        )],
        expect![[r#"
            past the last statement
            - App\Beta
        "#]],
    );
}

/// A container reached by a service id holds the class that id was registered as.
///
/// The declaration says `mixed`, so nothing about the call site says which class -- the id does, and
/// only a framework knows what it maps to. Written here rather than only end-to-end because this is
/// the layer that decides it.
#[test]
fn a_service_id_names_the_class_the_container_returns() {
    check_view_with(
        r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id): mixed
    {
        return NULL;
    }
}

//- /src/Manager.php
<?php
namespace App;

class Manager
{
    public function run(): void {}
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $manager = $container->get('app.manager');
        $man@here@ager;
    }
}
"#,
        &[Query::variable_class("service", "here", "manager")],
        |world| {
            world
                .contracts_mut()
                .bind_argument("Dep\\Container", "get", 0, "", "service");
            world
                .contracts_mut()
                .catalog_entry("service", "app.manager", Some("App\\Manager"));
        },
        expect![[r#"
            service
            - App\Manager
        "#]],
    );
}

/// An id the catalog does not hold falls to what the declaration says, which here is nothing.
///
/// The half that keeps this from being a guess: a binding that matches no key answers no, and the
/// declared `mixed` is then the honest answer rather than some other service's class.
#[test]
fn an_unregistered_service_id_names_nothing() {
    check_view_with(
        r#"
//- /src/Container.php
<?php
namespace Dep;

class Container
{
    public function get(string $id): mixed
    {
        return NULL;
    }
}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $manager = $container->get('nobody.registered.this');
        $man@here@ager;
    }
}
"#,
        &[Query::variable_class("missing", "here", "manager")],
        |world| {
            world
                .contracts_mut()
                .bind_argument("Dep\\Container", "get", 0, "", "service");
            world
                .contracts_mut()
                .catalog_entry("service", "app.manager", Some("App\\Manager"));
        },
        expect![[r#"
            missing
            - (type not known here)
        "#]],
    );
}

/// A binding on an interface covers the implementation, and a different method is untouched.
#[test]
fn a_binding_follows_the_hierarchy_and_stops_at_the_method_it_names() {
    check_view_with(
        r#"
//- /src/ContainerInterface.php
<?php
namespace Dep;

interface ContainerInterface
{
    public function get(string $id): mixed;

    public function has(string $id): bool;
}

//- /src/Container.php
<?php
namespace Dep;

class Container implements ContainerInterface
{
    public function get(string $id): mixed
    {
        return NULL;
    }

    public function has(string $id): bool
    {
        return TRUE;
    }
}

//- /src/Manager.php
<?php
namespace App;

class Manager {}

//- /src/Caller.php
<?php
namespace App;

use Dep\Container;

class Caller
{
    public function run(Container $container): void
    {
        $manager = $container->get('app.manager');
        $answer = $container->has('app.manager');
        $man@one@ager;
        $ans@two@wer;
    }
}
"#,
        &[
            Query::variable_class("through the interface", "one", "manager"),
            Query::variable_class("another method", "two", "answer"),
        ],
        |world| {
            world
                .contracts_mut()
                .bind_argument("Dep\\ContainerInterface", "get", 0, "", "service");
            world
                .contracts_mut()
                .catalog_entry("service", "app.manager", Some("App\\Manager"));
        },
        expect![[r#"
            through the interface
            - App\Manager
            another method
            - (type not known here)
        "#]],
    );
}
