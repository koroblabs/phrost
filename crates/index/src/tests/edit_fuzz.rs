//! Random edits against the `incremental == from-scratch` invariant.
//!
//! Level 5 of `docs/02-testing.md`, and its own reason for existing: *this finds what a curated
//! set of edits will not*. Every other incremental test here is an edit somebody thought of, and an
//! invalidation bug is by definition one nobody thought of -- it surfaces hours later as a stale
//! answer with no way to trace it back.
//!
//! The `adopt` bug this project shipped and then caught is exactly the shape: a loaded index dropped
//! the catalog keys its files had claimed, and it was found only by deliberately breaking the fix to
//! see whether anything would notice. A sweep like this one notices without being told where to look.
//!
//! # Deterministic on purpose
//!
//! The sweep itself -- the generator, the comparison after every edit, the shrinking of a failure
//! to the edits that still fail -- is `lspkit_fuzz`, shared with orthra; the corpus, the palette
//! and what is compared are phrost's. Its edits include same-length ones, which orthra's mutation
//! tests showed are the only random edits a stale cache cannot hide from.
//!
//! Seeded, and the seed is in every failure message. A fuzz run nobody can reproduce is a bug report
//! nobody can act on, and one that finds a different thing on every CI run is a test that gets
//! disabled within a week. The seeds are fixed here; widening the range is a one-line change when
//! somebody wants to hunt.

use lspkit_fuzz::{Files, Subject, Sweep};
use phrost_model::LineIndex;

use crate::{
    dump_skeleton,
    tests::{Order, build, fixture_path, origin_of},
};

/// How many edits one case applies. Each is checked, so this is the number of comparisons.
const EDITS: usize = 80;

/// The seeds run. Fixed, so a failure is reproducible and a pass means the same thing twice.
///
/// Twenty is what every run pays for. A hunt widens it -- `1..=800` was run clean over this corpus
/// on 2026-09-03, 64 000 comparisons in 10 s on release, which is worth knowing before somebody
/// spends an afternoon assuming the seed count is what limits this. What limits it is the *corpus*:
/// the sweep can only break shapes that are in it, which is why `src/stored.php` was added when
/// four stored fields turned out to be shapes nothing here wrote.
const SEEDS: &[u64] = &[
    1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181, 6765, 10946,
];

/// Text worth breaking: the shapes whose *declarations* the index keeps, which is what invalidation
/// is about. A file of plain statements would exercise the walk and nothing that outlives it.
const CORPUS: &[(&str, &str)] = &[
    (
        "src/Repo.php",
        r#"<?php

namespace App\Data;

use App\Support\Loggable;
use App\Contracts\Reads;

#[Plugin(id: 'repo', label: 'Repository')]
abstract class Repo implements Reads
{
    use Loggable;

    public const string VERSION = '1';

    protected ?Repo $parent = NULL;

    /** @param list<Repo> $children */
    public function __construct(private readonly int $size, array $children) {}

    public function find(int $id): ?Repo
    {
        return $this->parent?->find($id);
    }
}

interface Reads
{
    public function find(int $id): ?Repo;
}
"#,
    ),
    (
        "src/Stage.php",
        r#"<?php

namespace App\Data;

enum Stage: string
{
    case Draft = 'draft';
    case Shipped = 'shipped';

    public function title(): string
    {
        return ucfirst($this->value);
    }
}

trait Loggable
{
    public function log(string $message): void {}
}
"#,
    ),
    (
        // The facts a *body* and a docblock leave on a stored row, which nothing else in this
        // corpus carries: a by-reference parameter's written-back type, a documented callback's
        // parameters, a body that reads arguments its signature does not name, a constant whose
        // type comes off its value, and a declaration a guard makes conditional. Each is derived
        // at lowering and re-derived on every re-ingest, so each is a way for an edit to leave the
        // index disagreeing with a fresh build -- and until `dump_skeleton` printed them, this
        // sweep could not have told.
        "src/stored.php",
        r#"<?php

namespace App\Stored;

class Facts
{
    public const AUTO = 3;

    /**
     * @param array $matches
     * @param-out array<int, string> $matches
     * @param callable(Stage, int): void $each
     */
    public function captured(string $pattern, array &$matches, callable $each): bool
    {
        $rest = func_get_args();

        return $rest !== [];
    }

    /**
     * @return array{Facts, string}
     */
    public function pair(): array
    {
        return [];
    }
}

if (!function_exists('App\Stored\guarded')) {
    function guarded(int $count): int
    {
        return $count + func_num_args();
    }
}
"#,
    ),
    (
        "src/functions.php",
        r#"<?php

namespace App\Support;

const LIMIT = 10;

#[\NoDiscard('the count is the whole point')]
function helper(int $count): int
{
    return $count + LIMIT;
}
"#,
    ),
    (
        // The stored facts the four files above do not write, each added when the dump learned to
        // print it -- which is the order this has to happen in: a shape the sweep can break is
        // worth nothing while the yardstick cannot see the difference. A property's hooks and its
        // write scope, a `#[\Override]`, a class declared `#[Attribute]`, a `final readonly`
        // class, a member an `@method` tag declares, a `@mixin`, and the arguments an `@extends`
        // supplies.
        "src/shapes.php",
        r#"<?php

namespace App\Shapes;

use App\Data\Repo;

#[\Attribute(\Attribute::TARGET_CLASS)]
final readonly class Marker
{
    public function __construct(public string $why = '') {}
}

/**
 * @method static Marker of(string $why)
 * @property-read int $counted
 * @mixin Repo
 * @extends Holder<Marker>
 * @template T of object
 */
class Shapes extends Holder
{
    public private(set) string $name = 'n';

    public string $upper {
        get => \strtoupper($this->name);
        set (string $value) => \strtolower($value);
    }

    public int $virtual {
        get => 1;
    }

    #[\Override]
    public function held(): ?Repo
    {
        return NULL;
    }

    #[\NoDiscard]
    public function counted(): int
    {
        return 1;
    }
}

/**
 * @template T of object
 */
class Holder
{
    public function held(): ?Repo
    {
        return NULL;
    }
}
"#,
    ),
];

/// Snippets an edit may paste in, chosen to make and break structure rather than to look like PHP.
const PASTES: &[&str] = &[
    "}",
    "{",
    "<?php ",
    "class Extra {}",
    "\n    public function added(): void {}\n",
    "namespace Other;\n",
    "use App\\Data\\Repo;\n",
    ";",
    "case Added = 'added';",
    "/** @var Repo */",
    "*/",
    "\"",
];

#[test]
fn random_edits_keep_the_incremental_index_equal_to_a_fresh_one() {
    let files: Files = CORPUS
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect();
    let sweep = Sweep {
        seeds: SEEDS,
        edits: EDITS,
        palette: PASTES,
        span: 16,
    };
    sweep.run(
        &files,
        &mut Subject {
            open: |files: &Files| build(files, Order::ByPath),
            edited: |built: &mut super::Built, path: &str, _: &lspkit_fuzz::Edit, now: &str| {
                let id = built.ids.assign(path);
                built
                    .skeleton
                    .ingest(id, &fixture_path(path), origin_of(path), now);
                built.texts.insert(path.to_owned(), now.to_owned());
                built.skeleton.mark_ready();
            },
            answer: |built: &mut super::Built| render(&built.skeleton, &built.texts),
            // Built in the opposite order, so an answer that depends on which file got which id
            // differs here and fails.
            scratch: |files: &Files| render(&build(files, Order::ReversedByPath).skeleton, files),
        },
    );
}

/// The dump, keyed by the texts as they are now.
fn render(skeleton: &crate::ProjectSkeleton, files: &Files) -> String {
    dump_skeleton(skeleton, |entry| {
        files
            .get(skeleton.display_path(entry))
            .map(|text| LineIndex::new(text))
    })
}
