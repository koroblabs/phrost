# phrost

A fast, memory-lean PHP language server written in Rust.

phrost understands real-world PHP — generics and templates, conditional return types, array shapes,
narrowing through guards and assertions — and reports what it finds with PHPStan-style strictness,
while keeping resident memory proportional to _your_ code rather than to `vendor/`.

> **Status:** pre-1.0. Used daily on a large production Drupal codebase; the numbers below are
> measured, and every one of them can be re-taken with the tools in this repository.

## Why phrost

Measured on a production Drupal 10 checkout of ~31 000 PHP files (about 1 700 of them the project's
own), against the two servers people most often compare it with — see
[`docs/comparison.md`](docs/comparison.md) for the method and the raw tables:

|                                           |          phrost | phpantom 0.11 | intelephense 1.18 |
| ----------------------------------------- | --------------: | ------------: | ----------------: |
| Memory after opening 20–40 files          | **~240–310 MB** |     ~1 000 MB |         ~1 200 MB |
| Processor time for a whole session (cold) |        **13 s** |         218 s |                 — |
| Ready from a cold start                   |           2.3 s |     **2.0 s** |                 — |
| hover p95                                 |      **0.2 ms** |        2.6 ms |            5.7 ms |

On the third-party [php-typing-conformance](https://zonuexe.github.io/php-typing-conformance/) suite
— 249 cases, sixteen other PHP engines — phrost passes **98.0 %**, ahead of PHPStan (93.6 %),
phpantom (93.2 %), Mago (87.1 %), Psalm (81.5 %) and intelephense (76.7 %). Re-run it with
`python3 tools/typing-conformance.py <clone of the suite>`.

## Features

- **Navigation:** go to definition, declaration, type definition and implementation; find
  references; document highlight; call and type hierarchy; document and workspace symbols.
- **Editing:** completion (members, names, array keys, docblock tags) with commit characters;
  signature help; hover with rendered docblocks; inlay hints; rename, including moving a file and
  the namespace it declares; code actions (imports, docblock updates, quick fixes, `source.fixAll`).
- **Presentation:** semantic tokens, folding ranges, selection ranges, formatting (PER-CS by
  default, plus PSR-12, Laravel Pint, Tempest, hackfmt and Drupal styles).
- **Diagnostics as you type, and project-wide in the background:** undefined classes, functions and
  members; argument, return and property type mismatches; strict null- and `false`-safety (`?T`
  where a `T` is wanted is reported, on purpose); unhandled `match` cases; missing `@throws`; unused
  variables, imports and declarations; plus [mago](https://github.com/carthage-software/mago)'s
  linter for style, safety and correctness rules.
- **Type system:** `@template` with bounds and defaults, `@extends`/`@implements`/`@use`,
  conditional `@return` types, `class-string<T>`, array shapes and lists, `@phpstan-type` aliases,
  `@mixin`, `@phpstan-assert*`, `@param-closure-this`, and `$this` inside traits read as the classes
  that use them.

### Frameworks

Framework knowledge lives behind a contributor seam; no generic code names a framework.

- **Drupal** — first-class: the service container, entity types and their fields, routes, hooks,
  form element keys, plugin annotations and attributes, `*.info.yml` namespaces, and the
  docroot/`core` layout composer installs outside `vendor/`.
- **Laravel** — Blade templates read as the PHP their expressions are (goto, hover and completion
  inside `{{ }}` and directives; variables typed by the `view()` call, `@include` or `<x-...>` tag
  that renders the template, or by `{{-- @var --}}`; a Livewire component's template sees the
  component as `$this` and its public properties as variables; the checks run over the expressions
  too), Eloquent relations read as the properties they load (`$post->comments` is a
  `Collection<int, Comment>`, `$post->author` a `?User`, including relations written with a bare
  `BelongsTo` hint), accessors in both spellings (`getExcerptAttribute()` and `excerpt(): Attribute`
  read as `$post->excerpt`, typed by the getter closure the body hands over), attributes from
  `$casts`, `casts()`, `$fillable` and `#[Fillable]`, and the `id` key (a cast is nullable: whether
  a column can be null is in the database), local scopes (`scopePublished()` and `#[Scope]`) called,
  completed and navigated as `published()` on the model and on its builder, `Model::where(...)`
  forwarded to the query builder, dynamic wheres (`User::whereEmail($email)->first()`, offered per
  stated column), `auth()->user()` and `$request->user()` (the model `config/auth.php` names,
  nullable), the container's aliases through `app('router')`, `resolve('cache')` and
  `$app['config']`, the facades' global aliases (`use DB;`), `trans()` and `__()` as a string or an
  array by what the language files say the key holds, contracts read as the classes bound to them
  (`Cache::store()->lock()`, `Storage::disk()->url()`), and model factories that make one model
  unless the chain counted (`User::factory()->create()`).
- **Laravel, Symfony** — otherwise understood through their own PHPDoc and generics (collections,
  builders, the container's `@template` signatures), with generated directories (`bootstrap/cache`,
  `storage/framework`, `var/cache`) kept out of the index.
- **PHPUnit, Mockery, Pest** — test-runner control flow and mock objects are read the way those
  libraries behave at runtime, and a Pest test's `$this` is the test case its suite configures with
  `uses()` or `pest()->extend()`.

## Install

**Prebuilt binaries** for Linux (x86_64, aarch64) and macOS (Apple silicon, Intel) are attached to
every [GitHub release](https://github.com/koroblabs/phrost/releases). Each archive holds the
`phrost` binary with its licenses; put the binary somewhere on your `PATH`:

```sh
tar xzf phrost-aarch64-apple-darwin.tar.gz
install phrost-aarch64-apple-darwin/phrost ~/.local/bin/
phrost --version
```

**From source**, with a Rust toolchain (the exact version is pinned in `rust-toolchain.toml`, and
`rustup` picks it up automatically):

```sh
git clone https://github.com/koroblabs/phrost
cd phrost
cargo build --release -p phrost-cli
# put target/release/phrost somewhere on your PATH
```

## Editor setup

phrost speaks the Language Server Protocol over stdio: `phrost serve`.

**Zed** has no phrost extension yet, but its PHP extension's language server can be pointed at
phrost: Zed launches the binary a setting names instead of the one the extension would install. In
your `settings.json`:

```json
{
  "languages": {
    "PHP": {
      "language_servers": ["phpactor", "!intelephense", "!phptools", "!phpantom", "..."]
    }
  },
  "lsp": {
    "phpactor": {
      "binary": {
        "path": "/absolute/path/to/phrost",
        "arguments": ["serve"]
      }
    }
  }
}
```

The path has to be absolute (`which phrost` prints it). This attaches phrost to PHP files only: Twig
and Blade templates and Drupal's YAML files need an extension that registers the server for them
too.

**Neovim** (0.11+):

```lua
vim.lsp.config('phrost', {
  cmd = { 'phrost', 'serve' },
  filetypes = { 'php' },
  root_markers = { 'composer.json', '.git' },
})
vim.lsp.enable('phrost')
```

**Helix** (`languages.toml`):

```toml
[language-server.phrost]
command = "phrost"
args = ["serve"]

[[language]]
name = "php"
language-servers = ["phrost"]
```

**Claude Code:** a plugin is included in [`editors/claude-code`](editors/claude-code).

Any other editor that can launch a stdio language server works the same way.

## Configuration

Everything is optional. A `phrost.toml` at the project root:

```toml
# Switch a framework on that detection did not find.
framework = ["drupal"]

[files]
# Not this project's code, on top of what is skipped already.
exclude = ["docroot/sites/legacy", "tools/fixtures"]
# Code, whatever the ignore files say.
include = ["docroot/core/lib"]

[lint]
# Linter categories reported without being named.
categories = ["correctness", "safety"]
# Opt-in checks to switch on, and any code -- linter or type check -- to switch off.
on = ["doc-missing-tags", "unused-declaration"]
off = ["doc-undefined-type"]

[diagnostics]
# Check only open files rather than the whole project in the background.
project = false
```

The same settings can come from the client's `initializationOptions`.

## Command line

```sh
phrost serve                                 # the language server, over stdio
phrost check <project> [--ratchet]           # every diagnostic over a project, as CI would
phrost index <project> [--warm] [--memory]   # what gets indexed, and what it costs
phrost coverage <project>                    # how much of the project's code resolves
phrost query <project> <file> <line> <col>   # what an editor would show at a position
phrost perf <project>                        # request latency against the budgets
```

`phrost check --ratchet` compares a run against a `phrost-baseline.toml` beside the project and
fails on any code whose count rose, which makes it usable in CI on a codebase that is not clean yet.

## Development

```sh
just test        # the whole suite (cargo test --workspace works too)
just lint        # rustfmt, prettier for the Markdown, clippy -D warnings
just fmt         # format the Rust and the Markdown
just pr-ready    # everything CI checks
just deny        # licenses, bans, sources and advisories
```

`just`, `cargo-nextest` and `cargo-deny` are assumed:
`cargo install just cargo-nextest cargo-deny --locked`. Prettier runs through `npx`, so formatting
the Markdown needs Node.

The design is documented in [`docs/`](docs/README.md): the crate graph and its enforced boundaries,
the eleven test levels, the roadmap, and a record of the bugs already paid for. Performance budgets
live in `crates/cli/src/perf/budget.rs` and are checked by `phrost perf`.

## License

Copyright (c) 2026 Korob Labs <opensource@koroblabs.com>.

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option. The bundled PHP standard-library stubs are vendored from
[mago](https://github.com/carthage-software/mago)'s prelude under the same terms — see
[`third_party/php-prelude/README.md`](third_party/php-prelude/README.md) and [`NOTICE`](NOTICE).
Every release archive carries the licenses of the crates compiled into it, in
`THIRD-PARTY-LICENSES.md`.
