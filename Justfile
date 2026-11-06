# Run `just --list` for the full set. `just pr-ready` is the gate before asking for review.

# Full test suite.
test:
    cargo nextest run --workspace

# Fast inner loop for one crate.
test-one crate filter="":
    cargo nextest run -p {{crate}} {{filter}}

# Accept new snapshot output. Read the diff before committing it. `cargo test`, not nextest: nextest
# runs a process per test, and several of them rewriting one file leave only the last one's patch.
accept:
    UPDATE_EXPECT=1 cargo test --workspace --no-fail-fast

[doc("Everything this project changed in the vendored stubs, file by file.")]
stubs-diff:
    diff -ru third_party/php-prelude-upstream third_party/php-prelude \
        --exclude=README.md || true

[doc("Re-record the stub delta after a deliberate edit. Read `just stubs-diff` first.")]
stubs-accept:
    UPDATE_EXPECT=1 cargo test -p phrost-stubs --lib \
        the_local_delta_is_the_one_that_was_recorded

# Prettier formats the Markdown, at the same pinned version here and in CI. Hand-wrapped Markdown
# drifts with every edit.
prettier := "npx --yes prettier@3.8.0"

[doc("Formatting and clippy, the way CI runs them.")]
lint:
    cargo fmt --all --check
    {{prettier}} --check "**/*.md"
    cargo clippy --workspace --all-targets -- -D warnings

[doc("Format every crate, and the Markdown.")]
fmt:
    cargo fmt --all
    {{prettier}} --write "**/*.md"

[doc("Licenses of every crate the binary links, for a release archive: `just licenses [file]`.")]
licenses out="THIRD-PARTY-LICENSES.md":
    cargo about generate --locked --fail -c about.toml -m crates/cli/Cargo.toml -o {{out}} tools/dist/third-party-licenses.hbs

# Drive the real server over stdio against a project. See tools/lsp-smoke.py.
[doc("Drive the real server over stdio against a project.")]
[positional-arguments]
smoke *args:
    cargo build --release -p phrost-cli
    python3 tools/lsp-smoke.py "$@"

# Index a project and report what was found.
[positional-arguments]
index *args:
    cargo run --release -p phrost-cli -- index "$@"

# Ask what is at a position, the way an editor would.
[positional-arguments]
query *args:
    cargo run --release -p phrost-cli -- query "$@"

# Run every check over a project, the way CI would.
[positional-arguments]
check *args:
    cargo run --release -p phrost-cli -- check "$@"

# Refuse to let a project's finding counts grow.
#
# Per code and not by total, because a total hides a trade. The baseline lives beside the project it
# counts -- `phrost-baseline.toml` -- since a baseline taken on one checkout says nothing about
# another. `just ratchet-accept <project>` agrees to what a run found.
[doc("Refuse to let a project's finding counts grow.")]
[positional-arguments]
ratchet *args:
    cargo run --release -p phrost-cli -- check --ratchet "$@"

[positional-arguments]
[doc("Agree to what a ratchet run found, as the new baseline.")]
ratchet-accept *args:
    cargo run --release -p phrost-cli -- check --accept "$@"

# How much of a project's own code resolves, and where the gaps are.
[positional-arguments]
coverage *args:
    cargo run --release -p phrost-cli -- coverage "$@"

# Measure the request budgets on a real project. Release only, and one at a time:
# concurrent runs contend for the machine and produce numbers that describe the contention.
[doc("Measure the request budgets on a real project. Release only, one at a time.")]
[positional-arguments]
perf *args:
    cargo run --release -p phrost-cli -- perf "$@"

# Ask another language server the same questions and report where we lose. Results and method:
# docs/comparison.md. A utility, not a
# subcommand: both servers have to be black boxes reached over stdio, and a subcommand would link
# our index into the client. See tools/compare-lsp.py for the reference servers it knows.
[doc("Ask another language server the same questions and report where we lose.")]
[positional-arguments]
compare *args:
    cargo build --release -p phrost-cli
    python3 tools/compare-lsp.py "$@"

# What a session costs over its life, rather than what a server knows -- see tools/session-cost.py.
# `compare` cannot answer this: two memory readings at the ends of a run do not distinguish a
# server that has finished indexing from one still going, which is most of a memory comparison.
[doc("Measure what an editing session costs, phase by phase, for one server or two.")]
[positional-arguments]
session-cost *args:
    cargo build --release -p phrost-cli
    python3 tools/session-cost.py "$@"

# Whether a warm server answers what a cold one does, over real files of a real project.
# `check` and `query` are both cold starts, so neither can see a defect in the path that reads a
# stored index -- one such defect cost 1 315 catalog keys on every server start after the first while
# both of them passed every probe. This drives two servers, the second reading what the first wrote.
# Writes nothing into the project. Exits non-zero when the two disagree.
[doc("Compare what a cold server answers about real files against what a warm one does.")]
warm-start *args:
    cargo build --release -p phrost-cli
    python3 tools/warm-start-probe.py "$@"

# Score another project's fixture corpus against this server. Three exist and none of them is ours:
# PHPantom's, and the two the previous attempt left behind -- its own fixtures and the ones it had
# borrowed. A score is not a verdict; the ranked list of what we lose whole is what it is for.
#
# None of the three is a checkout this repo carries, and as of 2026-09-20 none of them is on this
# machine either -- the paths below are what to check out again, not what to run today. The tool
# refuses an empty directory rather than printing `0/0 passed`, which is what it used to do.
#
#     just corpus ../phpantom_lsp/tests/fixtures
#     just corpus ../phrost-old/crates/phrost-analysis/tests/fixtures
#     just corpus ../phrost-old/fixtures
[positional-arguments]
[doc("Score another project's fixture corpus against this server.")]
corpus *args:
    cargo build --release -p phrost-cli
    python3 tools/fixture-suite.py "$@"

# Structural invariants: the crate dependency matrix, and what may read from disk.
check-rules:
    cargo test -p phrost-checks

[doc("Dependency licences, advisories and sources.")]
deny:
    cargo deny check

[doc("Build the whole workspace in release.")]
build:
    cargo build --workspace --release

[doc("The gate before asking for review: lint, test, structural invariants.")]
pr-ready: lint test check-rules
