//! Which `use` lines nothing in the file names.
//!
//! Every fixture carries one import that genuinely is unused, so a snapshot is never empty by
//! accident. An empty answer would otherwise be indistinguishable from the projection declining to
//! answer at all — which it does for a file that did not parse — and a test asserting "nothing
//! unused" would pass for the wrong reason.

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World, with_open_scopes};

use crate::IndexView;

/// Every unused import in source order, with the local name a message would have to use.
fn check(fixture: &str, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let found = view
            .unused_imports()
            .iter()
            .map(|unused| {
                format!(
                    "{} use {} {} (local {})\n",
                    world.render_range(file, unused.range),
                    unused.kind.as_str(),
                    unused.path,
                    unused.local
                )
            })
            .collect::<String>();

        if found.is_empty() {
            "<nothing unused>\n".to_owned()
        } else {
            found
        }
    });

    expect.assert_eq(&rendered);
}

#[test]
fn an_import_no_name_reaches_is_reported() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Support\Repo;
use App\Support\Dangling;

class Service
{
    public function run(Repo $repo): void {}
}
"#,
        expect![[r#"
            5:5-5:25 use class App\Support\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn every_shape_of_type_hint_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Types\Plain;
use App\Types\Maybe;
use App\Types\Left;
use App\Types\Right;
use App\Types\First;
use App\Types\Second;
use App\Types\Held;
use App\Types\Returned;
use App\Types\Promoted;
use App\Types\Dangling;

class Service
{
    private Held $held;

    public function __construct(private Promoted $promoted) {}

    public function run(
        Plain $plain,
        ?Maybe $maybe,
        Left|Right $either,
        First&Second $both,
    ): Returned {
    }
}
"#,
        expect![[r#"
            13:5-13:23 use class App\Types\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn a_name_written_in_a_body_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Runtime\Made;
use App\Runtime\Tested;
use App\Runtime\Caught;
use App\Runtime\Called;
use App\Runtime\Named;
use App\Runtime\Cased;
use App\Runtime\Dangling;

class Service
{
    public function run(mixed $value): void
    {
        $made = new Made();
        if ($value instanceof Tested) {
            return;
        }
        try {
            Called::make();
        } catch (Caught $error) {
        }
        $name = Named::class;
        $case = Cased::First;
    }
}
"#,
        expect![[r#"
            10:5-10:25 use class App\Runtime\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn a_name_in_a_declaration_head_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Base;
use App\Contract;
use App\Helper;
use App\Attr;
use App\Dangling;

#[Attr]
class Service extends Base implements Contract
{
    use Helper;
}
"#,
        expect![[r#"
            8:5-8:17 use class App\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn a_docblock_type_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Docs\Param;
use App\Docs\Returned;
use App\Docs\Held;
use App\Docs\Thrown;
use App\Docs\Dangling;

class Service
{
    /** @var Held */
    private $held;

    /**
     * @param Param $param
     * @return Returned
     * @throws Thrown
     */
    public function run($param)
    {
        return null;
    }
}
"#,
        expect![[r#"
            8:5-8:22 use class App\Docs\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn a_grouped_import_is_used_or_unused_one_entry_at_a_time() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Group\{Wanted, Spare};

class Service
{
    public function run(Wanted $wanted): void {}
}
"#,
        expect![[r#"
            4:24-4:29 use class App\Group\Spare (local Spare)
        "#]],
    );
}

#[test]
fn an_import_used_only_as_a_namespace_prefix_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Models;
use App\Views;

class Service
{
    public function run(): void
    {
        Models\User::find(1);
    }
}
"#,
        expect![[r#"
            5:5-5:14 use class App\Views (local Views)
        "#]],
    );
}

#[test]
fn an_alias_counts_in_any_letter_case() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Support\Repository as Store;
use App\Support\Registry as Shelf;

class Service
{
    public function run(sToRe $store): void {}
}
"#,
        expect![[r#"
            5:5-5:25 use class App\Support\Registry (local Shelf)
        "#]],
    );
}

#[test]
fn a_class_name_does_not_use_the_function_or_constant_import() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Thing;
use function App\thing;
use const App\THING;

class Service
{
    public function run(Thing $thing): void {}
}
"#,
        expect![[r#"
            5:14-5:23 use function App\thing (local thing)
            6:11-6:20 use const App\THING (local THING)
        "#]],
    );
}

#[test]
fn a_call_and_a_constant_read_use_their_own_tables() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Thing;
use function App\thing;
use const App\THING;

class Service
{
    public function run(): void
    {
        thing(THING);
    }
}
"#,
        expect![[r#"
            4:5-4:14 use class App\Thing (local Thing)
        "#]],
    );
}

#[test]
fn a_constant_import_is_matched_case_sensitively() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use const App\THING;

class Service
{
    public function run(): void
    {
        echo thing;
    }
}
"#,
        expect![[r#"
            4:11-4:20 use const App\THING (local THING)
        "#]],
    );
}

#[test]
fn a_name_that_skips_imports_does_not_use_one() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Support\Repo;
use App\Support\Other;

class Service
{
    public function run(\App\Support\Repo $repo, namespace\Other $other): void {}
}
"#,
        expect![[r#"
            4:5-4:21 use class App\Support\Repo (local Repo)
            5:5-5:22 use class App\Support\Other (local Other)
        "#]],
    );
}

#[test]
fn a_file_with_no_namespace_reads_its_own_top_level() {
    check(
        r#"
//- /app.php
<?php

use App\Support\Repo;
use App\Support\Dangling;

$repo = new Repo();
"#,
        expect![[r#"
            4:5-4:25 use class App\Support\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn each_namespace_block_has_its_own_imports() {
    check(
        r#"
//- /app.php
<?php

namespace First {
    use App\Alpha;

    class One
    {
        public function run(Alpha $alpha): void {}
    }
}

namespace Second {
    use App\Alpha;

    class Two
    {
        public function run(): void {}
    }
}
"#,
        expect![[r#"
            13:9-13:18 use class App\Alpha (local Alpha)
        "#]],
    );
}

#[test]
fn a_name_nested_in_an_initialiser_or_a_closure_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Deep\Arrowed;
use App\Deep\Enclosed;
use App\Deep\Matched;
use App\Deep\Defaulted;
use App\Deep\Constanted;
use App\Deep\Stated;
use App\Deep\Dangling;

class Service
{
    public const KIND = Constanted::class;

    public static array $seeds = [Stated::class];

    public function run(int $value, string $class = Defaulted::class): void
    {
        $arrow = fn () => Arrowed::make();
        $closure = function () {
            return new Enclosed();
        };
        $picked = match ($value) {
            1 => Matched::First,
            default => null,
        };
    }
}
"#,
        expect![[r#"
            10:5-10:22 use class App\Deep\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn a_docblock_tag_that_says_which_class_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Docs\Mixedin;
use App\Docs\Methoded;
use App\Docs\Propertied;
use App\Docs\Extended;
use App\Docs\Templated;
use App\Docs\Dangling;

/**
 * @mixin Mixedin
 * @method Methoded make()
 * @property Propertied $thing
 * @extends Extended<int>
 * @template T of Templated
 */
class Service
{
}
"#,
        expect![[r#"
            9:5-9:22 use class App\Docs\Dangling (local Dangling)
        "#]],
    );
}

#[test]
fn an_attribute_argument_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Meta\Attributed;
use App\Meta\Argued;
use App\Meta\Dangling;

class Service
{
    #[Attributed(Argued::class)]
    public function run(): void {}
}
"#,
        expect![[r#"
            6:5-6:22 use class App\Meta\Dangling (local Dangling)
        "#]],
    );
}

/// The constructs a used import is invisible from, each one a false positive on real code.
///
/// Kept as a test rather than only as a comment so the list is measured rather than remembered: the
/// day lowering learns to see one of these, this snapshot fails and says so. Every entry is an
/// import the file genuinely uses.
///
/// It has already earned that three times. First-class callables -- `Statically::make(...)` and
/// `freely(...)` -- were recorded as *constants* by the generic walk, so the name was looked for in
/// the wrong symbol table. An anonymous class was lowered to nothing at all, taking its `extends`
/// and `implements` with it. And `@see Seen` became a name reference when `@see` was made
/// navigable, which is the same fact arriving from the other side: a name goto can follow is a name
/// the import is there for. Five entries have left this list; every time this snapshot is what
/// noticed, on the day the lowering changed.
///
/// What is left is `@phpstan-param Stanned`, and it is a real one: a prefixed tag's type is read
/// for what it *means* and the reference is not recorded, so an import a file uses only there still
/// reads as unused.
#[test]
fn a_used_import_is_still_invisible_from_these() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Blind\Statically;
use function App\Blind\freely;
use App\Blind\Based;
use App\Blind\Implemented;
use App\Blind\Seen;
use App\Blind\Stanned;

/**
 * @see Seen
 * @phpstan-param Stanned $value
 */
class Service
{
    public function run(mixed $value): void
    {
        $bound = Statically::make(...);
        $free = freely(...);
        $anonymous = new class extends Based implements Implemented {};
    }
}
"#,
        expect![[r#"
            9:5-9:22 use class App\Blind\Stanned (local Stanned)
        "#]],
    );
}

#[test]
fn nothing_is_reported_for_a_file_that_did_not_parse() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Dangling;

class Service
{
    public function run(): void
    {
        $this->
    }
}
"#,
        expect![[r#"
            <nothing unused>
        "#]],
    );
}

#[test]
fn nothing_is_reported_for_a_file_that_is_not_open() {
    // A closed file has no body IR, so nearly every name that reaches an import is missing from it.
    // The answer is refused rather than guessed -- the same shape as `locals_never_read`.
    let world = World::parse(
        r#"
//- /app.php
<?php
namespace App;

use App\Dangling;
"#,
    );
    let file = world.file_id("app.php");
    let open = world.open(OpenScope::CursorFileOnly, None);
    let view = IndexView::in_file(world.skeleton(), &open, file);

    assert_eq!(view.unused_imports(), Vec::new());
}

/// A name written only inside a docblock's **callable signature** counts.
///
/// The type keeps a callable's return and drops its parameters, deliberately -- and the names went
/// with them, so `use ...\Request;` beside `@param \Closure(Request): void` read as unused. 62 of
/// Symfony's 404 `unused-import` findings were on imports its own source needs, and this shape is
/// four of them; the rest are three further shapes named in the roadmap.
///
/// Both halves in one fixture, because only one of them was broken and a test that pins the working
/// half alone reads as coverage: the return already counted, being lowered into the type.
#[test]
fn a_name_in_a_callable_signature_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Docs\Taken;
use App\Docs\Given;
use App\Docs\Dangling;

class Service
{
    /**
     * @param \Closure(Taken): void $sink
     * @param \Closure(int): Given $source
     */
    public function run($sink, $source)
    {
    }
}
"#,
        expect![[r#"
            6:5-6:22 use class App\Docs\Dangling (local Dangling)
        "#]],
    );
}

/// A name written where a type has no written-PHP equivalent still counts as a use.
///
/// `lower_doc_type` answers `None` for a class constant, a conditional and a `key-of`, because none
/// of them is a type this layer models -- and answering `None` used to drop the *names* inside them
/// with the type. Three of the four shapes Symfony's own source writes; the fourth is
/// `\Closure<T of X>(...)`, whose bound mago's type parser does not model at all, so the name is not
/// in the tree to be found.
///
/// The conditional's `is` **target** is the one worth pinning: no other reader walks it. The
/// contract reader takes the branches, and a name written only in the test would be seen by nothing.
#[test]
fn a_name_in_an_unmodelled_type_counts() {
    check(
        r#"
//- /app.php
<?php
namespace App;

use App\Docs\Constant;
use App\Docs\Wildcard;
use App\Docs\Tested;
use App\Docs\Taken;
use App\Docs\Keyed;
use App\Docs\Dangling;

class Service
{
    /** @param Constant::KIND_A $one */
    public function plainConstant($one)
    {
    }

    /** @param list<Wildcard::KIND_*> $many */
    public function wildcard($many)
    {
    }

    /** @return ($x is class-string<Tested> ? Taken : int) */
    public function conditional($x)
    {
        return 1;
    }

    /** @param key-of<Keyed> $k */
    public function keyed($k)
    {
    }
}
"#,
        expect![[r#"
            9:5-9:22 use class App\Docs\Dangling (local Dangling)
        "#]],
    );
}
