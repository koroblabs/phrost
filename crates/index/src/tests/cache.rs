//! The invariant a cache exists to be trusted by: `cold == warm`.
//!
//! An index read back from disk has to be the same index, and "the same" means the whole canonical
//! rendering -- every name, every range, every type. A field left out of the codec is invisible to
//! anything else: it does not fail to compile, it does not fail a feature test on a freshly built
//! index, and it surfaces hours later as one wrong answer in a session that happened to start from
//! the cache.
//!
//! This is why the roadmap put the cache in Phase 2 rather than later: the invariant cannot be added
//! retroactively, and every field added between then and now would be a field to audit by hand.

use std::collections::BTreeMap;

use expect_test::expect;
use phrost_model::FileId;

use super::{Order, build, fixture_files};
use crate::ProjectSkeleton;

/// One build's stamp. Any value does, so long as the writer and the reader agree -- which is the
/// whole of what the field is for.
const BUILD: u64 = 0x5150_5f70_6872_7374;

/// Builds the fixture, writes it, reads it back, and requires the two to render identically.
/// Decodes bytes already in hand.
///
/// The production reader takes a stream, because reading the file into a `Vec` first cost a 46 MB
/// allocation the allocator keeps -- but a test that has just encoded an index has the bytes, and a
/// slice is a perfectly good `io::Read`.
fn decode(bytes: &[u8], assign: impl FnMut(&std::path::Path) -> FileId) -> Option<ProjectSkeleton> {
    let mut source = bytes;
    ProjectSkeleton::decode(&mut source, bytes.len() as u64, BUILD, assign)
}

pub(super) fn check_round_trip(fixture: &str) {
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);

    let bytes = built.skeleton.encode(BUILD);
    let mut next = 0_u32;
    let mut assigned: BTreeMap<String, FileId> = BTreeMap::new();
    let decoded = decode(&bytes, |path| {
        let id = FileId::from_raw(next);
        next += 1;
        assigned.insert(path.display().to_string(), id);
        id
    })
    .expect("what this build wrote, this build reads");

    // Decoded and then adopted, which is what the server does and is not a formality: a decoded
    // index has no contracts, because a cache holds what files said and contracts are configuration.
    // Comparing the decoded index directly would be comparing against a project that has no
    // framework -- and a claim only the contracts can derive would then be "missing" in both.
    let mut restored = super::build(&BTreeMap::new(), Order::ByPath).skeleton;
    restored.adopt(decoded);
    // A decoded index is not ready until something has checked it against disk; the renderings
    // below include that state, so the test says which it expects.
    restored.mark_ready();

    let render = |skeleton: &ProjectSkeleton| {
        crate::dump_skeleton(skeleton, |entry| {
            files
                .get(skeleton.display_path(entry))
                .map(|text| phrost_model::LineIndex::new(text))
        })
    };

    assert_eq!(
        render(&built.skeleton),
        render(&restored),
        "an index read back from disk is not the index that was written"
    );

    // The other half of the bug class. The rendering catches a field that is never written; writing
    // the restored index again catches one that is written and read back wrong, because everything
    // after it in the stream shifts.
    assert_eq!(
        bytes,
        restored.encode(BUILD),
        "writing the index again produced different bytes, so something was read back wrong"
    );
}

#[test]
fn a_project_survives_being_written_and_read_back() {
    check_round_trip(
        r#"
//- /src/Repo.php
<?php
namespace App;

use App\Nested\Thing as Aliased;
use function App\helper;

/**
 * The repository.
 *
 * @template K of int|string
 * @template V of Thing
 * @template U
 */
interface Reads
{
    public const string VERSION = '1';

    public function find(int $id): ?Thing;

    /**
     * @phpstan-assert-if-true \App\Nested\Thing $this
     */
    public function isThing(): bool;
}

#[Plugin(id: 'the_repo', label: 'Repository', links: ['canonical' => '/repo', 'edit-form' => '/repo/edit'])]
abstract class Repo implements Reads
{
    use Loggable;

    private array $cache = [];

    /** @param list<Thing> $items */
    public function __construct(private readonly Aliased $one, array $items) {}

    public function find(int $id): ?Thing
    {
        return null;
    }

    /**
     * A *method* reading arguments its signature does not name. Beside the free function that does
     * the same in `Contracts.php` because the two are different rows with different codecs, and a
     * sabotage of the member half passed while only the function half was covered -- which is the
     * gap this comment exists so nobody has to rediscover.
     */
    public function logged(string $message): void
    {
        $rest = func_get_args();
    }
}

enum Stage: string
{
    case Draft = 'draft';
}

function helper(int ...$values): void {}

const LIMIT = 10;

//- /src/Nested/Thing.php
<?php
namespace App\Nested;

trait Loggable
{
    public function log(string $message): void {}
}

class Thing {}

//- /src/Contracts.php
<?php
namespace App;

/**
 * Every shape a contract can take, because a branch left out of the codec is invisible until it
 * gives one wrong answer in a session that started from the cache.
 *
 * @template T
 * @param class-string<T> $class
 * @return T
 */
function make(string $class) {}

/**
 * @template T
 * @param T $item
 * @return T
 */
function same($item) {}

class Doubled {}

class Factory
{
    /**
     * The intersection a mock factory writes: what travels is the *fact* that one was written, and
     * a warm start that lost it would answer the declared class alone.
     *
     * @template T of object
     * @param class-string<T> $class
     * @return Doubled&T
     */
    public function double(string $class): Doubled {}
}

/**
 * @template T
 * @template U
 * @param class-string<T> $class
 * @param U $seed
 * @return Nested\Thing<T, Stage, U>
 */
function boxed(string $class, $seed) {}

/**
 * @template K as array-key
 * @template V
 * @param array<K, V> $array
 * @return V
 */
function first(array $array) {}

/**
 * @template K as array-key
 * @template V
 * @param array<K, V> $array
 * @return ($array is non-empty-array ? non-empty-list<V> : list<V>)
 */
function values(array $array): array {}

/**
 * @template K as array-key
 * @template V
 * @template U
 * @param (callable(V): U)|null $callback
 * @param array<K, V> $array
 * @return ($array is list<V> ? list<U> : array<K, U>)
 */
function mapped(?callable $callback, array $array): array {}

/**
 * @template I
 * @template R
 * @param callable(I, string): R $callback
 * @param I $initial
 * @return R
 */
function fold(callable $callback, $initial) {}

/**
 * @param string|null $body
 * @return ($body is null ? Nested\Thing : Stage)
 */
function respond(?string $body) {}

/**
 * A kind beside a class, which is a test of its own and the one added last to both halves of the
 * codec -- see `ConditionTest::OfKindOrInstance`.
 *
 * @param mixed $id
 * @return ($id is (Nested\Thing|array<mixed>) ? Stage : Nested\Thing)
 */
function located($id) {}

/**
 * A chain of string tests, the contract added after the one above -- see
 * `Contract::LiteralCases`.
 *
 * @phpstan-return ($shape is 'list' ? Stage : ($shape is 'map' ? Nested\Thing : null))
 */
function shaped($shape = 'object') {}

/**
 * A by-reference parameter's *written-back* type, which is a third type per parameter and the one
 * added last to both halves of the codec -- see `ParamRow::out_ty` and §8 on field order being read
 * order. Here because a field the codec writes and never reads back, or reads in the wrong order,
 * compiles clean and answers wrong only in a session that started from the cache.
 *
 * @param array $matches
 * @param-out array<int, string> $matches
 */
function captured(string $pattern, array &$matches): bool {}

/**
 * @phpstan-assert Nested\Thing $value
 */
function insist(mixed $value): void {}

/**
 * What a documented callback is *called with*, which is a fourth type per parameter and the one
 * added last to both halves of the codec after `out_ty` -- see `ParamRow::callback_params_ty`. A
 * real class rather than a template, because a template position is stored as nothing and would
 * leave the field empty on both sides of the comparison, which is a field that is not being tested.
 *
 * @param callable(Nested\Thing, int): void $each
 */
function visited(callable $each): void {}

/**
 * What `$this` is inside a closure passed here, which is a fifth type per parameter and the one
 * added last to both halves of the codec -- see `ParamRow::closure_this_ty`.
 *
 * @param-closure-this Nested\Thing $each
 */
function bound(\Closure $each): void {}

/**
 * A body reading arguments its signature does not name. Here for the same reason as the two above:
 * `reads_own_arguments` is a flag lowering derives and the codec carries, and a flag written and
 * never read back answers wrong only in a session that started from the cache.
 */
function legacy(string $first): void
{
    $rest = func_get_args();
}

//- /src/listeners.php
<?php

// The third kind of catalog claim: a key carried in a declaration's own *name*. Here because the
// two kinds beside it were in the codec and this one was not re-claimed when a stored index was
// adopted, so a restarted server held no key any name carried -- and this round trip could not see
// it, since nothing installed the rule and no fixture wrote a name that carries one.
function shop_order_placed(): void {}

function listener_cart_emptied(): void {}

// A name whose prefix is not a key, so nothing is claimed for it. Here so the claim is known to
// depend on the prefix catalog rather than on there being an underscore.
function unrelated_thing_happened(): void {}

//- /vendor/pkg/Vendored.php
<?php
namespace Pkg;

final class Vendored extends \App\Repo
{
    use \App\Nested\Loggable { log as private quietly; }
}
"#,
    );
}

#[test]
fn a_files_strict_types_directive_survives_the_cache() {
    // A single bit, and the kind the module docs warn about: leaving it out of the codec compiles,
    // passes every feature test on a freshly built index, and then makes a cache-started session
    // judge every argument in the file by the wrong rules.
    check_round_trip(
        r#"
//- /src/Strict.php
<?php
declare(strict_types=1);
namespace App;

class Payment
{
    public function total(int $cents): int
    {
        return $cents;
    }
}
//- /src/Lenient.php
<?php
namespace App;

class Refund
{
    public function total(int $cents): int
    {
        return $cents;
    }
}
"#,
    );
}

#[test]
fn the_words_a_file_holds_survive_the_cache() {
    // What a reference sweep asks instead of reading the project, so a warm start that lost it
    // would read all of it -- and read exactly the same answers back, which is why nothing but
    // the clock would say so. The round trip's re-encode is what catches it: the word list is
    // last in the stream, so a reader that skips it shifts nothing and stays silent.
    check_round_trip(
        r#"
//- /src/Repo.php
<?php
namespace App;

/** Holds {@see Account} rows, and a word only this comment has: bookkeeping. */
class Repo
{
    public function account(): Account
    {
        return new Account();
    }
}
//- /src/Account.php
<?php
namespace App;

class Account {}
"#,
    );
}

#[test]
fn both_spellings_of_an_attribute_key_survive_the_cache() {
    // A named argument and a positional one are two shapes in one field, and which it was decides
    // whether a contract matches. Written as the same bytes, a warm start would claim keys a cold
    // one does not -- for both of them, in opposite directions.
    check_round_trip(
        r#"
//- /src/Textfield.php
<?php
namespace App\Element;

#[Element('textfield')]
class Textfield {}

//- /src/Repo.php
<?php
namespace App\Data;

#[Plugin(id: 'the_repo', label: 'Repository')]
class Repo {}
"#,
    );
}

#[test]
fn a_project_that_declares_nothing_survives_too() {
    check_round_trip(
        r#"
//- /src/empty.php
<?php
$x = 1;
"#,
    );
}

#[test]
fn bytes_this_build_did_not_write_are_refused() {
    // A rebuild costs the two seconds it would have cost anyway. Reading a file half-way and
    // answering from it costs a session of wrong answers.
    let assign = |_: &std::path::Path| FileId::from_raw(0);
    assert!(decode(b"", assign).is_none());
    assert!(decode(b"nope", assign).is_none());
    assert!(decode(b"phrs\xff\xff\xff\xff", assign).is_none());

    let built = build(
        &fixture_files("//- /src/a.php\n<?php class A {}\n"),
        Order::ByPath,
    );
    let bytes = built.skeleton.encode(BUILD);
    let truncated = &bytes[..bytes.len() / 2];
    assert!(decode(truncated, assign).is_none());

    let mut extended = bytes.clone();
    extended.push(0);
    assert!(
        decode(&extended, assign).is_none(),
        "trailing bytes mean the writer and the reader disagree about the shape"
    );

    // And the whole of it still reads.
    assert!(decode(&bytes, assign).is_some());
    let _ = expect![[""]];
}

#[test]
fn a_stamp_from_another_build_is_refused() {
    // The point of the field. What invalidates a stored index is a change to the code that derived
    // it, and most of those change no byte of the format -- so nothing about the bytes could tell,
    // and reusing them makes a server wrong until somebody types something.
    let files = fixture_files(
        r#"
//- /src/Repo.php
<?php
class Repo {}
"#,
    );
    let built = build(&files, Order::ByPath);
    let bytes = built.skeleton.encode(BUILD);
    let assign = |_: &std::path::Path| FileId::from_raw(0);

    assert!(decode(&bytes, assign).is_some());
    let mut source = bytes.as_slice();
    assert!(ProjectSkeleton::decode(&mut source, bytes.len() as u64, BUILD + 1, assign).is_none());
}

#[test]
fn an_anonymous_class_survives_the_cache() {
    // A `new class { ... }` is claimed like any other declaration, because `$this` inside its body
    // names it and nothing else can. What must survive is the pair: the claim, so a restored index
    // still resolves the body, and the bit saying the name cannot be written, so a restored index
    // does not start offering `class@anonymous#77` in completion.
    check_round_trip(
        r#"
//- /src/Product.php
<?php
namespace App;

class Product
{
    public function base(): string
    {
        return '';
    }
}

//- /src/Factory.php
<?php
namespace App;

class Factory
{
    public function make(): Product
    {
        return new class extends Product {
            public function grid(): mixed
            {
                return NULL;
            }
        };
    }
}
"#,
    );
}

#[test]
fn a_class_string_argument_is_visible_in_the_dump_and_survives_the_cache() {
    // `check_round_trip` alone is not enough evidence: it only requires built and restored to
    // agree with *each other*, and would agree just as readily if both sides silently dropped the
    // `<Rule>` the same way. This pins the argument down in the dump first, with a real class
    // rather than a `@template` -- which stays a bare `string`, deliberately, because it resolves
    // to no class -- and then hands the same fixture to the cache half.
    let fixture = r#"
//- /src/Factory.php
<?php
namespace App;

class Rule {}

class Factory
{
    /** @var class-string<Rule> */
    public $current;

    /** @param class-string<Rule> $class */
    public function make($class) {}
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });

    expect![[r#"
        state ready
        file src/Factory.php [User]
          class App\Rule @ 4:7-4:11
          class App\Factory @ 6:7-6:14
          App\Rule is class
          App\Factory is class
          App\Factory::property class-string<Rule> $current  public  @ 9:12-9:20 (full 9:5-9:21)
          App\Factory::method make(class-string<Rule> $class)  public  @ 12:21-12:25 (full 12:5-12:36)
        catalog
          [class] app\factory -> src/Factory.php (App\Factory)
          [class] app\rule -> src/Factory.php (App\Rule)
        namespaces
          app -> App x2
        declared keys
        dispatched keys
    "#]].assert_eq(&dump);

    check_round_trip(fixture);
}

#[test]
fn what_a_declaration_documents_itself_as_throwing_is_in_the_dump_and_survives_the_cache() {
    // The two-part shape the two tests around this one use, and here it is load-bearing twice
    // over. A `@throws` is read out of a *docblock*, and the index keeps ranges rather than text --
    // so a warm start that lost this would have nothing to read it back from, and a caller asking
    // what a call throws would be told nothing on every server after the first. And the dump is
    // the only thing that can catch that: `cold == warm` compares this rendering, so a section the
    // codec wrote and never read back would agree with itself perfectly.
    //
    // The names are resolved against the declaring file's imports, which is why the fixture spells
    // one through a `use` and one relatively.
    let fixture = r#"
//- /src/Repo.php
<?php
namespace App;

use App\Failure\NotFound;

class Missing extends \RuntimeException {}

class Repo
{
    /**
     * @throws NotFound
     * @throws Missing
     */
    public function find(int $id): int {}

    /** No tag, so no row at all -- which is what keeps the table sparse. */
    public function all(): array {}
}

/**
 * @throws \LogicException
 */
function reset_everything(): void {}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });

    expect![[r#"
        state ready
        file src/Repo.php [User]
          App\Repo::find throws App\Failure\NotFound | App\Missing
          App\reset_everything throws LogicException
          class App\Missing @ 6:7-6:14
          class App\Repo @ 8:7-8:11
          function App\reset_everything @ 23:10-23:26
          function App\reset_everything(): void
          use App\Failure\NotFound as NotFound  [class]
          App\Missing is class
          App\Missing extends RuntimeException @ 6:23-6:40
          App\Repo is class
          App\Repo::method find(int $id): int  public  @ 14:21-14:25 (full 14:5-14:42)
          App\Repo::method all(): array  public  @ 17:21-17:24 (full 17:5-17:36)
        catalog
          [class] app\missing -> src/Repo.php (App\Missing)
          [class] app\repo -> src/Repo.php (App\Repo)
          [function] app\reset_everything -> src/Repo.php (App\reset_everything)
        namespaces
          app -> App x3
        declared keys
        dispatched keys
    "#]]
    .assert_eq(&dump);

    check_round_trip(fixture);
}

#[test]
fn a_body_derived_return_and_a_parameters_default_are_in_the_dump_and_survive_the_cache() {
    // Two facts added the same day and neither was covered by anything: one is a *description* of a
    // chain, which nothing else the dump prints implies, and the other is text the signature line
    // renders. A codec that wrote either and never read it back would agree with itself perfectly.
    //
    // Both in one fixture because they arrive together in the shape that motivated them: a wrapper
    // that forwards, declaring no return type, and a method with an optional parameter.
    let fixture = r#"
//- /src/Db.php
<?php
namespace App;

class Select
{
    public function fields(string $table, array $columns = ['id'], ?int $limit = null) {}
}

class Connection
{
    /**
     * @return \App\Select
     */
    public function select(string $table) {}
}

class Base
{
    public function connection(): Connection {}

    /** Wrapper for database select. */
    public function select($table)
    {
        return $this->connection()->select($table);
    }
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });

    expect![[r#"
        state ready
        file src/Db.php [User]
          App\Base::select returns $this->connection->select
          class App\Select @ 4:7-4:13
          class App\Connection @ 9:7-9:17
          class App\Base @ 17:7-17:11
          App\Select is class
          App\Select::method fields(string $table, array $columns = ['id'], ?int $limit = null)  public  @ 6:21-6:27 (full 6:5-6:90)
          App\Connection is class
          App\Connection::method select(string $table): Select  public  @ 14:21-14:27 (full 14:5-14:45)
          App\Base is class
          App\Base::method connection(): Connection  public  @ 19:21-19:31 (full 19:5-19:48)
          App\Base::method select($table)  public  @ 22:21-22:27 (full 22:5-25:6)
        catalog
          [class] app\base -> src/Db.php (App\Base)
          [class] app\connection -> src/Db.php (App\Connection)
          [class] app\select -> src/Db.php (App\Select)
        namespaces
          app -> App x3
        declared keys
        dispatched keys
    "#]].assert_eq(&dump);

    check_round_trip(fixture);
}

#[test]
fn a_phpstan_type_alias_expands_in_the_dump_and_survives_the_cache() {
    // The fixture `docs/03-roadmap.md` and this feature were built against: a class names a shape
    // once with `@phpstan-type` and a member's own `@return` writes the alias instead of the shape.
    // Same two-part shape as the `class-string<Rule>` test above and for the same reason: agreeing
    // with itself after a round trip is not evidence the alias expanded at all, only that whatever
    // came out (expanded or not) came out the same way twice. Pinning the dump first rules that out.
    let fixture = r#"
//- /src/Foo.php
<?php
namespace App;

class User {}

/**
 * @phpstan-type UserList array<int, User>
 */
class Foo
{
    /**
     * @return UserList
     */
    public function getUsers(): array
    {
    }
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });

    expect![[r#"
        state ready
        file src/Foo.php [User]
          class App\User @ 4:7-4:11
          class App\Foo @ 9:7-9:10
          App\User is class
          App\Foo is class
          App\Foo::method getUsers(): array<int, User>  public  @ 14:21-14:29 (full 14:5-16:6)
        catalog
          [class] app\foo -> src/Foo.php (App\Foo)
          [class] app\user -> src/Foo.php (App\User)
        namespaces
          app -> App x2
        declared keys
        dispatched keys
    "#]]
    .assert_eq(&dump);

    check_round_trip(fixture);
}

/// A table a declaration's *body* states, which is the one kind of fact a warm start can never
/// re-derive: the body is gone, so a codec field left out here is a feature that works on the
/// first server of the day and on none after it.
#[test]
fn a_table_a_body_states_survives_the_cache() {
    check_round_trip(
        r#"
//- /src/Shaped.php
<?php
namespace App;

#[Element('card')]
class CardElement
{
    public static function shapeOf($definition): array
    {
        $slots['title'] = Slot::create('text')->required(TRUE);
        $slots['weight'] = Slot::create('number');
        $slots['body'] = Slot::create('markup');
        return $slots;
    }
}
"#,
    );
}

#[test]
fn the_stored_facts_a_signature_does_not_show_are_in_the_dump_and_survive_the_cache() {
    // Four fields were carried by the codec, derived at ingest, and printed by nothing: a
    // `#[\Override]`, a `#[\NoDiscard]` on a free function, a `@mixin`, and the arguments an
    // `@extends` supplies. The two index equality invariants compare *this dump*, so each was a
    // field a stale re-ingest or a bad codec could get wrong in silence -- which is the failure
    // the note beside `stored_notes` already records, met four more times.
    //
    // Pinned here before the cache half, for the reason the test above gives: a round trip only
    // requires the two sides to agree with each other, and they agree just as readily when both
    // drop the same field.
    let fixture = r#"
//- /src/Shapes.php
<?php
namespace App;

class Holder
{
    public function held(): ?Holder {}
}

/**
 * @mixin Holder
 * @extends Holder<Shapes>
 */
class Shapes extends Holder
{
    #[\Override]
    public function held(): ?Holder {}
}

#[\NoDiscard]
function counted(): int {}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });

    expect![[r#"
        state ready
        file src/Shapes.php [User]
          class App\Holder @ 4:7-4:13
          class App\Shapes @ 13:7-13:13
          function App\counted @ 20:10-20:17
          function App\counted(): int  no-discard
          App\Holder is class
          App\Holder::method held(): ?Holder  public  @ 6:21-6:25 (full 6:5-6:39)
          App\Shapes is class
          App\Shapes supplies \App\Holder<\App\Shapes>
          App\Shapes extends App\Holder @ 13:22-13:28
          App\Shapes mixes in App\Holder @ 10:11-10:17
          App\Shapes::method held(): ?Holder  public override  @ 16:21-16:25 (full 15:5-16:39)
        catalog
          [class] app\holder -> src/Shapes.php (App\Holder)
          [class] app\shapes -> src/Shapes.php (App\Shapes)
          [function] app\counted -> src/Shapes.php (App\counted)
        namespaces
          app -> App x3
        declared keys
        dispatched keys
    "#]]
    .assert_eq(&dump);

    check_round_trip(fixture);
}

#[test]
fn a_chain_returned_through_a_local_keeps_its_first_argument_across_the_cache() {
    // `return $double;` read through the one assignment that gave `$double` a value, and the
    // argument that assignment's call was handed -- a `Foo::class` resolved against this file's
    // imports, and a string beside it. Both are printed on the step they were handed to, because
    // a codec that dropped either would otherwise agree with itself.
    let fixture = r#"
//- /src/Helpers.php
<?php
namespace App;

use App\Kernel\HttpKernelInterface as Kernel;

class Doubles
{
    public static function stub(string $class) {}
    public static function service(string $id) {}
}

class Helpers
{
    public function kernel()
    {
        $double = Doubles::stub(Kernel::class);
        $double->configure();
        return $double;
    }

    public function mailer()
    {
        $service = Doubles::service('mailer');
        return $service;
    }
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });
    let chains: String = dump
        .lines()
        .filter(|line| line.contains(" returns "))
        .map(|line| format!("{}\n", line.trim()))
        .collect();

    expect![[r#"
        App\Helpers::kernel returns App\Doubles->stub(App\Kernel\HttpKernelInterface::class)
        App\Helpers::mailer returns App\Doubles->service('mailer')
    "#]]
    .assert_eq(&chains);

    check_round_trip(fixture);
}

#[test]
fn a_chain_of_tests_and_a_shape_test_survive_the_cache() {
    // The contracts a conditional over the *shape* of an argument lowers to, and an empty-shape
    // test, printed and read back. Every one is a new arm in the codec, and a codec that wrote an
    // arm and read a different one would agree with itself in silence.
    let fixture = r#"
//- /src/Posts.php
<?php
namespace App;

class Post {}

class Posts
{
    /**
     * @phpstan-return (
     *     $args is array{ fields: 'ids', ... }
     *         ? int[]
     *         : ( $output is 'ARRAY_A' ? array<int, array> : Post[] )
     * )
     */
    public function children($args = '', $output = 'OBJECT') {}

    /** @phpstan-return ( $args is array{exit: false|0} ? void : never ) */
    public function die($message = '', $args = array()) {}

    /** @return ($names is list{} ? Post : Posts) */
    public function receive(string ...$names) {}
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });
    let members: String = dump
        .lines()
        .filter(|line| line.contains("::method"))
        .map(|line| format!("{}\n", line.trim()))
        .collect();

    expect![[r#"
        App\Posts::method children($args = '', $output = 'OBJECT'): int[]|array<int, array>|Post[]  public returns, if argument 0 is array{fields: 'ids', ...}, int[] (by default false); if argument 1 is 'ARRAY_A', array<int, array> (by default false), and otherwise \App\Post[]  @ 15:21-15:29 (full 15:5-15:64)
        App\Posts::method die($message = '', $args = array()): void  public returns, if argument 1 is array{exit: false|0}, void and otherwise never by default false  @ 18:21-18:24 (full 18:5-18:59)
        App\Posts::method receive(string ...$names): Post|Posts  public returns, if argument 0 is empty, \App\Post and otherwise \App\Posts by default true  @ 21:21-21:28 (full 21:5-21:49)
    "#]].assert_eq(&members);

    check_round_trip(fixture);
}

#[test]
fn a_string_test_and_a_link_that_picks_where_its_test_fails_survive_the_cache() {
    // `wpdb::get_row()`'s shape: the query is tested first, and the rest of the chain is nested in
    // the branch where it passed -- so the first link picks `null` where its test *fails*. Two new
    // tests and the negation are three arms the codec has to agree with itself about.
    let fixture = r#"
//- /src/Db.php
<?php
namespace App;

class Row {}

class Db
{
    /**
     * @phpstan-return (
     *     $query is non-falsy-string
     *         ? ( $output is 'OBJECT' ? Row|null : ( $output is 'ARRAY_A' ? array|null : null ) )
     *         : null
     * )
     */
    public function row($query = null, $output = OBJECT) {}

    /** @return ($text is not non-empty-string ? null : Row) */
    public function parse(string $text = '') {}
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });
    let members: String = dump
        .lines()
        .filter(|line| line.contains("::method"))
        .map(|line| format!("{}\n", line.trim()))
        .collect();

    expect![[r#"
        App\Db::method row($query = null, $output = OBJECT): Row|array|null  public returns, if argument 0 is not a non-falsy string, null (by default false); if argument 1 is 'OBJECT', ?\App\Row; if argument 1 is 'ARRAY_A', ?array, and otherwise null  @ 15:21-15:24 (full 15:5-15:60)
        App\Db::method parse(string $text = ''): ?Row  public returns, if argument 0 is a non-empty string, \App\Row and otherwise null by default false  @ 18:21-18:26 (full 18:5-18:48)
    "#]].assert_eq(&members);

    check_round_trip(fixture);
}

#[test]
fn an_instance_test_and_a_scalar_test_survive_the_cache() {
    let fixture = r#"
//- /src/Builder.php
<?php
namespace App;

class ExprBuilder {}

class Builder
{
    /** @return ($closure is \Closure ? $this : ExprBuilder) */
    public function validate(?\Closure $closure = null) {}

    /** @return ($value is scalar ? string : array) */
    public function resolve(mixed $value) {}
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });
    let members: String = dump
        .lines()
        .filter(|line| line.contains("::method"))
        .map(|line| format!("{}\n", line.trim()))
        .collect();

    expect![[r#"
        App\Builder::method validate(?Closure $closure = null): static|ExprBuilder  public returns, if argument 0 is a \Closure, static and otherwise \App\ExprBuilder by default false  @ 9:21-9:29 (full 9:5-9:59)
        App\Builder::method resolve(mixed $value): string|array  public returns, if argument 0 is a scalar, string and otherwise array  @ 12:21-12:28 (full 12:5-12:45)
    "#]].assert_eq(&members);

    check_round_trip(fixture);
}

#[test]
fn a_chain_read_through_its_locals_roots_where_they_were_assigned() {
    // `$document = new Document(); return $document->createElement($tag);` -- a local in the chain
    // is read through the one value the body gave it, the way a returned local already was. The
    // last three are refusals: a parameter is the caller's, a local written twice has no one value,
    // and `$a = $a;` would read itself forever.
    let fixture = r#"
//- /src/Nodes.php
<?php
namespace App;

class Element {}

class Document
{
    public function createElement(string $tag): Element {}
}

class Nodes
{
    public function direct($tag)
    {
        $document = new Document();
        return $document->createElement($tag);
    }

    public function throughTwo($tag, $attributes = [])
    {
        $document = new Document();
        $node = $document->createElement($tag);
        foreach ($attributes as $name => $value) {
            $node->setAttribute($name, $value);
        }
        return $node;
    }

    public function parameter(Document $document)
    {
        return $document->createElement('a');
    }

    public function twice($tag, bool $flag)
    {
        $document = new Document();
        if ($flag) {
            $document = Document::fresh();
        }
        return $document->createElement($tag);
    }

    public function itself()
    {
        $a = $a;
        return $a->createElement('a');
    }
}
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });
    let chains: String = dump
        .lines()
        .filter(|line| line.contains(" returns "))
        .map(|line| format!("{}\n", line.trim()))
        .collect();

    expect![[r#"
        App\Nodes::direct returns App\Document->createElement
        App\Nodes::throughTwo returns App\Document->createElement
    "#]]
    .assert_eq(&chains);

    check_round_trip(fixture);
}

#[test]
fn a_constant_built_from_others_keeps_its_parts_through_the_cache() {
    // `60 * MINUTE` is an int only because `MINUTE` is one, which no single file can say -- so the
    // parts are kept, each name as the candidates PHP would try where it is written: a `define()`
    // inside a namespace declares a global name and still reads its value in the namespace. A part
    // that is neither a constant nor settled by the syntax -- a call -- keeps nothing.
    let fixture = r#"
//- /src/defs.php
<?php
namespace App;

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const FLAGS = \E_ALL & ~\E_NOTICE;
const NEGATED = -namespace\MINUTE;
define('DAY', 24 * HOUR);
const CALLED = strlen('a') * 2;
"#;
    let files = fixture_files(fixture);
    let built = build(&files, Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |entry| {
        files
            .get(built.skeleton.display_path(entry))
            .map(|text| phrost_model::LineIndex::new(text))
    });
    let constants: String = dump
        .lines()
        .filter(|line| line.trim_start().starts_with("constant "))
        .map(|line| format!("{}\n", line.trim()))
        .collect();

    expect![[r#"
        constant App\MINUTE @ 4:7-4:13
        constant App\HOUR @ 5:7-5:11
        constant App\FLAGS @ 6:7-6:12
        constant App\NEGATED @ 7:7-7:14
        constant DAY @ 8:9-8:12
        constant App\CALLED @ 9:7-9:13
        constant App\MINUTE: int
        constant App\HOUR: (int * value of App\MINUTE or MINUTE)
        constant App\FLAGS: (value of E_ALL bitwise (value of E_NOTICE bitwise value of E_NOTICE))
        constant App\NEGATED: (int - value of App\MINUTE)
        constant DAY: (int * value of App\HOUR or HOUR)
    "#]]
    .assert_eq(&constants);

    check_round_trip(fixture);
}

#[test]
fn a_call_rendering_a_template_survives_the_cache() {
    check_round_trip(
        r#"
//- /src/Pages.php
<?php
namespace App;

class Pages
{
    public function show(Page $page): mixed
    {
        return render('pages.show', ['page' => $page, 'title' => $page->title]);
    }

    public function edit(Page $page): mixed
    {
        return render('pages.edit', compact('page'));
    }

    public function boot(): void
    {
        Shared::share('site', new Site());
    }

    public function own(): mixed
    {
        return render('pages.own');
    }

    public function inherited(): void
    {
        partial('pages.part');
    }
}
"#,
    );
}

#[test]
fn a_call_configuring_what_closures_run_as_survives_the_cache() {
    check_round_trip(
        r#"
//- /tests/Setup.php
<?php
use App\Suite;
use App\Helpers;

configure(Suite::class)->with(Helpers::class)->in('Feature', __DIR__);
configure(Helpers::class);
"#,
    );
}

#[test]
fn what_a_migration_writes_and_the_table_a_class_names_survive_the_cache() {
    check_round_trip(
        r#"
//- /database/migrations/2020_01_01_books.php
<?php
use Db\Schema;

return new class {
    public function up(): void
    {
        Schema::create('books', function ($table) {
            $table->number('id');
            $table->text('title')->nullable();
        });
        Schema::table('books', function ($table) {
            $table->forget('title');
            $table->move('id', 'key');
        });
        Schema::rename('books', 'volumes');
        Schema::drop('old');
    }
};

//- /src/Shelf.php
<?php
namespace App;

class Shelf extends \Db\Model
{
    protected $table = 'shelves';
}
"#,
    );
}

/// A class a method's body returns, read as a mixin of the class -- a facade's root.
#[test]
fn a_mixin_a_body_states_is_in_the_dump_and_survives_the_cache() {
    let fixture = r#"
//- /src/Theme.php
<?php
namespace App;

use App\Theming\ThemeService;

class Theme
{
    protected static function getFacadeAccessor()
    {
        return ThemeService::class;
    }
}
"#;
    check_round_trip(fixture);

    let built = build(&fixture_files(fixture), Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |_| None);
    let mixes = dump
        .lines()
        .filter(|line| line.contains("mixes in"))
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r#"  App\Theme mixes in App\Theming\ThemeService @ 138..150"#]].assert_eq(&mixes);
}

/// A class a property's default names, resolved where it is written -- the one thing a factory's
/// `$model` says that its name does not.
#[test]
fn a_class_a_default_names_is_in_the_dump_and_survives_the_cache() {
    let fixture = r#"
//- /database/factories/ShelfFactory.php
<?php
namespace Database\Factories;

use App\Shelf;

class ShelfFactory extends \Db\Factory
{
    protected $model = Shelf::class;
}

class LooseFactory extends \Db\Factory
{
    protected $model = 'App\Shelf';
}
"#;
    check_round_trip(fixture);

    let built = build(&fixture_files(fixture), Order::ByPath);
    let dump = crate::dump_skeleton(&built.skeleton, |_| None);
    let kept = dump
        .lines()
        .filter(|line| line.contains("defaults to"))
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r#"  Database\Factories\ShelfFactory::$model defaults to App\Shelf::class"#]]
        .assert_eq(&kept);
}
