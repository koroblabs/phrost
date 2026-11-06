# Indexing, lowering, catalogs and the disk cache

> Part of the record §8 of `CLAUDE.md` began. Each note is a bug that was paid for once; the point
> of writing it down is that the _general_ shape at the end of each one recurs. Where a note says
> "§8", it means the record as a whole — `CLAUDE.md` §8 plus this family of documents. **Read this
> file before working in the area it names.** Add new notes here rather than to `CLAUDE.md`, and
> keep them in the shape the existing ones have: what looked true, what was true, what the tell was,
> and what generalises.

What lowering keeps and drops, how a file's claims are installed and withdrawn, how a catalog is
filled and ranked, discovery, and what a warm start does differently from a cold one.

**The notes, by their tell:**

- A number that looks plausible on one checkout is not evidence, and three separate classes of error
  came out of running the same report against a second project.
- Removing a file's catalog claims must be keyed, never scanned.
- Binding a method to a _closed_ catalog turns every non-key argument into an error.
- A lowering gate has to reach every place that lowers a file.
- mago only _drops_ a trailing statement at end of input
- The generic children walk must stop at a body lowered elsewhere.
- Lowering a `function_exists`-guarded declaration costs the standard library, unless it is ranked
  last.
- A nested declaration hides in nine places, not one.
- A matcher answering "is this reachable" and one answering "which is it" want opposite defaults.
- Indexing more declarations puts duplicates in every list that iterates declarers.
- A catch-all in lowering throws away a distinction the node still had.
- A tag can be read for one parameter shape and silently skipped for another, and the neighbouring
  line already knew.
- A catalog key that names a _class_ is a return type, so registering one that names nothing needs
  its own kind of claim.
- A catalog filled from the calls being checked cannot judge them, and the answer is a second source
  rather than a cleverer check.
- A cache codec's field order is the read order, and a struct literal does not tell you.
- A feature that works once and then never again is a _warm start_, and the CLI cannot see it.
- Two spellings of a project root are one project, and only a _key_ is allowed to know that.
- A file's rows are `install`ed by two paths, and only one of them knew where the text came from.
- What kept eleven thousand generated files out of the index was one line of the project's
  `.gitignore`.
- `list()` is an array literal wearing a keyword, and lowering it as an unmodelled node made its
  variables read as _uses_.
- A fact read out of a body has _two_ places to be said again, and `adopt` keeping `returns` is what
  makes the second one invisible.
- A report and the diagnostic beside it are two assemblies of one rule set, and the second was
  missing two rules.
- A word test cannot tell two classes' methods apart, and the fix is to record what an analysis
  already resolved.

---

- **A number that looks plausible on one checkout is not evidence, and three separate classes of
  error came out of running the same report against a second project.** A dead-code report read 61
  findings on the reference project and looked finished; on Symfony it read 188, of which 75 were
  trait members that no single file can settle. Read against Laravel it reported **1 050 of 4 590
  class-likes**, which is not noise to filter but the question asked of the wrong kind of thing -- a
  library exports its API to consumers a checkout does not contain, and composer says which is which
  in one word. And the `*Action` classes it listed on the reference project were services keyed by
  their own class name, a whole reference form nothing read. None of the three is visible from the
  project it was developed against, because a project is consistent with itself. The corollary is
  cheap to act on: before believing a report, run it over something of a different shape, and read
  what it _groups_ rather than what it totals.

- **Removing a file's catalog claims must be keyed, never scanned.** `remove_file` used to `retain`
  over every table, so each of 29 000 ingests paid for the whole catalog built so far -- quadratic
  bootstrap wearing the costume of a per-file operation. Withdraw the exact keys the file claimed,
  derived from the symbols still held for it. Adding the standard library (12 000 entries, ingested
  first) is what made it visible: +1.4 s from 137 files.

- **Binding a method to a _closed_ catalog turns every non-key argument into an error.**
  `ClassResolverInterface::getInstanceFromDefinition` takes a class name _or_ a service id, and
  binding it to the `service` catalog looked free -- the catalog exists, the binding is one row. It
  would have made every `getInstanceFromDefinition(Foo::class)` in the project an
  `unknown-catalog-key`, because a closed catalog reports anything it does not hold and a class name
  is not a service id. A catalog binding is only safe where the argument is _always_ a key of that
  catalog; "sometimes a key" is a different contract and needs a different mechanism.

- **A lowering gate has to reach every place that lowers a file.** Three do --
  `ProjectSkeleton::ingest`, `OpenSet::open`, and the scratch `OpenSet` in `references_in_file` --
  and `reset_index` has to clear it. Installed on the skeleton alone, _opening_ a file withdraws the
  catalog keys its declarations claimed and closing it puts them back: a catalog that empties itself
  while somebody works, with nothing failing. Same shape as "a local exists in two places". This is
  how positional attribute literals are gated -- the name set is derived from the provider's own
  rules, so a provider adding a rule and forgetting the list is not a state that can exist.

- **mago only _drops_ a trailing statement at end of input**, and this note used to overstate it.
  With no closing token to resynchronise on, the tail of the file is thrown away -- but inside
  braces mago recovers on the `}` and keeps a partial node, and mid-list it merges the unfinished
  statement forward into the next one. So "recovery ate it" is a file-tail problem, not a general
  one. Anything repairing what recovery lost belongs per _body_, over the window after the last
  statement mago returned; per-statement-list is the wrong grain and mid-list needs no repair at
  all. Probes for all three shapes are in `lower/src/tests/recovered_annotations.rs`.

- **The generic children walk must stop at a body lowered elsewhere.** An unmodelled _statement_
  keeps its subtree via mago's `Node::children()`. A class declaration is a statement, so that walk
  descended into every method body and lowered it a second time into the file's own body -- doubling
  the body IR of a typical PHP file. Nothing failed: `member_site_at` picks the narrowest match, so
  identical duplicates were invisible, and the only symptom was memory. Step around the body and
  keep the rest, because a parameter default is written in the declaration and has nowhere else to
  go.

- **Lowering a `function_exists`-guarded declaration costs the standard library, unless it is ranked
  last.** A guarded declaration means _only if nothing else declared this_, and on one real checkout
  402 names are written that way -- **113 of them polyfills of functions the stubs already
  declare**, every `ctype_*`, `mb_*`, `str_contains`, `array_key_first`. Ranked by origin a vendor
  declarer beats a stub, so lowering them alone silently swaps PHP's
  `str_starts_with(string, string)` for symfony's `(?string, ?string)` -- and three real
  `possibly-null-argument` findings disappear, which is the `?T where T expected` suppression §5
  forbids, arriving by the back door. That is why the first attempt was reverted, and the gain it
  bought was one name. What was missing was a precedence tier _below_ `SourceOrigin::Stub`:
  `DeclaredSymbol::is_conditional` now carries the fact from lowering through the def-map and the
  disk cache, and `CONDITIONAL` in `skeleton.rs` ranks it below all four origins. The general lesson
  is the one to keep -- when a declaration states its own precedence, indexing it and ranking it are
  one change, and doing either half alone is a regression wearing the costume of a fix.

- **A nested declaration hides in nine places, not one.** `if`/`elseif`/`else`, `switch` cases,
  `try`/`catch`/`finally`, the four loops -- each in both the braced and the colon-delimited form --
  and a function's or a method's own body, which is not a statement and so is reached by neither the
  statement walk nor anything else. Covering only the shape that prompted the work (`if`) leaves a
  name in no index, which reads to `undefined-function` exactly as a typo does. Measured on 30 871
  files: the control-flow shapes are 406 names and cost ~0.09 s of bootstrap, the function and
  method bodies are 9 names and ~0.06 s. The walk deliberately stops at an _expression_ -- a
  `function` statement inside a closure would then exist while a buffer is open and not while it is
  closed, which is worse than the gap.

- **A matcher answering "is this reachable" and one answering "which is it" want opposite
  defaults.** The hook-name matcher returns `true` past `PART_LIMIT`, which for reachability is
  silence and is safe -- refusing to claim a function is unreachable on absent evidence. Reused to
  _find_ a declaration, that same `true` sends a reader somewhere, so the limit has to be applied
  before the call and decline instead. Reusing a matcher across both directions is right; inheriting
  its default is not.

- **Indexing more declarations puts duplicates in every list that iterates declarers.** Ranking a
  `function_exists`-guarded declaration below the stubs made the polyfill's `str_starts_with`
  indexable, correctly -- and completion then offered the name twice, because the loop pushed one
  row per _declarer_ rather than per name. Two declarers of one name is ordinary (a stub and a
  polyfill, a class in two packages during a move) and `rank` already picks the one goto lands on;
  the second row asks a person to choose between things they cannot tell apart, and in
  `workspace/symbol` it also eats a page slot, because the cap runs before anything is compared.
  Whenever the index starts holding a kind of declaration it did not, walk the readers that iterate
  declarers rather than names.

- **A catch-all in lowering throws away a distinction the node still had.** Every statement phrost
  does not model became one `Unmodelled`, so the flow walk could not tell an `echo` from a `goto`
  and answered both "unreadable". The doc-comment called the cost "an `echo` after a `return`, and
  the one worth paying" -- it was not: a dead region _beginning_ with an `echo` was dropped whole,
  and a dead region usually begins with a log line, so from outside the check read as not working at
  all. mago's `Statement` enum is closed at 33 and exactly 14 reach that arm, so the classification
  is exact and belongs where the node is still in hand: `Passage::{Onward, Elsewhere, Arrival}`.
  Whenever a catch-all is about to answer a question its inputs could answer better, count the
  inputs first -- a closed list of fourteen is not a fallback, it is a table.

- **A tag can be read for one parameter shape and silently skipped for another, and the neighbouring
  line already knew.** `@param-out` was gated on mago's `ampersand` _token_, which a parameter
  written `&$matches` carries and one written `?int &$count` does not -- so `preg_match`'s tag was
  read and `str_replace`'s was not. The field being set two lines below in the same function asks
  `param.is_reference()`, which is the other spelling of the question and the right one. Two
  spellings again, this time inside one function body, and what made it _look_ like a stub problem
  is that both failing cases were stubs. The instrument that settled it was a test at the lowering
  layer -- and it could not see the field until `check_hir` was taught to render it, which is its
  own lesson: **a snapshot that does not show a field cannot test it.**

- **A catalog key that names a _class_ is a return type, so registering one that names nothing needs
  its own kind of claim.** The route catalog missed the 277 names Drupal generates from entity link
  templates -- `entity.node.canonical`, 43 call sites on the reference project -- and the obvious
  fix is to claim them the way an entity type id is claimed, with the declaring class as the value.
  That claim is read by `catalog_class`, and an argument bound to a catalog reads a key's class as
  the _call's return_: `Url::fromRoute('entity.node.canonical')` would have started answering
  `Node`. It is the same trap `entities.rs` documents for `getStorage`, arriving from the other
  direction, and nothing would have failed -- goto and hover on every `Url` chain in the project
  would simply have gone somewhere else. A claim that names nothing (`Claim::class` is an `Option`)
  is the shape; what it carries instead is a line of description, which for a link template is the
  path and is exactly what a route read from YAML already shows.

- **A catalog filled from the calls being checked cannot judge them, and the answer is a second
  source rather than a cleverer check.** The `hook` catalog was filled from the project's own
  `invokeAll`/`alter` calls, so a typo registered itself and the near-miss reader was structurally
  silent -- which reads as the check being broken and is the catalog being circular. What fixed it
  was measuring the sources _independent_ of the dispatch site: 333 `function hook_x()`
  declarations, 1 601 `#[Hook('x')]` attributes, and the names of the functions that implement
  hooks. Only the third covers the reference project's own hooks, and only measurement said so -- I
  had refused the whole piece on the circularity without asking what else could fill it. When a
  check is defeated by its own input, enumerate what else knows the same fact before concluding it
  cannot be done.

- **A cache codec's field order is the read order, and a struct literal does not tell you.** Rust
  evaluates struct fields in source order, so
  `Some(Branch { absent: read_flag()?, parameter: read_u32()? })` reads the flag out of the
  parameter's bytes while the writer wrote them the other way round -- the round-trip test says
  "what this build wrote, this build reads" and nothing points at the order. Twice in one session:
  once writing a flag the reader read (caught by an unused-variable warning), once reading two in
  the wrong order. Put a new field last in _both_ halves and keep them adjacent in the diff.

- **A feature that works once and then never again is a _warm start_, and the CLI cannot see it.**
  `ProjectSkeleton::claim` records catalog keys three ways; `adopt`, which takes on a stored index,
  re-claimed one of them. So the first server of the day held 1 038 hook names and 277 generated
  `entity.*` routes and every server after it held neither -- 1 315 keys, restored by nothing, with
  the near-miss check silent and hover over a hook name answering `NULL`. Three things about this
  are worth keeping. The report from outside was "reports once in five runs", which sounds like a
  race and is a cache. `phrost check` and `phrost query` **cannot reproduce it at all** -- both are
  cold, so a matrix of receivers, file locations and names run through them is 30 probes that all
  pass and prove nothing; the only instrument that sees it is a driven `serve`, and
  `tools/lspclient.py` is there for exactly that. And the round-trip test could not see it either,
  because nothing installed a derived-_name_ rule and no fixture wrote a key-carrying name, so the
  claim was absent from both sides of an equality. When a claim has three kinds and a caller names
  one of them, the fix is the reader they all go through -- and the fixture that would have failed.

- **Two spellings of a project root are one project, and only a _key_ is allowed to know that.**
  `phrost index --warm .` reused **0** of 30 965 rows where the same command spelled absolutely
  reused all of them; nothing failed, and the run printed `warm start` while doing the cold one's
  work -- so the instrument that asks whether a 45 MB cache earns its keep answers no. Two halves,
  and fixing either alone leaves the other: a file's stored path is kept _relative to the root_, so
  `.` stores `./docroot/x.php` where a server stored `docroot/x.php`, and the cache file is keyed by
  a digest of the root. Fixing only the key got **137 of 30 965** -- the 137 being the standard
  library, which lives outside any project and so matched, which is exactly the shape of a partial
  fix that reads as progress. The distinction that makes it safe is `absolute` against
  `canonicalize`: canonicalising the root itself was tried first and **hung a flow test**, because
  it resolves symlinks and macOS resolves a temporary directory's `/var` to `/private/var`, so the
  watcher stopped recognising notifications about its own project and the re-index never came. Every
  use of a root but a key is a comparison against a path the _client_ spelled. And resolving it at
  all breaks any caller still holding the argument -- three did, printing whole absolute paths where
  they meant short ones and looking for a file nobody indexed -- so `set_root` hands back what it
  resolved rather than leaving it to be asked for.

- **A file's rows are `install`ed by two paths, and only one of them knew where the text came
  from.** `ingest_lowered` builds a `FileSkeleton` from text and a HIR and cannot tell an editor's
  buffer from a disk read, which is why the fingerprint is set by the caller afterwards rather than
  passed in -- the same shape as the stamp parameter added in the commit before it, arriving from
  the other direction: the stamp is true of any text, the fingerprint only of a file.

- **What kept eleven thousand generated files out of the index was one line of the project's
  `.gitignore`.** Discovery walks the project root honouring ignore files, and PHPStan's result
  cache (`tmpDir: .phpstan`, 11 664 `<?php return [...]` files on the reference checkout) and
  Drupal's `sites/*/files` twig cache were absent only because the project ignored them -- a
  checkout that did not would have indexed them as user code, offered their classes in completion
  and checked them on save. Two answers, at two layers. The generic walk prunes the caches PHP's
  tools keep by convention (`.phpstan`, `.phpunit.cache`, `.psalm`) beside `.git` and
  `node_modules`. And a framework can now say what is PHP by extension and not code by nature --
  `FrameworkProject:: excluded_paths` -- which for Drupal is `sites/*/files`, `sites/simpletest` and
  the settings (`settings*.php`, `sites.php`): credentials in arrays, no declaration, every security
  lint firing on the passwords they exist to hold. Excluded means neither discovered nor accepted as
  a buffer, so a settings file opened in the editor gets no diagnostics and no lint at all rather
  than the `no-literal-password` it got.

- **`list()` is an array literal wearing a keyword, and lowering it as an unmodelled node made its
  variables read as _uses_.** PHP has had one destructuring construct since 7.1 -- `list()` takes
  keys, holes and nesting exactly as `[]` does, and neither can be written anywhere but an
  assignment or a `foreach` target -- so the two are one shape and every reader of a target knows
  both for free. Left unmodelled, the names inside kept their nested expressions, and
  `collect_bindings` recognises a target by matching `ArrayLiteral`:
  `list($xlen) = unpack('v', $s); return $xlen;` reported the binding itself as a variable nothing
  gives a value. **127 findings on WordPress**, whose bundled libraries write the long spelling
  throughout, and none anywhere else -- modern code writes `[]`, which is what kept it invisible on
  four corpora and on the reference project. The general form is §8's oldest one: when two spellings
  of one construct exist, the second is found by whichever reader was written against the first. The
  tell here was a _use_ reported at a position that is plainly a binding.

- **A fact read out of a body has _two_ places to be said again, and `adopt` keeping `returns` is
  what makes the second one invisible.** A file's own `propertyDefinitions()` states what a kind
  holds; `install` puts that into `returns`, and a warm start reaches `install` through
  `install_cached` on the **loaded** skeleton -- whose `returns` `adopt` then throws away, rightly,
  because the rule saying which declaration states a table is configuration. So every warm session
  answered `mixed` and every cold one answered correctly. The catalog claims two screens above do
  exactly this re-statement and say why; a new per-file fact needs the same line and there is
  nothing that fails without it. What makes it worse than the claim version: **`phrost query` and
  `phrost check` are both cold**, so the CLI measured the whole feature working -- 47 of 47 sites --
  while the thing a person uses did not. Four notes here now say some version of "only a driven
  `serve` sees this"; when a feature is derived from a body and stored, drive one before believing a
  number.

- **A report and the diagnostic beside it are two assemblies of one rule set, and the second was
  missing two rules.** What "nothing reaches this declaration" means is a list: a namespace a
  framework scans, a supertype it builds, a class the configuration names, a member above it in the
  hierarchy. `phrost unused` gathers that list itself and `unused-declaration` asks the view for it,
  and the view's copy had the namespace rule and not the _supertype_ one, and named no configuration
  **member** at all. So a Drush command class, the abstract base a project writes under it, and
  every command method were reported as dead -- 8 of 76 findings on one real checkout, every one of
  them a live entry point. §8.1's oldest form, arriving between a CLI and a server rather than
  between two spellings of a type, and the tell was that the CLI was **right**: when two tools
  disagree about the same project, the one with fewer rules is not the conservative one. Two details
  are worth carrying past the fix. The per-class question is not the report's question:
  `discovered_by_inheritance` enumerates every subclass of every discovered supertype, which is a
  pass over the whole index and right for a report run once -- and per keystroke the same fact is a
  climb up _one_ class's ancestors, bounded by `DISCOVERY_DEPTH`. The `NeverReturns` shape again,
  where what is expensive is the enumeration. And a rule that needs a framework provider **and** a
  configuration file has no unit test that can see it: neither exists in a view fixture, which is
  exactly how both went missing without anything failing.
  `lsp/src/tests/flow/unused_declarations.rs` is the layer where they do.

- **`factory: ['@id', 'method']` reads the method as a class, so a live service reads as dead
  code.** Drupal's container writes a factory three ways -- `Foo::create`, `\Foo`, and a pair naming
  a service and one of its methods -- and the rule for `factory` said `Class`. The bare method name
  then resolved to nothing (harmless, and invisible) and, worse, the method it named was reached by
  _nothing_: `CloudClientFactory::authClient` is called by that YAML line and by no PHP in the
  project, so it was reported as dead while the container builds a service out of it every request.
  The fix is a target of its own rather than a rule about `::` inside `Callable`, because the three
  shapes are what the key means and a reader lands somewhere different for each. What it still does
  not do is resolve the `@id` to its class, so the site names a member and no class -- enough to say
  the method is reached, not enough to send a reader there. Worth knowing when adding a
  `ConfigTarget`: four matches on it are exhaustive by design, and the compiler names all four.

- **A word test cannot tell two classes' methods apart, and the fix is to record what an analysis
  already resolved.** `unused-declaration` asked _is this name written in another file_, which is
  what made it affordable and is wrong three ways one word test cannot separate: a class called
  `StoreClient` counts as a use of a method called `storeClient`; a name in one namespace counts for
  the same name in another; and `StockDataManager::importStockData` being called keeps the
  `importStockData` declared **in the same file as the call** alive, which is the shape a reporter
  found and the coarse rule cannot even be asked about. The exact question needs the receiver
  resolved, and that was already being paid for: every file the project owns is analysed anyway --
  the project pass does the closed ones, the buffer check the open ones -- and the one thing that
  analysis settles about _other_ files was thrown away. `Reached` records it, ~1 MB, and the lookup
  is one hash. **68 -> 99 findings** on the reference project. Four things about it are load-bearing
  and three were false positives before they were rules. An access is a claim on **every declaration
  in the receiver's hierarchy that it satisfies**, not on the one class it was written on: downward,
  an inherited method is called on the subclass and declared on the parent; upward, an interface
  method is the contract, and a project whose callers happen to be typed as the implementation would
  otherwise have every interface it defines reported. An access that resolves to _nothing_ is a
  claim on the bare **name**, or 1.6 % of accesses become findings about code that runs -- the
  coarse rule survives exactly where the precise one has nothing to say, a string-written callable
  included. A container's `calls: [[addExportType, ['@a']]]` is a call site, and 33 methods on one
  checkout have no other. And the pass has to **gather before it judges**: the first sweep published
  every file while the record was still being filled, so it answered coarsely for all of them and
  the numbers did not move at all -- the second sweep is inside the same pass, not scheduled after
  it, so that one pass is one thing and a test waiting for it waits for the answer it will keep. The
  cost is that second sweep: time-to-quiet on 1 711 files **0.73-0.76 s -> 1.43-1.61 s**, RSS 180 ->
  182-187 MB, and it is paid only where the check is switched on -- gathering rides on an analysis
  already happening, judging again does not. What makes it safe to answer before the sweep lands is
  that the coarse rule accepts everything the precise one does, so the switch only ever _adds_
  findings: monotone, and nothing a reader saw is withdrawn. The class half is the same rule one
  level up -- a reference is recorded against what it _resolved to_, so `App\Billing\Repo` and
  `App\Legacy\Repo` stop being one word -- and it moved **no finding** on the reference project: the
  same fourteen classes before and after. That is the honest reading rather than a dead rule; a
  project's classes are named in code, where they resolve, and the hole needs two namespaces sharing
  a short name to fall into. One detail is load-bearing: a file's references to what _it_ declares
  are left out, matching what the coarse rule does by skipping the declaring file whole. Counting
  them would let the precise rule _withdraw_ a finding the coarse one gives, and monotonicity is the
  whole reason the check may answer before the sweep lands. Three traps in testing it. A fixture
  where the enclosing class is itself unreached pins the class rule, because a class nothing reaches
  is reported _instead of_ its members -- the member test needs a third file that names the class. A
  snapshot of what a pass published is a snapshot of four worker threads' arrival order, taken
  twice; compare the set, not the sequence, or the test fails one run in three and reads as a bug in
  the feature. And a published line carries the file's base name and the declaration's short one, so
  two `Repo.php` declaring two `Repo` are indistinguishable in it -- which is fatal for the one test
  about telling them apart. Renaming a file to separate them trips `psr4-mismatch`; declaring one
  lower down separates them by line and keeps the fixture valid.

- **A stored fact is only covered by the two invariants if the dump renders it _and_ a fixture
  contains one.** The `@throws` table was added with a cache round-trip test that pins the dump and
  compares built against restored, which is the right shape -- and `incremental == from-scratch`
  still covered it by nothing at all, because **no fixture in `incremental.rs` or `edit_fuzz.rs`
  held a `@throws`**. The invariant passed, the renderer printed the section, and the section was
  empty in every case it saw. The same family as §8.4's accepted-empty snapshot, one layer down: a
  dimension every fixture runs through is only as good as what the fixtures say. The edit has to
  walk **both** directions for a sparse table -- a tag added where there was none, and a tag dropped
  where there was one -- because a re-ingest that only ever added would pass the first and leave a
  stale row claiming a method still throws. That direction cannot fail today, since the table
  travels on `FileSkeleton` and `install` replacing a file's rows takes its old entries with it; the
  test is the guard for the day it moves to a file-keyed side table beside `dispatches`, where
  withdrawal is a line somebody has to remember to write.

- **Only the server writes the index cache, so `phrost index --warm` on a CLI-only workflow reuses
  nothing -- and that reads exactly like a broken codec.** Measured while verifying that a `@throws`
  survives a warm start: `stored index loaded in 49.62µs`, `0 reused from a stored index`, on a 28
  818-file project. The store is `Workspace::write_to`, called from the LSP workspace and from
  nowhere else, so a cache written by an older binary is rejected by the build stamp and nothing the
  CLI does replaces it. Not a regression and not something to fix -- but the first reading of those
  two numbers is "my new cache section broke decoding", and an hour goes into that. What settles it
  is §8's own rule, applied properly: drive `serve` twice and read the bootstrap line. Both runs
  reported **28 818 reused from cache, 28 818 of them without reading**, and the findings were
  identical -- which is stronger evidence than a CLI comparison could ever be, because it says the
  answers were produced from the codec with no file opened at all.

- **Two stored facts landed the same day and neither invariant saw either.** The note above records
  it for `@throws`; it happened twice more within hours, which is what makes it a habit rather than
  an incident. A body-derived return _path_ is rendered in the dump and no fixture in `cache.rs` or
  `incremental.rs` held a wrapper; a parameter's default is rendered inside the signature line and
  no fixture held one. Both invariants passed, both sections were empty in every case they saw. The
  fixture for a sparse table has to walk **both directions** -- a row added where there was none,
  and a row dropped where there was one -- because a re-ingest that only ever added passes the first
  and leaves a stale claim. For the default that is one edit: give the method a declared return type
  and take the default off, and the dump shows the chain row gone and the signature changed in the
  same pass. The working rule, after three of these in one day: **when a fact is added to the codec,
  the same commit adds a fixture that contains one.** Rendering it in the dump is necessary and is
  not enough.

- **The owner catalog has two tables and one of them is keyed by a name nobody holds.** `class_like`
  is keyed by the **fully-qualified** folded name and `short_names` by the short one; both take a
  `NameKey`, so a reader holding a short name and calling `class_like` compiles, type-checks, and
  answers for exactly the classes in the global namespace. Which is the worst possible failure,
  because the stub set is almost entirely global: a new attribute-completion tier came back with
  `Attribute`, `Override`, `Deprecated` and `SensitiveParameter` and nothing a project declares, and
  that reads as "the feature works, the project's own classes are not marked" rather than as a wrong
  lookup. Two hours went into lowering and into `install`, both of which were correct the whole
  time; what settled it was a unit test at the _view_ layer printing the list, the bit and the page
  in one panic -- the list had the name, `class_is_attribute` said true, and the page did not hold
  it, which leaves only the step between them. `short_named` now exists so the question can be asked
  by the name a reader actually has. Three spellings of one thing, §8.1, with the added sting that
  the wrong one is right for the global namespace.

- **A duplicate claim is silence, and the catalog is the only thing that can say so.** `docs/08-php`
  and §8.4 both record what two files declaring one name does to a probe -- the catalog refuses to
  rank two, the name resolves to nothing, and every row of a matrix reads as a missing feature. What
  neither said is that the index _knows_: `owners().class_like(key)` holds both declarers, and
  nothing was reporting it. `twice-declared` now does, and it would have saved an hour twice in one
  session. The rule took three passes over real code to get right, and each pass is a filter worth
  keeping. **Anonymous classes** carry a synthetic `class@anonymous#88` so `$this` inside one
  resolves, and two files whose anonymous classes land on the same counter are not a conflict --
  Symfony and psl had four of those. **A guarded declaration** is a shim and the guard is the point;
  asking that of one side only made a shim pair report exactly once, which is the tell. And the big
  one: **a declaration the autoloader would never be asked for.** A test fixture, a dumped container
  and an example script are included _by path_, so two of them may share a name and PHP never sees a
  second declaration -- without that filter Symfony reported 39 and Laravel 22, all of them
  fixtures. What separates a conflict from a coincidence is PSR-4: a class the autoloader finds by
  name has exactly one file it may come from, so both halves must sit where the map says. The shape
  that produces a real one is two directories under one prefix, which is an ordinary
  `composer.json`. Final measurement: 0 on all five projects and on the stub set, and it fires on a
  scratch project built with a dumped `autoload_psr4.php`. Which is also the note: a fixture has no
  map, so this check is dead in one and correct on the real thing -- the direction §8.4 already
  warns about.

- **Four more stored fields the dump did not print, which is four more the index invariants could
  not see.** `incremental == from-scratch` and `cold == warm` both compare `dump_skeleton`, so the
  note beside `stored_notes` -- "a field the dump does not print is a field a stale re-ingest can
  get wrong quietly" -- is a standing audit rather than a one-off. Met four more times:
  `MemberModifiers::is_override` (`#[\Override]`, one bit through the codec),
  `FunctionRow::is_no_discard` (`#[\NoDiscard]` on a _free function_, where the member's own bit was
  already rendered), `ClassSkeleton::mixins` (`@mixin Other`, resolved at ingest) and
  `ClassSkeleton::supertype_arguments` (`@extends Bar<Baz>` -- a class that lost these answers every
  generic question with the declaration's _bound_ instead of the argument, which is a wrong answer
  nothing fails on). The audit is mechanical and worth repeating after any field is added: list what
  `ClassSkeleton`, `MemberRow` and `FunctionRow` hold, grep `dump.rs` for each, and the misses are
  the answer.

- **A shape the sweep cannot break is worth nothing, and the two have to be widened together.**
  `edit_fuzz`'s own doc says the corpus is what limits it. Printing the four fields above made the
  yardstick able to see them and changed nothing on its own, because no file in the corpus wrote
  one. `src/shapes.php` does now: property hooks in both their forms, `public private(set)`, a
  `#[\Override]`, a class declared `#[Attribute]`, a `final readonly` class, a member an `@method`
  tag declares, a `@mixin`, and the arguments an `@extends` supplies. The order matters and is the
  lesson: **print first, then write the shape**. A corpus entry added while the dump is still blind
  to the field passes for the wrong reason, and reads as coverage.

## `$this` inside a trait does not reach the class that uses it, and the measurement says leave it

A trait method's `$this->m()` resolves against the trait alone. Where `m` is declared in the trait
it answers; where the _using class_ declares it -- which is how Laravel's
`HasTimestamps::freshTimestampString()` calls `$this->fromDateTime()` -- goto, hover and completion
answer nothing and every check on the value stays silent. Coverage classifies those accesses as
`dynamic class`, which is the honest reading: the receiver is a class and the member is not on it,
so nothing is reported either.

PHPStan analyses a trait once per using class and says so in its own output --
`HasTimestamps.php (in context of class Illuminate\Database\Eloquent\Model)`. That is a
multiplication phrost's one-analysis-per-file shape does not have.

Priced before building. `dynamic class` is **67 of 50 264** member accesses on the reference project
(0.1 %) and **6 393 of 202 908** on Laravel (3.2 %) -- and not all of either is this shape, since
the bucket also holds `__call` receivers and the subtype-declares rule. Drupal's traits mostly
declare their own members or an abstract, and what a Drupal class reaches through `$this` comes from
a base class by `extends`, which already resolves. So the benefit is concentrated on one framework's
style and is near zero on the checkout a person here actually reads.

What it would cost is the part that decides it: resolving a missed trait-member access needs the
list of classes using the trait, which is a pass over the class table -- affordable where a _report_
is about to be made, as `a_subtype_declares` is, and not affordable per goto, per hover and per
keystroke of completion. A `trait -> sole user` table built at index time would avoid the scan and
is the shape to reach for if this is ever worth it; recorded here so the next reader does not
re-derive either half.

## A fourth bool costs a word, and four bits cost nothing

`DeclaredSymbol` carried three bools -- `is_nameable`, `is_conditional`, `is_ignored`. Adding
`is_deprecated` grew the struct past its padding and the index by **0.3 MB** on a 28 851-file
checkout, measured with `phrost index --memory`: 104.0 MB to 104.3 MB, all of it in `scopes`.
Packing all four into one `SymbolWords` byte put it back to 104.0 MB exactly, so the new fact is
free and there is room for four more.

The same argument `MemberModifiers` already makes about member rows, and the same place to look
next: a struct the index holds one of per declaration has no spare bools, only spare _bits_.

## The start a promoted parameter's docblock is read from is not the constructor's

`Lowerer::param` receives the enclosing function's `declaration_start`, and a promoted parameter's
own docblock sits above `public array $rules` inside the parameter list. The `@var` reading already
knew this and computed its own `own_start` inside a closure; a second reading added beside it used
the argument, so a `/** @deprecated */` above a promoted property read the _constructor's_ docblock
and never the parameter's. Nothing failed -- the answer was just always the wrong declaration's.

Hoisted now, so both readings take the same offset. The general form is the one §8.1 names: a
correction written inline in one reader is a correction the next reader does not get.

## `"order.{$field}[0]"` is a string, and lowering said nothing about it

PHP writes a string three ways: a literal, a concatenation and an interpolation. The first two
produce a `string` -- the literal by its own kind, the concatenation through `ExprData::Operator`,
whose whole job is an operator that decides its own result -- and the third fell through to
`Unmodelled`, which says nothing at all. So **every local assigned an interpolated string was
untyped**: `$path = "a/$b"; f($path);` was judged by nobody, and `phrost coverage` counted it among
the assignments with no type without anybody reading the list far enough to notice the shape.

It is the same node the concatenation already uses, with the holes left where every other nested
expression goes. The whole change is one arm.

Measured on the reference project: assignments typed **93.9 % -> 94.5 %**, 71 more of 11 439, and
untyped receivers 331 -> 328. On the corpora it is worth a handful of findings and every one was
read: WordPress's `$src = $_POST['src']; if (…) { $src = "http://$src"; }` joins a `string` with the
superglobal's `string|array`, and `wp_basename($src)` takes a string; Laravel's
`->after(! is_null($after) ? "{$name}_type" : null)` really does pass null. Nothing moved on the
reference project itself, and `phrost check` is unchanged at 2.0-2.8 s.

**One reader matched the node by shape and had to be told.** `literal_returns::integral_holes` takes
an interpolated argument apart -- `strtotime("-$i months")` cannot fail for any integer `$i`, and
only the text around the hole says so -- by matching `ExprData::Unmodelled`. The note above that
function already asks for exactly this: "a walk that matched by shape and quietly disagreed is the
failure this refuses rather than guesses past." Both arms are named there now rather than a `_`
fall-through, and the test that covers it is a live A/B: the recognised hole is silent and the one
that spells a unit out of a value is still reported.

Seen while reading the new findings and not chased: Symfony's `CliDumper::style` renders as
`array|null|?string`, a `?` _inside_ a union, which `docs/08-types.md` records as the tell for an
inference bug rather than a cosmetic one. The finding itself is true -- `preg_replace_callback`
returns `string|null` and the value is returned out of a `: string` -- so it is the _rendering_ that
is worth a look, not the answer.

## `@` is not an operator that produces anything, and it produced nothing

`$x = @$row['k'];` was untyped. The error-control operator fell through to `Unmodelled`, which says
nothing at all, so every value written with one was invisible to every check -- 622 occurrences in
Symfony, 226 in WordPress, 26 in Laravel, 11 on the reference project.

What `@` suppresses is a **diagnostic of PHP's**, a warning the engine would print, and not a type:
a `TypeError` still throws straight through it. So the honest lowering is that it leaves no node --
the operand _is_ the expression -- and the rule that objects to writing `@` at all is a syntax rule,
which is where that objection belongs.

Found by the same sweep as the interpolated string above it: a scratch file that assigns one of
every expression shape PHP has to a local and passes each to an `array` parameter, so anything not
reported is a shape lowering says nothing about. The others it named, with how often each is
_written_ as a value across four corpora: unary `-`/`+` (65), `~` (10), `$x++` as a value (15),
`print` (rare). `@` was worth more than all of them together, which is why it went first.

Measured: **nothing on the reference project**, 2 on Laravel, 27 on Symfony, 105 on WordPress -- 72
of those in one vendored ZIP library and the rest on socket and stream handles, which is what code
written around `@` looks like. Read rather than counted:
`$this->_ftp_data_sock = @socket_create(…); if ($this->_ftp_data_sock < 0)` is a real defect of
WordPress's vendored FTP class, because PHP 8 hands back a `Socket` object and `< 0` does not test
one. `phrost check` unchanged at 2.0-2.4 s.

## `__CLASS__` is a string, and not typing it cost 157 findings a call away

`__LINE__` is an `int` and PHP's nine other magic constants are `string`s. The compiler substitutes
a literal, so this is the same kind of fact as a quoted string and not a name anything resolves --
mago hands them over as `Expression::MagicConstant` and lowering let them fall through to
`Unmodelled`.

What that cost is not the assignment nobody writes. It is what a **call around one** then answers:
`str_replace('\\', '.', __CLASS__)` declares `($subject is array ? array : string)` and can only
pick the branch when the subject says which it is, so an untyped `__CLASS__` left `array|string`
standing and every `string` parameter below it was reported. **153 findings on Symfony, 4 on
Laravel, and nothing new anywhere** -- the largest single removal of false positives this file
records, from an eight-line arm.

Worth keeping as the lesson: a shape lowering says nothing about does not cost what that shape is
worth, it costs what everything _downstream of it_ would have decided.

### The sweep this came out of, and what it refused

A scratch file that assigns one of every expression PHP has to a local and passes each to a
parameter of an incompatible type: anything not reported is a shape lowering says nothing about. Two
passes, one over operators and one over the value-producing constructs. What it named, with how
often each is written as a value across four corpora:

| shape                                                                                                                                    | verdict                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `@$x`                                                                                                                                    | fixed -- 885 occurrences, the note above this one                                                                                                                                                                                                                                                |
| `"a$b"` and heredocs                                                                                                                     | fixed -- the note above that                                                                                                                                                                                                                                                                     |
| magic constants                                                                                                                          | fixed here                                                                                                                                                                                                                                                                                       |
| a global or namespace `const`, and `define()`                                                                                            | **refused**: `ConstantData` carries no type at all, so this is a HIR field, two lowering sites, an index row, a codec and a cache bump -- where a _class_ constant's identical question is already answered by `constant_value_type`. Worth doing when something needs it; nothing measured does |
| unary `-` and `+` (65)                                                                                                                   | **refused**: PHP implements `-$x` as `$x * -1`, so the honest node is an `Arithmetic` with a synthetic `-1` in the tree or a new `ArithmeticOp`, and the vocabulary is closed by convention. Visible payoff on the reference project: `$days >= 0 ? 1 : -1` becomes an `int`                     |
| `~$x` (10), `$x++` as a value (15), `print`, `$s .= 'x'` as a value                                                                      | **refused**: same shape, an order of magnitude rarer                                                                                                                                                                                                                                             |
| `instanceof`, `**`, `%`, `&`, `\|`, `^`, `<<`, `<=>`, `::class`, `!`, `(object)`, a spread literal, a destructuring target, an enum case | already typed                                                                                                                                                                                                                                                                                    |

And the list the sweep was drawn from is itself now read out: `phrost coverage`'s **559 remaining
untyped assignments** are element reads off untyped arrays (`$element['#value']`,
`$data[$k] ?? NULL`), calls on `object`-declared parameters, and dynamic calls -- all of them
untypeable rather than missed, bar the unary minus above.

## A helper that returns a local described nothing, and a `Foo::class` fell off the chain

`returned_path_from_body` read a returned _call_ -- `return $this->getDatabase()->select($t);` --
and stopped at a returned variable, while `returned_local` read a variable for a _type_ and not for
a chain. So `$connection = Mockery::mock(Connection::class); ...; return $connection;`, which is how
every test helper builds a double, left what it returned untyped. The one assignment that gave the
local its value is read now, by the closed set of writers `returned_local` already states. Laravel:
177 696 -> 178 325 member accesses resolved, 31 391 -> 31 636 assignments typed.

The first corpus run found what the chain was missing:
`$kernel = $this->createStub(Kernel::class); return $kernel;` stored `createStub` with no argument,
so the double read as a bare `Stub` and `$kernel::SUB_REQUEST` was an undefined constant. A chain's
first argument is a typed key now -- `ReturnedKey::{Text, Class}` -- and a `Foo::class` is resolved
at ingest against the file that wrote it, since nothing can once the file is shut. The view answers
it through the same `class-string<T>` contract a written call goes through
(`returned_class_naming`), so the double is `Stub&Kernel`.

And the run after that found the cost of the first way of storing it: the key was a recorded name
_reference_, which every check walking references saw a second time at the same place -- three
`deprecated-use` findings on a `::class` the expression already referenced and nothing reported. It
is a written path and an anchor now (`Lowerer::written_name`), resolved without being recorded.
Cache version 39, and the dump prints the key on the step it was handed to, since `cold == warm`
compares only what the dump prints.

## A returned chain stopped at the first local in it

`returned_path_from_body` reads a method with no declared return through what its body returns, and
the root of that chain had to be `$this`, a `new`, or a static call. A _local_ ended it --
`$document = new DOMDocument(); return $document->createElement($tag);` stored nothing -- although
the returned local itself was already read through its one assignment (`the_one_value_of`).
Symfony's DomCrawler tests build every field through `createNode()`, which is that shape one hop
further (`$node = $document->createElement(...); ...; return $node;`), and Laravel's
`createTestBatch()` returns `$repository->store(...)` off a `new` two lines up.

A local in the chain is now read through the same closed set of writers, so it roots where it was
assigned: a parameter is the caller's, a name written twice has no one value, and either refuses the
chain. `LOCAL_HOPS` bounds the reading apart from `CHAIN_LIMIT`, because `$a = $a;` adds no step and
would read itself forever; both refusals are pinned in the cache test that prints the chains.

Measured: Laravel 92 assignments typed that had none and 77 accesses resolved, Symfony 112 and 25,
none lost. Findings +36 on Laravel and +81 -3 on Symfony, all of them declarations speaking that
nothing reached before: `Batch::add()` is `self|null`, so `$batch = $batch->add(...)` in the batch
tests is a `?Batch`, and `DOMDocument::createElement()` is `DOMElement|false` in PHP's own
signature, so 72 fields built from it in the DomCrawler tests may be handed a `false`. 338 Laravel
accesses moved from no receiver to `object`, every one after
`(new ReflectionClass(Store::class))->newInstanceArgs(...)`, whose `T` a stored chain does not carry
-- `object` is what the stub says without it, and asks nothing of a member. `check` unchanged on
Symfony; `cold == warm` there, 12 407 both ways.

## A constant built from other constants is kept as its parts

`define('HOUR_IN_SECONDS', 60 * MINUTE_IN_SECONDS)` had no type, and nor did the 190 reads of
WordPress's time and size constants built that way, or psl's `const INFINITY = INF;` and
`const SEPARATOR = DIRECTORY_SEPARATOR;`. `constant_value_type` reads only what the syntax settles,
and said so: "a constant naming another constant ... finding it means resolving a name -- which is
the def-map's job". It is, but not this file's def-map alone: which declaration `MINUTE_IN_SECONDS`
reaches is a fact about every file, and the declaring file is closed by the time anyone asks.

So the _shape_ is lowered (`ConstantValueData`: settled parts, other constants, and the arithmetic
and bitwise operators joining them), the index resolves each written name as far as one file can --
to the candidates PHP would try, through `scope_at` where the name is written, because a `define()`
inside a namespace declares a global name and reads its value in the namespace -- and the view
settles the type on demand (`locals/constants.rs`), through the same `arithmetic_result` a body's
`+` goes through and a `bitwise_result` beside it. The first candidate that is _declared_ is the one
followed, and only that one: a namespaced constant that exists and says nothing does not hand the
question on to the global it shadows. Eight constants deep at most, which is what stops the stubs'
`const PHP_EOL = PHP_EOL;` from following itself.

Not a new reference: the initialiser's expression already records one for each name, so a part keeps
the name as text (`written_name`, the helper made for a returned chain's `Foo::class`) rather than
as a `NameRefId` -- two would put every use twice into find-references.

Stored, so `VERSION` is 43; a row is 32 bytes where it was 8, which is 0.1 MB on WordPress's 15.6 MB
index with the stubs' 6 000 constants included. Class constants are not done: their value would ride
on `MemberRow`, which every member of every class carries, and Symfony's `self::A | self::B` flags
are a few dozen declarations.

Measured: WordPress 23 assignments typed and one shape's `interval` from `mixed` to `int`; no
finding moved on any project, and none was expected -- most reads of these constants are arguments
and operands, which the coverage dump does not list. `cold == warm` on WordPress; `check` with
`--jobs 1` unchanged.

## A package's IDE stub shadowed the class the autoloader loads

Eloquent Power Joins ships `vendor/kirschbaum-development/eloquent-power-joins/.stubs.php`: an IDE
helper declaring `Illuminate\Database\Eloquent\Builder` with only the six methods the package adds.
Discovery walks `vendor/` whole and with dotfiles, so it is indexed -- correctly, since nothing says
a dependency's file is not code -- and two vendor declarations of one name were ranked by path,
where `kirschbaum-development` sorts before `laravel`. Every access on Laravel.io's builders reached
the stub: 86 member accesses with no such member, `where`, `get` and the project's own scopes among
them, and no other corpus had the package. `rg` did not find the second declaration either, because
it skips dotfiles too; `phrost query`'s goto on the `use` line named the file at once.

The rule that settles it is PHP's own: composer decides which file is read for a class, so within
one tier a class-like whose file is where a PSR-4 rule maps its name ranks before one that is not
(`ProjectSkeleton::autoloads`). Within a tier and not above it, because a project's own copy of a
dependency's class is still the one its developer means. A classmap-only class gains nothing from
this and loses nothing -- both declarations read as not autoloaded and path decides as before. Only
names with two declarers pay, once per declarer now that the sort caches its key. Laravel.io 330 ->
228 findings; every other corpus unchanged.
