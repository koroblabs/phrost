//! Working out what a receiver holds.
//!
//! Every source of a receiver's class, one test each: a hint, an import, a closure's own parameter,
//! an assignment, a property, a return type, a chain of them. And every source that says nothing,
//! because offering a plausible wrong class is worse than offering none.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_typed_parameter_offers_its_classes_members() {
    // The common case on type-hinted code, and the reason member rows carry resolved types: the
    // hint names a class, the class has a member table, and no inference is involved.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): ?Repo {}
    private function internal(): void {}

    public function run(Repo $other): void
    {
        $other->@here@
    }
}
"#,
        &[Query::completion("typed parameter", "here")],
        expect![[r#"
            typed parameter
            - find [method]  : ?Repo  writes `();`
            - internal [method]  : void  writes `();`
            - run [method]  : void  writes `(|);`
            - accepting the first leaves `$other->find`
        "#]],
    );
}

#[test]
fn a_nullable_parameter_offers_the_class_it_may_hold() {
    // `?Repo` describes Repo. Whether the value is null is a diagnostic's question -- the members
    // being asked about are the same either way, and offering none would make every nullable
    // parameter feel broken.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run(?Repo $other): void
    {
        $other->@here@
    }
}
"#,
        &[Query::completion("nullable parameter", "here")],
        expect![[r#"
            nullable parameter
            - find [method]  : void  writes `();`
            - run [method]  : void  writes `(|);`
            - accepting the first leaves `$other->find`
        "#]],
    );
}

#[test]
fn a_parameter_typed_by_an_import_resolves_through_it() {
    check(
        r#"
//- /src/Entity/User.php
<?php
namespace App\Entity;

class User
{
    public function name(): string {}
}

//- /src/Repo.php
<?php
namespace App;

use App\Entity\User;

class Repo
{
    public function run(User $user): void
    {
        $user->@here@
    }
}
"#,
        &[Query::completion("imported type", "here")],
        expect![[r#"
            imported type
            - name [method]  : string  writes `();`
            - accepting the first leaves `$user->name`
        "#]],
    );
}

#[test]
fn a_vendor_type_is_described_without_the_file_being_open() {
    // The reason vendor is in the skeleton at all. Nothing about it is analysed -- no bodies, no
    // inference -- but the declarations it makes have to be readable, or every parameter typed by a
    // dependency goes silent.
    check(
        r#"
//- /vendor/acme/src/Client.php  vendor
<?php
namespace Acme;

class Client
{
    public function send(string $body): int {}
}

//- /src/Repo.php
<?php
namespace App;

use Acme\Client;

class Repo
{
    public function run(Client $client): void
    {
        $client->@here@
    }
}
"#,
        &[Query::completion("vendor type", "here")],
        expect![[r#"
            vendor type
            - send [method]  : int  writes `(|);`
            - accepting the first leaves `$client->send`
        "#]],
    );
}

#[test]
fn a_closure_parameter_offers_its_own_classes_members() {
    // The case that needs the body tree: an outer parameter of the same name has a different type,
    // and answering from it would offer a list that looks right and is not.
    check(
        r#"
//- /src/Two.php
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

class Two
{
    public function run(Alpha $item): void
    {
        $callback = function (Beta $item) {
            $item->@here@
        };
    }
}
"#,
        &[Query::completion("closure parameter", "here")],
        expect![[r#"
            closure parameter
            - betaOnly [method]  : void  writes `();`
            - accepting the first leaves `$item->betaOnly`
        "#]],
    );
}

#[test]
fn a_methods_return_type_carries_the_chain_further() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Label
{
    public function text(): string {}
}

class Repo
{
    public function label(): Label {}
}

class Service
{
    private Repo $backend;

    public function run(): void
    {
        $this->backend->label()->@here@
    }
}
"#,
        &[Query::completion("through a return type", "here")],
        expect![[r#"
            through a return type
            - text [method]  : string  writes `();`
            - accepting the first leaves `$this->backend->label()->text`
        "#]],
    );
}

#[test]
fn a_chain_that_breaks_partway_offers_nothing() {
    // `$this->backend` has no declared type, so nothing after it is knowable. Falling back to the
    // enclosing class's members would produce a list that reads as if it belonged to the property.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    private $backend;

    public function open(): void {}

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("untyped property", "here")],
        expect![[r#"
            untyped property
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_untyped_parameter_offers_nothing() {
    // Nothing says what it is. A guessed class's members read as authoritative and are accepted by
    // one keystroke, so the honest answer is silence -- until a docblock `@param` can be read, which
    // needs the docblock type parser.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run($other): void
    {
        $other->@here@
    }
}
"#,
        &[Query::completion("untyped parameter", "here")],
        expect![[r#"
            untyped parameter
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_variable_assigned_from_a_constructor_offers_that_classes_members() {
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
        $repo = new Repo();
        $repo->@here@
    }
}
"#,
        &[Query::completion("assigned variable", "here")],
        expect![[r#"
            assigned variable
            - find [method]  : void  writes `();`
            - run [method]  : void  writes `();`
            - accepting the first leaves `$repo->find`
        "#]],
    );
}

#[test]
fn a_property_of_this_offers_what_its_type_has() {
    // Two hops: `$this` is the enclosing class, its `$backend` property has a declared type, and
    // that type has members. Neither hop is a guess.
    //
    // The property is deliberately not named after its class. Reading only the last token before the
    // arrow and looking it up as a class name passes whenever the two happen to match -- and class
    // names are matched case-insensitively, so `$this->repo` finding class `Repo` looks like the
    // feature working when it is a coincidence.
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
    private Repo $backend;

    public function run(): void
    {
        $this->backend->@here@
    }
}
"#,
        &[Query::completion("property of this", "here")],
        expect![[r#"
            property of this
            - find [method]  : void  writes `();`
            - accepting the first leaves `$this->backend->find`
        "#]],
    );
}

#[test]
fn the_parameter_of_the_method_the_cursor_is_in_is_the_one_that_answers() {
    // Two methods, both with a `$thing` parameter of a different type. Answering from the wrong one
    // would offer a plausible, wrong list -- the failure mode that is hardest to notice.
    check(
        r#"
//- /src/Two.php
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

class Two
{
    public function first(Alpha $thing): void {}

    public function second(Beta $thing): void
    {
        $thing->@here@
    }
}
"#,
        &[Query::completion("second method", "here")],
        expect![[r#"
            second method
            - betaOnly [method]  : void  writes `();`
            - accepting the first leaves `$thing->betaOnly`
        "#]],
    );
}

#[test]
fn a_self_typed_parameter_means_the_enclosing_class() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run(self $other): void
    {
        $other->@here@
    }
}
"#,
        &[Query::completion("self parameter", "here")],
        expect![[r#"
            self parameter
            - find [method]  : void  writes `();`
            - run [method]  : void  writes `(|);`
            - accepting the first leaves `$other->find`
        "#]],
    );
}

/// `Alpha|Beta` offers every operand's members, and says which operand a name is only on.
///
/// This used to offer the intersection alone, on the grounds that `onlyAlpha()` is a fatal error
/// whenever the value turned out to be a `Beta`. What changed it is that the checker cannot report
/// such a call either -- a union here is often built by inference that is approximate in places, and
/// a report on an approximate union is a report on our own imprecision -- so withholding the row
/// prevented no mistake. It only left the operands-share-nothing case with an empty list, which is
/// what an editor shows when a server is broken. The `Alpha only` column is what makes the risk
/// visible where hiding the row did not.
#[test]
fn a_union_typed_parameter_offers_every_operands_members_and_says_whose() {
    check(
        r#"
//- /src/Two.php
<?php
namespace App;

class Alpha
{
    public function shared(): void {}

    public function onlyAlpha(): void {}
}

class Beta
{
    public function shared(): void {}

    public function onlyBeta(): void {}
}

class Two
{
    public function run(Alpha|Beta $thing): void
    {
        $thing->@here@
    }
}
"#,
        &[Query::completion("union parameter", "here")],
        expect![[r#"
            union parameter
            - shared [method]  : void  writes `();`
            - onlyAlpha [method]  : void — Alpha only  writes `();`
            - onlyBeta [method]  : void — Beta only  writes `();`
            - accepting the first leaves `$thing->shared`
        "#]],
    );
}

/// Two operands with nothing in common: the whole of both, each marked.
///
/// The case that decided the rule above. An empty list here reads as a broken server, and the two
/// names it was hiding are the only two things anybody could have wanted.
#[test]
fn a_union_whose_operands_share_nothing_still_offers_both() {
    check(
        r#"
//- /src/Apart.php
<?php
namespace App;

class Reader
{
    public function read(): void {}
}

class Writer
{
    public function write(): void {}
}

class Apart
{
    public function run(Reader|Writer $io): void
    {
        $io->@here@
    }
}
"#,
        &[Query::completion("nothing in common", "here")],
        expect![[r#"
            nothing in common
            - read [method]  : void — Reader only  writes `();`
            - write [method]  : void — Writer only  writes `();`
            - accepting the first leaves `$io->read`
        "#]],
    );
}

/// A union one of whose operands names no class at all.
///
/// This used to offer nothing, on the argument that `Alpha|string` reached with `->` may be a
/// string and a list built from `Alpha` alone would say the call is fine. What refuted it is that
/// **goto through the same receiver has always answered** -- so the position was not a policy, it
/// was one of the two paths not knowing what the other did -- and that nothing *reports* such a
/// call either, so the empty list prevented no mistake and only read as a broken server. Laravel
/// declares `Request::json()` as `@return ParameterBag|mixed`, which is a docblock written loosely
/// rather than a person saying the value may be a scalar.
///
/// The risk is said instead of hidden, the same way a name only one operand of `Reader|Writer`
/// declares is said: the row names the class it came from.
#[test]
fn a_union_with_a_scalar_operand_offers_the_class_and_names_it() {
    check(
        r#"
//- /src/Loose.php
<?php
namespace App;

class Alpha
{
    public function shared(): void {}
}

class Loose
{
    public function run(Alpha|string $thing): void
    {
        $thing->@here@
    }
}
"#,
        &[Query::completion("a class or a string", "here")],
        expect![[r#"
            a class or a string
            - shared [method]  : void — Alpha only  writes `();`
            - accepting the first leaves `$thing->shared`
        "#]],
    );
}

/// PHP 8.4's `new Foo()->` -- the same expression as `(new Foo())->` without the parentheses the
/// language stopped requiring.
///
/// Two spellings of one thing, and the scanner read only the older one. They part company at the
/// exact point that branch is decided: with the parentheses nothing is named left of them, and
/// without them the class *is* the trailing identifier -- so the parenthesised arm never saw this
/// and this fell through to "a name with `new ` left of it", which is no operator and so a
/// deliberate miss. 111 sites in the reference project's own modules and 220 in Symfony against 42
/// for the parenthesised spelling: a project on 8.4 stops writing the older one.
///
/// One incomplete statement per method, which §8.4 records and which cost an hour here: three of
/// them in one body and only the first is read, because the dangling `->` does not parse and mago
/// drops what follows the break.
#[test]
fn an_instantiation_without_parentheses_offers_the_classes_members() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Widget
{
    public function render(): string {}
}

class Builder
{
    public function withName(string $n): self {}
    public function build(): Widget {}
}

class Use1
{
    public function bare(): void
    {
        new Builder()->@bare@
    }

    public function parenthesised(): void
    {
        (new Builder())->@wrapped@
    }

    public function chained(): void
    {
        new Builder()->build()->@chained@
    }

    public function qualified(): void
    {
        new \App\Builder()->@qualified@
    }

    public function withArguments(): void
    {
        new Builder('a', 2)->@arguments@
    }
}
"#,
        &[
            Query::completion("new Foo()->", "bare"),
            Query::completion("(new Foo())->", "wrapped"),
            Query::completion("new Foo()->build()->", "chained"),
            Query::completion("new \\App\\Foo()->", "qualified"),
            Query::completion("new Foo($a, $b)->", "arguments"),
        ],
        expect![[r#"
            new Foo()->
            - build [method]  : Widget  writes `()`
            - withName [method]  : self  writes `(|)`
            - accepting the first leaves `new Builder()->build`
            (new Foo())->
            - build [method]  : Widget  writes `()`
            - withName [method]  : self  writes `(|)`
            - accepting the first leaves `(new Builder())->build`
            new Foo()->build()->
            - render [method]  : string  writes `()`
            - accepting the first leaves `new Builder()->build()->render`
            new \App\Foo()->
            - build [method]  : Widget  writes `()`
            - withName [method]  : self  writes `(|)`
            - accepting the first leaves `new \App\Builder()->build`
            new Foo($a, $b)->
            - build [method]  : Widget  writes `()`
            - withName [method]  : self  writes `(|)`
            - accepting the first leaves `new Builder('a', 2)->build`
        "#]],
    );
}

/// A parenthesised expression is a receiver, read by reading what is inside the brackets.
///
/// Three shapes in one fixture, one per method, because a completion marker leaves a dangling `->`
/// and mago drops what follows the break -- see §8.4. The pipe is the one that cannot be written as
/// a scanner unit test, since that helper reads the first `|` as its cursor.
#[test]
fn a_parenthesised_expression_offers_what_is_inside_it() {
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function label(): string {}
}

class Reader
{
    public static function head(array $rows): Row {}

    public function cloned(Row $row): void
    {
        (clone $row)->@cloned@
    }

    public function piped(array $rows): void
    {
        ($rows |> self::head(...))->@piped@
    }

    public function nested(Row $row): void
    {
        ((($row)))->@nested@
    }
}
"#,
        &[
            Query::completion("a clone holds what it cloned", "cloned"),
            Query::completion("a pipe holds what the call returns", "piped"),
            Query::completion("and brackets nest", "nested"),
        ],
        expect![[r#"
            a clone holds what it cloned
            - label [method]  : string  writes `()`
            - accepting the first leaves `(clone $row)->label`
            a pipe holds what the call returns
            - label [method]  : string  writes `()`
            - accepting the first leaves `($rows |> self::head(...))->label`
            and brackets nest
            - label [method]  : string  writes `()`
            - accepting the first leaves `((($row)))->label`
        "#]],
    );
}

/// A chain through a union whose operands are intersections reaches what each of them declares.
///
/// What an `if` choosing which class to mock leaves behind: `$product` is
/// `PriceList&Mock|Product&Mock` after the join, and `->method()` is declared by the `Mock` both
/// halves carry. Goto had learned that -- it asks the full ladder of which operand owns a name --
/// and the chain completion walks had two rungs of it, a single class and an intersection's parts,
/// so `$product->method('x')->` offered nothing on the reference project's own tests.
#[test]
fn a_chain_through_a_union_of_intersections_reaches_the_shared_part() {
    check(
        r#"
//- /src/Mock.php
<?php
namespace Dep;

interface Mock
{
    public function method(string $name): Stub;
}

interface Stub
{
    public function willReturn(mixed $value): self;
}

//- /src/Test.php
<?php
namespace App;

use Dep\Mock;

class Product {}
class PriceList extends Product {}

class Test
{
    public function product(bool $credit): void
    {
        if ($credit) {
            /** @var PriceList&Mock $product */
            $product = make();
        }
        else {
            /** @var Product&Mock $product */
            $product = make();
        }
        $product->method('getCharges')->will@here@
    }
}
"#,
        &[Query::completion("through the join", "here")],
        expect![[r#"
            through the join
            - willReturn [method]  : self  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$product->method('getCharges')->willReturn`
        "#]],
    );
}
