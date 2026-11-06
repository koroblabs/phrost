//! Enums, whose most-used members no file declares.
//!
//! `Stage::Shipped`, `$stage->value` and `Stage::cases()` are most of what anyone writes with an
//! enum, and none of the three is declared in the enum: PHP hands every enum `UnitEnum` and every
//! backed one `BackedEnum`. The interfaces here stand in for the standard library's, which is where
//! the real ones come from.

use expect_test::expect;

use super::{Query, check};

/// A project with the two enum interfaces and one backed enum.
const FIXTURE: &str = r#"
//- /src/enums.php
<?php

interface UnitEnum
{
    public readonly string $name;

    /** @return list<static> */
    public static function cases(): array;
}

interface BackedEnum extends UnitEnum
{
    public readonly int|string $value;

    public static function from(int|string $value): static;
    public static function tryFrom(int|string $value): ?static;
}

enum Stage: string
{
    case Draft = 'draft';
    case Shipped = 'shipped';
}
"#;

#[test]
fn a_case_is_reached_through_a_constant_access() {
    check(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(): void
    {
        $stage = Stage::@case@Shipped;
    }
}
"#,
        ]
        .concat(),
        &[Query::goto("case", "case"), Query::hover("case", "case")],
        expect![[r#"
            case
            - case Stage::Shipped @ src/enums.php 22:10-22:17 (full 22:5-22:30)
            case
            - code: <?php
            enum Stage
            {
                case Shipped = 'shipped'
            }
        "#]],
    );
}

#[test]
fn the_implicit_members_of_a_backed_enum_are_offered() {
    check(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(Stage $stage): void
    {
        $stage->@member@
    }
}
"#,
        ]
        .concat(),
        &[Query::completion("on an enum value", "member")],
        expect![[r#"
            on an enum value
            - cases [method]  : array<int, static>  shown last  writes `();`
            - from [method]  : static  shown last  writes `(|);`
            - tryFrom [method]  : ?static  shown last  writes `(|);`
            - name [property]  string
            - value [property]  string
            - accepting the first leaves `$stage->name`
        "#]],
    );
}

/// The declaration a reader is sent to is the interface's, because that is where `$value` is
/// written -- but the type is this enum's own backing type. `BackedEnum` has to promise `int|string`
/// for every backed enum at once, and `enum Stage: string` said which.
#[test]
fn a_backed_enum_value_has_the_backing_type_its_own_declaration_named() {
    check(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(Stage $stage): void
    {
        $stage->@value@value;
    }
}
"#,
        ]
        .concat(),
        &[Query::hover("value", "value")],
        expect![[r#"
            value
            - code: <?php
            interface BackedEnum
            {
                public readonly string $value
            }
            - note: interface BackedEnum
        "#]],
    );
}

#[test]
fn a_static_access_offers_the_cases_and_the_static_methods() {
    check(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(): void
    {
        Stage::@member@
    }
}
"#,
        ]
        .concat(),
        &[Query::completion("after ::", "member")],
        expect![[r#"
            after ::
            - cases [method]  : array<int, static>  writes `();`
            - from [method]  : static  writes `(|);`
            - tryFrom [method]  : ?static  writes `(|);`
            - Draft [case]
            - Shipped [case]
            - class [const]  Stage
            - accepting the first leaves `Stage::cases`
        "#]],
    );
}

#[test]
fn a_case_used_as_a_value_is_an_instance_of_its_enum() {
    // The only member whose type no declaration writes: PHP says a case is an instance of its enum,
    // so nothing in the file states it and a chain through one had nowhere to go. Two markers,
    // because the enum's own method needs no standard library and `->value` does -- and the second
    // is what a reader of a backed enum actually writes.
    check(
        &[
            FIXTURE,
            r#"
//- /src/app.php
<?php

enum Scope: string
{
    case User = 'user';

    public function describe(): string
    {
        return self::User->la@own@bel();
    }

    public function label(): string
    {
        return self::User->va@backed@lue;
    }
}
"#,
        ]
        .concat(),
        &[
            Query::goto("its own method", "own"),
            Query::goto("a backed enum's value", "backed"),
        ],
        expect![[r#"
            its own method
            - method Scope::label @ src/app.php 12:21-12:26 (full 12:5-15:6)
            a backed enum's value
            - property BackedEnum::value @ src/enums.php 13:32-13:38 (full 13:5-13:39)
        "#]],
    );
}

#[test]
fn a_class_constant_carries_the_type_it_was_declared_with() {
    // The same missing arm, and the broader half of it: no class constant of any kind could be a
    // receiver, because reading one as a path was not modelled at all.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

//- /src/Registry.php
<?php
namespace App;

class Registry
{
    public const Repo DEFAULT = new Repo();

    public function run(): void
    {
        self::DEFAULT->fi@here@nd();
    }
}
"#,
        &[Query::goto("through a class constant", "here")],
        expect![[r#"
            through a class constant
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:36)
        "#]],
    );
}

#[test]
fn the_class_pseudo_constant_is_a_string_and_has_no_members() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run(): void
    {
        self::class->fi@here@nd();
    }
}
"#,
        &[Query::goto("on the class string", "here")],
        expect![[r#"
            on the class string
            - (nothing to go to)
        "#]],
    );
}

/// A pure enum has no `$value` at all, and the retyping must not invent one.
#[test]
fn a_pure_enum_offers_only_the_name() {
    check(
        r#"
//- /src/stubs.php
<?php

interface UnitEnum
{
    public readonly string $name;
}

interface BackedEnum extends UnitEnum
{
    public readonly int|string $value;
}

//- /src/Colour.php
<?php

enum Colour
{
    case Red;
    case Blue;
}

class Service
{
    public function run(Colour $colour): void
    {
        $colour->@member@
    }
}
"#,
        &[Query::completion("on a pure enum value", "member")],
        expect![[r#"
            on a pure enum value
            - name [property]  string
            - accepting the first leaves `$colour->name`
        "#]],
    );
}

/// The standard library gives a string-backed enum `__INTERNAL_DO_NOT_USE__StringBackedEnum` --
/// a real interface, declared on purpose, that narrows `$value` for the type checker and that
/// nothing in application code is meant to write. A hover on the enum has to say what it *is*
/// without repeating a name that only exists for the compiler.
#[test]
fn a_backed_enums_hover_hides_the_stub_internal_interface() {
    check(
        r#"
//- /src/stubs.php
<?php

interface UnitEnum {}

interface BackedEnum extends UnitEnum {}

interface __INTERNAL_DO_NOT_USE__StringBackedEnum extends BackedEnum {}

//- /src/Stage.php
<?php

enum St@here@age: string implements __INTERNAL_DO_NOT_USE__StringBackedEnum
{
    case Draft = 'draft';
}
"#,
        &[Query::hover("hover", "here")],
        expect![[r#"
            hover
            - code: <?php
            enum Stage
        "#]],
    );
}
