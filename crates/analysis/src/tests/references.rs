//! Everywhere one declaration is used.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{reference_target, references_in};

/// Renders, for the marked cursor, every reference in every file of the fixture.
///
/// Project-wide by opening the whole fixture, which is what a workspace's sweep does one file at a
/// time. The target is decided once, from the cursor, and then matched everywhere -- so a same-named
/// declaration in another namespace shows up here as an answer this must not include.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));

    let target = reference_target(
        &IndexView::in_file(world.skeleton(), &open, cursor),
        position.offset,
    );
    let Some(target) = target else {
        expect.assert_eq("(nothing to search for)\n");
        return;
    };

    // A target with no search name never leaves its file, so the sweep is not run for it -- which
    // is what the workspace does too, and for the same reason: the sweep matches a name, and a
    // local's name means a different variable in every function that shares the spelling.
    let searched = match target.search_name() {
        Some(_) => spec.files().iter().map(|file| file.path()).collect(),
        None => vec![position.path.as_str()],
    };

    let mut out = String::new();
    for path in searched {
        let id = world.file_id(path);
        let view = IndexView::in_file(world.skeleton(), &open, id);
        for range in references_in(&view, &target) {
            let _ = writeln!(out, "{path} {}", world.render_range(id, range));
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn a_class_is_found_where_it_is_named_and_where_it_is_imported() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/Other.php
<?php
namespace Other;

/// A different declaration with the same short name. Not an answer.
class Repo {}

//- /src/Service.php
<?php
namespace App;

use App\R@cursor@epo;
use Other\Repo as Alias;

class Service extends Repo
{
    public function run(Repo $repo, Alias $alias): Repo
    {
        return new Repo();
    }
}
"#,
        "cursor",
        expect![[r#"
            src/Repo.php 4:7-4:11
            src/Service.php 4:5-4:13
            src/Service.php 7:23-7:27
            src/Service.php 9:25-9:29
            src/Service.php 9:52-9:56
            src/Service.php 11:20-11:24
        "#]],
    );
}

#[test]
fn a_method_is_found_through_every_class_that_inherits_it() {
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function ru@cursor@n(): void {}
}

class Child extends Base {}

class Unrelated
{
    /// The same name, a different declaration. Not an answer.
    public function run(): void {}
}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function go(Base $base, Child $child, Unrelated $other): void
    {
        $base->run();
        $child->run();
        $other->run();
    }
}
"#,
        "cursor",
        expect![[r#"
            src/Base.php 6:21-6:24
            src/Service.php 8:16-8:19
            src/Service.php 9:17-9:20
        "#]],
    );
}

#[test]
fn a_local_variable_answers_from_its_own_function_only() {
    // Bound, read, and reached through a closure that captured it -- and *not* the `$item` in the
    // method next door, which is a different variable with the same spelling. That is why a local
    // has no search name: the sweep matches names, and this one means something else everywhere
    // else.
    check(
        r#"
//- /src/Service.php
<?php
namespace App;

class Service
{
    public function go(array $rows): void
    {
        $it@cursor@em = 1;
        $item = $item + 1;
        $add = function () use ($item) {
            return $item;
        };
        $other = fn () => $item;
        $shadow = function ($item) {
            return $item;
        };
    }

    public function elsewhere(): void
    {
        $item = 2;
    }
}
"#,
        "cursor",
        expect![[r#"
            src/Service.php 8:9-8:14
            src/Service.php 9:9-9:14
            src/Service.php 9:17-9:22
            src/Service.php 10:33-10:38
            src/Service.php 11:20-11:25
            src/Service.php 13:27-13:32
        "#]],
    );
}

#[test]
fn a_class_constant_is_found_where_it_is_read() {
    check(
        r#"
//- /src/Notifications.php
<?php
namespace App;

class Notifications
{
    public const string ADMIN = 'admin';
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): string
    {
        Notifications::AD@here@MIN;
        return Notifications::ADMIN;
    }
}
"#,
        "here",
        expect![[r#"
            src/Notifications.php 6:25-6:30
            src/Caller.php 8:24-8:29
            src/Caller.php 9:31-9:36
        "#]],
    );
}

#[test]
fn an_enum_case_is_found_where_it_is_read() {
    // The asymmetry this exists for: a case is *declared* as a case and *written* exactly like a
    // constant, so the syntax at a use site cannot tell the two apart. A filter that compared the
    // declared kind against the written one matched nothing at all.
    check(
        r#"
//- /src/Stage.php
<?php
namespace App;

enum Stage: string
{
    case Shipped = 'shipped';
}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): Stage
    {
        Stage::Ship@here@ped;
        return Stage::Shipped;
    }
}
"#,
        "here",
        expect![[r#"
            src/Stage.php 6:10-6:17
            src/Caller.php 8:16-8:23
            src/Caller.php 9:23-9:30
        "#]],
    );
}

#[test]
fn a_method_named_by_a_callable_pair_is_a_reference_to_it() {
    // How a framework is handed a method before PHP 8.1's `(...)`, and still how most of them
    // are: `new TwigFilter('x', [LabelFormatter::class, 'format'])`. Left as a string the method
    // had one reference -- its own declaration -- and a rename rewrote the declaration and left
    // the string spelling a method that no longer existed.
    check(
        r#"
//- /src/Formatter.php
<?php
namespace App;

class Formatter
{
    public static function form@subject@at(string $value): string { return $value; }
}

//- /src/Extension.php
<?php
namespace App;

class Extension
{
    public function filters(): array
    {
        return [
            [Formatter::class, 'format'],
            [$this, 'wrap'],
        ];
    }

    public function wrap(string $value): string { return $value; }
}
"#,
        "subject",
        expect![[r#"
            src/Formatter.php 6:28-6:34
            src/Extension.php 9:33-9:39
        "#]],
    );
}

#[test]
fn a_pair_of_class_names_is_not_a_call_and_names_no_reference() {
    // `[\DateTimeInterface::class, 'null']` is how Symfony writes a list of allowed types, and
    // the syntax is a callable's down to the character. What separates them is whether the class
    // has the member: a list of class names resolves to none, so the presumed site is dropped
    // before anything reads it -- here, before `references` claims a use of a `null` method.
    check(
        r#"
//- /src/Formatter.php
<?php
namespace App;

class Formatter
{
    public static function nu@subject@ll(string $value): string { return $value; }
}

//- /src/Other.php
<?php
namespace App;

class Other
{
    public function allowed(): array
    {
        return [\DateTimeInterface::class, 'null'];
    }
}
"#,
        "subject",
        expect![[r#"
            src/Formatter.php 6:28-6:32
        "#]],
    );
}

/// Every way PHP writes the name of a free function, and the one way it writes it in a string.
///
/// A function used to be reached as a class-shaped target, and nothing that answers about classes
/// answers about a function: the class-like projection skips a function name and `class_location`
/// has nothing for one. So this listed the *string* and nothing else -- not the declaration, not
/// one call -- and a rename accepted at that cursor renamed `'App\alpha'` in an autoload map and
/// left the function called `alpha`.
#[test]
fn a_free_function_is_found_at_its_declaration_and_at_every_way_of_calling_it() {
    check(
        r#"
//- /src/alpha.php
<?php
namespace App;

function al@cursor@pha(int $n): int
{
    return $n + 1;
}

//- /src/bootstrap.php
<?php
namespace App;

const MAP = [
    'App\alpha' => __DIR__ . '/alpha.php',
];

function boot(): int
{
    return alpha(1);
}

//- /src/Imported.php
<?php
namespace App\Deep;

use function App\alpha;

class Imported
{
    public function go(): int
    {
        return alpha(2);
    }
}

//- /src/Qualified.php
<?php
namespace Other;

class Qualified
{
    public function go(): int
    {
        return \App\alpha(3);
    }
}
"#,
        "cursor",
        expect![[r#"
            src/alpha.php 4:10-4:15
            src/bootstrap.php 5:6-5:15
            src/bootstrap.php 10:12-10:17
            src/Imported.php 4:14-4:23
            src/Imported.php 10:16-10:21
            src/Qualified.php 8:16-8:26
        "#]],
    );
}

/// The control beside it: a function of the same short name in another namespace is another
/// function, and an unqualified call in *its* namespace reaches it and not this one.
#[test]
fn a_function_of_the_same_name_in_another_namespace_is_a_different_function() {
    check(
        r#"
//- /src/alpha.php
<?php
namespace App;

function al@cursor@pha(int $n): int
{
    return $n + 1;
}

//- /src/other.php
<?php
namespace Other;

function alpha(int $n): int
{
    return $n - 1;
}

function use_it(): int
{
    return alpha(1);
}
"#,
        "cursor",
        expect![[r#"
            src/alpha.php 4:10-4:15
        "#]],
    );
}

/// A global constant, which PHP keeps in a third table and which nothing here used to answer about.
///
/// Reached as a class-shaped target, find-references at a `const MAX = 10;` listed nothing at all
/// -- not one use, not the declaration -- and the rename offered at that cursor came back with an
/// empty edit and no error, which an editor shows as a dialogue that does nothing. Every spelling
/// is here because they go through different halves: an unqualified name inside the declaring
/// namespace, a `use const` line and the short name it binds, a fully-qualified name, and a string.
#[test]
fn a_global_constant_is_found_at_its_declaration_and_at_every_way_of_naming_it() {
    check(
        r#"
//- /src/limits.php
<?php
namespace App;

const MAX_IT@cursor@EMS = 10;

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function go(): int
    {
        return MAX_ITEMS;
    }
}

//- /src/Deep.php
<?php
namespace App\Deep;

use const App\MAX_ITEMS;

class Deep
{
    public function go(): int
    {
        return MAX_ITEMS + \App\MAX_ITEMS;
    }
}
"#,
        "cursor",
        expect![[r#"
            src/limits.php 4:7-4:16
            src/Uses.php 8:16-8:25
            src/Deep.php 4:11-4:24
            src/Deep.php 10:16-10:25
            src/Deep.php 10:28-10:42
        "#]],
    );
}

/// And the case rule that tells the third table from the other two: PHP compares a constant's name
/// exactly, so `MAX_ITEMS` and `max_items` are two constants and only one of them is declared here.
#[test]
fn a_constant_written_in_another_case_is_another_constant() {
    check(
        r#"
//- /src/limits.php
<?php
namespace App;

const MAX_IT@cursor@EMS = 10;

const max_items = 20;

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function go(): int
    {
        return MAX_ITEMS + max_items;
    }
}
"#,
        "cursor",
        expect![[r#"
            src/limits.php 4:7-4:16
            src/Uses.php 8:16-8:25
        "#]],
    );
}

/// A constant `define()` declares, which is how WordPress declares most of them.
///
/// 636 calls on that checkout against 560 `const` statements, where Laravel has two. The name is
/// the string whole and carries no namespace -- see `phrost_lower`'s `defined_constants` -- so a
/// use written inside a namespace reaches it by PHP's fallback to global, and that is the site
/// this has to find.
#[test]
fn a_constant_declared_by_define_is_found_where_it_is_used() {
    check(
        r#"
//- /src/boot.php
<?php

define('APP_@cursor@ROOT', __DIR__);

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function go(): string
    {
        return APP_ROOT . \APP_ROOT;
    }
}
"#,
        "cursor",
        expect![[r#"
            src/boot.php 3:9-3:17
            src/Uses.php 8:16-8:24
            src/Uses.php 8:27-8:36
        "#]],
    );
}

/// The method name written in a `use T { ... }` clause, which PHP checks when the class loads.
///
/// `An alias (welcome) was defined for method missing(), but this method does not exist` -- a fatal
/// on 8.5, asked of it rather than assumed. So a rename that steps over one produces a class that
/// no longer loads, which is the worst a rename can do. Both spellings are here because they go
/// through different halves: `greet as welcome` names the method through whichever used trait
/// declares it, and `Greets::greet insteadof Shouts` names the trait outright.
#[test]
fn a_trait_adaptation_names_the_method_it_renames_or_keeps() {
    check(
        r#"
//- /src/Greets.php
<?php
namespace App;

trait Greets
{
    public function gr@cursor@eet(): string
    {
        return 'hi';
    }
}

//- /src/Shouts.php
<?php
namespace App;

trait Shouts
{
    public function greet(): string
    {
        return 'HI';
    }
}

//- /src/Aliased.php
<?php
namespace App;

class Aliased
{
    use Greets {
        greet as welcome;
    }

    public function run(): string
    {
        return $this->greet();
    }
}

//- /src/Chosen.php
<?php
namespace App;

class Chosen
{
    use Greets, Shouts {
        Greets::greet insteadof Shouts;
    }
}
"#,
        "cursor",
        expect![[r#"
            src/Greets.php 6:21-6:26
            src/Aliased.php 7:9-7:14
            src/Aliased.php 12:23-12:28
            src/Chosen.php 7:17-7:22
        "#]],
    );
}

/// A string naming something in the global namespace is a bare word, and a bare word is prose.
///
/// The loose comparison is right where the name is qualified -- nothing writes `'App\Repo'` by
/// accident -- and wrong where it is not. Renaming a global `function fail()` turned `new
/// \LogicException('Fail')` and `'body' => 'FAIL'` into `'fail'` across four of Symfony's test
/// files, which the rename round trip caught by renaming it back and finding the project changed.
/// Exact case keeps what the rule exists for and drops the word that merely reads like the name.
#[test]
fn a_string_naming_a_global_declaration_has_to_spell_its_case() {
    check(
        r#"
//- /src/fail.php
<?php

function fa@cursor@il(string $why): void
{
    throw new \LogicException($why);
}

//- /src/uses.php
<?php

const MESSAGES = [
    'body' => 'FAIL',
    'other' => 'Fail',
    'callable' => 'fail',
];

fail('a');
"#,
        "cursor",
        expect![[r#"
            src/fail.php 3:10-3:14
            src/uses.php 6:20-6:24
            src/uses.php 9:1-9:5
        "#]],
    );
}

/// And the qualified half, which is what the loose comparison is for.
#[test]
fn a_string_naming_a_qualified_declaration_is_matched_however_it_is_cased() {
    check(
        r#"
//- /src/random.php
<?php
namespace Psl\Iter;

function ra@cursor@ndom(array $rows): mixed
{
    return $rows[0];
}

//- /src/bootstrap.php
<?php
namespace Psl;

const MAP = [
    'Psl\Iter\random' => __DIR__ . '/random.php',
    'psl\iter\RANDOM' => __DIR__ . '/random.php',
];
"#,
        "cursor",
        expect![[r#"
            src/random.php 4:10-4:16
            src/bootstrap.php 5:6-5:21
            src/bootstrap.php 6:6-6:21
        "#]],
    );
}

/// What the project-wide sweep is asked to grep for, which decides whether it runs at all.
///
/// Its own test rather than a line in the snapshots above, because the decision is invisible in a
/// result: a sweep for a name no file contains and a sweep that was never run both come back with
/// the same list, so only the name itself distinguishes them.
#[test]
fn an_anonymous_class_has_no_name_for_a_sweep_to_look_for() {
    // A synthesised name is written in no file, so a sweep for it reads every candidate and matches
    // nothing -- and find-references on `new class { ... }` came back empty while the highlight at
    // the same cursor drew its uses. `None` is what both callers read as "this file and no other",
    // which is the whole of where an unwritable name can be used.
    let fixture = r#"
//- /src/Factory.php
<?php
namespace App;

class Factory
{
    public function make(): object
    {
        return new cl@anonymous@ass {
            public function go(): void {}
        };
    }
}
"#;
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);

    let named = search_name_at(&world, &spec, "anonymous");
    assert_eq!(
        named, None,
        "an anonymous class is not searched for by name"
    );

    // The same question of a name somebody wrote, so that `None` above is the exception it claims
    // to be rather than the answer this always gives.
    let spec = FixtureSpec::parse(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Re@named@po {}
"#,
    );
    let world = World::build(&spec);
    assert_eq!(
        search_name_at(&world, &spec, "named"),
        Some("Repo".to_owned())
    );
}

fn search_name_at(world: &World, spec: &FixtureSpec, marker: &str) -> Option<String> {
    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    reference_target(
        &IndexView::in_file(world.skeleton(), &open, cursor),
        position.offset,
    )
    .expect("the cursor is on a declaration")
    .search_name()
}

#[test]
fn a_constructor_is_found_where_it_is_reached_by_new() {
    // `new Repo()` names neither the member nor -- for a subclass that declares none of its own --
    // the class that declares it, so a member sweep found the declaration and nothing else: a
    // constructor with four call sites read as one nobody uses. Reported by a person looking at
    // exactly that. The call hierarchy has known the rule all along, through the same query, and
    // its own note spells out the three ways PHP reaches a constructor.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function __co@here@nstruct(string $dsn)
    {
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Repo
{
}

//- /src/Owns.php
<?php
namespace App;

class Owns extends Repo
{
    public function __construct(string $dsn)
    {
        parent::__construct($dsn);
    }
}

//- /src/app.php
<?php
namespace App;

function run(): void
{
    new Repo('a');
    new Child('b');
    new Owns('c');
}
"#,
        "here",
        expect![[r#"
            src/Repo.php 6:21-6:32
            src/Owns.php 8:17-8:28
            src/app.php 6:9-6:13
            src/app.php 7:9-7:14
        "#]],
    );
}

#[test]
fn a_name_a_magic_accessor_answers_is_found_where_that_accessor_serves_it() {
    // Drupal's field items: `$this->file` is a typed-data property a `__get` serves, declared in no
    // PHP. There was no declaration to be about, so a search listed nothing while the name was
    // written five times. Two accesses are one member when one accessor serves both -- the item
    // and its subclass here, and not an unrelated class whose own `__get` answers the same word.
    check(
        r#"
//- /src/Item.php
<?php
namespace App;

class ItemBase
{
    public function __get($name) {}
    public function __set($name, $value) {}
}

class FileItem extends ItemBase
{
    public function normalize(): void
    {
        if (isset($this->file)) {
            $this->fi@here@le = (int) $this->file;
        }
    }
}

class Other
{
    public function __get($name) {}
}

//- /src/Use.php
<?php
namespace App;

function read(FileItem $item, Other $other): void
{
    echo $item->file;
    echo $other->file;
}
"#,
        "here",
        expect![[r#"
            src/Item.php 14:26-14:30
            src/Item.php 15:20-15:24
            src/Item.php 15:40-15:44
            src/Use.php 6:17-6:21
        "#]],
    );
}

#[test]
fn a_local_compact_names_is_found_in_the_string() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function render(array $rows): array
{
    $t@here@itle = 'Report';
    $count = count($rows);
    return compact('title', 'count') + ['other' => extract(['title' => 1])];
}
"#,
        "here",
        expect![[r#"
            src/app.php 6:5-6:11
            src/app.php 8:21-8:26
        "#]],
    );
}

#[test]
fn the_string_compact_collects_is_a_cursor_on_its_local() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

function render(array $rows): array
{
    $title = 'Report';
    return compact('ti@here@tle');
}
"#,
        "here",
        expect![[r#"
            src/app.php 6:5-6:11
            src/app.php 7:21-7:26
        "#]],
    );
}
