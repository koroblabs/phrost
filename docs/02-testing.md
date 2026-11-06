# Testing

The most important document. Everything else describes what to build; this describes the only thing
that stops it rotting.

Eleven levels, ordered so that each is cheap only because the one before it exists. Where a level is
partly built, this says so — a testing document that describes intentions is the thing it exists to
prevent.

---

## Level 0. One fixture format

`phrost-testkit` reads one format, and every harness in the workspace uses it. The syntax is
`lspkit-fixture`'s, shared with orthra; what the flags mean is phrost's.

```
//- /composer.json
{"autoload":{"psr-4":{"App\\":"src/"}}}

//- /src/Repo.php
<?php
namespace App;
class Repo {
    public function find(int $id): ?User {}
}

//- /src/Use.php  open
<?php
namespace App;
function go(Repo $r): void {
    $u = $r->fi@member@nd(1);
    $u->na$0me;
}

//- /vendor/psr/log/LoggerInterface.php  vendor
<?php
namespace Psr\Log;
interface LoggerInterface {}
```

- `//- /path` starts a file. The path is always from the workspace root, with the slash; once
  parsed, phrost spells it without (`spec.file("src/Repo.php")`), as every layer above names files.
- Header flags: `vendor` / `stub` / `user` (origin), `open` / `closed` (**closed by default**, which
  is the state the previous attempt under-tested), `php:8.3` (per-file language version).
- `$0` is the default cursor, **with no lookahead**: `fin$0d()` puts it inside the identifier, which
  is the main case. `@name@` is a named marker, and a name appears once in the whole fixture. `@`
  rather than `$name$`, because `"$a$b"` read as a marker and a marker could never sit inside a
  variable name. A backslash makes either sigil text: `\$0`, `{\@inherit@at@doc}`.
- **Mandatory marker consumption** -- `assert_all_used()` fails with the name of an unused marker,
  so an accidental one is visible at once rather than as silently changed text.
- Trailing blank lines are layout, so every file ends in exactly one newline: the blank line that
  separates two files is not the first file's content.
- A duplicate path, an unknown flag, or content before the first header is a panic. A fixture is
  test code, and a typo must stop the test rather than quietly build a different project.
- `MaterializedFixture` writes to a tempdir, only for what genuinely reads disk: composer autoload,
  the workspace scan, the disk-cache round trip.
- Stubs in unit fixtures are minimal and synthetic. The real phpstorm-stubs appear only at level 8,
  because they make unit tests slow and flaky.

---

## Level 1. Layered fixtures

Each layer renders **its own** data and builds the project only as far as itself. This is the main
debugging tool: when an answer is wrong, the layer that first disagrees is where the bug is.

- `phrost-lower` renders bodies (`check_bodies`) and declarations, with every range in the snapshot
  — a tree with the right shape and wrong positions is worse than no tree, because it answers
  confidently about the wrong place.
- `phrost-body` renders a value path (`$this->find()[element]`, `not null instanceof Repo $x`).
- `phrost-index` renders the canonical index dump.
- `phrost-view` renders member accesses with their outcome.
- `phrost-analysis` renders whole feature answers.

---

## Level 2. `expect-test`, never a substring

**Every result check is a snapshot of the whole result.** Not "is `find` in the list".

A substring assertion lets ten junk completion items through, which is exactly how the previous
attempt hid regressions. `UPDATE_EXPECT=1 cargo test` rewrites snapshots, and the diff is where a
reviewer sees "this change rewrote forty completion lists" — which is the feature, not the cost.

Snapshots live **inline**, next to the test that owns them. A shared `expected.txt` covering a whole
suite is a permanent conflict magnet; the previous attempt had exactly that, and it is the single
worst thing it did to itself.

---

## Level 3. N queries per fixture

One setup, several checks that have to agree with each other:

```rust
check(
    FIXTURE,
    &[
        Query::goto("member goto", "member"),
        Query::hover("member hover", "member"),
        Query::completion("prop completion", "0"),
        Query::signature("argument", "arg"),
    ],
    expect![[r#" ... "#]],
);
```

This is what catches features drifting apart — goto landing in one place while hover describes
another. It happened twice this project: a hover card reading "no declared type" while the
completion list beside it offered the type's members.

---

## Level 4. Invariants as harness dimensions

The answer to "catch bugs I don't know about". These are not tests; they are **dimensions every
fixture runs through**, so one new fixture buys several invariants.

The previous attempt had `open_vs_closed_parity.rs` as _one file_ instead of a dimension over its
392 fixtures, and had no incremental check at all — which is why invalidation bugs shipped.

### `open == closed` — built

Every analysis fixture runs with its target file open and closed, and the answers must be identical.
`with_open_scopes` in `phrost-harness` is the dimension. Only the type of a local in a dirty buffer
may legitimately differ, and that is a separately marked category.

### `cold == warm` — built

`crates/index/src/tests/cache.rs` writes the index and reads it back, and compares the **whole
canonical rendering**. A field left out of the codec is invisible to everything else: it does not
fail to compile, it does not fail a feature test on a freshly built index, and it surfaces hours
later as one wrong answer in a session that started from the cache.

This has caught three real defects. Two were a branch of a contract written to the cache and never
read back. The third was not in the codec at all: `adopt` re-claimed one of the three kinds of
catalog key, so a warm start held 1 315 fewer keys than a cold one on the reference checkout.

Neither the codec pair nor that one was visible to any other test, and the third makes the reason
explicit: **this invariant only sees what the fixture declares and the rules install.** The fixture
had to carry a contract of every shape before the first two could be seen, and a _key-carrying name_
plus a _derived-key literal_ — and a rule for each, installed in `tests/mod.rs::build()` — before
the third could. A claim missing from both sides of an equality is an equality that holds.

The other half of the same lesson is where a warm start can be observed at all. `phrost check` and
`phrost query` are both cold, so neither can reproduce a warm-start defect however many probes are
run through them. Two instruments can: `phrost index <root>` against `phrost index <root> --warm`,
which compares the catalog _counts_ and so sees a lost key whether or not any file mentions one; and
`just warm-start <root> <file>...`, which drives two real servers over files that already exist and
compares what each publishes and answers.

The second is worth knowing the shape of, because the obvious version of it does not work. Comparing
_diagnostics_ only sees a lost key where the project has a mistake -- an open catalog says nothing
about a key that is merely absent, so correct code is silent on both runs. What sees it on correct
code is hover over **every string literal in the file**: a catalog key answers with what registers
it and a plain string answers nothing, so a lost key turns an answer into a null. No knowledge of
which literal is a key is needed, and none is used. Checked against the real defect: with `adopt`
put back the way it was, `just warm-start` names `NotificationManager.php:166:58` and exits 1.

### `incremental == from-scratch` — built

`crates/index/src/tests/incremental.rs` and `edit_fuzz.rs`. A fixture gives a sequence of edits; the
harness builds the index twice, incrementally and from the final state, and the canonical dump must
be **byte-identical**. `edit_fuzz` does the same with random breakage.

### `framework-agnostic` — partly built

A fixture with no framework marker should answer the same with detection on and off. The provider
seam makes this structurally true — nothing outside `providers/` names a framework — but it is not
yet a dimension every fixture runs through.

### `no-fallback` — built, differently than planned

The plan called for a `deny-fallbacks` cfg that panics on any second path. That was wrong: a cfg
only fires from code that calls it, so it can only catch a fallback whose author remembered to mark
it — which is the author who would not have written one.

What a fallback actually _is_ in a language server is a **read**: something asks the index, gets
nothing, and goes to disk to make up for it. So `crates/tools/checks/src/no_fallbacks.rs` holds a
file-by-file allow-list of every filesystem read with a reason for each, and a new read fails the
test until somebody writes down what it is for. Tests and the CLI are unpoliced; neither is on the
path a request takes.

---

## Level 5. Parser and lowering

- **Inline tests beside the rule.** A lowering rule and its fixture are in the same file.
- **A corpus parse test.** `crates/lower/src/tests/corpus.rs` — real PHP, zero parse errors, and
  every node's range contains its children's. That last one is why a position lookup can descend.
- **Fuzz.** `edit_fuzz` breaks a file at random and requires the index to stay equal to a fresh one.

---

## Level 6. Flow tests at the LSP boundary

`crates/lsp/src/tests/flow/` drives the server through the protocol: document lifecycle,
cancellation, readiness and progress, diagnostics publication, project-wide operations,
foreign-language buffers.

These catch what no unit test can. Two examples this project paid for: Zed silently drops
`$/progress` for a token it was not asked to create, so the indexing bar never appears and the
server looks hung; and a `.yml` buffer registered by the extension will pollute the index unless the
server refuses non-PHP buffers.

---

## Level 7. Differential testing against other servers

Two tools, both black-box over stdio, and neither links our index into the client.

**`just compare <project>`** asks another server the same questions over the same real checkout and
reports `match` / `phrost-only` / `reference-only` / `conflict`. Results live in `comparison.md`,
which is refreshed rather than edited. References: intelephense (the de-facto PHP baseline) and
PHPantom.

**`just corpus <dir>`** scores somebody else's fixture corpus. Three exist and none of them is ours:

| corpus                                                | what it is                                  |
| ----------------------------------------------------- | ------------------------------------------- |
| `../phpantom_lsp/tests/fixtures`                      | PHPantom's, much of it ported from Phpactor |
| `../phrost-old/crates/phrost-analysis/tests/fixtures` | the previous attempt's own                  |
| `../phrost-old/fixtures`                              | what it wrote beside the ones it borrowed   |

Worth borrowing for one reason: they were written by somebody else, about a server that is not ours,
so they ask the questions we did not think to ask. A score is not a verdict — a fixture we fail may
be a feature nobody built — but the **ranked list of what we lose whole** is what decides what to
build next.

The runner scores knowledge rather than layout: a hover written `$k: int` by one corpus, `$k = int`
by another and `int $k` by us is the same claim three ways, and an exact string would mark every
hover wrong.

---

## Level 8. Real projects

**`phrost check <project> --ratchet`** counts findings **by code** against `phrost-baseline.toml`
beside the project. By code and not by sum: four closed `undefined-method` against four new
`possibly-null-receiver` is the same total and not the same project. A code absent from the baseline
fails too — a new check is a decision, not an accident.

This is what makes strict nullable checks liveable. Strictness without a ratchet becomes noise.

**`phrost coverage <project>`** answers the other half: how much of a real project the index
actually understands. A check written against a layer that fails to type a quarter of the code
reports a quarter of the project as broken. It prints resolved / untyped / unknown-class counts and
— more usefully — the **ranked list of the shapes it could not read**, which is where most of this
project's work has come from.

What is still missing: a corpus in CI. The one everything is measured on is private.

---

## Level 9. Performance and memory as tests

- `just perf <project>` measures every request budget against a real checkout and exits non-zero
  when one is exceeded. `crates/cli/src/perf/budget.rs` is the source of truth for the numbers, not
  any document.
- `lspkit-memsize` attributes memory **by type**, not by RSS. "OwnerCatalog::classes — 38 MB" is a
  task; "RSS 400 MB" is a feeling.
- A perf-touching change with no measurement is incomplete. Report the p95 and RSS deltas in the
  commit.
- **Leaks are tests, not measurements.** `tests::a_session_gives_its_memory_back` drives the server
  in process through what a session repeats -- a buffer's open, edit and close; a save; a reindex; a
  server built and dropped -- and fails when `lspkit_heap::stats().live()` keeps growing after a
  warm-up. Something new a session does over and over gets a test there: one more `async fn`, the
  same shape. Each runs alone in its process, since the count is the process's.

Two ways to measure wrong, both paid for:

- **Run sweeps sequentially on an idle machine.** A concurrent `cargo build` inflates bootstrap from
  2.4 s to 4.4 s, which reads exactly like a regression.
- **Rebuild the release binary after `git stash pop`.** A stale binary has produced a wrong ratchet
  baseline once and an afternoon of chasing a phantom divergence once.

---

## Level 10. The zero-diagnostics corpus

Real code that is known to be valid, where the expectation is **no findings at all**. This is the
only way to keep strict rules and not drown in noise, and it is the missing piece: today the ratchet
plays the role, and a ratchet holds a number rather than proving a zero.
