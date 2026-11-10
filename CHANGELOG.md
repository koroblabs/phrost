# Changelog

A release's section is its notes on GitHub: the release workflow publishes the section whose heading
names the tag's version.

## Unreleased

- A variable completes again in Zed. `$ap` was answered with `$app` and the editor showed nothing:
  the item was sent to be filtered without its `$`, and Zed counts the `$` as part of the word.
  Every variable and static property was affected, in 0.1.0 through 0.1.2.
- Accepting a variable in a closure's `use (...)` no longer doubles the `$`: `use ($al` became
  `use ($$alpha)`.
- Of several classes with one short name, the one the file imports is offered first. In a file with
  `use RuntimeException;`, `new RuntimeE` put the MongoDB extension's class ahead of PHP's and wrote
  its full path when accepted.
- `setrawcookie()` takes its options array: `setrawcookie($name, $value, ['expires' => $at])` was
  reported as an array given for an `int`, because the stubs declared the function's two forms
  separately and only the first was read.

## 0.1.2

**The linter**

- `printf`, `sprintf` and `fprintf` given an unpacked value -- `printf('%s %d', ...$row)` -- are no
  longer reported as missing arguments, and a call that really is short says how many values it was
  given rather than repeating how many it needs.
- A deprecation raised the way Symfony and Drupal ask for it,
  `@trigger_error('...', E_USER_DEPRECATED)`, is no longer reported as an unsafe use of `@`. Every
  other silenced call still is. In Drupal core that was 368 of the rule's 483 findings.
- The deprecation rules that need PHP 8.1 or later run: an implicitly nullable parameter, a class
  named `_`, `(integer)` and the other long casts, a `case` ended by a semicolon, the backtick
  operator, a reference returned from a `void` function. They never had, because the linter was told
  no PHP version and took 8.0. A deprecation is shown as a warning, and a rule that advises newer
  syntax follows the oldest PHP `composer.json` requires.
- `assert-description` and `sensitive-parameter` are reported only where a project names them in
  `phrost.toml`: the first objects to `assert($x instanceof T)`, which the type checker narrows
  through, and the second decides by a parameter's name.
- `strict-behavior` asks `base64_decode` for strict decoding, where it said strict comparison.

**The type checker**

- A method an extension overloads, or declares differently for the library it was built against,
  takes every form the extension does: `Imagick::convolveImage()` with a kernel object or a matrix,
  `setImageBluePrimary()` and its siblings with two coordinates or three, `IntlCalendar::set()` with
  a field and a value or a whole date. The stubs declared each twice and only the last counted, so
  the other form was reported as a wrong or an extra argument.

**Faster**

- Diagnostics for a file of `define()` calls, or any long run of calls taking literals: a 3317-line
  stub file went from 145 ms to 6 ms.
- Code actions in a file naming classes nothing declares, which is every file of a project before
  `composer install`: at a cursor in Laravel's own tests, p95 from 1 489 ms to 19 ms.
- A value handed to hundreds of calls in one function: 800 calls, from 335 ms to 53 ms.

## 0.1.1

- Less memory on macOS: mimalloc is the process's allocator under `lspkit_heap::Heap`, so what the
  parser frees leaves the process. On a 29 094-file Drupal checkout the server after bootstrap went
  from 170 MB to 151-156 MB, and with 20 files open from 178 MB to 164-170 MB, with request
  latencies unchanged. Linux keeps glibc.
- `phrost perf` reads its memory figure after the release the server does at the end of bootstrap,
  and on macOS reports the footprint, which is what the machine gives up, rather than `ps`'s
  resident set, which never falls there.

## 0.1.0

The first release.

**The language server**, `phrost serve`, over stdio, for Zed, Neovim, Helix, Claude Code and any
other LSP client:

- go to definition, declaration, type definition and implementation; find references; document
  highlight; call and type hierarchy; document and workspace symbols;
- completion of members, names, array keys and docblock tags, with commit characters; signature
  help; hover with rendered docblocks; inlay hints;
- rename, including moving a file together with the namespace it declares; code actions for imports,
  docblock updates and quick fixes, and `source.fixAll`;
- semantic tokens, folding and selection ranges, and formatting in PER-CS, PSR-12, Laravel Pint,
  Tempest, hackfmt and Drupal styles;
- diagnostics as you type, and for the whole project in the background.

**What it checks:** undefined classes, functions and members; argument, return and property type
mismatches; strict null and `false` safety, where `?T` given for a `T` is reported on purpose;
unhandled `match` cases; missing `@throws`; unused variables, imports and declarations; and mago's
linter for style, safety and correctness.

**Types:** `@template` with bounds and defaults, `@extends`, `@implements` and `@use`, conditional
`@return` types, `class-string<T>`, array shapes and lists, `@phpstan-type` aliases, `@mixin`,
`@phpstan-assert*`, `@param-closure-this`, and narrowing through guards and assertions. On the
php-typing-conformance suite it passes more cases than PHPStan, phpantom, Mago, Psalm and
intelephense.

**Frameworks**, each behind its own contributor, none named by the generic code:

- Drupal: the service container, entity types and their fields, routes, hooks, form element keys,
  plugin annotations and attributes, `*.info.yml` namespaces, Twig templates, and a core installed
  outside `vendor/`;
- Laravel: Blade templates, Eloquent relations, accessors, casts and scopes, dynamic wheres, the
  container's aliases, facades, translations, factories, console signatures and Livewire;
- Symfony and Laravel through their own generics; PHPUnit, Mockery and Pest as they behave at
  runtime.

**Fast and lean.** On a production Drupal checkout of about 30 000 files, a cold server is ready in
about 2 s at about 195 MB, and goto, hover and completion answer in well under 1 ms at the 95th
percentile. Memory follows the project's own code, not `vendor/`.

**The command line:** `phrost check` runs every diagnostic over a project as CI would, and
`--ratchet` fails only when a count rises above a `phrost-baseline.toml` beside it. `index`,
`coverage`, `query` and `perf` show what is indexed, how much resolves, what an editor would see at
a position, and what each request costs.
