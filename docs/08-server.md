# The server: protocol, lifecycle and features

> Part of the record §8 of `CLAUDE.md` began. Each note is a bug that was paid for once; the point
> of writing it down is that the _general_ shape at the end of each one recurs. Where a note says
> "§8", it means the record as a whole — `CLAUDE.md` §8 plus this family of documents. **Read this
> file before working in the area it names.** Add new notes here rather than to `CLAUDE.md`, and
> keep them in the shape the existing ones have: what looked true, what was true, what the tell was,
> and what generalises.

What stops the dispatch loop, what a client is allowed to ignore, what bootstrap does to open
buffers, and the feature-level bugs (completion, rename, highlight, tokens) that follow from reading
a position rather than a tree.

**The notes, by their tell:**

- The smallest stack in the process is a tokio worker's, not the main thread's.
- `tower-lsp-server` 0.23 contains nothing.
- Zed drops `$/progress` for a token it was not asked to create.
- The token layer replaces the grammar, so its kind choices _are_ the colours.
- A rename must match the name as _written_, not only what it resolves to.
- A response the client is slow to read stops the server just as dead as a blocked handler.
- A notification handler that does real work stops the whole server, and no gate inside a request
  can help.
- Every server-to-client request at startup is a place the whole server can stop.
- Bootstrap replaces the skeleton out from under the editor's open buffers, three times.
- The completion path reads its receiver from _text_, so every rule about written names has to be
  written there too -- and one was not.
- `(new Foo())->` was a deliberate miss that had outlived its reason.
- One declaration PHP reads as two needs both searches, and neither half is a subset of the other.
- A capability can be missing with no symptom but silence, and the instrument for that is the list
  of what is _not_ advertised.
- A project-wide pass publishes for the files it found something in, and "none" only for a file it
  had reported before.
- An optional field a client fills in by guessing is not optional, and the guess is about a language
  the client is not parsing.
- An A/B that looks like it isolates one variable, because the variable it isolates is the one you
  are working on.
- The completion scanner threw away half a vocabulary the paths it builds already had.
- Three answers to "where is this used", and the third had no copy of the rule at all.
- Every oracle read its sites off masked text, so nothing asked a question inside a string or a
  comment.

---

- **The smallest stack in the process is a tokio worker's, not the main thread's.** This note used
  to say full mago analysis recurses ~110 KiB per AST nesting level and a ~150-deep chain overflows
  2 MiB. That crate is no longer in the graph and the real figure is ~0.5 KiB a level -- but the
  conclusion survived and pointed at the wrong thread. macOS gives main 8 MiB and tokio gives every
  worker 2 MiB, and an open buffer's debounced diagnostics run on a worker. Measured: a 4 000-deep
  method chain in an open file overflowed one and aborted the whole server
  (`thread 'tokio-rt-worker' has overflowed its stack`, exit 134). A stack overflow is the one
  failure `catch_unwind` cannot contain, so the guard has to be the stack itself -- `main` runs on a
  64 MiB thread and the runtime is built with `thread_stack_size(64 MiB)`. Reserved address space,
  faulted a page at a time, and measured at no change in RSS.

- **`tower-lsp-server` 0.23 contains nothing.** It polls every request future in one task through
  `buffer_unordered` with no `tokio::spawn` in between, so a panic in a handler unwinds out of
  `block_on` and out of `main`: exit 101, the request never answered, the editor's language server
  simply gone. `lspkit_runtime::panics::Contained` wrapped the service in `serve()` and only there,
  until lspkit's own loop took the containment over (below). Two things to keep in mind:
  `panic = "abort"` in any profile removes it with nothing failing, which is why `Cargo.toml` states
  `panic = "unwind"`; and tokio's `RwLock` does not poison, so a panic in a _write_ path leaves the
  index readable and possibly half-updated. Containment buys a chance to restart, not a correct
  index. Since lspkit 56a95cb the loop is lspkit's own and phrost is no longer on tower: a request
  runs on a task of its own and its panic answers it, in the words `Contained` used, so containment
  no longer hangs on a wrapper somebody has to remember.

- **Zed drops `$/progress` for a token it was not asked to create.** The spec has a server send
  `window/workDoneProgress/create` before reporting against a token it chose; skipping it is
  tolerated by some clients and silently ignored by Zed, so the indexing bar simply never appears
  and the server looks hung. There is no error to notice — only an absent bar.

- **The token layer replaces the grammar, so its kind choices _are_ the colours.** A client told
  "full" draws nothing for a range nobody claims, which is why every keyword, string and comment is
  emitted -- and it also means a kind chosen carelessly is not a shade off, it is the whole colour.
  There are two other implementations to check against and they agreed with each other and not with
  us: tree-sitter's PHP grammar captures `(primitive_type)` and `(cast_type)` as `@type.builtin`,
  phpantom emits its `TYPE` for a cast, and phrost called both `keyword` -- so
  `function f(int $n): string` drew `int` and `string` in the same colour as `function`. Worth
  knowing where the queries live when comparing: a Zed extension's `languages/<lang>/highlights.scm`
  is the mapping from tree nodes to theme keys, and `phrost-zed` ships none, so Zed's own PHP
  grammar paints and our tokens override it wholesale.

- **A rename must match the name as _written_, not only what it resolves to.** `references_in`
  matched a class by resolution alone, so every name reaching it through `use X as Y;` was rewritten
  -- while the `as Y` binding that made those names work was left untouched, which breaks exactly
  the site the binding was keeping correct. Measured: 30 edits across 6 files, 8 of them misplaced.
  Resolution-only matching is right for _finding_ references and wrong for _editing_ them. Renaming
  an alias is also a different operation from renaming the class: it is file-scoped, so a
  project-wide count of what is "left in code" reads as a miss and is not one.

- **A response the client is slow to read stops the server just as dead as a blocked handler.**
  `serve()` handed `tokio::io::stdout()` straight to the transport, so the write happened inside the
  dispatch loop; a completion response is 80-135 KB on a real project and the pipe buffer is 64 KB.
  Measured with a client that stops reading: 0.0 % CPU, a hover sent afterwards **never processed**,
  and both arriving at once on drain. It reads exactly like a deadlock and the stack says so -- main
  thread parked in `block_on`, workers idle, and _no frame for the request_, because the future had
  already finished and only the write remained. `$/cancelRequest` cannot help; the loop never
  reaches it. A queue between the two fixes it: 510 of 2000 requests accepted became 2000 of 2000.
  Bounded at 8 MiB rather than unbounded, because trading a server that stops for one the OS kills,
  on growth driven entirely from outside the process, is not an improvement.

- **A notification handler that does real work stops the whole server, and no gate inside a request
  can help.** `initialized` ran bootstrap inline; the protocol has notifications processed in order,
  so until it returned no other future was polled. From outside that is a hang: no response, no
  panic, empty stderr, and the process not busy -- a warm bootstrap is 30 000 file reads, not
  compute. The tell that it is the _loop_ and not a lock: a request that should have been refused in
  microseconds came back after 3.8 s having been **answered**, which can only mean it was never
  polled. Whether it answers late or refuses late depends on where bootstrap happens to yield, so a
  client that asks for a progress bar sees a different bug from one that does not. Fixed by making
  `Backend` a handle over an `Arc<Session>` and spawning the pass; 3771 ms became 0.2 ms and a
  `shutdown` sent mid-index is now answered at all. Two consequences worth keeping: a panic in a
  spawned task is caught by tokio rather than killing the process, so a failed pass has to say so or
  the server claims to be indexing forever; and a reindex can now arrive _during_ startup, so a pass
  holds a mutex for its whole length.

- **Every server-to-client request at startup is a place the whole server can stop.** There are
  three -- create the progress token, register the file watchers, offer the type hierarchy -- and
  all three were `await`ed without a bound, with indexing behind them. A client is allowed to ignore
  an unsolicited request, and the first one that did left the process at 18 MB answering four
  minutes of requests with "phrost is still indexing this project". Nothing times out, nothing logs,
  and the index is simply never built. All three are optional, so `Backend::ask_client` gives each
  five seconds and carries on. The reason the tests never saw it is that the flow harness's drainer
  answers everything; `Fixture::with_silent_client` is the one that does not.

- **Bootstrap replaces the skeleton out from under the editor's open buffers, three times.**
  `load_cache` adopts a stored skeleton whole, the batch loop re-reads each file from _disk_ and so
  discards unsaved edits, and `forget_undiscovered` drops anything discovery did not return -- and
  discovery is autoload-driven, so a scratch file outside every source root is not in it. An editor
  restores its tabs the instant `initialized` goes out, so `didOpen` overlaps all three on every
  real start. The symptom is a class reading as `undefined-type` in the file that declares it,
  published once and corrected by nothing: `check_open_documents` at the end of bootstrap recomputes
  against the index the file has already fallen out of, so the _last_ publish is the wrong one.
  Typing a character cures it, which is what makes it read as a diagnostics bug rather than an index
  one. `Workspace::reingest_open_documents` puts the buffers back, after `load_cache` and again
  after discovery settles; the editor is the authority on a file it holds open.

- **The completion path reads its receiver from _text_, so every rule about written names has to be
  written there too -- and one was not.** A dangling `::` does not parse, so there is no expression
  to read; `context.rs` scans backwards instead. It stored the class name with the leading separator
  stripped, and a stripped `\Drupal` resolves as `Drupal`, which inside `namespace Drupal\app_core;`
  is `Drupal\app_core\Drupal` and is nothing. So completion after **the commonest static receiver a
  Drupal project has** offered nothing -- 256 sites of one project's own code, 469 leading-separator
  receivers, 7 076 across its checkout -- while the same class reached through an import offered
  everything, which is what made it look like a resolution problem rather than a scanner one. A test
  asserted the stripping, so it pinned the bug and read as coverage; that is the second time this
  session a test named a rule it did not test. Two general points. `NameAnchor::of` now holds the
  three spellings (`\Foo`, `namespace\Foo`, `Foo`) in one place, because there were already two
  copies and the scanner was about to be a third. And the blast radius of a text-scanner defect is
  wider than the capability it lives in: `written_class` also reads a `class-string<T>` argument, so
  a fully-qualified `\App\Foo::class` was resolving relatively too -- fixing the separator moved
  `declared object` 232 -> 115 and resolved receivers 97.9 % -> 98.5 % with no other change.

- **`(new Foo())->` was a deliberate miss that had outlived its reason.** The same scanner answers
  `ReceiverRoot::Unknown` for "a function call's result, a parenthesized expression, an invoked
  callable", which is right for `($a ?? $b)->` and wrong for the one parenthesised shape that names
  its own type. 42 sites in one project's own code. The tell that it was cheap: goto and hover on
  `(new Foo())->bar()` already worked, because a _complete_ expression parses -- so only the
  incomplete-buffer path was blind, and the fix is eight lines of scanner plus nothing downstream,
  since what separates `Foo::` from `(new Foo())->` is the operator and the steps already carry it.

- **One declaration PHP reads as two needs both searches, and neither half is a subset of the
  other.** `__construct(public int $across)` binds a parameter _and_ declares a property; the
  parameter is found by the body's scope chain and the property by the member tables, so rename took
  whichever half the cursor was on and stopped. From the parameter: 4 edits in 1 file, every
  `$this->across` in the project left spelled the old way. From the property: 3 edits, the
  constructor body left reading a variable nothing binds. Both directions destroy the class and
  nothing fails -- the audit reads "4 edits" as a working rename. 1508 of them in 431 files on the
  reference checkout, so this is how modern PHP writes a constructor rather than a corner. What made
  the fix cost no index bytes is that a promoted property's `name_range` **is** its parameter's, so
  asking the body what binds that offset is what tells promoted from ordinary -- no flag to store,
  no row to widen. Two details are load-bearing: the body half must be added only when the
  declaration is in the file being swept, because the offset means a different variable everywhere
  else; and `prepareRename` needed the same widening, since it looked for a declaration or a member
  access and a variable in the constructor body is neither -- so the dialogue refused to open at a
  position the rename already handled. Related and worth checking together: `documentHighlight`
  loses its read/write colouring on such a parameter, because it now answers as a member, and gains
  the `->across` sites in the file, which is the better half of that trade.

- **A capability can be missing with no symptom but silence, and the instrument for that is the list
  of what is _not_ advertised.** Every capability phrost declares measured "real", and `@see` was
  navigable nowhere -- goto over it answers exactly what it answers over a comma, so nothing
  distinguishes "unsupported" from "nothing there". It was found by asking what a docblock's _other_
  tags do: `@param`, `@return` and `@throws` all navigate, because lowering records a name reference
  for the type it read, and `@see` is the one that names a class without being the type of anything.
  Drupal writes it 11 435 times in core and vendor. Two things made it cheap and one of them is the
  general point. `NameRefData` lives in a `FileHir`, which exists for an open buffer -- there is no
  `name_references` row in the index memory report -- so a pass gated on the same flag as the bodies
  costs the project nothing: 88.9 MB and 3.06 s either way. And a docblock name is already exempt
  from `undefined-type`, `undefined-function` and the add-import action, three readers that each
  independently state a missing name in a docblock is a wrong annotation rather than an error -- so
  emitting a reference for prose (`@see Monica`, and 435 like it) is safe without judging it, which
  lowering could not do anyway. What is worth carrying: **before adding data to answer a question,
  check whether the exemptions the question needs are already written down somewhere.** Three were.

- **A project-wide pass publishes for the files it found something in, and "none" only for a file it
  had reported before.** A client keeps exactly what it was last sent, so the two rules together are
  what keep the panel right without thirty thousand notifications saying nothing at start-up:
  `Session::note_project_findings` remembers which closed files were reported, and a later pass or a
  closing tab clears exactly those. Closing a tab now publishes what the _disk_ holds rather than
  "none" -- a problem in the file stays in the panel as it would for a file never opened. The pass
  skips open buffers (theirs come from the buffer, on the buffer's schedule), yields between files,
  reads a generation counter so a save mid-pass supersedes it rather than queueing a second one, and
  is `check`-shaped work: 1 711 files in 1.15 s on one thread and 0.32 s on four, where `check`
  takes 2.4 s with its own start-up. The workers are threads under `spawn_blocking` taking the next
  file nobody has taken, each holding the workspace's read lock for one file, and eight buy 0.21 s
  for two more megabytes -- the memory barely moves with the count, because a project's own files
  are small. What the pass leaves behind is ~30 MB on a warm server (144 -> 175 MB): the
  flattened-member memo filled to its 256 tables by a walk over every class, plus ~9 MB of freed
  small allocations. That is the price of the memo that makes every request 2-5x faster, priced here
  so nobody hunts for a leak. Two test-harness rules followed from it: the fixture's `initialized()`
  waits for the project pass as well as the indexing one, and a pass that had nothing to do --
  switched off, or a bootstrap that fell over -- still signals its end, or every test waits forever.

- **An optional field a client fills in by guessing is not optional, and the guess is about a
  language the client is not parsing.** `originSelectionRange` is what an editor underlines for a
  ctrl-click and follows the click inside; the protocol says omitting it means "the word range at
  the mouse position", and _word_ is the client's idea, applied to PHP by something that is not
  parsing PHP. Every jump this server answers left it `None`, so over the 43 characters of a real
  project's `use Drupal\app_core\Common\StorefrontUrl;` the link an editor draws is either the whole
  name or the one segment under the mouse, depending on whose word characters include a backslash.
  The span was never missing: `CursorTarget::range` is the range `documentHighlight` already answers
  with, and the one arm that is not a name -- a `{@inheritdoc}` tag -- was returning its own range
  from `inheritdoc_location` and having it dropped one line later. What it cost to say was a field
  and a split of `declaration_at` into `cursor_target` + `declaration_of`, so the position is read
  once rather than twice; `goto definition` p95 0.23 ms over 3 486 sites, unmoved.

- **And the report that prompted it was not about this field at all, which is the lesson.** Zed's
  ctrl-click on a class name "worked on some random attempt", with a log of 21 definition requests
  at one name. **All 21 answers were byte-identical**, so whatever varied was downstream of a
  constant input -- that much was sound. Then came an A/B: rust-analyzer's ctrl-click is reliable in
  the same editor, and driven directly it sends `originSelectionRange` where this did not. Same
  editor, same gesture, one field apart: I wrote that down as settled. It was not. **The two arms
  differed in two things**, and the reporter found the second: a PHP class called `Repo` lives in
  `Repo.php`, and a Rust struct called `Repo` lives in `repo.rs`. The collision between a symbol's
  name and its file's name is Zed's bug, and the example this record prints two paragraphs below --
  `clicked 6:6-6:10 -> Repo.php 6:6-6:10` -- is _exactly_ that case, sitting in the evidence the
  whole time. §8.2 already says a probe whose arms differ in two things can name neither; what is
  new is how easily the rule is broken on one's own probe, by an A/B that looked like it isolated
  one variable because the variable it isolated was the one being worked on. The field stays, and
  the paragraph above is its reason: the 43-character `use Drupal\app_core\Common\StorefrontUrl;` is
  a real span a client should not have to guess, and the protocol says so. It simply was not the fix
  for what was reported. The other half of that report is not a defect at all: goto on a declaration
  answers with that declaration (`declaration_at` says so in a doc-comment), so origin and target
  are the same span and nothing moves -- which is why `flow/goto_links.rs` pins that case by name,
  with the wire showing `clicked 6:6-6:10 -> Repo.php 6:6-6:10`.

- **Three answers to "where is this used", and the third had no copy of the rule at all.**
  Configuration is part of the reference space -- a service's `class:` names a class as surely as a
  `new` does -- and find-references and rename each grew their own `match` over which lookup a
  target kind gets. The **code lens**, whose whole content is a count of references, grew none: it
  counted the PHP and called that the total, so over a class wired up in `services.yml` it said
  nought while the list it sits above opened with one. A reader who can open the list and count a
  different number than the lens stops believing the lens, which is worse than having no lens.
  §8.1's oldest form, and the new part is the failure mode: the two copies _agreed_, so an A/B
  between them found nothing -- what was missing was a third caller nobody compared against.
  `answers::config_sites_for` is the one place now. Rename's extra step stays rename's: where the
  sites are is one question and _what to replace inside them_ is another, and only the second is a
  rename's business -- a class is written qualified in YAML and only its last word changes. Worth
  knowing when reading that count: `answer_references` ignores `includeDeclaration` and always lists
  the declaration, and the lens always excludes it, so the two differ by exactly one by design.

- **The completion scanner threw away half a vocabulary the paths it builds already had.**
  `ElementKey` has told a _literal_ index from a _variable_ one since it existed, and for two good
  reasons: a literal is what a shape is keyed by, and a variable names no shape key but still
  identifies the slot -- `$rows[$id]` read twice is the same element. The text scanner kept the
  first and rendered the second `[?]`, with a comment justifying it as refusing to read `$key` as
  the name of a field -- which is true and is the _other_ question. So
  `assert($rows[$id] instanceof Widget); $rows[$id]->` offered **nothing** while the same two lines
  with `'a'` for `$id` offered everything, and on a real file
  `\assert($fields[$field_name] instanceof FieldConfigInterface);` left the next line with no
  completion at all -- 0 items where another server offered 96, now 87. The tell is the one §8
  already records for this scanner: hover and goto worked at the same position, because they read
  the _expression_ and only completion reads characters. What is new is the shape of the loss -- not
  a rule missing from the scanner, but a distinction the scanner declined to carry that everything
  downstream was built to use. `ChainKey` carries both now, and `ChainKey::literal` is what a shape
  lookup asks, so the refusal the old comment was about is still in force one layer down.

- **Whose code a file is cannot be read off `vendor/`, and a framework that keeps its code elsewhere
  is checked in the one place nobody can dismiss it.** `SourceOrigin` is the package manager's
  directory and nothing more elaborate -- its own doc-comment insists on that, and is right to,
  because anything cleverer is a guess about a layout composer lets a project choose. What it
  therefore cannot see is Drupal: `Drupal\Core\` is registered at `<docroot>/core`, which composer
  installs, gitignores, and does not put under `vendor/` at all. So all **11 256** files of core
  counted as the project's own, in the worst of the two possible ways -- **not swept**, because
  discovery honours the gitignore that hides them, and **fully checked the moment one was opened**.
  Measured on the reference project: `docroot/core/lib/Drupal/Core/Url.php` published **10**
  diagnostics, index checks and doc checks and syntax lint together, to a person who opened it to
  read one method and can change none of it. The fix is not a cleverer `SourceOrigin` but a second
  question asked beside it: a framework provider knows where its own code lives, so
  `FrameworkProject::dependency_paths` says so and Drupal answers `docroot/core` plus
  `{modules,themes,profiles}/contrib` -- `custom` beside them is the project's own and stays
  checked. It is deliberately _not_ `excluded_paths`, which says the opposite thing about indexing:
  an excluded path is not indexed, and one of these is indexed in full, because goto and hover into
  a framework's source is most of what a person does with it all day. Only the checking stops. Two
  consequences worth knowing before reading a surprise. **Rename in there is now refused**, with the
  message `vendor/` already got -- which is right and was previously wrong: a sweep rewrites the
  call sites it can see and leaves the declaration alone, and the next `composer update` would
  revert the declaration anyway. And this rule has no home below the server: a provider is installed
  at this layer, so no view unit test has a `dependency_paths` to read, which is why core could be
  checked for as long as it was without a test failing.

- **Opening a buffer re-decided whose code the file was, from the one rule that cannot tell.**
  `set_document` recomputed `SourceOrigin` on every `didOpen` with `ProjectLayout::source_origin`,
  which reads the `vendor/` prefix and nothing else. Discovery decides the same thing from much more
  -- the stub set's own file list, and a walk of the roots composer named -- so every open
  **demoted** whatever discovery knew to whatever the prefix could see. A stub is extracted to a
  cache directory outside the project entirely, so cmd-clicking `DateTime` published **222**
  `doc-missing-tags` about `date.php`; Drupal's core, outside `vendor/` too, arrived the same way.
  §8.1's oldest form, with the tell reversed: the two constructors did not _diverge_, one silently
  overwrote the other, and only for files opened -- so a swept project and an opened file disagreed
  about the same file and nothing compared them. `set_document` now keeps the origin the vfs holds
  and guesses only for a path discovery never saw, which is the case that rule was written for: a
  scratch file the editor made a minute ago. The second half is not diagnostics at all.
  `origin.wants_full_analysis()` gates body lowering, and the comment beside it argues that an open
  vendor file must keep the declaration-only path "or the memory rule would start depending on which
  files somebody happens to have open" -- which is exactly what was happening to every stub and
  every core file anybody opened.

- **An apostrophe in English prose inverted the quote state for the rest of the file, and only the
  editor showed it.** The exact reachability record is fed by `quoted_words`, a scan for what is
  written inside a string -- a callable, a handler map, a route's `_controller`. It had no notion of
  a comment, so `Drupal's native authentication stack` in a docblock read as an opening quote and
  swallowed everything to the next apostrophe. Every string after that point was parsed inside-out:
  on one real subscriber `'onException'` eight lines below landed _outside_ a string, was never
  recorded, and a live event handler was reported as reached from nowhere. **The tell was two
  instruments disagreeing, and the honest one was the quiet one.** `phrost check` answers the coarse
  rule -- `written_in(text, name)`, a plain substring test that cannot be fooled by quoting -- and
  the exact record is consulted only after a full pass, which the CLI never reaches. So the server
  reported it and `check` did not, and the difference read like an open-vs- closed parity bug. It is
  not: driving one server and comparing what the project pass published for a file against what
  opening it published gave the same seven findings both times. Comparing a _server_ against a _CLI
  run_ is comparing two rules, and the probe that cost the hour was the one that did that without
  saying so. The fix is a state machine over code / line comment / block comment / string rather
  than a search for quotes, with one PHP-specific arm: `#[` opens an attribute and not a comment,
  and skipping to the end of that line would lose every name an attribute names. **What it uncovers
  is the point.** The broken scan did not merely lose names -- after inverting, it harvested words
  out of _code and prose_ and recorded them as reached, which spared declarations nothing calls.
  Fixing it moved the reference project from 128 to 185 members reached by nothing: 5 gone (its own
  false positives) and 62 new, sampled and true. `Vendor\VendorStockReport::isArchived` is called
  and `Vendor\ValueObjects\StockReport::isArchived` is not, which is exactly the class-from-method
  precision the exact record was built for -- and a spurious loose word for `isarchived` had been
  sparing both. §8.2's note in the other direction: a correctness fix can measure _worse_ because it
  stops masking a second one.

- **The checker and the editor answered two different reachability rules, and nothing said so.**
  `unused-declaration` has a coarse rule and an exact one, and which answers is a question about the
  _index_: until every file has had its say, all that is held is which names are written where, so
  the check falls back to "is this word written anywhere". The server gathers after bootstrap and
  then answers the exact rule; `phrost check` never gathered at all, so it answered the coarse rule
  for the life of the process. Same project, same version: **70 findings from CI and 131 from the
  editor**, with no way to tell from either output that two rules were involved. It also cost an
  hour of this session directly. A parity probe compared what a driven server published for a file
  against what `phrost check` reported for it, called the difference an `open == closed` failure,
  and sent me reading the open-document path. Comparing a _server_ against a _CLI run_ is comparing
  two rules; the honest probe drives one server and compares its project pass against its own
  open-file publish, and that one agreed all along. `Sweep::record_what_each_file_reaches` is the
  gather half, and it is free: three runs each way on a 1 725-file project gave 1.64-1.74 s with it
  and 1.67-2.30 s without, because the pass renders nothing and runs no check. `phrost unused` had
  always paid it, which is why _that_ CLI agreed with the editor and the one people run did not.

- **A diagnostic whose quick fix is not reachable from the squiggle has no quick fix.** When
  `doc-missing-throws` started reporting what a _call_ declares, 840 findings on the reference
  project arrived with nothing to apply: the action read `declaration.throws`, which is what the
  body _writes_. It also still offered `Add @throws X` for a class the interface already documented
  -- a fix for a finding nobody was being shown. Three readers of one question, and the two that
  were not the check had each drifted. `IndexView::undocumented_throws` is the seam now: the check
  reports one diagnostic per entry and the action offers one tag per entry, off the same list. The
  parsed docblock is handed in rather than read there, because parsing a tag belongs to the analysis
  layer and both callers had already done it -- what moved is the _rule_. **Reaching the squiggle
  took two more gates, and neither was in the offer.** `SubjectSites:: declarations` admits a
  declaration at its name, a parameter, or a _written_ throw -- the positions its offers act on --
  so a call site was filtered out before anything ran; and `holds(Declaration)` decides whether an
  offer may even look, using the same narrow list. Widening the shared list would have put "Write a
  docblock" on the menu from the middle of a method, so the wide list is a second accessor only the
  tag offer reads, and the gate consults it. What may be offered from a body position is then
  settled by the offer's own per-position guards, which already say "name, or parameter, or a site
  that raises this". Cost, and it is real: `code actions at a cursor` p95 0.77 ms -> 2.55 ms against
  a 12 ms budget, because working out what a body's calls throw means resolving them. A position
  prefilter (`a_call_is_named_at`, §8.1's `NeverReturns` shape) settles every position that is not a
  call name without resolving anything -- and it does **not** show in that number, because the sweep
  probes 3 489 member-access sites and those are exactly call names. It is worth keeping for
  precision rather than for the measurement: a cursor on a blank line no longer reports as holding a
  declaration at all. Its own note, because it is the one that cost the time: `call_site_at` is the
  _signature help_ query and skips any offset at or before the callee's end -- a cursor on `find` in
  `$repo->find(` is not yet writing an argument. Reaching for it as "is there a call here" answers
  no on exactly the position a throw is reported at.

- **Two spellings of one expression, and the completion scanner read only the older one.** PHP 8.4
  lets `new Foo()->method()` drop the parentheses `(new Foo())->method()` required, and the scanner
  had an explicit arm for the parenthesised form -- added when 42 sites of one project were found
  written that way. The arm fires when **nothing is named left of the parentheses**, which is
  exactly what tells an argument list from a parenthesised expression. Without the parentheses the
  class _is_ the trailing identifier, so that arm never sees the new spelling and it falls through
  to "a name with `new ` left of it", which is no operator and therefore the deliberate miss the
  scanner is built on. §8.1's three-spellings form, where the second spelling arrived after the rule
  was written. **111 sites in the reference project's own modules and 220 in Symfony** against 42
  for the older one: a project on 8.4 stops writing it. Worth knowing for the next reader of that
  entry: goto, hover and every check answered on the new spelling all along, because they read the
  _expression_. Only completion reads characters -- the same split §8 already records for this
  scanner. **Three things in this investigation were the instrument and not the code**, which is
  most of what it cost. A goto probe "failed" because the column was computed by hand onto the `->`
  instead of the name. A checker "lost" a finding because `rg ... | head -4` cut it off behind four
  `unused-variable` lines. And a probe with three dangling `->` in one method body reported only the
  first, because an unparseable statement drops what follows it -- §8.4 records that one by name,
  and reading it is evidently not the same as remembering it at the keyboard.

- **Hiding a member answers a question nobody asked, and the counts said so.** Completion left out
  every `__`-prefixed method, on the argument that PHP calls them rather than a person -- with one
  exception carved for `parent::__construct()`. The argument is wrong for four of them, and three
  real trees say how wrong: `->__toString()` is written by hand **51 times in Drupal core and 59 in
  Symfony**, `->__invoke()` 33 and 18, `->__construct()` 17 and 17, `->__serialize()` 15. They are
  members of the language, and a person reaching for one was told the class did not have it. What
  the complaint behind the rule was about is **crowding**: a class with a handful of them pushes the
  wanted members off the visible part of the list. Ranking answers that and hiding does not -- so
  `CompletionItem::demoted` carries the decision and the protocol carries it as a `sortText` of `~`
  plus the label, `~` being greater than every letter and digit. The key goes on the demoted rows
  only: the spec says an absent `sortText` falls back to the label, so mixing is defined and the
  other rows need nothing. Measured against intelephense afterwards: the "items only intelephense
  offers" list lost **every** magic method -- `__construct` alone was 822 offers, `__sleep` 451,
  `__wakeup` 177 -- and what is left at its top is static methods offered after `->`, which this
  project declines for a reason of its own written beside the rule. **Its neighbour was checked the
  same way and survives.** `reachable` declines a _static_ method offered after `->`, and that
  family is what now tops the "only intelephense offers" list -- `create`, `load`, `loadMultiple`,
  `preCreate`, `postLoad`, `sort`, about 720 offers. Counted the same way: a static method called
  through a receiver that is **not** `$this`/`self`/`static`/ `parent` is written **0 times in the
  reference project's own modules and 34 in the whole of Drupal core**. The relative case, which is
  `$this->assertSame(...)` and 1 182 sites of their own tests alone, is already offered -- the rule
  has carried that carve-out from the start. So the same principle that overturned the magic-method
  rule leaves this one standing, and the difference is that this one had been measured.

  **The way it was found is the part worth keeping.** It was reported by a person reading a summary
  that called the whole family "a position rather than a gap", and the answer was not to argue but
  to count what the corpora actually write. The first count came back all zeros, which was the
  instrument: a `rg` pattern beginning with `-` is read as a flag, so `rg -F "->__toString("`
  matched nothing and said so quietly. `-e` passes it as a pattern. §8 already records `-r`; this is
  the second letter of that family.

- **`= ...` was never a default; it was the index saying it had not kept one.** `ParamRow` recorded
  that a parameter _had_ a default and not what it was, so a signature rendered `int $flags = ...`
  -- which hover showed to a reader on every such method, and which is not PHP. The consequence was
  a repair that stopped short: `writable_signature` returned `None` for any method with a defaulted
  parameter, so "implement the methods this class requires" wrote some of them and said so in its
  title. Honest, and it fired on **21 % of the reference project's own interfaces and 30 % of Drupal
  core's** -- a quarter of every use of that action. Dropping the value was never an option: an
  optional parameter made required is a class that still does not load, which is why the refusal
  existed. Keeping it is 25 029 of a checkout's 148 917 parameters, 0.1 MB of text and **2.9 MB of
  index** (90.9 -> 93.8 MB), sliced from the source at lowering because what the reader wants is the
  characters the author typed. Every request budget unmoved. Seven snapshots changed and every one
  of them for the better, which is the tell that the old rendering was a placeholder rather than a
  decision: hover, signature help, the outline, override completion and two index dumps all went
  from `= ...` to the written value. The remaining refusal is a _promoted_ parameter, which declares
  a property as well and cannot be written back from a signature at all.

- **The rename audit's "left in code" column is a question, not a defect count, and an aliased class
  is where it reads high for a good reason.** It reported `QuoteId` -- which five files each import
  independently as `use ...\UUIDv7 as QuoteId;` -- as **5 edits in 1 file, 16 left**. Read as a
  silent partial rename, which is the worst thing rename can do. It is not one. An alias is
  file-local: renaming a use of it in `QuotesRepositoryPort.php` edits all five of that file's
  occurrences and leaves the other four files' own aliases standing, and every one of those files
  still compiles against the class they alias. Driven directly at four of the five, rename edits
  **every** occurrence in each: 7, 7, 5 and 6, with nothing untouched -- including both `@param`
  docblock lines and a PHP 8.4 property hook's parameter, which were the two shapes worth doubting.
  The column earns its place by making a human look. What it cannot do is tell "the same name in
  another file" from "the same _symbol_ in another file", which for an alias is the whole question.

- **The formatter is sound against the standard's own executable, measured rather than argued.**
  `tools/drupal-style.sh` over all 1 695 files of the reference project's custom modules, 697 of
  them reprinted: **introduced 0, removed 55** by the project's own phpcs -- 17
  `MultiLineFunctionDeclaration.Indent`, 9 each of `CloseBracketLine`, `FirstParamSpacing` and
  `MissingTrailingComma`, 7 `FunctionSpacing.After`. A day of type-layer work moved none of it,
  which is the point of running it: the formatter shares a parser with everything that changed.

- **`_ => {}` in the docblock painter swallowed ten tags mago had modelled all along, one of them
  `@phpstan-type`.** Reported from a real file as "there are no colours here, we have not covered
  everything", and _covered_ was the wrong word: `TagValue::TypeAlias` has existed since before
  `tokens/docblock.rs` did, and the match simply never grew an arm for it. Nothing failed, nothing
  was missing, and the tag had been grey for the life of the feature -- because a wildcard answers
  "not one of these" for a variant nobody added, which is §8.1's walk-by-shape form applied to an
  enum of fifty-eight. Ten were being swallowed: both type-alias tags, `@where`, `@sealed`,
  `@inheritors`, three more `@param-*` spellings, `@pure-unless-callable-is-impure` and `@trace`,
  plus `@template`'s default type, whose bound was read and whose fallback was not. The arms are the
  fix and the exhaustive tail is what keeps it fixed -- every remaining variant is now named as
  carrying no type and no variable, so the next tag mago models fails the build. Worth noticing for
  its own sake: an alias _definition_ drawn as prose is conspicuous precisely because every _use_ of
  it is already coloured as a type, so the gap reads as the feature breaking on one line rather than
  as a tag nobody taught it. That asymmetry is the tell for the rest of this crate -- a feature that
  colours a name in one place and not another is more visible than one that colours neither.

- **The project check reads `own_closed_files`, so for a long time the one file on screen was the
  only file in the project that never heard about a change on disk.** A watched change reindexes,
  logs `reindexed N`, and starts the project pass -- and that pass exists for the files nobody has
  open. So a method added by a colleague's commit, a `git switch`, or a `composer update` cleared
  the error everywhere except the buffer the person was looking at, where it stayed until a restart
  or a keystroke in that file. Nothing failed and the log said the right thing, which is why it
  survived: the symptom is "phrost needs restarting" and the evidence is a correct-looking reindex
  line. `check_open_documents_in_the_background` beside the project pass is the fix, off the message
  loop for the reason every other pass is. The neighbouring test
  `flow::project_diagnostics::a_change_on_disk_checks_the_project_again_and_clears_what_was_fixed`
  had made the same claim for a _closed_ file since the feature shipped, which is the tell for this
  whole family: an invariant tested on one side of a split is not tested.

- **The globs decide what the server can ever hear, and a framework's own configuration was not in
  them.** `watch_globs` listed PHP's extensions, composer's files and `phrost.toml`. Everything a
  provider contributes -- a service id, a route, a module's namespace, a field's kind -- is read out
  of the project's `.yml` by `set_root`, once, so adding a service to a `services.yml` changed
  nothing until the server was restarted; and even a client that volunteered the notification got
  `WatchedOutcome::Ignored`, because the path is not PHP. Two halves and the first is the one that
  cannot be debugged from inside the server: a file nobody was asked to watch produces no event to
  find. `FrameworkProvider::configuration_extensions` is the seam -- extensions rather than names,
  because the names live across four of the provider's own modules and a second list of them is the
  list that goes stale, and empty by default so a project with no framework never asks its client to
  report every YAML it happens to contain. The outcome is `ConfigurationChanged`, the same full
  re-read composer gets, because a provider reads its whole docroot to answer one question and there
  is no such thing as re-reading one of its files.

- **A `composer update` was read at its first write, and the client never reported its last.**
  Composer writes `composer.lock` _before_ it replaces a package, then the packages, then
  `vendor/composer/installed.json`; and it rewrites an autoload file only when the content changed,
  which a version bump does not. Measured on composer 2.10: lock at 8.0 s, package files at 10.6 s,
  record 11 ms later, autoload map untouched -- and on the reference project the same, four seconds
  apart. So the only configuration file a version bump writes was the one written at the _start_,
  the reindex it triggered walked a vendor tree half replaced, and every class of a package caught
  mid-replacement read as undefined until a restart. Nothing came to correct it because **Zed
  reports nothing under a gitignored directory**: its worktree holds 4 614 entries for a checkout
  with 52 000 more under `vendor/` and `docroot/core`, and a `**` glob is answered from that tree.
  It loads an ignored directory only for a watch whose pattern _starts_ with a literal path
  (`add_path_prefix_to_scan` in its `lsp_store.rs`), which a `RelativePattern` rooted at
  `vendor/composer` is and `**/vendor/composer/autoload_*.php` -- the glob phrost had always sent --
  is not. That watcher had never fired in Zed. Three halves now, each with a test that hangs or
  fails without it: the record is watched, rooted where the client says it can root a watch; a lock
  newer than the record is a run still going, so the read waits and the old index keeps answering;
  and the wait _looks_ at the two stamps every half second rather than trusting an event, because a
  client that cannot root a watch never sends one and FSEvents drops events under a burst of
  thousands of writes (`rescan: user dropped`, 464 times in one Zed log). The general shape: **a
  file that marks the start of an operation is the wrong trigger for reading its result**, and the
  file that marks the end can live where the client does not look. One case the rooted watch cannot
  reach: a checkout where `vendor/composer` does not exist when the watch is registered -- a fresh
  clone opened before `composer install`, or `rm -rf vendor` -- since Zed loads a prefix only if it
  is there to load, and the lock does not change either. What every client does report is the
  dependency directory's **own entry** appearing or going, because it is a child of the root;
  `**/vendor` asks for exactly that without a literal prefix that would load the whole tree, and the
  server treats it as the start of a run.

## `#[` ends in a bracket, so the attribute position has to be tested before the subscript one

The completion context scanner reads backwards from the cursor and asks "what stands immediately
before this name". `#[Rou|` ends in `[`, which the subscript branch claims -- so an attribute's
class name was answered with the keys of an array shape, and when that found none, with the whole
catalog including free functions and constants.

Two things fall out of fixing it, and the second is the one that broke a test. The attribute test
has to run _before_ the subscript test, because both match the same character. And it must not claim
the attribute's **arguments**: `#[Route(path: |)]` is an argument position like any other, so the
test is "a `#[` with no `]` **and no `(`** after it". Without that second half, named-argument
completion inside an attribute stops working -- which is what the existing test caught.

The class header is read the same way and has its own rule: the _last_ of `extends` and `implements`
on the line decides, since `extends A implements B|` has left the `extends` behind, and the keyword
the declaration opened with says which kind `extends` takes -- an interface extends interfaces, a
class extends a class. Both are matched without a trailing space, because the cursor sits where the
first letter of the first name goes.

## A class named by a string is a place a cursor can be

`class_exists('App\Repo')`, `is_a($x, 'App\Repo')`, a container definition keyed by a class name, a
Drupal annotation's `'entity_class'`: each holds a name the index knows, and none of them was
anything to goto, hover or rename. A rename moved every other spelling and left these naming a class
that no longer exists, which is worse than not offering the rename at all.

Three things make it safe. The string has to resolve to a class the index holds -- a string is data,
and nearly none of them do; the ones that do were written to name a class, because nothing else is
spelled `App\Repo`. It is read as **fully qualified**, which is what PHP does: a string is not
affected by the file's `use` statements, so `'Repo'` inside `namespace App;` names the global one
and sending goto to `App\Repo` would be wrong in exactly the files where both exist. And a
double-quoted string holding a `$` is refused, because the text at rest is not the text at run time.

The rename replaces the **last segment** of the string and leaves the namespace in front of it.
Handing back the whole path turns `'App\Repo'` into `'Store'` -- a class in the global namespace,
which is not where it lives, and which no test of the declaration alone would have caught.

It sits after the catalog reader at the same position, because a catalog key is the more specific
reading of the same text: a service id resolves to the class that implements it, and answering with
the name of a class that happens to be spelled like one would be the wider guess.

## `prepareRename` built its own list of what is at the cursor, and it was a shorter one

The dialogue must open exactly where the rename edits, and the two halves were built separately: the
protocol layer scanned the file's name references, its member accesses and the declaration under the
cursor, while the rename resolved a target and swept for it. A class named by a **string** is in
none of those three lists -- and it is a rename target, since `string_class_references` landed.

So on the reference project `prepareRename` at `is_a($x, 'Drupal\...\HomeDeliveryOrder')` answered
nothing while `rename` from the _same position_ edited **40 sites in 18 files**. An editor that asks
first offers nothing; one that goes straight to `rename` does all forty. Which of those a person
sees depends on their editor, which is the worst way for a rename to differ.

`rename_span` is the one answer now, in `phrost-analysis` where it can be tested -- the protocol
layer had no test for it, and that is half of why the gap lasted. Found by `tools/rename-audit.py`,
whose whole design is to ask each case three questions rather than "did it answer": prepare, edits,
and what is left behind.

Worth keeping as the tool's own lesson: its expectation for that row was `refuse`, written before a
string could name a class, and it read as a _pass_ the whole time -- a stale expectation in an
instrument is indistinguishable from a correct one until the behaviour under it changes.

## Renaming a member renames a contract, and phrost used to refuse the whole thing

`AttachmentProcessorInterface::processBatch` is not one declaration. Six classes implement it, and
an implementation left answering the old name no longer satisfies the interface -- so renaming one
half is a project that no longer runs, and renaming only the implementations is the same mistake
upside down. The refusal that stood in for the feature said as much and told the person to use
find-references on each declaration and change them together, which is a workaround rather than an
answer to the commonest refactor there is.

The family is walked both ways and each direction has its own reason. **Upwards**, transitively,
because a cursor is more often on an override than on what it overrides -- and because a class can
implement two interfaces that both declare the name, where renaming the first without the second
leaves it satisfying neither. **Downwards**, and _through_ a subtype that does not redeclare,
because `C extends B extends A` overriding at `C` is invisible from `A` if the walk stops at `B`.
That last one is what makes this more than the old one-hop check, whose own note admitted the hole.

Three refusals are left, and each is a case nothing can make safe:

- **A declarer the project does not own.** An interface under `vendor/` or in `docroot/core`
  declaring the same method makes the whole rename one the next install undoes, and renaming only
  our half leaves a contract nothing satisfies. Found on the real project at once: `AppWidgetBase`
  has 75 subclasses and its `__construct` reaches Drupal core's `WidgetBase`.
- **A family too large to enumerate.** The walk stops at 40 classes and says so. A hierarchy that
  big is not a rename an editor should offer, whatever it costs.
- **A method the language calls itself.** This one was a live bug rather than a design point:
  nothing in a project writes `__construct` -- its call site is a `new`, `__toString`'s is a cast,
  `__clone`'s is a `clone` -- so the rename edited the declaration, reported success, and left a
  class whose constructor no longer ran. The one shape where "no references found" is not evidence.
  Refused in both directions, because renaming `build()` _to_ `__construct` is the same mistake
  arriving from the dialogue.

**Measured on the reference project**, 28 851 files: `prepareRename` 1.1 ms and `rename` 3.5 ms at
the six-implementer interface, and 24.6 ms at the 75-subclass base class where the walk reaches core
and refuses. Each step downwards is a pass over the class table, which is what the bound is really
pricing; nothing here has a request budget, and `rename` was already sweeping every file in the
project for the name.

## A completion list filtered by what was typed is not a complete list

`isIncomplete` was set only when a list had been cut short by a limit, and nearly every list is cut
short by something else: the prefix. `$x->fund` is filtered by a loose subsequence match --
deliberately, since PHP member names carry their subject in the middle -- and the list then said it
was everything.

What that costs is the backspace. A client told the list is complete narrows the _cached_ items as
the prefix changes, with its own matcher, over a set that was already cut down by a different one.
So deleting back from `Attribute::TARGET_CLA` to `Attribute::TARGET_` shows two constants where the
class has eight: the other six were never sent, and nothing will ask for them again.

One rule now, set once in `completions` for every producer: anything typed means ask again. The
server cost is unchanged -- the flag adds no work, and the client re-asking per keystroke is what
the 12 ms budget is for, where completion measures 0.03 ms median and 0.06 ms p95 on the reference
project.

Worth recording how it was found, because the instrument got the _reason_ wrong: `compare-lsp`
reported `Attribute::TARGET_CONSTANT` as a completion only intelephense offers. It is in our stubs
and we do offer it -- at a cursor with nothing typed. The comparison was between our filtered list
and their unfiltered one, which is the half-normalisation the tool's own notes warn about; chasing
the wrong number found a real bug one layer down.

## The one local binding nobody writes

Every binding a local can have is a write somebody wrote -- a parameter, an assignment, a `foreach`,
a `catch`, a `static`, a destructuring -- and `preg_match($p, $s, $matches)` is none of them. PHP
creates `$matches` when the call runs, because the third parameter is by reference, so the search
that asks _what in this body writes this name_ found nothing and goto answered nothing. The **type**
was already right: `PathRoot::ByReference` exists for exactly this, and hover said `string[]`. Two
halves of one local and only one of them knew, which §8.1 calls "a local exists in two places" --
the sixth instance, after the destructuring one.

Which argument is by reference is the callee's business, and no layer below the index can say. What
makes answering safe is that it is asked **only** when nothing else bound the name at all: either
the parameter is by reference, or the code reads a variable nothing set -- and the second is what
`unbound-variable` reports, with the call still being where a reader would look. The match is on the
bare name and nothing reached through it, since `f($rows['k'])` never writes `$rows`.

Found by `compare-lsp` against intelephense on the reference project, where it was the _whole_ of
the remaining goto difference: 293 sites agreed, two did not, and both were `$matches`. Hover agreed
at 298 of 298. Worth recording what else that run said, since it is the instrument's own account:
the outline difference is intelephense listing 491 local variables and 39 namespace nodes that
phrost deliberately does not, and the completion difference was the `isIncomplete` bug above.

## A named argument says which parameter it fills, and signature help was counting commas

`store(overwrite: true)` fills the **third** parameter from the first position. Signature help
answered the first, because `active_parameter` took the argument's index and then refused whenever
anything before the cursor was named -- so of eight ways a person writes a named call, three pointed
at the wrong parameter and three pointed at none:

| written                                   | said | is  |
| ----------------------------------------- | ---- | --- |
| `store(overwrite: \|)`                    | 0    | 2   |
| `store('x', overwrite: \|)`               | 1    | 2   |
| `store(ttl:\|)`                           | 0    | 1   |
| `store(where: 'x', ttl: \|)`              | —    | 1   |
| `new Document(slug: 'a', title: \|)`      | —    | 1   |
| `str_pad('a', length: 3, pad_string: \|)` | —    | 2   |

The guard's own comment said why: "past a named argument, an argument no longer corresponds to the
parameter at its index." True, and the wrong conclusion -- `ArgumentSite::name` is right there, and
a name is the _most_ certain answer there is, not the least. §8.1's catch-all answering a question
its inputs could answer better, and the doc-comment claiming it shared the parameter hints' rule is
what made it read as settled. The two questions are different: a hint asks whether naming an
argument adds anything and a name already written answers no; this asks which parameter is being
filled. `free_parameters`, three functions down the same file, had been reading the names all along.

What survives unchanged is the positional half. `store(ttl: 5, |)` still marks nothing, because PHP
allows nothing positional after a named argument -- there is no parameter at that index to point at.
And a name no parameter answers to (`store(tga: 'x')`) marks nothing, which is what a mistake in the
call deserves.

Found by driving a real server over stdio and reading the whole envelope at eight cursor positions,
which is the loop the 2026-09-19 pass records: none of this is visible to any of the four
instruments, because no corpus score counts what a request _says_.

## PHP writes three things with `use`, and completion answered all three as the first

`use Acme\Report;` at the top of a file imports a **path**, anchored at the root. `use Timestamps;`
inside a class body takes a **trait**, resolved the way every other name in the body is.
`function () use ($rows)` captures a **variable**, and nothing else can stand there at all. Three
languages, one keyword — and `in_use_statement`'s own doc-comment named two of them in one sentence
("a `use` inside a class body imports a trait, and its own line still starts with the keyword")
before routing both to the import list.

The trait one was not a ranking complaint. `import_paths` labels an item with the **whole path** and
inserts the label, because a file-level `use` is root-anchored whatever it says — so inside
`namespace Ed` the offered item wrote `use Ed\Timestamps;` in the class body, which PHP resolves
relatively and reads as `Ed\Ed\Timestamps`. A fatal error accepted from a list, not a bad
suggestion. And the list itself was the whole catalog: interfaces, enums, functions, 200 items of
which one was a trait.

Which of the two a line is **takes the tree to say**, so the split is made where the view is:
`class_body_at(offset)` already existed for the override completion and answers exactly this. The
capture list is the other way round — textual and exact, since the `(` after the keyword is a mark
no other `use` in PHP has. Its search for that bracket is bounded to the current statement, the way
every other scan in `context.rs` is, so a completion that is not one of these does not pay for it.

Two refusals came with it and both are somebody else's rule restated: `$this` is not offered in a
capture list because a closure binds it itself and PHP refuses it as a lexical variable, and a name
the list already holds is not offered because capturing one twice is fatal — the same argument
`free_parameters` makes about a parameter something already filled.

Measured: completion p95 0.07 ms and completion-of-a-name p95 0.74 ms, both unmoved; 17/17 budgets.
Found by driving a real server and reading what came back at seven `use` positions, none of which
any corpus score can see.

## A type position had no types in it

`function run(): st|` answered `Stack`, `stat` and eighteen `stats_cdf_*` functions, and did not
answer `string`. `vo|` answered `Volatile` and not `void`. `private ar|` answered forty `array_*`
functions and not `array`. The commonest thing anybody writes in PHP, missing from the one position
where it is the only thing that can be written — and what stood in its place was a function and a
constant, neither of which can ever be a type.

Two halves, and the second is why the first went unnoticed: `NamePlace` had `New`, `Implements`,
`Extends` and `Attribute` and no notion of a type at all, so every signature, property and docblock
tag fell into `Anywhere`, which means `NameKinds::Any` — all three of PHP's tables. A place that
knew what could stand there would have had somewhere to put the words.

`TypePlace` names six, and the keywords differ between them because the _language_ does: `void`,
`never` and `static` describe a return and nothing else; `callable` is refused as a property's type;
an enum's backing type is `int` or `string` and not one name a project can declare
(`NameKinds:: Nothing`); a `catch` takes a class and no keyword at all; and a docblock has a
vocabulary PHP has no syntax for — `list`, `class-string`, `non-empty-string`.

Three things worth keeping from building the scanner:

- **The negative half is the load-bearing one.** A ternary's `:`, a named argument's `:` and a
  bitwise `|` all end where a type position ends. What tells them apart is that a return type's
  colon follows a `)` in a statement holding `function`, and that a parameter list's `(` follows the
  keyword rather than a callee's name. Each of those is a probe in `context::tests`, and they matter
  more than the positive ones: offering `void` inside a ternary is the failure this would have.
- **A docblock type holds hyphens and no identifier scanner counts them as one word.** Somebody who
  has typed `non-` has an empty prefix and an empty range to replace, so accepting `non-empty-array`
  would have left `non-non-empty-array` in the file. The prefix is re-read where the tag is known;
  widening the identifier in the scanner would have taken the minus sign with it.
- **Type keywords match case-sensitively and statement keywords do not**, which is what the two
  lists are for. `RETURN` is written in old PHP and runs; a type keyword is lowercase in every
  project and every formatter, so a capital at a type position is somebody reaching for a class.
  Matching loosely put `resource` above `Repo` for a typed `Re`, which is the whole cost of getting
  it backwards.

Measured: completion p95 0.06 ms and 17/17 budgets, five ratchets held.

## A declaration with no body is a scope, and had none

Standing on `$candidate` in `interface PriceFormula { public function score(Document $candidate); }`
answered nothing at all: no highlight, no rename, no references, no goto-type-definition. The same
cursor one word left, on the type, works; the same parameter in a method that has a body works.

`parameter_declaration`'s own doc-comment describes this failure and fixes it one layer in — a
signature is not inside the body, so a parameter's _name_ is a second way into the enclosing
declaration. What it did not fix is the declaration being found at all: `BodyFacts::declarations`
was built by a closure that pushed a row **only when there was a body**, so an `interface`'s method
and an `abstract` one were in no list, and the parameter-name branch had nothing to reach them
through. §8.1's two constructors of one rule, where the boundary runs along "has a body".

What made it a small fix rather than twenty is `FileHir::empty_body`: one body with nothing in it,
allocated per file, handed to a declaration that has none. Every reader that walks a scope's
statements — and there are about twenty — finds an empty list instead of learning what a missing
body means. It is not a claim that the declaration _has_ a body: `MemberData::body` is still `None`,
which is what tells an abstract declaration from a concrete one, and the `Declaration`'s own range
is `None` so the only way to reach one is by standing on a parameter's name.

Found by `tools/capability-audit.py` on the reference project, which scored `documentHighlight`
18/20 and `typeDefinition` 17/20 and named the two parameters in both lists. Worth recording what
else that run said, since the tool over-counts in one place: its `references` "bad" rows compare the
files it found against the files that write the _word_, so a declaration's own file, a
`services.yml` and `ClientResolverInterface` all count against `ClientResolver` — read by hand, all
three were exactly right. Index memory unchanged at 103.7 MB, 17/17 budgets, five ratchets held.

## Goto-type-definition searched backwards where hover read the site

`$result = $this->handler()->handle();` — hover on `$result` names the class and
goto-type-definition answered nothing, on the same offset, one key apart.

A local is typed three different ways depending on which _site_ the cursor is on: a parameter by its
hint, an assignment target by its own right-hand side, a use by searching backwards for what bound
it. `variable_at` is the function that knows which is which, and it is what hover asks.
`goto_type_definition` took the variable's **name** from it and then asked `variable_class`, which
only searches backwards — and an assignment target has nothing above it to find, because the one
thing that binds it is on the same line.

§8.1's oldest form, and the repair it prescribes is the seam rather than the catch: the class now
travels on `VariableInfo` beside the rendered type, out of the one function that worked out which
site this is. Not the `Held` — that stays inside the view, where `phrost-analysis` cannot see PHP
semantics — so what crosses is a class name, which is all a jump needs.

Measured by `capability-audit.py` on the reference project: `typeDefinition` 17/20 answered and 16
useful, now **20/20 and 19**; `documentHighlight` 18/20, now 20/20 with the bodyless-declaration fix
above. 17/17 budgets, five ratchets held.

## A chain rooted at a function call had no root

`reset($rows)->`, `array_pop($queue)?->`, `make_thing()->` — completion offered nothing. A method
call, a static call, `new Foo()->` and `(new Foo())->` all worked. The scanner's own comment said
why and had said it for a long time: a bare `helper()->` "needs a return type looked up by name
rather than a class -- not read yet."

So it is a root of its own, `ReceiverRoot::Function`, and the two things that answer it are the two
a completed statement would have used:

- **The declaration's return**, which is the whole of it for a project's own `make_thing(): Row` —
  and a framework whose `.module`, `.inc` and `.install` files are free functions by definition has
  a great many of those.
- **The written argument**, because `reset($rows)` is declared `mixed` and gives back a `Row` only
  on account of `$rows`. The contract says which parameter's elements come back, the characters say
  which local was passed, and resolving that local is what the call expression would have given.
  `reset`, `end`, `current` and `array_pop` are the whole family, and every one of them is written
  with a plain `$local` at a chain's root.

The arguments travel as **written text** rather than as literals, which is the one thing that had to
change beside the root: a `ReceiverStep::Member` keeps literals because what settles a method is
`Foo::class` or an id, and what settles one of these is `$rows`, which is no literal at all.

Three tests had pinned the old refusal and each named the rule correctly, which is why they were
worth reading rather than accepting: `a_plain_function_call_is_not_an_invoked_value` is still about
`__invoke` and now offers the return; `a_receiver_nothing_can_read_offers_nothing_to_terminate`
needed a receiver that is _still_ unreadable, since a call stopped being one; and
`what_this_scanner_cannot_read_it_says_so_about` lost two of its five cases to the new root —
`helper()[0]->` among them, because an element of a call's result was only unreadable while the call
was.

Found by `compare-lsp` against intelephense on the reference project: 1424 completion sites, both
non-empty at 1413, and of the eleven that differ eight were ours — two of them
`array_pop($grades)?->getBasePrice()`. Worth recording what the _rest_ of that run said, because the
ranked list reads as a gap and is not one: the 89 labels "only intelephense offers" are led by
`_serviceIds` and `_entityStorages` at 75 each and eight members of one trait at 16 each, and every
one of them is **`protected`**, offered on a foreign receiver where PHP would fatal. Read by
occurrence rather than by label, the gap is intelephense ignoring visibility. 17/17 budgets, five
ratchets held.

Attribute completion is the fourth use of the prefilter shape and the first outside a check. `#[`
takes any class-like the catalog holds, so the page was two hundred alphabetical rows that began
`AbstractClientStrategy`, `AccessToken`, `AccountPermissions` and reached no attribute at all;
measured on the reference checkout, 441 classes of 30 916 carry `#[Attribute]` and 21 787 places
write one. Three things decided the design. It is a **ranking and not a filter**, because PHP
refuses a non-attribute class only when something reflects on it and a declaration written before
its marker is ordinary. The tier sits **ahead of the origin tiers**, so Symfony's `#[Route]` and
Drupal core's `#[Action]` outrank the project's own ordinary classes -- the one position where that
is the right order, and the reason it could not be done by sorting the page after the cap, which
only ever reorders rows the alphabet already let through. And the list is **named by the index**
rather than found by walking: a bare `#[` has no prefix to stop a walk, so asking each candidate in
turn is a class lookup per name in the project on every keystroke.

Two things about that list are worth keeping. It holds folded _short_ names and is only ever added
to, so the class's own row still decides -- a stale name costs one lookup and cannot produce a wrong
row. And it is kept **sorted** rather than in claim order, because files are indexed in parallel and
a completion page that reshuffles between two identical sessions is worse than one that is merely
long. 17/17 budgets after, five ratchets held, index memory 104.0 MB.

Three notes from a side-by-side run that are about the _instrument_ rather than the server. The
outline row read `function 0` for phrost against 14 for intelephense, which looks like a whole kind
missing; driven at a real Drupal `.module` phrost answers **32 functions**, so the row is about what
the forty-file sample happened to hold and what the other server counts as one — its 623 variables
say what that is. Read the `function` row the way the tool's own note already reads the `variable`
one. Second, the `namespace` row is a genuine difference and a presentational one: intelephense
emits a namespace symbol per file and phrost does not.

Third, and the one that cost time twice in one session: **the probe harness takes the first `@` (or
`|`) in the body as its cursor**, so a fixture holding `{@inheritdoc}` or PHP 8.5's `|>` marks a
position inside the docblock or the pipe and reports the answer from there. Both read as the feature
being dead — `$out = $t |> trim(...)` hovering as nothing, an inherited parameter resolving to
nothing — and both were the marker. The tell is the printed `line:col`: check it against the fixture
before believing a `(nothing)`. Pass a `cursor` the language cannot write, such as `§`.

- **One walk answered "where is this used" and "what may a rename replace", and it answered the
  rename's question.** A file writing `use Illuminate\Support\Collection as BaseCollection;` reaches
  the class at every `BaseCollection`, and find-references, the code lens count and the in-file
  highlight all stopped at the alias -- because `references_in` filtered name references down to the
  ones spelling the class's own short name. That filter is right for a rename and only for a rename:
  the `as` line keeps working, so replacing the spelling breaks exactly the sites the alias was
  keeping correct. The split follows the one already there for members (`Member` against
  `MemberFamily`): a rename-only `ClassUnderItsOwnName`, produced by `rename_target` and by nothing
  else, with `Class` meaning every spelling. Made in that direction rather than the other because
  there are exactly three rename entry points and all three go through `rename_target`, so the
  narrow reading cannot be reached by accident -- where a search path left on the old variant would
  merely under-report. A method call written in another case was the same bug in the other half: PHP
  folds case on a method name and the narrowed projection did not, so `$this->assertInstanceof()`
  was missing from `Assert::assertInstanceOf`'s references, and from the rename that has to rewrite
  it.

- **`tools/navigation-roundtrip.py` is the instrument both of those came out of, and it needs no
  expected output.** If goto at a site answers declaration D, then references at D must list that
  site: one direction is the index's forward edge and the other its reverse, they are built by
  different code, and a disagreement is a bug in whichever is smaller. Nothing about a project has
  to be known to say one exists, which is what makes it runnable against any checkout. Two things it
  taught about writing such a probe. A site is asked about from _inside_ its identifier, because
  servers disagree about whether the first column is on the name -- so comparing the cursor against
  each reference's `start` scored 145 of 150 as missing on a server that had answered all of them,
  and the comparison has to be containment. And three of its findings were its own: `new class` is
  an anonymous class with no name to reference, `self`/`static`/`parent` are deliberately not listed
  as the class, and the `X` of `use A\B as X;` is the alias's declaration rather than a use of the
  class. All five corpora round-trip whole once those are named.

- **Completion offered nothing at three shapes goto answered, and the instrument that found all
  three asks only that.** `tools/completion-offers-what-is-written.py` takes every `->` and `::`
  site the project already writes, keeps the ones goto resolves, and asks whether completion at that
  very position offers the name written after it. A miss is then completion's own -- the ranking,
  the cap or the filter -- and not a type the project failed to state, which is what makes it
  answerable without an expected output and on any checkout. The three:
  - **A union with an operand naming no class.** Covered above; the reading is
    `ReceiverClasses::Any`, and whether _every_ operand named one now travels with it so the row can
    say `Alpha only` rather than read as certain.
  - **A call anchored with `namespace\`.** `returned_by_written_call` took the anchor from
    `NameAnchor::of` and passed the written name on with the marker still in it, so it looked for a
    function literally called `namespace\here`. The class half of the same question has taken both
    halves of that pair since the `\Drupal::` note; the function half had not. Two constructors of
    one rule, with the _pair_ being the seam that already existed.
  - **A conditional `@return`.** The path layer has read `Contract::ConditionalReturn` since it
    existed; the chain walk never did, so it fell through to the declaration -- which a method
    written that way does not have -- and then to the body. Laravel's `Request::json()` returns a
    property, so `$request->json()->` offered nothing. What made this one work is that a conditional
    turns on what the _call_ wrote, and a text scan can read two things about a call: how many
    arguments it had, and which of them were strings. So `CallFacts::written` carries the scanned
    argument list -- `None` for a reader with a real call, which reads the expressions instead, and
    for a step that is not a call at all, which is how "a call with no arguments" stays apart from
    "not a call". That count settles `($key is null ? ...)` and the strings settle
    `($something is string ? ...)`, which is Mockery's `expects()` and every `->with()` after it.
    All five corpora offer what the code writes at every site once those are closed, where Laravel
    was 430 of 440.

- **Filtering the anonymous class out of the outline took every method of one with it.** The row
  itself is unnavigable -- its name is synthesised from an offset -- and that was the argument for
  dropping it. The members are the point: `new class { public function reset(): void {} }` is how
  Symfony and Laravel write a test double, and eleven methods in one Laravel file were invisible to
  "go to symbol in file", to the breadcrumb, and to the docblock reader that walks the same answer.
  Listed now under `class@anonymous`, which is what PHP's own errors say; the offset stays in the
  index, where telling two of them apart is what it is for. The perf sweep's `document symbols` row
  went from 95 % answered to 100 % on the same sample, which is the same fact seen from the other
  side. `workspace/symbol` still holds none of them, and that is right: a name nobody can type is
  not a name to search for.

- **`tools/outline-lists-what-is-declared.py` is the instrument, and it needed one rule nothing else
  here needed.** It asks whether every declaration a text scan of a file finds is in that file's
  `documentSymbol` answer, and whether a class the project declares comes back from
  `workspace/symbol` asked for its own short name -- neither needing an expected output. Its first
  run reported three WordPress functions missing, and they were `function pickColor(color)` inside a
  `<script>` block after a `?>`: JavaScript, in inline HTML, which no PHP server should list.
  `compare-lsp`'s `mask` blanks strings and comments and knows nothing about _leaving PHP mode_, so
  a scanner built on it has to blank from each `?>` to the next opening tag itself. Third instrument
  this month whose first findings were its own.

- **The implementer sweep found two declarations and then looked each of them up by name, which
  answers a different question.** `ProjectSkeleton::implementers_of` hands back
  `(FileId, &ClassSkeleton)` -- it has already decided _which_ declaration it means -- and the view
  threw the file away and called `class_location(class.fqn())`. That is the catalog's question,
  "which of the project's claimants on this name wins", and it has one answer: so two files
  declaring one class rendered the winner twice and left the other out of the list entirely. psl's
  two `docs/examples/types/comparison-*.php` each declare a `Money`, and `Equable`'s implementers
  held `comparison-comparable.php` twice and `comparison-sorting.php` not at all. The seam is
  `IndexView::located(file, symbol)`, split out of `best_location`: the half that has nothing to do
  with ranking, for every caller that already knows which declaration it means. A duplicate class
  name is a project that does not run, and §8.4 already records that it makes probes lie -- this is
  the first case where it made a _feature_ lie, and in a way that reads as the right list. Found by
  `tools/implementers-are-what-is-written.py`, added here: direct inheritance is a fact a text scan
  can state exactly, so a file writing `class C extends S` has said so and asking at S's own
  declaration must hand C back. All five corpora agree once the duplicate is handled. **Priced as a
  form rather than a catch**, and the second instance was one grep away: `twice_declared` walks the
  classes of the _current_ file and took each report's range from `class_location(fqn)` -- so the
  losing file's finding underlined a range belonging to the winner's file, which on a shorter one is
  a position that does not exist. Its own test rendered the message and not the range, which is why
  a year of green said nothing; rendering the range showed `7:1-7:1` on a five-line file.
  `declared_in(file, fqn)` is the seam both now go through, and the rule it states is the one the
  shape is about: **a caller holding a file has already answered the question the catalog would
  re-ask.**

- **"Who calls this" dropped every call no declaration encloses, and the list read as complete.**
  PHP lets a call be written at a file's top level and inside a closure the file returns -- a
  script, a bootstrap, Symfony's PHP-DSL configuration, a WordPress plugin's body -- and
  `incoming_calls` skipped a site whose `enclosing_declaration` was `None`. The view's own doc
  called that "an honest nothing: the protocol's unit here is a declaration, and there is none",
  which is true about the _view_ and was the wrong conclusion one layer up: 12 of 250 sampled call
  sites on one real project and 30 of 240 on Symfony, every one of them silent. The caller a reader
  wants there is the **file**, and the protocol has `SymbolKind::FILE` for it. That needed a
  `SymbolKind::File` of our own, which is the vocabulary change §8.1 warns about -- six exhaustive
  matches, all named rather than swept into a `_`. The row points at the top of the file and is a
  leaf: nothing calls a file, so a client expanding it is answered with nothing, and that is the
  honest shape rather than a limitation. A `Name` for a path is the other half.
  `phrost_intern::display_name` exists for it alone, sound for the reason `slot_name` already
  records -- a `Name` compares by its text, not its allocation -- and deliberately awkward to reach
  for. Measured after, by the same probe: 250 of 250, 241 of 241, 151 of 151, and 239 of 240 on
  Symfony, whose one remaining miss is a private trait method reached through the composing class.

- **A compound assignment lowered to an unmodelled node, so every reader that asks what _writes_ a
  name saw a read there.** `$n .= 'x'` is not an `Assign`, and deliberately: what the target ends up
  holding is the operator's answer rather than the right-hand side's, and calling it an assignment
  would type `$n` by whatever `'x'` is. That is right about the **type** and was wrong about the
  **write**, and the two are different questions. What it cost: `Inline $offset` is offered only for
  a name written once -- "a second write means the variable was answering which value a use meant"
  -- and `$offset += $batch_size` was not one of them, so the offer appeared on a real project's
  script and accepting it produced `0 += $batch_size;`. A refactoring that does not parse afterwards
  is the one wrong answer that destroys code rather than disappointing.
  `ExprData::CompoundAssign { target, value }` is the shape now: its own variant, so no reader of
  `Assign` changes behaviour and the type layer still declines to answer. And the occurrence it
  produces is `OccurrenceKind::ReadWrite` rather than either one, because the two readers want
  opposite answers from it -- "is this ever read" has to say yes, since `+=` takes the old value,
  and "how many writes" has to count it. Folding it into either gets the other wrong, silently.

- **`tools/code-actions-apply-cleanly.py` is the instrument, and it is the cheapest kind there is:
  apply what the offer says and hand the result to `php -l`.** No expectation is written down, and
  the runtime cannot be argued with. Three questions per offer -- every range inside the file, no
  two edits over the same text, and the result parses -- and the middle one catches a range bug
  before anyone sees it. 229 offers over 15 files of one real project found seven failures in four
  distinct shapes. One trap in writing it: `php -l` writes the same complaint to stdout and stderr
  and stdout opens with a blank line, so `split("\n")[0]` reported every failure with no reason at
  all. Seven findings, all reading as an empty string, which is exactly how a real instrument looks
  when it is broken.

- **Extract read "inside this statement's range" where it meant "part of this statement", and a
  declaration is where the two differ.** A property default, a class constant, a parameter default
  and an attribute's argument all sit inside the range of the declaration around them and none of
  them is code that runs there -- they are constant expressions, lowered as nested expressions of
  the unmodelled statement the declaration becomes so goto and hover can reach them. Reading them by
  containment offered `$value = ['a' => 1]; class Other { public array $shape = $value; }`. Four
  shapes, one wrong question. `StmtData::Unmodelled` now carries `declares`, set from mago's own
  statement kind, because nothing else in HIR tells a `class` from an `echo` -- both are unmodelled
  and only one of them is a place a statement may precede. Three more came out of the same probe,
  each the same kind of category error: an **arrow function's** body is an expression, so there is
  nowhere in it to put a statement and writing one there produced
  `fn ($k) => $value = f($k); $value)`; a **write target** is a place and not a value, so
  `$rows[] = $item` yielded `$value = $rows[];`; and `...$args` is the _call's_ syntax, so
  `$value = ...$args;` is not something PHP can read. And an anonymous class is called
  `class@anonymous#4450`, which is a name nothing may be assigned to -- the suggestion now has to be
  a name PHP would accept.

- **`$i++` and `use (&$i)` are writes, and both were invisible for the same reason the compound
  assignment was.** An increment names no right-hand side, so `ExprData::CompoundAssign` carries an
  `Option` value and `None` is what a `++` or a `--` means. A by-reference capture hands the
  _variable_ over, so whatever the closure does to it is done here -- `$shouldStop = false;` beside
  `use ($output, &$shouldStop)` is two writes, and inlining wrote `use ($output, &false)`. One
  regression came with it and is worth the note: `early_reads` had recognised a compound assignment
  _by shape_ inside an unmodelled node -- "the target starting where the whole expression does" --
  and that hand-rolled recognition stopped matching the moment lowering modelled the shape properly.
  `@$undefVar++` in Symfony's own test stopped being reported until the arm was written. When a hack
  exists because a shape was unmodelled, modelling the shape is what breaks it, and the ratchet is
  what says so.

- **Four requests answer with positions rather than with facts, and nothing the server asks itself
  would catch a malformed one.** Semantic tokens colour the text, folding ranges collapse it,
  selection ranges are what "expand selection" walks, and inlay hints are written between the
  characters -- a client draws all four without checking, so a range outside the file, a chain that
  does not nest or a token overlapping its neighbour shows up as an editor behaving strangely rather
  than as a wrong answer anybody can point at. `tools/positions-are-well-formed.py` asks only about
  shape and needs no expectation. Over 200 files of five projects: every token inside the file, in
  order and non-overlapping; every folding range nested; 1 785 selection chains each held by its
  parent and holding its cursor; no inlay hint repeating the text it sits on. Nothing found, which
  is the answer -- and the reason to keep the probe is that none of those would have been noticed
  any other way. **What it did find was itself, three times, and always the same rule.** "Two
  folding ranges overlap" was written `b_start <= a_end`, then as an adjacent-pair test after
  sorting, then as a stack popping on `<`. Each version reported real code: PHP puts two boundaries
  on one line constantly -- `): T {` closes a parameter list and opens a body, `], $defaults->all()`
  closes an array inside a call that is still open, `})` closes both at once -- and `endLine` is
  _inclusive_, so sharing a line is nesting rather than crossing. The pairwise version also misfired
  on two regions starting on the same line, where one contains the other. The rule that holds is a
  stack that pops while the open region ends **at or before** the next one starts. Two smaller ones
  in the same hour: an inlay-hint repetition test that compared the bare word matched `array`
  against `array_values(` and called it a repeat -- what would actually repeat is the _named
  argument_ spelling, `name:` written where the hint says `name:`. Every one of these read as a
  finding on somebody else's code, which is the shape §8.2 already records about instruments: a rule
  encoded slightly wrong is indistinguishable from a bug until the rule is checked against what the
  format actually promises.

- **`incremental == from-scratch` is checked over a skeleton built in-process, and everything
  between the protocol and that skeleton is outside it.** `tools/edits-answer-like-a-fresh-open.py`
  asks the same question of a driven `serve`: open a file, type into it one `didChange` at a time,
  and compare what it answers against a server that opened the same text cold. The document store,
  the version guards, which buffers count as open and the analysis a request runs over are all
  inside what that measures. 80 edits over 16 files of each of five projects: every answer well
  formed, and edited and fresh agree on every file. **Both of its findings were its own, and both
  are about how a publication works.** A server publishes diagnostics when the answer _changes_, so
  an edit producing the same findings publishes nothing -- reading that timeout as "no diagnostics"
  reported a file as losing four findings it still had. And a publication is asynchronous, so one
  for the text before the last keystroke arrives _after_ it; taking whatever landed reported ranges
  past the end of the shorter buffer as "a diagnostic outside the buffer". The protocol's `version`
  on `PublishDiagnosticsParams` is exactly for the second, phrost stamps every publication with one,
  and a probe that ignores it is measuring a race rather than the server.

- **A tree the client walks in two directions is two readings of one relation, and neither had ever
  been compared against the other.** `tools/a-hierarchy-walks-both-ways.py` asks the type hierarchy
  for a class's supertypes and then asks each of those for _its_ subtypes, expecting to arrive back;
  the same downwards, and for the call graph it walks outgoing and then asks the callee who calls
  it. No expectation is written anywhere -- the two halves have to agree with each other, and where
  they do not, one of them is wrong. Four findings on the reference project in two shapes, both
  real, and 531 of 531 after. **A constructor a subclass inherits had no callers.**
  `new PremiumTier($entity)` runs `BaseTier::__construct` when the subclass declares none of its
  own, which is what the outgoing half resolves it to -- and the incoming half was wrong about it
  twice over. `constructions_of` compared the _instantiated_ class against the subject, so the site
  would not have matched even if it had been read; and the project-wide sweep greps for
  `__construct` and the declaring class's short name, so the file that writes neither was never a
  candidate to read in the first place. The same rule written twice, once as "the class the access
  reached is not necessarily the one that declared the member" in `member_uses` and once as
  `class.as_str() == fqn` here, which is §8.1's first form exactly. Both halves now ask the class
  table: the match is by the constructor the `new` reaches, and the sweep's needle list is the
  classes that inherit it -- one pass over the table per _level_ of the hierarchy rather than one
  per class, which is what `implementers_matching` exists for. Measured on the reference project: a
  constructor subject sweeps 1 111 candidate files in 208 ms against a 1 500 ms budget. Worth the
  note on its own: **the perf row for that request sampled only methods**, because it takes the
  first declaration per file whose outgoing list is non-empty, and a constructor's often is not. The
  wider of the two search shapes had no measurement at all until the sample was made to prefer one.

- **A flat cap on "who inherits from this" spends its places on whoever the class table reaches
  first.** `IMPLEMENTERS = 100` bounded goto-implementation, the subtype tree, the template-method
  rule behind `missing-member`, and the member-family walk a rename uses -- and the comment above it
  said what mattered was that the project's own classes were in the list, "which the ordering
  guarantees". It does not: ordering puts them first, and a cap smaller than the project's own count
  still cuts them. On the reference checkout Drupal's `UnitTestCase` has 732 subclasses in core and
  contributed modules and **155 in the project's own code**, so the list held a hundred of somebody
  else's and dropped 55 classes the person wrote. Found because `subtypes` of a supertype did not
  lead back to the class it had just been reached from. The bound now counts only what comes from a
  dependency or a stub, which is the split `view::candidates` already makes for a completion page:
  the cost belongs on what `vendor/` contributes and never on what the project does. The general
  form is worth more than the fix -- **a bound whose justification is an ordering has to be checked
  against the size of what the ordering puts first**, and nothing about "the project's own come
  first" bounds how many there are.

- **The checker and the server are two entry points into one set of checks, and nothing had ever
  compared them.** `tools/the-checker-and-the-server-agree.py` runs `phrost check` over a project
  and drives a server over the same one, then compares three answers per file: what the CLI prints,
  what the server publishes for the file closed from its background project pass, and what it
  publishes once the file is open. The stakes are asymmetric and easy to miss -- **every ratchet in
  this project is a CLI number**, so a drift makes the instrument describe something nobody sees.
  There was already a note in `sweep.rs` about this exact shape, from the time the checker answered
  the coarse reachability rule while the editor answered the exact one. Two more instances, both
  found in the first run. **The syntax lint categories came from the command line and nowhere
  else.** `phrost.toml` names them, the server resolves them from there and falls back to safety and
  security; the CLI started from an empty list unless `--lint` was passed. So a project configuring
  `correctness` saw it in the editor and not in CI, and a project configuring nothing saw two
  categories in the editor and none in CI. The doc comment beside the CLI's own lint call already
  said "a checker that ignored `phrost.toml` would report what the editor does not" -- it read the
  project's _named_ rules and left the categories behind, which is half a rule. The reference
  project asks for four categories, so `check` there had been silent about all of them. It costs
  what the pass costs: the serial sweep went from 0.9 s to 1.7 s, and on Symfony 9.54 s to 11.40 s,
  because a lint pass re-parses the file. **And `path:line:col` counted bytes.** `LineCol::col` is a
  byte offset into the line -- which is what every conversion in this codebase wants and what
  nothing printed for a human does. Eight sites across Laravel's and Symfony's test suites disagreed
  with the server by one column to twenty, all of them lines holding `'Привет, мир!'` or the like,
  and an editor following the format lands where the CLI said. `LineIndex::character_col` is the one
  rule now, used by `check`, `coverage` and `perf`. Worth noticing that this is the third thing in
  these notes found by asking two halves of the server the same question rather than by writing an
  expectation down.

- **Every migration in a Laravel application was one class.** The name lowering synthesises for
  `new class { ... }` was the byte offset of its `class` keyword, which is unique inside one file
  and not in a project -- and a generator writes the same header into every file it makes, so the
  keyword lands on byte 146 of every migration Laravel produces. A catalog ranks two claimants for a
  name and the loser is silently invisible, so `$this->` inside one migration answered about
  another's members and the reference list of its `up()` held a declaration from a different file.
  Measured across six checkouts: 5 of Laravel's own fixtures collide, 3 of 3 in a fresh
  `laravel new`, one pair each in Symfony and psl -- and a skeleton app understates it, because a
  real application has thirty migrations and they all collide into one. The name now carries an
  FNV-1a hash of the file's text beside the offset. Not the path and not the file id, and the
  reasons are worth keeping: a path is absolute, so it would put this machine's directories into a
  name the index dump prints and the disk cache stores; a file id is assigned by discovery order, so
  a from-scratch index and an incremental one would disagree about it, and
  `incremental == from-scratch` compares exactly those renderings. The text is the one input that is
  the same in every run, on every machine, and for an open buffer as much as for a file read off
  disk. Two byte-identical files still collide, and what they hold is then identical in everything
  but which file to jump to. **The second half of the same bug:** a target whose name nobody can
  write has nothing for a sweep to grep, so find-references on `new class { ... }` read every
  candidate file in the project and came back empty -- while the highlight drawn at the same cursor,
  which never leaves the buffer, showed its uses. `ReferenceTarget::search_name` answers `None` for
  it now, which both callers already read as "this file and no other". Found by
  `tools/a-reference-list-agrees-with-itself.py`, which asks what else means this in the three ways
  a server answers it -- the lens over a declaration, find-references at the same position, and the
  highlight at a use -- and requires the first two to differ by exactly the declaration and the
  third to be what the second says about this file. 676 questions over six projects, and the two
  shapes above were all it found. One thing noticed while writing the test and left alone: an
  anonymous class written in a _file-level_ statement is claimed in the global namespace rather than
  in the file's, where one written inside a method body is claimed in the file's. Harmless, because
  the reference to it is qualified the same way the claim is, and both are unwritable -- but it is a
  real asymmetry and the next person to find it should know it was seen.

- **The formatter had two front doors and they printed differently.**
  `tools/one-formatter-two-front-doors.py` reprints a project through `phrost reprint`, asks a
  running server to format the same buffers, and requires the two to be byte-identical -- plus
  `textDocument/rangeFormatting` over the whole document to equal the whole-document answer, which
  is what the server's own doc comment claims it is a subset of. On the reference checkout, **29 of
  30 sampled files differed**. The command took its style from `--style` and the built-in default
  and nothing else. The server resolves it through `Workspace::set_root`, which is where a
  _framework_ gets to say how its own projects are printed -- and Drupal's provider is the whole
  reason a Drupal checkout formats to two-space indent with no configuration at all. So the
  instrument that judges the formatter, `tools/drupal-style.sh`, only measured the right thing
  because somebody had written `--style drupal` into it by hand; pointed at anything else it
  measured a formatter nobody runs. It resolves through `set_root` now, searching upwards for the
  project because the command is pointed at a _directory of user code_ four levels below the
  `composer.json`, and the script no longer names the style. Third instance of one shape in two
  sessions -- the checker's lint categories, the checker's columns, and this -- and the form is
  worth stating plainly: **a CLI and the server reading one setting from two places is not a
  duplication, it is a divergence waiting for a project that configures something.** The range half
  and the parse check found nothing.

- **Half of `unused-declaration` was silenced by the word "required" in a docblock.** The rule that
  stops the check on a file naming a member at runtime -- `->$name`, `call_user_func`, an `include`
  pulling a template into a method scope -- was a plain substring search over the file's text. So
  `Constructs a new instance with the required dependencies` in a docblock, a `$required` local and
  a `getRequired()` call each read as an `include`, and one of those is in most files somebody wrote
  prose for. On the reference project it silenced **97 of 196** findings: half the check, dead, in
  the direction nothing reports. It is asked of the _code_ now, through the same comment- and
  string-skipping scan that already served `quoted_words`, with `include` and `require` matched as
  whole words. Same family as the apostrophe note above it and as `core.api.php` teaching the
  annotation syntax it was scanned for: **a text search whose vocabulary is not the language's.**
  Found by running `phrost unused` and `phrost check` over one project and diffing --
  `tools/two-reports-of-what-nothing-reaches.py`, which is now the standing form of that question.
  The report said 196 and the check said 99, and the check's 99 were a strict subset. **Closing the
  rest took three more rules the report did not have**, each of which the check did: a template
  calling a method under another name (`{{ x.options }}` is `getOptions()`), a magic method PHP
  calls without anyone writing its name, and the rule that a dead class's members say nothing more
  than the class does -- plus an anonymous class, which the report named as dead when it is
  instantiated where it is written. The report's own module doc listed the templates as one of its
  four sources and the code never consulted them. The three that both sides need are one method now,
  `ProjectSkeleton::member_is_reached_without_a_call`, because this is the _second_ time these two
  assemblies of one rule have been found disagreeing and the first fix treated its instance. 150 and
  150 after, identical line for line. The rise is a precision gain and was verified before it was
  accepted: 17 of the 51 new findings checked by hand on the reference project, every one a
  declaration written in exactly one file and reached from nowhere.

- **Renaming a free function renamed a string and nothing else.** `ReferenceTarget` had no variant
  for a function, so one was reached as a class-shaped target -- and nothing that answers about
  classes answers about a function. `name_references` skips everything that is not class-like, and
  `class_location` has nothing for a function, so find-references at a function's own declaration
  listed neither the declaration nor a single call. What it did list was the one thing that happened
  to work: `string_class_references`, which matches a _string_ naming the fqn. Accept the rename
  offered there and `'App\alpha' => __DIR__ . '/alpha.php'` in an autoload map is rewritten while
  the function is still called `alpha`. The fix is a `ReferenceTarget::Function`, built on the
  `function_references` projection that already existed for the undefined-function check, plus the
  `use function` line -- which is not a call, so that projection is right to leave it out, and which
  a rename that leaves it out turns into an import of something that no longer exists. Six sites on
  the smallest fixture that has all of them: the declaration, an unqualified call in the same
  namespace, a `use function` line, a call written under it, a fully-qualified call, and the string.
  PHP refuses `function list()` exactly as it refuses `class list`, checked against 8.5 rather than
  assumed, so the reserved-word refusal applies here too. **What kept it hidden is the instrument.**
  `compare-lsp`'s site scanner matched a class-like name as `[A-Z]\w*` and a member after `->` or
  `::`, and nothing else -- so a lowercase free function was never a site, and
  `navigation-roundtrip.py` scored 100 % over two years of runs without ever asking about one. Same
  family as §8.2's note that `coverage` is blind to a function that does not exist. The scanner has
  a `function` kind now: 77 sites in a 20-file sample of the reference project, 8 in psl, and the
  round trip is 300 of 300 with them in. Found by `tools/a-rename-goes-there-and-back.py`, which
  renames a declaration, applies every edit to disk, tells the server the files changed, renames it
  back and requires the tree to be byte for byte what it was -- with `php -l` over every edited file
  in between. A missed site comes back as a file the second rename had nothing to change in; an
  over-eager one comes back changed twice. **Four of its first five findings were its own**, and
  worth the note: the probe keyed files by `os.path.realpath`, which on macOS turns a `/var/...`
  temporary directory into `/private/var/...` -- so it told the server a _second_ path for a file
  already indexed, got a second file id, and a rename answer naming one file under two uris.
  Applied, that is text nobody wrote, and it reads exactly like a rename that mangles a docblock.
  The project's own rule is that a client spells a path one way and keeps to it; an instrument has
  to keep to it too. The one real finding underneath was the function.

- **PHP has three symbol tables and find-references knew about one and a half.** The free function
  was the first half; a _global constant_ was the other. A `const MAX = 10;` was reached as a
  class-shaped target, so find-references at its own declaration listed nothing at all -- not one
  use, not the declaration -- and the rename offered at that cursor came back with an **empty edit
  and no error**, which an editor shows as a dialogue that takes a new name and does nothing.
  `ReferenceTarget::Constant` now, with a `constant_references` projection beside the function one.
  Two things about a constant that the other two tables do not share, both asked of PHP 8.5 rather
  than assumed: its name is compared **exactly**, so `MAX_ITEMS` and `max_items` are two constants;
  and `const list = 1;` is a parse error exactly as `class list` is, so the reserved-word refusal
  applies. **Two arms of one match were the whole of it**, and both carried a comment explaining the
  gap: `ImportKind::Constant => None` said constants "have no catalog projection yet" when
  `constant_location` had existed for some time, and `CursorTarget::Declaration` sent every kind but
  a function to `class_location`. So a _use_ of a constant answered and its _declaration_ did not,
  which is the divergence shape §8.1 names -- along a place rather than a spelling.

- **`define('NAME', ...)` declared nothing, and WordPress is written in it.** 636 calls on that
  checkout against 560 `const` statements, where Laravel has two and the reference project none --
  so goto on `ABSPATH` answered nothing and every question after it did too. Lowering records it
  now, from the declaration walk rather than the body one, so a dependency's constants are indexed
  as well. **The name is the string, whole, and PHP's rule here is the opposite of the one `const`
  follows.** Inside `namespace App;`, `define('MAX', 1)` declares the **global** `MAX` and
  `defined('App\MAX')` is false, while `define('App\INSIDE', 2)` declares exactly that. Prefixing
  the current namespace would put a name in the index the runtime does not have and drop the one it
  does -- four lines run against 8.5 settled it, as §9's seventh instrument keeps being right about.
  A `define` inside `if (!defined('X'))` is a _conditional_ declaration and falls out of the walk
  that reached it, which is what keeps a project's fallback below an unguarded declarer.

- **Two symbol tables were invisible to every instrument built on `compare-lsp`.** Its site scanner
  matched a class-like name as `[A-Z]\w*`, a member after `->` or `::`, and nothing else -- and it
  excluded an all-caps name by its own `isupper()` rule. So neither a lowercase free function nor a
  constant was ever a site, and `navigation-roundtrip.py` scored 100 % for as long as it has existed
  without asking about either. It has a `function` and a `constant` kind now: 548 and 59 sites in a
  20-file sample of WordPress, 77 and 5 on the reference project. Both bugs above were behind that
  blindness, which is the same lesson §8.2 records about `coverage` -- **an instrument's vocabulary
  decides what it can find, and a scanner that only knows one of PHP's three tables measures one
  third of the language.**

- **A rename stepped over `use T { greet as welcome; }`, and PHP makes that a fatal.** A method name
  written in a trait-adaptation clause is a reference to a declaration exactly as a call is, and the
  language checks it when the class is declared:
  `An alias (welcome) was defined for method missing(), but this method does not exist`, asked of
  8.5 rather than assumed. So renaming `Greets::greet` edited its declaration and its call sites and
  left the line between them naming a method that no longer exists -- a class that no longer loads,
  which is the worst thing a rename can do. Both spellings go through one rule now, the same one
  `member_uses` applies to a call: matched by the declaration the clause _reaches_, so
  `greet as welcome` -- valid only when one used trait declares it -- and
  `Greets::greet insteadof Shouts` are found alike, and the same method name on another trait is
  not. Found by walking the shapes a probe's site scanner cannot reach and asking about each by
  hand. Worth recording what that pass also found _not_ wrong, because two of the four looked broken
  and were the instrument: a call through a trait alias resolves, completion offers the alias beside
  the original, an enum case in a match arm resolves, and a named argument goes to the method. The
  two that read as gaps were both my column arithmetic, which is §8.2's `phrost query` note for the
  second time in one session -- **compute the column, do not count it.**

- **A rename rewrote prose, and the probe that found it is the one that renames back.** A string is
  matched against a declaration's fully-qualified name to catch what a name walk cannot see -- a
  container definition, a Drupal annotation, `class_exists('App\Repo')`, an autoload map. The
  comparison was case-insensitive, as PHP compares class names, and that is right exactly where the
  name is **qualified**: nothing writes `'App\Repo'` by accident. A name in the global namespace is
  a bare word. Renaming a global `function fail()` in Symfony's `.agents` scripts turned
  `new \LogicException('Fail')` and `'body' => 'FAIL'` into `'fail'` across four of its test files.
  Exact case where the name is unqualified keeps every real use -- `class_exists('WP_Query')`, a
  callable `'fail'`, `'Psl\Iter\random'` in a map, which still matches however it is cased -- and
  drops the word that merely reads like the name. Worth the note twice over. The loose reading had
  been there for classes all along and only reached lowercase words once free functions and
  constants got a reference target; and what caught it was `tools/a-rename-goes-there-and-back.py`
  renaming the function back and finding the project no longer byte-identical. A find-references
  list holding four extra strings reads as a wide net; the same four applied read as a rename that
  edits prose, and only the round trip tells them apart.

- **Two measured decisions not to build something, recorded so they are not re-derived.**
  `missing-return` inside a closure is the same gap the three other return checks had, and closing
  it would need an anchor the HIR does not carry -- a closure has no name to underline. Priced
  first: `missing-return` fires **8 times across five projects**, every one of them in Symfony and
  none in a closure. Not worth a HIR field. And `phrost check third_party/php-prelude` -- §8.2's
  corpus nobody wrote for us -- reports the same 49 index findings it did before this session's
  work, every one already named in that directory's README as mago's. The two that were once
  phrost's are still fixed. A clean negative from the sharpest corpus available is worth as much as
  a finding.

- **Signature help was timed and audited and never read.** The perf sweep measures how long it takes
  and the capability audit asks whether it answers; neither looks at what came back, and an editor
  draws the active parameter in bold -- so a server naming the wrong one tells the reader which
  value they are typing and is wrong about it. `tools/a-signature-points-at-the-argument.py` reads
  the expected answer off the _text_: the arguments of a call split on the commas that are not
  inside brackets or a string, so the slot a cursor sits in is arithmetic. It then requires the
  indices to point at something that exists, a positional slot `k` to highlight parameter `k` unless
  a trailing variadic has swallowed it, and a named `name:` to highlight the parameter called `name`
  wherever it sits. **835 slots over five projects, every one answered and every one right**, with a
  single silence on Laravel. A clean negative from a request nobody had read. Its first finding was
  its own, which is now the pattern rather than the exception: the site scan matched a method
  _declaration_'s parameter list as a call, so 41 `__construct(` headers on one project read as a
  third of the questions going unanswered. `compare.DECLARES` already names that distinction for the
  other probes. **Every instrument written this session has reported on itself before it reported on
  the server, and the ones that did not were the ones that had nothing to say.**

- **Moving a file offered an import PHP refuses, and the file stopped loading.** PSR-4 makes a
  class's name a function of where its file is, so a drag in the sidebar is a rename -- and the half
  that is not a rewrite is that a neighbour reached _bare_, by sharing a namespace, has no line to
  change and needs one written. `orphans_of` excluded one name from that: the class the file is
  named after. A PHP file may declare several, and a test file with its fixtures beside it is the
  ordinary case -- so moving `QueueRoutesTest.php` offered
  `use Illuminate\Tests\Queue\BaseNotification;` for a class declared 190 lines further down **the
  same file**, which travels with it. PHP answers
  `Cannot use ... as BaseNotification because the name is already in use`, and the file no longer
  loads. Two of Laravel's test files, and the same shape in a second spelling:
  `Cannot redeclare class ... (previously declared as local import)`. Every class the moved file
  declares is excluded now, not only the one it is named after. Found by
  `tools/moving-a-file-keeps-the-project-whole.py`, which moves a file into a sibling directory,
  applies what `willRenameFiles` offers, moves it back, applies that, and requires the tree to be
  byte for byte what it was -- with `php -l` over every edited file in between. **Only `php -l` saw
  these two**: the round trip is what runs it, and a workspace edit that reads perfectly well in a
  diff is a file that does not parse. Two things the probe had to learn before it could say
  anything, both recorded because the next one will meet them. The edits come _before_ the move --
  `willRenameFiles` answers about the file where it still is, so applying them after has nowhere to
  put the moved file's own edit. And the neighbours it edits have to be announced: the server
  refuses to write an import a file already has, it can only know that from its index, and a probe
  that moved the file without saying what else it had written got a second `use` for a name already
  imported and read it as a server bug. What it also found and is **not** a bug: moving a file out
  and back leaves the imports the first move wrote. PHP accepts a class imported from its own
  namespace and `unused-import` is right not to report one, so the project that comes back is
  correct and is not the same bytes. The probe counts those apart -- 6 on Laravel, 5 on Symfony, 3
  on the reference project -- rather than calling a correct project a failure.

- **The save path had no check at all.** An editor with `code_actions_on_format` asks for
  `source.fixAll` over the whole document on every save and applies what comes back **unread**, so
  an edit that overlaps another, or one that fixes one thing and breaks a second, is text nobody
  wrote appearing in a file somebody just saved. `tools/code-actions-apply-cleanly.py` asks at a
  cursor and applies one offer at a time; this is the composition, which is where edits meet.
  `tools/a-fix-all-settles-the-file.py` requires four things of the answer: every edit is its own
  place, the result parses under `php -l`, the file says no _more_ than it did by code and count,
  and the passes **end**. 150 files over five projects, 73 edits, all four hold. **What it found
  first was a rule of its own that was wrong.** The obvious invariant is that a second
  `source.fixAll` offers nothing, and one file broke it -- `doc-missing-throws` is transitive, so
  documenting one method makes its _caller_ owe the same tag, and each save takes one step up that
  chain. The count never rises and the chain is short, so the honest requirement is that it settles
  rather than that it is empty in one. Worth knowing as behaviour: with that check on, a save can
  change the file again on the next save until the chain ends.

## A navigation answer's shape is the client's to choose

`LocationLink` carries the span the answer was asked _from_, which is what lets an editor underline
the word under the cursor -- and the protocol makes it opt-in, one bit per request under
`textDocument.definition.linkSupport` and its three neighbours. Every one of the four navigation
answers built a `GotoDefinitionResponse::Link` unconditionally, so a client that had not asked got
back an object with no field it could read, and goto looked like it answered nothing at all.

Found by a probe that declared `linkSupport: false` and scored **0 of 100** -- which reads as a
missing feature and was a shape mismatch. Every real editor asks for links, which is why it survived
this long: nothing anybody uses exercised the other arm. `Backend::navigation` is the one place the
choice is made now, and a flow test drives both arms of it.

The general point is the one §8.2 keeps making about instruments: a capability a client _declares_
is a request, and a server answering past it is answering into a void. The other direction cost the
capability audit a 0/20 on semantic tokens -- it declared `tokenTypes: []` and measured the absence
it had asked for.

## A completion card and a hover card describe the same declaration

Two cards, two assemblers, and the only difference anybody chose was _when_ they are built: a list
per keystroke, a card for the one item somebody highlighted. Everything else had drifted. The
resolve card dumped its docblock tags into a plain fence where hover labels and reorders them --
which hover's own comment says it replaced, because a sentence of prose inside a fence reads as code
and the type ends up nowhere in particular. It never said where an inherited member came from, and
after the `{@inheritdoc}` work it rendered a different return type from the hover at the same
member.

`describe_completion` returns hover's blocks now and the protocol layer renders both through one
function. The one place a card is still built by hand is a key a framework documents, which has no
declaration and no docblock range -- a sentence in somebody else's class.

Two things the probe that found it had to learn, both of them its own fault before they were the
server's. It matched the completion item by parsing a member name out of a window round the site,
and a window holds whatever else the line calls -- so it compared `abstract()` against `args()` and
reported every site in the sample. Taking the name from the card hover just rendered is the fix, and
it is the general one: when an invariant compares two answers, let one of them _name_ the subject
rather than deriving it twice. Then it matched a property `$hasErrors` against a method
`hasErrors()`, because a list labels both with the bare name -- so the kind has to be matched too.

## `member_declaration` may climb and `MemberRow::signature` may not

Both render a member. One is asked for the item a person highlighted; the other for every member of
a class on every keystroke, which is where the flattener's note about carrying contracts down the
hierarchy comes from -- climbing per member cost goto p95 0.41 ms -> 0.88 ms. So
`resolved_signature` is reached from hover and from a completion _resolve_, and never from the list.
The rule is not "never climb", it is "never climb per item in a list".

## Deprecation is three tags, and phrost set none of them

The protocol carries "this declaration should not be reached for" once per surface:
`CompletionItemTag::DEPRECATED` on a completion row, and `SymbolTag::DEPRECATED` on an outline entry
and on a workspace-symbol hit. A client draws each struck through. phrost rendered `@deprecated` as
a line on a hover card and nothing anywhere else, so a deprecated class read exactly like a current
one at every position where a person _picks_ a name.

On the list rather than on the card `completionItem/resolve` fetches: a warning that arrives when
somebody highlights a row has arrived after they chose it. That is what puts the bit on
`DeclaredSymbol` rather than behind a lookup -- see `docs/08-index.md`.

`SymbolInformation` and `DocumentSymbol` each carry a `deprecated: bool` of their own that the
protocol replaced with `tags`. Set `tags` and leave the old field `None`: a server that sets only
the deprecated field is invisible to a modern client, and setting both says the same thing twice.

## `static` on a card names the class that declared it, and the receiver is not in a `SymbolLocation`

Found by `a-type-definition-lands-on-what-hover-names`, which is written for exactly this: hover
renders what a value _is_ and go-to-type-definition resolves where that is declared, and nothing
compared them until it existed. Hovering `User::load($id)` said the result is a `?EntityBase` -- a
class that is **abstract**, so one `static` can never be -- while type-definition at the same site
landed on `User`. 279 of 281 sites agreed; the two that did not were both this.

`EntityBase::load()` is `{@inheritdoc}` over an untyped signature, so `resolved_signature` climbs to
`EntityInterface::load()`'s `@return static|null` and renders it with a scope whose `called` is
`members_owner(class_fqn)`. The comment beside it said "`static` means the class the card is
describing, which is the one that was asked about" -- and the one asked about is whatever
`SymbolLocation::container` holds, which is the class that **declared** the member. The receiver is
not in a location at all.

So `called_on` is now the caller's to supply. Completion holds a receiver and passes it, which is
what makes `$user->` offer `load(): ?User`. Hover has none and passes `None`, which renders the
keyword -- and `: ?static` is the honest line for a card headed `class EntityBase`, because that is
what the declaration says. A reader who wants the class follows the type definition, which was right
all along.

The form is the one §8 keeps recording, from the other end: **two answers about one value reached by
different code**, and the one that looked more specific was the wrong one. A resolved class name
reads as a better answer than a keyword, which is why it survived -- nothing about `?EntityBase`
looks like a bug until something asks what `User::load()` actually returns.

### And the instrument's half, which is two more shapes

The probe documents that a relative return is skipped, and it missed both of these. `RELATIVE` was
anchored as `:\s*(?:static|self|\$this)\s*$`, so `: ?static` did not match -- and a nullable is how
the shape is nearly always written, because a loader that may find nothing returns `static|null`.
One character of pattern hid the exception the probe was built to make.

## `Rule[]` has no `Rule[]` to jump to, so go-to-type-definition answered nothing

`typeDefinition` asks where the type of the thing under the cursor is _declared_, and a value typed
`@var \App\Rule[]` answered with nothing at all. The rule it fell out of was written as "empty where
the index cannot name a class: a variable with no declared type, a member declared `array`, a name
that resolves to nothing" -- and a list is not that. There is no `Rule[]` anywhere to go to, and
`Rule` is the only declaration a reader on that line could have wanted; every other server lands on
it.

So the answer falls back to the type's **element** where the type names no class of its own. Only
where it names none, which is what keeps `Collection<Rule>` answering `Collection`: a class that
holds elements is still the thing the cursor is on, and answering `Rule` there would be a jump to
what it contains. A bare `array` still answers nothing, because it says nothing about what it holds.

Carried beside the class rather than recomputed, in both places the feature reads one:
`VariableInfo::element_class` and `IndexView::member_element_class`. The reason is the one already
written on `VariableInfo::class` -- what a local holds depends on _which site_ the cursor is on, and
that is knowledge the resolution has and a caller holding a name does not.

Found by `capability-audit`, which scored `typeDefinition` 19/20 and named the site. Worth noting
what that costs in an editor: every `@var Foo[]` property and every `@param Foo[]` in a project had
the key bound to nothing.

### And the eleventh correction to the same instrument

`references` scored 19/20 useful on the same run: phrost answered two sites in one file for
`RegionDistrictDefaultFormatter` where four files "write the word". Three of them write it in
`Constructs a RegionDistrictDefaultFormatter object.` -- a copy-pasted docblock sentence, in files
that name the class nowhere else. **Prose is not a reference**, phrost is right, and the tool was
counting a word in a comment.

The server has this rule and a test for it
(`a_word_in_prose_is_not_a_construct_that_reaches_anything`); the instrument grading it did not.
Same family as the ten already recorded in §8.2 -- an instrument that judges an answer has to know
what the answer is about -- and the same ordering trap as the view's own reader: strings are blanked
first, or `$url = "https://x/y";` loses its tail to a `//` that is data. `#[` is an attribute and
not a hash comment.

Both projects now read 20/20 answered and 20/20 useful on every row but the bare-cursor
`codeAction`, whose provoked twin is 20/20.

## `elseif` is not a statement boundary, and extract-variable wrote a file PHP refuses

Extracting `$buyer->isAddressEqual()` out of an `elseif` condition produced
`syntax error, unexpected token "elseif"`. Lowering turns each clause of a chain into an `if` of its
own whose range **begins at the keyword** -- `clause_range` says so, and it has to, because a node
whose range does not contain its children breaks every lookup that descends by position -- so the
statement the refactor inserts above lands between the previous arm's closing `}` and the `elseif`.

The refusal is the whole fix, because there is nowhere else for it to go. Above the chain would run
the expression on the path the chain exists to keep it off, and `if (...) {} else { if (...) {} }`
is a rewrite rather than an extraction. That is the same reading the file's other refusals take of a
short-circuit, a branch arm and a closure body.

Told apart from a _real_ `else { if (...) {...} }`, which is a place a statement can go, by where it
ends: a synthesized clause owns everything after it including the final `else`, so its range ends
with the whole chain, while a nested `if` written inside an `else` block ends before its own closing
brace. The control test extracts into that block and produces the right file.

Found by `tools/code-actions-apply-cleanly.py`, which applies every action a real project offers and
parses the result: one of 257 edits. The offer is gone rather than corrected, so the count is now
256 of 256 -- which is what a refusal should look like in that instrument.

## `references` on a constructor found the declaration and nothing else

Asked by a person looking at `ClientNotConfiguredException::__construct` and getting **one** answer,
for a class their own code writes `new` on four times.

`new Repo()` names neither the member nor -- for a subclass that declares no constructor of its own
-- the class that declares it, so a sweep over member _accesses_ finds the declaration and stops. A
constructor is reached three ways and only one of them writes the name: `parent::__construct()`,
`new Repo()`, and `new Child()` for every subclass that inherits it.

**The call hierarchy has known this all along.** `CallSubject::search_names` adds the class's short
name and `classes_reaching_constructor` for exactly this reason, and `incoming_calls` extends its
member sweep with `view.constructions_of(class)` -- with a note beside it reading "a list without
those is a constructor nothing appears to use." `references` is the other half of the same question
and never learned it: §8.1's oldest form, one rule in two places and only one of them taught.

So `references_in` extends the same way, through the same query. The name is compared before the
class table is asked anything, which keeps the question off every other member's path.

Safe for rename without a second thought, and not by luck: `magic_method_refusal` already refuses
`__construct` outright, and its own comment gives this as the reason -- "the call site of
`__construct` is a `new`". The audit still refuses it, `documentHighlight` and `references` still
score 20/20, and `a-reference-list-agrees-with-itself` still 166 of 166.

The test pins all three spellings and the exclusion that goes with them: a subclass that declares
its own constructor is _not_ a site, because `new Owns()` runs that one instead.

## Every oracle read its sites off masked text, so nothing asked a question inside a string or a comment

`[$this, 'validateBudget']` went to the method on goto and completed nothing. Asked why the
completion oracle had never said so, the answer was in its first line: `compare.mask` blanks every
string body and comment before sites are read, on the grounds that a `->` inside a docblock is not a
site -- true, and it made the instrument blind to every feature that _lives_ in a string or a
comment. Callable pairs, catalog keys, `@param $name`, `{@inheritdoc}`, an interpolated variable, a
class named by a string: goto had been taught all of them, and no instrument asked the other three
questions at any of them.

`tools/where-goto-answers-the-rest-do.py` asks at every word, masked or not, and holds the answer
goto gives against completion, hover and references at the same word -- the divergence §8.1 calls
two constructors of one rule, with goto as the half that is usually right. Its first two days, over
the reference project and five corpora, found: a callable pair's method; an array key `'name'` read
as an `enum Name` in PHP_CodeSniffer's test data, case-folded (references had learned the case rule
and the cursor had not); a docblock's `$name` that went somewhere and described nothing, and whose
rename left the tag behind; an interpolated variable at a string part's first offset; no namespace
ever offered; `time()` cut off the page by the project's own loose matches; no docblock tag ever
offered; `@use`, `@global`, `@see` and a map's value type read as prose; `\sprintf` and
`use function` matched against classes only; an import alias the catalog had never heard of;
destructured and by-reference locals out of scope; `User::where` unreachable through `__callStatic`;
and a catalog key whose references were its class's.

What generalises is about instruments rather than features. **A filter that makes an instrument's
sites cleaner also decides which bugs it can find**, and the cost of the mask was invisible because
its benefit -- no junk sites -- was the visible half. Ask what a sampler _excludes_ and whether a
feature lives there. And a probe that compares two answers needs no expected output, which is what
let it run over six projects in one sitting: the second-best oracle is one half of the server
checked against the other.

## A save is a fixpoint, and `source.fixAll` was a ratchet

A project with `code_actions_on_format: {"source.fixAll": true}` -- the reference project has it --
runs fixAll and then the formatter on every save, and nothing ever checked what a _second_ save did.
Driven over the 117 files fixAll changes on that project, 32 changed again. `doc-missing-throws`
read a callee's throws off its docblock, so a save documented `load()`, the next one `find()` that
calls it, the next the callers of `find()` -- each one an edit the person did not make, on a file
they had just saved. A callee in the same file is now read for what it throws, which is what its
docblock says once that same save is done; across files the one-level reading stays, since the body
there is not visible. Second save: 0 of 117. The finding count rose with it -- 368 to 485 on that
project -- which is the same findings arriving at once instead of one save at a time.

The other half was about _which_ fixes run unattended: fixAll applied every tag fix whether or not
the project had asked for the check, and over Laravel's source that was 2 321 boilerplate lines in
419 files. An `ActionRequest` carries the project's switched-on codes now, and fixAll applies a fix
only for a finding the project reports. Both were found by driving a save the way the editor makes
one -- fixAll, then format, twice -- which is the instrument: a save is the unit a person
experiences, and every earlier check asked about one half of it.

## A rename took its target from goto, and goto lands where the code is

Two renames in one pass edited the wrong things, and both had the same root: the reference target
was the declaration goto reaches, and that is the right answer for goto and the wrong one for a
name.

- **A trait alias.** `use Greets { hello as greet; }` and then `$this->greet()`: goto lands on the
  trait's `hello`, so a rename asked at `greet` renamed `hello` -- the trait's method, the class's
  own `hello`, the clause's left half -- and left every call to `greet` saying `greet`. The alias is
  now a target of its own (`ReferenceTarget::TraitAlias`): the clause that writes it and the calls
  reaching it, and a rename is refused where a subclass declares a method of that name, which PHP
  8.5 treats as an override of the alias. The check that a call goes through an alias is two words
  compared -- the name written against the name of the declaration reached -- because a highlight
  asks for the target at every cursor movement, and resolving the access a second time cost 0.05 ms
  on every one.
- **A parameter a call names.** `find(id: 5)` has been part of how `find` is called since PHP 8.0,
  and renaming `$id` as a local left the label, which PHP stops on with
  `Unknown named parameter $id`. The reference project writes about 1 300 named arguments. The
  target (`Parameter`) carries the callee's identity and the name and no position, so it is found
  again by name in whichever file a sweep reads -- which is also what lets a search start at the
  label in one file and edit the parameter in another without the index storing where any parameter
  is written. A promoted parameter's label is the _property's_, whose rename now takes
  `new Money(amount: 5)` along.

Neither was found by the round-trip rename tool, which renames _declarations_. What found them was a
probe asking a rename at **uses** -- calls, labels -- and requiring the edit set to contain the word
under the cursor: a rename whose dialogue says `greet` and whose edits do not touch `greet` is the
tell, and it needs no expected output.

## Inlining a variable wrote two files PHP refuses

`tools/code-actions-apply-cleanly.py` found both, one on the reference project and one on WordPress,
and both are positions where a variable is not standing for its value:

- `"...: $violationMessage"` interpolates the name, and only a variable, a property or an element
  may stand there -- the `array_reduce(...)` put in its place is a parse error. A read is
  interpolated exactly when a string's own text runs up to it, since outside a string a literal
  never touches a `$`; braced `{$x}` included.
- `$this->$option_tag`, `$$name`, `new $class` and `instanceof $class` read the value _as a name_.
  Each could be made legal with braces or parentheses, four different ways.

Both are refusals rather than rewrites, for the reason the `elseif` note above gives: the instrument
counts a withdrawn offer as the right answer, and a rewrite into a concatenation is a different
refactor. Clean on five projects after.

## Completion's scanner counted the parentheses in a comment

`run(function () { ... // expected :) ... })->` offered nothing while goto on the call answered. The
receiver of a chain being typed is read out of the characters before the cursor -- the statement
does not parse -- and the `)` of a smiley in a comment unbalanced the count. The lexer pass that
already finds an open buffer's strings records its comments too, and the scanner reads the text with
every comment that closed before the cursor turned to spaces; lengths and line breaks kept, the
comment the cursor is in left alone, since that is where a docblock tag is being typed. Two answers
moved with it, both toward what the same code gets without the comment: a statement after a line
comment is a whole statement, and one after `/* aside */` is offered the statement keywords.

Measuring the neighbouring change, `$class::` completing the class a value names, the first A/B read
289 -> 280 completed and was wrong: the probe took the first N sites from `rg`, which searches in
parallel and returns them in a different order each run, so the two runs asked different questions.
With `--sort path` and the previous commit built in a scratch worktree for the "before", the same
739 sites read 503 -> 512, none lost. A sampler that stops at N has to be deterministic before its
counts can be compared.

## A broken buffer reaches code no whole file does, and three things were waiting there

Every consistency tool here samples whole files, so a capability was only ever asked about code that
parses. `tools/mangled-buffers-answer-without-failing.py` asks about what an editor actually sends
-- a file truncated mid-call, a span deleted, a `->` typed into the middle of a line -- and its
first runs over five projects found three things, none of them a crash and all of them a lost or
slow answer:

- **Inlay hints sliced the file with a range past its end.** A cut buffer is repaired by writing the
  tokens it lacks after the end, and the session's own doc said so: the repaired statement ends past
  the last byte. Every reader above slices the caller's text, and the hint over the last assignment
  of a truncated WordPress file failed. Lowering now holds every range it hands out to
  `ParsedFile::written_len`, in one method rather than at every reader, and a test cuts a function
  at every offset and asks that no expression range reaches past the cut.
- **`prepareRename` on a test's `setUp()` took 456 ms on Symfony.** The member family was walked
  down from PHPUnit's `TestCase` through every test that overrides it, to conclude what the
  declaration above already said: a member declared outside the project is not renamed. The
  ancestors are asked first now -- a header read each -- and the walk below only when they pass.
- **And 539 ms on a Laravel model's `belongsTo()`.** The walk below asked `declares_member` of every
  subtype, and that flattened each subtype's whole table -- hundreds of members inherited from
  `Model` -- to learn whether one line was in its body. `MemberTable::position` topped the profile.
  A flattened table's own members are exactly the class's own rows, so those are read directly: 539
  ms -> 194 ms on the first ask and single digits after.

The general point is the one the warm-start note makes about `serve`: a tool that only asks about
the states the author thought of reaches only the code paths the author thought of. A half-typed
file is the commonest state an editor's buffer is in, and it was the one nothing sampled.

## An open buffer had two spellings, and every "is it still open" asked the file table

A configuration buffer is held as text keyed by path, deliberately without a `FileId`: the index
holds nothing about a `services.yml`, and a row in the file table would make every reader of it have
an opinion about a file that declares nothing. That is right, and it made "an open buffer" a thing
with two spellings -- a VFS document and a held configuration text -- while the three places that
ask about open buffers each knew only the first. The quiet publish compared the scheduled version
against `file_id(path).and_then(document_version)`, which for YAML is always `None`, so no edit to
one was ever published; the sweep that checks what is open when indexing ends walked the VFS, so a
buffer opened during bootstrap was never published at all. Findings arrived only for a file opened
after indexing, and then never changed however it was edited -- a squiggle that outlived its cause.

Found while confirming an unrelated fix on the reference project, by a driven `serve` whose control
returned nothing where it should have returned two findings. `Workspace::version_at` answers by path
for both spellings and `open_documents` lists both. The next reader that asks "is this open" should
ask by path, or it will rediscover this.

## Two kinds of client filter a completion list, and the range was the thing to move

A client keeps or drops each item by matching something the person typed against the item's
`filterText`, or its label. Which something differs: Zed takes the _word_ at the cursor by its
language's word characters, and VS Code and the completion that follows it take the text the item's
edit _replaces_. The server is not told which it is talking to. Wherever an edit covered more than
the word -- a `use` path, a name written from the root, a service id with dots -- those were two
strings, and three fixes in a row treated it as a question about `filterText`: the rest past the
last dot for the word (`manager` for `application.ma`), then the text written for the range
(`\RuntimeException` for `\RuntimeE`), then a note saying one string cannot serve both. All
forty-four `use` lines sampled in Symfony's source were hidden from the second kind of client.

The string was never the thing to change. An item that begins with what was typed before the word
only writes it back, so its edit can leave that standing and replace the word alone: `Resp` with
`Response`, `ma` with `manager`. The buffer afterwards is the same byte for byte, the replaced text
_is_ the word, and both clients compare the same two strings. `TypedHead::kept` decides it in one
place for the PHP path and the configuration path. What is left is an item that does not go on from
what was typed -- a path spelled in another case -- whose edit still covers all of it.

The form is §8.1's: when the same shape of bug recurs, the fix to reach for is the one that removes
the shape. And the instrument is the one that found the first of these:
`tools/a-half-typed-name-is-shown-and-accepted.py` reports both rules side by side, and the column
for the second had been reading as a limitation rather than as a count.
