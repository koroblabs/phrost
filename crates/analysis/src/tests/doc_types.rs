//! Types written in a docblock.
//!
//! Where the type comes from on code that predates PHP's own type declarations, which is most of a
//! project of any age. A native declaration always wins; the docblock speaks when it is silent.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_var_tag_types_an_untyped_property() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /**
     * @var Repo
     */
    private $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("var tag", "here")],
        expect![[r#"
            var tag
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend->find`
        "#]],
    );
}

#[test]
fn a_param_tag_types_an_untyped_parameter() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    /**
     * @param string $label
     * @param Repo $other
     */
    public function run($label, $other): void
    {
        $other->@here@
    }
}
"#,
        &[Query::completion("param tag", "here")],
        expect![[r#"
            param tag
            - find [method]  : void  writes `();`
            - run [method]  : void  writes `(|);`
            - accepting the first leaves `$other->find`
        "#]],
    );
}

#[test]
fn a_return_tag_types_an_untyped_method() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /**
     * @return Repo
     */
    public function backend()
    {
    }

    public function run(): void
    {
        $this->backend()->@here@
    }
}
"#,
        &[Query::completion("return tag", "here")],
        expect![[r#"
            return tag
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend()->find`
        "#]],
    );
}

#[test]
fn a_native_declaration_wins_over_a_docblock_that_disagrees() {
    // The native one is what the runtime enforces. A docblock contradicting it is a mistake in the
    // source, and believing the docblock would describe code that cannot run that way.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Alpha
{
    public function alphaOnly(): void {}
}

class Beta
{
    public function betaOnly(): void {}
}

class Service
{
    /**
     * @var Alpha
     */
    private Beta $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("native wins", "here")],
        expect![[r#"
            native wins
            - betaOnly [method]  : void  writes `();`
            - accepting the first leaves `$this->backend->betaOnly`
        "#]],
    );
}

#[test]
fn a_docblock_type_is_resolved_through_the_files_imports() {
    // Exactly as a native hint is. A docblock naming `Repo` in a file that imports `App\Entity\Repo`
    // means that class, and resolving it any other way would find whichever `Repo` came first.
    check(
        r#"
//- /src/Entity/Repo.php
<?php
namespace App\Entity;

class Repo
{
    public function entityOnly(): void {}
}

//- /src/Other/Repo.php
<?php
namespace App\Other;

class Repo
{
    public function otherOnly(): void {}
}

//- /src/Service.php
<?php
namespace App\Other;

use App\Entity\Repo;

class Service
{
    /**
     * @var Repo
     */
    private $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("through imports", "here")],
        expect![[r#"
            through imports
            - entityOnly [method]  : void  writes `();`
            - accepting the first leaves `$this->backend->entityOnly`
        "#]],
    );
}

#[test]
fn a_nullable_docblock_type_names_the_class_it_may_hold() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /** @var ?Repo */
    private $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("nullable", "here")],
        expect![[r#"
            nullable
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend->find`
        "#]],
    );
}

#[test]
fn a_docblock_shape_with_no_written_php_equivalent_says_nothing() {
    // `list<Repo>` is an array, and an array has no members. Offering `Repo`'s would describe the
    // elements rather than the value, which is the sort of nearly-right answer worth refusing.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /** @var list<Repo> */
    private $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("a list", "here")],
        expect![[r#"
            a list
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_param_tag_for_a_different_parameter_is_not_borrowed() {
    // Matched by name, because `@param` lines are written in whatever order the author liked and one
    // of them is often missing.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    /**
     * @param Repo $documented
     */
    public function run($documented, $undocumented): void
    {
        $undocumented->@here@
    }
}
"#,
        &[Query::completion("wrong parameter", "here")],
        expect![[r#"
            wrong parameter
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_inline_var_annotation_types_a_local_the_value_does_not_describe() {
    // The Drupal idiom, and the reason it exists: a service container lookup returns `mixed`, so the
    // annotation is the only thing that says what came back.
    check(
        r#"
//- /src/Container.php
<?php
namespace App;

class Manager
{
    public function load(int $id): void {}
}

class Container
{
    public static function service(string $name) {}
}

//- /src/script.php
<?php
namespace App;

/** @var \App\Manager $m */
$m = Container::service('manager');
$m->@here@
"#,
        &[Query::completion("annotated local", "here")],
        expect![[r#"
            annotated local
            - load [method]  : void  writes `(|);`
            - accepting the first leaves `$m->load`
        "#]],
    );
}

#[test]
fn an_annotation_naming_a_different_variable_is_not_borrowed() {
    check(
        r#"
//- /src/script.php
<?php
namespace App;

class Manager
{
    public function load(): void {}
}

/** @var Manager $other */
$m = 1;
$m->@here@
"#,
        &[Query::completion("wrong variable", "here")],
        expect![[r#"
            wrong variable
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_annotation_is_overtaken_by_a_later_assignment() {
    check(
        r#"
//- /src/script.php
<?php
namespace App;

class Alpha
{
    public function alphaOnly(): void {}
}

class Beta
{
    public function betaOnly(): void {}
}

/** @var Alpha $thing */
$thing = null;
$thing = new Beta();
$thing->@here@
"#,
        &[Query::completion("later assignment", "here")],
        expect![[r#"
            later assignment
            - betaOnly [method]  : void  writes `();`
            - accepting the first leaves `$thing->betaOnly`
        "#]],
    );
}

#[test]
fn a_return_of_a_class_or_null_still_describes_the_class() {
    // `Foo|null` is `?Foo` written the other way, and PHPDoc writes it that way far more often --
    // Drupal core's `ListInterface::first()` is `@return TypedDataInterface|null`, and until this
    // worked, every chain through it went dead at the next `->`.
    //
    // Whether the value is null is a diagnostic's question. The members being asked about are the
    // same either way.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    /**
     * @return Repo|null
     */
    public function backend()
    {
    }

    /**
     * @return null|Repo
     */
    public function other()
    {
    }

    public function run(): void
    {
        $this->backend()->@first@
    }

    public function again(): void
    {
        $this->other()->@second@
    }
}
"#,
        &[
            Query::completion("class or null", "first"),
            Query::completion("null or class", "second"),
        ],
        expect![[r#"
            class or null
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend()->find`
            null or class
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->other()->find`
        "#]],
    );
}

#[test]
fn a_return_of_two_classes_or_null_offers_what_both_have() {
    // Seeing through `null` is not the same as picking one of several: the null is taken off and
    // what is left is still two classes, so what they both declare is the answer.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Other
{
    public function find(): void {}
}

class Service
{
    /**
     * @return Repo|Other|null
     */
    public function backend()
    {
    }

    public function run(): void
    {
        $this->backend()->@here@
    }
}
"#,
        &[Query::completion("two classes or null", "here")],
        expect![[r#"
            two classes or null
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend()->find`
        "#]],
    );
}

#[test]
fn repro_complex_data_get() {
    check(
        r#"
//- /src/app.php
<?php
namespace Drupal\Core\TypedData;

interface TypedDataInterface
{
    public function getValue();
}

interface ComplexDataInterface extends TypedDataInterface
{
    /**
     * Gets a property object.
     *
     * @param string $property_name
     *   The name of the property to get; e.g., 'title' or 'name'.
     *
     * @return \Drupal\Core\TypedData\TypedDataInterface
     *   The property object.
     *
     * @throws \InvalidArgumentException
     *   If an invalid property name is given.
     */
    public function get($property_name);
}

class Service
{
    public function run(ComplexDataInterface $item): void
    {
        $item->get('entity')->@here@getValue();
    }
}
"#,
        &[Query::completion("through get", "here")],
        expect![[r#"
            through get
            - getValue [method]  writes `()`
            - accepting the first leaves `$item->get('entity')->getValuegetValue();`
        "#]],
    );
}

#[test]
fn an_override_with_no_type_inherits_the_one_it_overrides() {
    // `{@inheritdoc}`, which is how most of Drupal's API is written. `ConfirmFormBase::buildForm`
    // has neither a return hint nor an `@return`; `FormInterface::buildForm`, several levels up, has
    // `@return array`. An override saying nothing about its type is saying the type has not changed.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

interface Builds
{
    /**
     * @return \App\Repo
     */
    public function build();
}

abstract class Base implements Builds
{
    /**
     * {@inheritdoc}
     */
    public function build()
    {
    }
}

class Service extends Base
{
    /**
     * {@inheritdoc}
     */
    public function build()
    {
    }

    public function run(): void
    {
        $this->build()->@here@
    }
}
"#,
        &[Query::completion("through two untyped overrides", "here")],
        expect![[r#"
            through two untyped overrides
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->build()->find`
        "#]],
    );
}

#[test]
fn an_override_that_declares_its_own_type_keeps_it() {
    // The narrowing is the whole reason an override writes a type. Climbing past it would report
    // the wider one, which is the answer the author replaced.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Wide
{
    public function wide(): void {}
}

class Narrow extends Wide
{
    public function narrow(): void {}
}

interface Builds
{
    /**
     * @return \App\Wide
     */
    public function build();
}

class Service implements Builds
{
    public function build(): Narrow
    {
    }

    public function run(): void
    {
        $this->build()->@here@
    }
}
"#,
        &[Query::completion("the override's own type", "here")],
        expect![[r#"
            the override's own type
            - narrow [method]  : void  writes `();`
            - wide [method]  : void  writes `();`
            - accepting the first leaves `$this->build()->narrow`
        "#]],
    );
}

#[test]
fn a_property_and_a_constant_inherit_the_same_way() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

abstract class Base
{
    /**
     * @var \App\Repo
     */
    protected $backend;
}

class Service extends Base
{
    /**
     * {@inheritdoc}
     */
    protected $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("an inherited property type", "here")],
        expect![[r#"
            an inherited property type
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend->find`
        "#]],
    );
}

/// `array{app: Repo, count: int}` says which key holds what, which is the whole difference from
/// `array<string, mixed>` -- there every key holds the same thing, and no single element type could
/// say both.
#[test]
fn a_shape_says_what_each_key_holds() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function id(): int
    {
        return 0;
    }
}

class Service
{
    /** @return array{app: Repo, count: int} */
    public function shaped(): array
    {
        return [];
    }

    public function run(): void
    {
        $shape = $this->shaped();
        $shape['app']->i@here@d();
    }
}
"#,
        &[Query::goto("through the key", "here")],
        expect![[r#"
            through the key
            - method Repo::id @ app.php 4:21-4:23 (full 4:5-7:6)
        "#]],
    );
}

/// And it survives to be read back, rather than being flattened to `array` on the way.
#[test]
fn a_shape_annotation_on_a_local_shows_its_keys() {
    check(
        r#"
//- /app.php
<?php
class Repo {}

class Service
{
    public function run(): void
    {
        /** @var array{app: Repo, count: int} */
        $shape = [];
        $sh@here@ape;
    }
}
"#,
        &[Query::hover("the shape as written", "here")],
        expect![[r#"
            the shape as written
            - code: array{app: Repo, count: int} $shape
            - note: assigned
        "#]],
    );
}
