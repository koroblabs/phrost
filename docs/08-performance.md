# Performance and memory

> Part of the record §8 of `CLAUDE.md` began. Each note is a bug that was paid for once; the point
> of writing it down is that the _general_ shape at the end of each one recurs. Where a note says
> "§8", it means the record as a whole — `CLAUDE.md` §8 plus this family of documents. **Read this
> file before working in the area it names.** Add new notes here rather than to `CLAUDE.md`, and
> keep them in the shape the existing ones have: what looked true, what was true, what the tell was,
> and what generalises.

Where a cold start's time and a session's resident memory actually go, which remedies were measured
and refused, and which instrument answers which question. Read with §3 of `CLAUDE.md`, which owns
the budgets.

**The notes, by their tell:**

- A rule measured with the rule switched off reads exactly like the rule working.
- Two different things look like "the stored index saves nothing", and neither is that.
- A request can be slow while the feature inside it is fast.
- An id that is a bare `u32` makes every `Option` of it cost eight bytes, and the index is full of
  them.
- A cold start held 13.4 MB a warm one did not, and the tell was in the memory report all along.
- A rule asked per member access cannot afford a walk of the enclosing body.
- Recording a fact at every call site is priced by the _interning_, not by the row.
- Price the _test_ before indexing around it.
- A server's own "ready" is a claim, and two memory readings cannot check it.
- Indexing used one core of twelve, and the phase split is what said where to cut.
- A `Name` that reaches a stored row uninterned is invisible to every reader and to the instrument
  that exists to catch it.
- Two thirds of what a session held was not the index, and `index --memory` could not see it because
  it never printed what the process held.
- `heap` and `malloc_history` name the allocation sites, and the answer was not what the size report
  suggested.
- Reusing a bump arena across files made it 165 MB worse, and the API said so in one line.
- On macOS `open()` is the cost of reading a file, not the bytes.
- `sample` is the profiler, and its tree splits one function across several nodes.
- ~100 MB of a cold server is the system allocator's large cache, and both obvious remedies make it
  worse. (Resolved by `lspkit_heap::Heap`.)
- Four questions answered from one directory tree were four walks of it.
- A memo across requests pays only if the walk that fills it reads it too.
- A property of the tree computed per position is a walk per position.
- To profile a keystroke, drive the server and space the edits past the debounce.
- `malloc_zone_pressure_relief` does nothing against macOS 26's allocator, and says so.
- `Path::starts_with` is a component walk, and two checks ran it over 849 roots per file.
- A rule about a position has a half the tree fixes, and that half was being recomputed per
  position.
- A start-up walk's cost is `opendir` per directory, and the way to make it cheaper is to visit
  fewer directories -- not to read less in them.

---

- **Two different things look like "the stored index saves nothing", and neither is that.** The
  first: `phrost index` was unconditionally a _cold_ start, so running it twice -- the obvious way
  to ask whether a cache earns its 45 MB -- measured two full indexes and came out flat. The second:
  `build_stamp()` is the executable's mtime XOR its size, so **every rebuild throws the whole cache
  away**, which means anyone developing phrost measures a cache that is never valid, and a warm
  start that reused nothing takes exactly as long as a cold one. Only the reuse _count_ separates
  them, so `index` now prints it and takes `--warm`. Measured with a cache the running binary
  actually wrote: indexing 30 871 files goes 2.9 s cold to 1.2 s warm, 30 871 of 30 871 rows reused.
  Note also that `Start::Warm` in the sweep loads a stored index and never writes one -- only the
  server writes -- so `check --warm` on a project no server has opened is a cold start wearing a
  different label.

- **A rule measured with the rule switched off reads exactly like the rule working.**
  `unused-member` is opt-in, and `phrost check` does not turn an opt-in rule on. So the run that was
  supposed to show a new filter removing two false positives showed **zero findings**, which is what
  a filter removing everything also looks like and what a working one looks like too. Twice in one
  piece of work: the same zero appeared again when the filter really did suppress all nineteen,
  because it matched any call named `bind` and Symfony writes `->bind()` throughout its dependency
  injection. The tell for both is the same and it is not the number: a change that should remove
  _two_ things and removes _all_ of them has not been checked, it has been agreed with. Force the
  rule on and compare the list, never the count.

- **A request can be slow while the feature inside it is fast.** The index side of a 128-hit
  `workspace/symbol` is 1.2 ms; the request takes 26-34 ms cold. The difference is
  `answers/positions.rs`: turning a byte offset into a line and column needs that file's text, and
  128 hits in 128 files is 128 cold reads plus 128 line indices. Not a fallback and not new -- it is
  where the fact was always going to come from -- but the perf row and the wire then disagree by an
  order of magnitude, and anyone tuning the feature by watching the row tunes the wrong five per
  cent. `references` and `rename` share `Positions` and have the same shape.

  **And that last clause was wrong, which is worth keeping beside it.** Measured per phase on
  `references`: scan 29.5 ms, analyse 1.1 ms, positions 0.3 ms. `Positions` is real for
  `workspace/symbol`, whose hits are scattered across as many files as there are hits; a reference
  sweep converts a handful of ranges and reads the _whole project_ to decide which files to look in.
  Same symptom, different cause, and the note read as though the diagnosis travelled. Which half of
  the scan cost was measured rather than reasoned about too, and it was the other one: stubbing the
  matcher out while leaving the read in measured 26-29 ms against 29.5 with it, so megabytes of
  `eq_ignore_ascii_case` are free and `memchr` would have bought nothing. What the sweep costs now
  is 1.3-4.7 ms of scan and however many candidates genuinely mention the name -- and the fix for
  the reading was to stop doing it, `WordSet` in `docs/08-index.md`.

- **An id that is a bare `u32` makes every `Option` of it cost eight bytes, and the index is full of
  them.** `TypeId(u32)` has no value a compiler can spend on `None`, so `Option<TypeId>` was 8 bytes
  -- three of them on every parameter row alone (`ty`, `doc_ty`, `out_ty`), more on a member row.
  Making it `NonZeroU32` with the table's slot zero reserved took the index from **92.6 MB to 88.9
  MB** on a 30 916-file checkout, `members` 32.4 -> 29.0, at no measurable cost to bootstrap
  (2.93-3.01 s either way). What made it cheap to do is that only three places in `ty/src/table.rs`
  turn an index into an id or back, and `as_raw` keeps meaning _the index_ -- so the disk format
  does not move and the off-by-one never leaves that file. **And it was the only one**: the same
  question asked of every other id came back empty, which `crates/index/src/tests/row_sizes.rs` now
  prints rather than leaving to be re-derived. The arena ids live only in a `FileHir`, which exists
  for an open buffer -- twenty of those cost 2.5 MB in total. `TextSize` cannot have a niche at all,
  because zero is a real position rather than a spare slot, and an off-by-one that works for a table
  index does not work for a type that is added and compared; and it would buy nothing anyway, since
  `DeclaredSymbol` is 55 bytes in a 56-byte struct and taking four off leaves 51. `Name` already has
  one, being an `Arc<str>`. The general lesson is the second half rather than the first: a niche
  pays where the optional field sits in a row there are _hundreds of thousands_ of, and alignment
  can eat the saving even then -- so price the row, not the type.

- **A cold start held 13.4 MB a warm one did not, and the tell was in the memory report all along.**
  `FileScope::build` grew its lists by pushing, `from_parts` is handed vectors of a known length,
  and `lspkit-memsize` counts _capacity_ because capacity is what is resident -- so `scopes` read
  27.4 MB cold and 14.0 MB warm on the same tree, and the row had said so for as long as both paths
  existed. A scope is never appended to after it is built, so that was held for the life of the
  process; a session that started cold carried it and an identical warm one did not. Two lessons,
  and the second is the useful one. **Comparing the cold and warm memory rows is free and finds this
  class by itself** -- `phrost index --memory` against `--warm --memory`, one line each, and any row
  where they differ is either waste or a lost fact. And **sizing beats shrinking**: `shrink_to_fit`
  on all four lists bought the memory and cost 3.03 s -> 3.17 s of bootstrap, while sizing the two
  whose counts the HIR gives in O(1) and shrinking only the three that cannot be sized bought the
  same 13.4 MB and went _faster_ than the original, 3.03 s -> 2.89 s, because it removes the
  grow-chain instead of adding a pass. What is pinned is the property rather than the number, and
  the test deliberately **enumerates no list**: it rebuilds the scope through the cache's own
  `from_parts` and compares `heap_size`, so a sixth list added and not sized fails it without
  anybody remembering to add it anywhere.

- **A rule asked per member access cannot afford a walk of the enclosing body.** Deciding whether
  control went through a write to a slot is `visit_reaching_expressions` over the body, and paying
  it per access cost `phrost check` 2.62 s -> 2.78 s, +6 %, with every budget still green -- which
  is the headroom §3 exists to protect rather than a failure anything reports. The `NeverReturns`
  move removes it whole: a _name_ nothing in the file ever assigns cannot have a write to find, so
  `assigned_properties` is one file-level walk the view memoizes and one comparison per access. Back
  to 2.51-2.63 s with the same five findings. The tell that a prefilter of this shape exists is a
  question whose expensive answer is almost always "nothing".

- **Recording a fact at every call site is priced by the _interning_, not by the row.** A
  conditional return type can turn on a number -- `pathinfo`'s `($flags is 15 ? array : string)` --
  so the number a call writes has to reach the reader. Recording every integer argument cost
  `phrost check` 2.50 s -> 2.60 s, +4 %, and the cost is one `to_string()` and one interner lookup
  for the second argument of every `substr`, `round` and `str_pad` in the project rather than the
  `LiteralArgument` rows themselves. A static table of the digits `0`-`15` -- a flag is a small
  number, and a declaration turning on `4096` is not a thing anybody writes -- gives the same
  answers at 2.42-2.47 s. The general form: when a per-site fact turns out expensive, price the
  _construction_ of the key before dropping the feature; here the two answers were "record nothing"
  and "record everything", and the third one costs nothing.

- **Price the _test_ before indexing around it.** A loose name search over a 30 964-file checkout
  took the completion row from 0.38 ms to 1.79 ms at p95, and the obvious reading is that walking
  sixty thousand names is what costs. It is not: stubbing the matcher to reject everything, and
  leaving the walk in place, measured **0.47 ms** -- the walk is a tenth of a millisecond and
  scoring every string is all the rest. So what pays is a cheaper _rejection_, not a smaller walk: a
  32-bit mask of which letters each name holds, computed once when the name is claimed, turns "could
  this be a subsequence" into one `&`. Back to 0.60 ms for +0.3 MB. One build settled which of the
  two designs to write, and the wrong one -- an index by first letter -- would have cost a structure
  and bought a fifth as much.

- **A server's own "ready" is a claim, and two memory readings cannot check it.** `compare-lsp` read
  RSS at the end of indexing and at the end of the run, which cannot tell a server that has finished
  from one still going -- and that difference is most of what a memory comparison is about. Measured
  with a phase that idles and samples: phpantom reports ready at **1.1 s holding 80 MB** and reaches
  **468 MB** about a second later, asked nothing at all. A harness reading once at the end of
  `ready` would have published the 80 MB and been wrong by six times. It also makes the _next_ row
  lie in the other direction -- its cost to open forty files then looks like +1.5 MB against our
  +11.7, which is not economy, it is having already paid. `tools/session-cost.py` is the instrument;
  the phase that earns it is the one where nothing is asked.

- **Indexing used one core of twelve, and the phase split is what said where to cut.** Not "profile
  before optimising" in the abstract: the two obvious guesses were both wrong. `phrost index` on 30
  965 files measured 4.12 s wall for 2.42 user + 1.68 sys, and the split is **read 1.019 s, lower
  1.627 s, register 16 ms, claim 255 ms** -- so 90 % of a cold start touches nothing shared and 9 %
  touches everything, which fixes the design rather than leaving it to taste. The first wrong guess
  was mine and it is worth keeping: I wrote here that a warm start is "mostly `stat`" and that
  parallelising the parse would not help it. A file's stamp hashes its **content**, so a warm start
  reads every file whole -- 921 ms of its 1.08 s -- and parallelism helps the start a person sees
  every day more than the one they see once. Measured after: 2.92 s -> 1.19 s on eight workers, and
  4.8-5.3 s to 2.7-3.4 s of server bootstrap on four workers, at 12-15 MB of resident memory a
  worker. **And the memory question could not be answered until cold and warm were separated**: the
  same instrument read 285, 313, 343 and 409 MB across runs of one binary and looked like pure
  noise, because some runs were cold and some warm. Forced cold and repeated, it is flat -- 258 MB
  on one worker, 306 on four, 352 on eight -- and a _warm_ start costs the same 205-209 MB whatever
  the worker count, because nothing is parsed. A 148 MB "regression" measured early was a cold run
  against a warm one. The cost is the allocator holding each thread's high-water mark, most of it
  the bump arena a parse needs: one per file, since the tree borrows the arena and there is no
  reset. That is structural to parsing in parallel, and it is what caps the worker count -- not the
  batching, which accounted for 8 MB of it.

- **A `Name` that reaches a stored row uninterned is invisible to every reader and to the instrument
  that exists to catch it.** Lowering on a worker means lowering against _that worker's_ interner,
  and `ingest_lowered` has always documented accepting one -- so the moment it ran over a whole
  project rather than over one open buffer, every member name, template name, parameter name,
  contract parameter, namespace name and global-scope symbol became a second allocation of a string
  the project already had. Nothing reads differently: a `Name` compares by content, so goto, hover
  and completion are all correct. And `lspkit-memsize` counts an interned string once and the `Arc`s
  pointing at it **not at all**, which is right and is exactly what hides this -- the report went
  from 87.9 MB to 82.7 MB, interned names from 208 700 to 29 484, while resident memory rose. A
  memory report getting _better_ while RSS gets worse is the tell. What checks it is identity rather
  than text: walk the stored rows and compare the **address** of each name's bytes against
  `Interner::lookup_name`. Note also what is _not_ the check -- comparing the two interners' sizes
  is unsound, because the serial path also interns every name lowering merely _sees_.

- **Two thirds of what a session held was not the index, and `index --memory` could not see it
  because it never printed what the process held.** The gap is now a row of that report, and finding
  where it went took `vmmap --summary` rather than any instrument this project owned: on a server
  that had written a cache, **93.8 MB of empty-but-dirty `MALLOC_LARGE`** -- more than a third of
  the process, held by nothing. `encode` built the whole index into a `Vec` that doubled its way to
  46 MB, so the 32 MB and 64 MB blocks it passed through were both freed, and macOS keeps a freed
  large region rather than returning it. `load_cache` was the same shape read back: `fs::read` of 46
  MB to decode from a slice. Neither needed the buffer -- the format is written once in order and
  read once in order -- and streaming both took a warm session **209 -> 161 MB** and a cold one
  **261 -> 237 MB**, with no change in decode time. The general shape: when resident memory is twice
  what your own accounting says, the question is not "which structure is too big" but "what did the
  allocator keep", and the answer comes from `vmmap`, not from `size_of`.

- **`heap` and `malloc_history` name the allocation sites, and the answer was not what the size
  report suggested.** 731 837 live allocations, and the ranked sites are all `Reader::list::<T>` --
  one `Vec` per class's members (26 623 calls), one per method's parameters (**126 292**), one per
  file's namespaces and imports. It is worth knowing that this is _not_ where the memory went:
  `MALLOC_SMALL` holds about 105 MB dirty for 88 MB of rows, so the per-`Vec` overhead is real but
  is a fifth of the waste, where the two large buffers were half of it. Rank by what `vmmap` says is
  dirty, not by what looks wasteful.

- **Reusing a bump arena across files made it 165 MB worse, and the API said so in one line.**
  Parsing allocates its tree into a fresh `LocalArena` per file, which hands the allocator a chunk
  it then keeps -- so reusing one arena per worker and resetting between files looks obviously
  cheaper. `reset`'s own doc-comment reads "keeping the arena's chunks for reuse", and that is the
  whole problem: an arena grows to the largest file that worker ever sees and never gives it back.
  Measured, cold RSS went **237 -> 401 MB** and empty large regions 68.6 -> 224.3. Reverted. The
  churn is cheaper than the retention, which is the opposite of the usual advice about arenas, and
  the tell was in the documentation of the function being called.

- **On macOS `open()` is the cost of reading a file, not the bytes.** Measured over 41 825 files: 21
  us for `open`+`close` against 2.5 us for `stat`, and 27 % of a cold start was the `open` syscall
  alone -- more than blake3, UTF-8 validation and the line map together. A warm start was nearly all
  of it, because a row is trusted on its content hash and a hash needs the bytes. That is what
  `FileFingerprint` is for: mtime, ctime and length asked with one `stat`, the hash only when they
  differ, and the fingerprint refreshed when the hash then matches. Two things about it are
  load-bearing. The ctime is what makes the metadata trustworthy -- a restore can set mtime and
  length, and cannot set ctime. And a row built from an editor's buffer has _no_ fingerprint,
  because the buffer is not the disk; `ingest_lowered` sets none and only the reader of a file sets
  one. Warm indexing 450 ms -> 78 ms. The reuse count is two counts for the same reason the reuse
  count exists at all: a cache whose fingerprints never match reads every file and reports
  everything reused, exactly like one that works.

- **`sample` is the profiler, and its tree splits one function across several nodes.** Attach it to
  a running `phrost index` for three seconds and the leaf table names the syscalls and the hot
  functions with nothing to build. The tree is by _return address_, so `prepare_one` appears three
  times at one depth with 1179, 873 and 104 samples -- read the sum, or a phase looks a third of its
  size. Also: an aggregation script that `break`s after the first node of a name reports one of the
  three.

- **~100 MB of a cold server is the system allocator's large cache, and both obvious remedies make
  it worse.** `vmmap --summary` shows `MALLOC_LARGE (empty)` at 90 MB after a cold bootstrap: freed
  parse arenas and file buffers that macOS keeps for reuse rather than returns. `MallocLargeCache=0`
  in the environment takes a cold server from 270 MB to **166 MB** and shows the ceiling; it is not
  a fix, because a process cannot set it for itself. Measured and refuted: mimalloc 0.1.52 as the
  global allocator (cold RSS 394-406 MB, footprint 330 MB at ready, warm +10 MB), and a per-worker
  `LocalArena` reused across files and gated at 128 KB of input (cold +25 MB, large-empty 121.6 MB)
  -- `blink_alloc`'s `reset` allocates one merged chunk on growth and frees the old ones, so reuse
  _adds_ large frees; the unbounded version had already measured +165 MB. What is left is
  `malloc_zone_pressure_relief` after bootstrap, which is one unsafe FFI call and so a policy
  decision, or making fewer large frees. The small-allocation half was real and is fixed: a cache
  decoded into a vector and then into tables grown by doubling left 12.7 MB of large and 13 MB of
  small empty regions in every warm server; decoding into a presized table removed both. **Resolved
  without the re-exec, 2026-11-06**: `lspkit_heap::Heap` maps blocks of 256 KiB and up from the
  system itself, keeps at most 32 MiB of freed ones for reuse and unmaps the rest. Cold sessions on
  the reference checkout: ready at 193 MB against 240-268, 230-234 against 247-278 after the project
  check -- the `MallocLargeCache=0` ceiling (195, 235) with ready time unchanged, and
  `index --memory` reports 30.5 MB actually given back after indexing where macOS's own relief
  gave 0. The remedy that worked was the third one: not a better allocator, but not asking one to
  keep what it was never going to give back. **And then the allocator after all, 2026-11-09**: the
  394 MB that refuted mimalloc was `ps`'s RSS, which on macOS keeps counting pages an allocator has
  given back with `MADV_FREE_REUSABLE` and so never falls; the footprint (Activity Monitor, the
  `footprint` tool, what `lspkit_perf::rss` reads there since lspkit 0.1.1) is what the kernel
  charges, and one process stood at 150 MB of RSS and 47 MB of footprint. Read that way, mimalloc v2
  as the process's `malloc` under `Heap` (lspkit-heap's `override`, macOS only) takes the footprint
  after bootstrap and `release_freed` on the reference checkout from 170 MB to 151-156, and with 20
  files open from 178 to 164-170, request p95s unchanged, while `ps`'s RSS rises from 186 to 223 MB
  in the same run. Two tells worth keeping: the perf sweep read its memory figure _before_ the
  release the server does at the end of bootstrap, which hid the whole difference (181 against
  177-179), and reads after it now; and Linux stays on glibc, which gives a C library's freed blocks
  back by itself -- with mimalloc there orthra measured 4.5 MB more RSS per project.

- **A leak in a language server is a list somebody appends to, and the first instrument found its
  own.** `tests::a_session_gives_its_memory_back` drives one thing a session does over and over -- a
  buffer opened, edited and closed; a file saved; the project indexed again; a server built and
  dropped -- and requires that `lspkit_heap::stats().live()` stops moving. Its first run found two:
  `Workspace::watched_since_discovery` kept the path of **every file changed on disk** for the life
  of the server (83 bytes a save; it was cleared only by a bootstrap, and a session runs one), and
  lspkit's socket kept open the channel its own stream read, so **every server a flow test built**
  left 6.5 KB behind. Its first draft found a third that was not there: it noted each step's reading
  in a `String`, and live bytes grew 8 a step with a jump at every doubling of its own vector --
  exactly the shape of a leak. What settled each one was `MallocStackLogging=1` and
  `malloc_history <pid> -allBySize` at 20 steps and at 220, diffed by stack frames with the
  addresses stripped (ASLR moves them between runs, and a diff keyed on them is all noise): the
  stack whose count grows with the steps is the leak, named to the line. Two rules follow, and the
  test's own docs carry them: a count that is the whole process's needs the test alone in its
  process, and nothing a leak test keeps may allocate inside its loop. And one thing `Heap` cannot
  see: mago's interner takes its 256 shards straight from `System` (`LeakyBumpAlloc`), about 17 MB
  of address space a process, so `stats()` is what went through the global allocator and not
  everything the process maps.

- **Four questions answered from one directory tree were four walks of it.** The Drupal provider
  walked the docroot for the extension descriptors, again for the same descriptors, again for the
  procedural files, and discovery then walked the tree a fourth time: 0.5 s of `set_root` on every
  start, cold and warm. The seam is now `FrameworkProvider::project(root)` returning a
  `FrameworkProject` that holds one `Survey`; the generic layer makes it once and asks it four
  times. Related, and measured the hard way: `ignore`'s parallel walker starts its threads per
  `run()`, so handing it a thousand file roots one at a time took discovery from 0.3 s to 2.9 s.
  Gather the roots, walk once, and `stat` a root that is a file instead of walking it.

- **A memo across requests pays only if the walk that fills it reads it too.** The first version of
  the cross-request member-table memo cached the class a request asked for and nothing else, and
  bought `check` almost nothing: a sweep meets a new class in nearly every file, and the flattener
  rebuilt `FormBase` under each one from scratch because it never consulted the memo for an
  _ancestor_. Handing the memo to the flattener -- `FlattenMemo`, read at every level and filled
  with every complete table on the way -- is what made it 2-5x on every per-site row. Same shape as
  the arena-reuse trap in the other direction: a cache placed one level too high caches the wrong
  unit. Also: `MemberTable::contributors` had been kept for exactly this eviction since the table
  was written, and the LRU is by _use_, not clear-all, because clear-all was measured buying nothing
  under a sweep's churn.

- **A property of the tree computed per position is a walk per position.** `Scope::chain_at` runs
  for every position a request resolves, and its closure step walked every statement of the
  enclosing body -- 9 % of a diagnostics publish on the generated file that is the worst case.
  `FileHir::closures_directly_in` derives it once per revision by range containment, which is the
  same answer because the statement walk never descended into a closure's own body. The remaining
  shape of the same kind is the real one: `reaching_expression` and `visit_guarding_operands` walk
  the enclosing body _per lookup_ and legitimately depend on the position, and they are now about
  half of what diagnostics cost on that file. Replacing them needs a per-body flow index, which is a
  design and not a fix.

- **To profile a keystroke, drive the server and space the edits past the debounce.** `check`
  measures the pass and not the publish: the real keystroke also lints (a second mago parse, 9 % on
  a big file, and legitimate because `phrost.toml` asks for it) and lowers the buffer. A driver that
  re-sends the file as `didChange` and `sample`s the server sees all of it -- and sent every 120 ms
  it saw _one_ publish in 49 edits, because `QUIET` is 300 ms and every edit reset it. That first
  profile was all lowering and read as "diagnostics are cheap".

- **`malloc_zone_pressure_relief` does nothing against macOS 26's allocator, and says so.** Both a
  zero and a maximal goal returned 0 bytes with 92 MB of empty large regions still resident. The
  xzone allocator that replaced the magazine one does not honour the public relief API; the only
  thing seen to empty that cache is `MallocLargeCache=0` in the environment before the process
  starts, which a process could only arrange by re-executing itself. `phrost index --memory` prints
  the returned count as `given back`, so a platform where the call works reports itself.

- **`Path::starts_with` is a component walk, and two checks ran it over 849 roots per file.** 15 %
  of a diagnostics pass, for a question a byte-prefix and a separator test answer identically when
  both paths are absolute and spelled by the same code. When a path comparison is on a hot path, ask
  what it compares.

- **A rule about a position has a half the tree fixes, and that half was being recomputed per
  position.** `reaching_expression` and the guard visitors walked the enclosing body for every
  position resolved, and a diagnostics pass resolves every position in the file -- on the 1 500-line
  generated file that was the worst case, half of a 35 ms publish. Which writes a body holds, which
  branch each sits in, whether a branch can be left, which conditions guard which spans: the tree's.
  Whether the position is inside a span: the position's. `facts.rs` derives the first half once per
  `FileHir` revision (`FileHir::derived` is the typed slot for facts a layer above computes) and
  applies the second per candidate write: 35 -> 14.8 ms on that file, p50 7.9 -> 5.6 ms, `check` 3.1
  -> 2.8 s, findings byte-identical. Two things made it safe rather than brave. The old walks stay
  in `reach.rs` and `walk.rs` as the _statement_ of the rule, `#[cfg(test)]`, and `facts_oracle.rs`
  asks both about every assignment of every body at every statement and expression boundary of eight
  fixtures -- if/elseif/else, every loop, try/catch/finally, switch fall-through,
  match/ternary/coalesce/logical, closures, property and element writes, file scope -- and demands
  the same writes in the same order with the same reach, and the same guards. And a prefilter on the
  variable under a write's target must pass a target it cannot read (`None` matches every name), or
  a static-property or computed target that the precise test would have judged is dropped before it
  gets there. Worth knowing when reading the numbers: `check` on the live reference project went 96
  -> 94 findings during this work, and a worktree build of three earlier commits said 94 for all of
  them -- the project had changed under the instrument, not the checker.

- **A start-up walk's cost is `opendir` per directory, and the way to make it cheaper is to visit
  fewer directories -- not to read less in them.** A sample of the walker threads from launch put
  seven samples in ten in `__opendir2` and the rest in `getdirentries64`; telling `ignore` not to
  probe for `.ignore` files and global excludes, the obvious guess, changed nothing (205-287 ms
  either way) and was reverted. What did work was pruning the Drupal docroot survey the way Drupal's
  own `ExtensionDiscovery` prunes -- `src`, `assets`, `css`, `js`, `images`, `misc` -- after
  checking with `fd` that not one descriptor or procedural file lives under any of them on the
  reference docroot: 24 395 of 25 404 files skipped, setup 235-259 -> 104-132 ms, catalogs
  identical. Two of Drupal's own skips had to stay _in_: `lib` holds a dozen `*.api.php` documenting
  hooks and `templates` holds a contrib module's `theme.inc`, and a prune list copied whole would
  have lost both silently. Count what a prune would lose before writing it, and count it on the real
  tree.

- **Drupal core is eleven thousand files of real PHP that no instrument here had ever been pointed
  at, and it is one command away.** `phrost check <project>/docroot/core` runs the whole check set
  over a codebase written in a different decade and a different style from the five corpora --
  procedural hooks, untyped parameters, `{@inheritdoc}` everywhere. Read it with one caveat firmly
  in mind: core has no `vendor/` of its own, so its dependencies do not resolve and the run reports
  **36 776 `undefined-type`** and tens of thousands of downstream nulls. The totals are noise. What
  is worth reading is a _new_ check's rows, because those mostly do not depend on resolving vendor.
  It earned its keep the first time: eight `impossible-use` on `new $this->containerClass(...)`,
  every one false, because a kind read off an **untyped property** is a starting value rather than a
  claim -- the line `SubscriptSite::base_is_local` already draws and that a new check has to draw
  again. And three true `bad-string-offset`, all the same defect: `JoinPluginInterface` documents
  `@param string $table` and every implementation writes `$table['alias']`.

## The diagnostics row measured a set of checks no editor ran

`perf` timed `diagnostics(&view)`, the default set, while the server runs
`diagnostics_asked_for(&view, workspace.asked_for())` -- the default set plus whatever the project's
`phrost.toml` switches on. The reference project switches on nine, among them `assumed-non-null` and
`unused-declaration`, which were the two most expensive checks on its largest file. So the row read
**16 ms p95** for a server that was spending **29.6 ms** -- a hair under the 30 ms budget, and 42.8
ms at worst -- and `CLAUDE.md` recorded 7.5 ms from an older run of the same blind instrument. An
instrument that understates is as wrong as one that flatters (§8.2); this one understated exactly
for the projects that ask for more, which are the ones that care.

`perf` now measures what the server runs, and prints the slowest checks on the worst file by median
of seven runs (`phrost_analysis::check_costs`), which is how the two fixes below were found rather
than guessed:

- **`unused-declaration`, 9.5 -> 4.3 ms.** Before the project has been swept its answer is words:
  whether another file's word set may hold the name. `mentioned_outside` walked the whole file table
  -- 28 886 entries, 1 753 of them with a word set -- per declaration, and `written_in` re-tokenised
  the file's quoted strings per declaration. `WordEvidence` asks the word sets about every name the
  file declares in one pass, and takes the quoted words once and only if needed.
- **`assumed-non-null`, 8.9 -> 4.9 ms.** It re-runs the sentinel checks in a twin view with
  contracts lifted, and the twin started with empty caches, so it re-resolved every path in the file
  -- seven milliseconds for one contracted call. Each memoized resolution now records whether a
  contract changed anything on the way to it (`contract_applied`, set in `contracted()`, the one
  place a contract is applied, and propagated through cache hits), and the twin inherits every
  answer no contract touched: for those the two worlds compute the same thing step for step.
  Verified by whole finding lists rather than counts, identical line for line on three projects.

Measured after: **23-25 ms p95**, 28 ms at worst, under load.

Then two more, by the same breakdown. The word sets' needles are hashed once per name rather than
once per name per file (`unused-declaration` 4.3 -> 2.1 ms), and a view now keeps the file's calls
and each call's parameters for its lifetime, as it already kept member accesses -- six argument
checks walked the same calls and resolved the same callees. The second was worth about a
millisecond, not the four the per-check column suggested: `check_costs` charges a shared resolution
to whichever check asks first, which is what its doc-comment warns. **~20 ms p95**, 25 ms at worst.

## The corpora were never measured by `perf`, and one file took seven seconds

`perf` had been run against the reference project only. Run over the corpora on 2026-09-25 it failed
four budgets on WordPress and Laravel, and the worst was not close: **7 002 ms** to check
WordPress's `wp-includes/script-loader.php`, all of it `incompatible-argument`. Not a regression --
the session's first binary measured the same -- but a server that freezes for seven seconds on a
file people open. Four quadratic shapes, every one §8.1's _a property of the tree recomputed per
position_, found in this order by `sample` (macOS's own profiler; read "Sort by top of stack" for
self time, the call graph is inflated by recursion):

- **`passed_to_calls`**, which finds the calls a value was handed to, visited every preceding
  statement and read each one's receiver and arguments, per read of a value. `PassedPaths` indexes
  which statements pass which path once per view; the walk still decides which of them precede the
  position, and reads only those. 7.0 -> 5.0 s.
- **`declaration_called`** resolved a call's receiver below full budget, which the path cache did
  not keep, so each call's callee was re-resolved for every later read of every value passed to it.
  Remembered per `(call, budget)`. 5.0 -> 0.9 s.
- **`resolve_path` kept only full-budget answers**, for the good reason that a budget-limited answer
  must not stand in for a real one. The budget is in the key now, which keeps that promise and
  caches every level of a chain. `class-wp-query.php` 466 -> 268 ms.
- **`inside_an_assignment_to`** walked every expression in the file per read of an annotated local.
  It reads the statement around the position now.

One tried and reverted, with the measurement: batching `claim_through`'s write test per branch made
`class-wp-query.php` _slower_, 272 -> 330 ms, reproducibly; the cause was not found and the change
is not in.

Then five more of the same shape, each by `sample` on the worst file:

- **`enclosing_call`**, asked per read of a closure's parameter, walked every expression -- a
  Laravel test of two hundred `->where(function ($q) ...)` walked itself two hundred times. The
  file's call expressions are a `Derived` slot now.
- **`member_site_at`**, asked per candidate by `undefined-member` (was the access guarded by
  `method_exists`?), walked every member site. Also a `Derived` slot, in walk order so the narrowest
  tie resolves as before. Laravel 500 -> 227 ms worst.
- **`writes_subject`**, asked per statement of every narrowed branch, read the body's every write.
  `BodyFacts` keeps the writes ordered by start and hands out the window a statement covers -- which
  is what the reverted batching above should have been: it moved the full scan instead of removing
  it. WordPress 272 -> 181 ms.
- **`proved_over_a_null_coalesce`** walked every sub-condition for a `?? NULL`; the file's are a
  `Derived` slot and a condition holding none is skipped. And **`Flags::value`** found a flag's
  latest write by reading every write; `BodyFacts` indexes writes by the variable they are rooted
  at. 181 -> 160 ms.

A third round, profiled with `phrost perf <project> --hold <seconds>`, which loops the worst file's
diagnostics after the sweep so `sample <pid>` sees nothing else:

- `passed_to_calls` tested membership in its statement list linearly per statement visited; the list
  is sorted now and searched.
- `Scope::enclosing_declaration` read every declaration's parameters per position -- seven hundred
  test methods in one Laravel file. `BodyFacts` keeps each declaration's reach and a nesting index:
  the declarations holding a position are the ancestors of the last one to start before it, and a
  file whose reaches do not nest falls back to the whole list.
- `MemberTable::find` read every member of a framework class per access; it keeps a folded-name
  index, reset by every change to the members.
- `FileHir::closures_directly_in` filtered every closure in the file per call; it is a slice of a
  list sorted by parent.

Then the narrowing itself. Every read of a local asked each statement before it, and each guard
around it, what it proves about the value -- an answer that belongs to the statement and not to the
read. `phrost_body::ProvedMemo`, kept per view and handed to the walk on the guard path as
`Asserts::memo`, remembers it. Exact, not approximate: the one input that depends on the position is
the reader's "does this call ever return", which resolves at the position asked from and refuses
while another such question is open -- so the view counts every such answer that got past the name
filter, and an answer computed while the count moved is not kept. The one walk in `phrost-body` that
asks with a "never" of its own passes `WithoutMemo`.

Two things measured before it paid. Remembering _every_ statement made Laravel slower -- ten million
lookups, most for statements that answer from their own shape in less than a lookup costs -- so only
an `if` is remembered. And the first version keyed the tables by `(statement, owned subject)` with
std's SipHash, which was 15 % of the check on its own; the tables are `FxHashMap`s keyed by the
statement, with the subject compared rather than cloned.

After that, three more of the same shape: the memo keyed by the subject's hash (a statement asked
about many values scanned them all), a body's `@var` statements and the names its `foreach` targets
bind written down once (two walks per read of a local), and the call around a closure found through
a `RangeNest` of the file's calls.

Then resolution itself got cheaper, by allocating less: the path memo is keyed by position and
budget with the path compared rather than cloned into every lookup, a member table is found by the
spelling it was asked with before the name is folded into a new string, and the three walks over a
body's writes for one name are handed only the writes that can be rooted at it
(`BodyFacts::writes_that_may_root_at`, in the body's own order, rootless writes included).

And the nesting index answers from the stack: the positions holding an offset used to come back as
two short vectors per ask, built and freed hundreds of thousands of times, which was a tenth of the
check on its own (`RangeNest::holding` returns a `Holding` with sixteen inline slots, and skips the
sort where every parent precedes its children). `perf --hold-on actions` loops code actions instead,
which showed they spend their time in the same resolution machinery.

Where it stands, under load: the reference project **~12.5 ms p95**; WordPress **~70 ms p95, 85 ms
worst**, code actions 41 -> 15 ms and in budget at the cursor; Laravel **~84 ms p95, ~105 ms by
median of seven on its worst file** (`tests/Database/DatabaseQueryBuilderTest.php`), code actions 50
-> 38 ms. What is left is spread thin -- nothing above 5 % of self time bar `narrowing_at`'s own
loop -- and each further step is worth a few percent.

What Laravel's worst file is made of is no longer repeated work. By inclusive time 60 % is the
argument checks and 22 % the member check, and both are resolving receivers and argument values -- a
test of seven hundred methods writing `$this->getBuilder()->select(...)->where(...)` thousands of
times. `extra-argument` reads as the most expensive check only because it asks first and pays for
the resolutions everyone after it shares. Going further means making a resolution cheaper, not
avoiding one twice. RSS with 20 files open on the corpora is +19-21 MB, the same as the session's
first binary. What is left is the narrowing itself -- `proved_about`, `claim_through`, `path_of`,
asked per guard per position. §8.1's split is the likely next step: what the tree settles about each
guard derived once per body, applied per position -- a larger change than another index, and not yet
measured. The corpora are still over budget and this is the next place to look -- `perf <corpus>`
prints the slowest checks on the worst file. Every step above was verified by whole finding lists,
identical line for line on three projects.

## What is left on the corpora is position, inventoried before anything is cached across it

Measured on Laravel's worst file (`tests/Database/DatabaseQueryBuilderTest.php`), one diagnostics
pass: **33 955** uncached path resolutions, of which **20 239 (60 %)** are a `(path, budget)`
already resolved in the same view at another position. That is the whole remaining gap to budget on
the corpora, and it cannot be taken by dropping the position from the memo key: the position is in
the key because the answer can depend on it, and a cache that shared an answer across positions
where it does differ returns a wrong type silently -- in a place no finding list would show, since
only the places a check speaks are compared.

Where a resolution reads its position, and so what a position-free memo would have to rule out:

| entry                                                                      | from                                                                                                                   | what the position decides                                             |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `phrost_body::local_binding`                                               | `locals/bindings.rs`, `variables.rs`, `resolve/target.rs`, `locals/produced_by.rs`, `hints.rs`, `symbols/contracts.rs` | which write of a local is the last one before it                      |
| `phrost_body::slot_value`                                                  | `locals/path.rs`, `locals/steps.rs`, `chain.rs`                                                                        | which writes and guards of a property or element slot stand before it |
| `phrost_body::element_written`                                             | `locals/steps.rs`, `chain.rs`                                                                                          | which element writes precede it                                       |
| `phrost_body::passed_to_calls`, `passed_to_any_call`                       | `locals/steps.rs`, `locals/path.rs`                                                                                    | which calls the value was handed to before it                         |
| `phrost_body::narrowed_at`, `narrowed_this`                                | `symbols/contracts.rs`, `resolve/name.rs`                                                                              | which guards hold there                                               |
| `phrost_body::bound_closure_scope`                                         | `resolve/hierarchy.rs`                                                                                                 | what `$this` is inside a bound closure                                |
| `IndexView::enclosing_class`                                               | every relative type (`self`, `static`)                                                                                 | which class the position is in                                        |
| `IndexView::resolve_name`                                                  | every written name                                                                                                     | which namespace block's imports apply                                 |
| `IndexView::call_never_returns`                                            | the narrowing and flow walks                                                                                           | the position a `never` call is resolved at                            |
| `phrost_body::local_name`, `call_site_at`, `member_site_at`, `variable_at` | cursor readers                                                                                                         | what is under the cursor -- not on the diagnostics path               |

The chains that repeat most are rooted at `$this`, and those read `enclosing_class` and
`narrowed_this` at every position -- so a coarse "did this resolution read its position" flag, the
shape `contract_applied` has, would mark nearly every repeated answer as position-dependent and buy
nothing. What could work is finer: have each entry above report _what_ it read -- the class the
position is in, the binding it found, whether any guard or write applied -- and key the shared memo
by those facts rather than by the offset, so two positions share an answer exactly when every fact
the answer read was the same at both. That is a change to every row of the table, each one a place a
missed fact is a wrong type, and it wants its own session with an A/B over whole finding lists _and_
over hover at every member site of the corpora, since the finding lists alone do not reach every
answer.

## What the 60 % of repeated resolutions actually are, measured

Taken on the same Laravel file (`DatabaseQueryBuilderTest.php`), counting each uncached
`resolve_path` whose `(path, budget)` the view already answered at another offset: **31 308 repeats
against 3 433 fresh**, and 27 592 of the repeats are more than 2 000 bytes from the first answer. So
they are not one expression asked from several offsets by several checks -- which would have been
cheap to share -- but the same _spelling_ in different methods. `CallSource` compares equal on
purpose (a guard about a call has to match the call it guards), which is what makes
`$this->getBuilder()` in method 12 the same path as in method 400.

By shape: bare roots dominate -- a class named outright 7 207, `$this` 6 299, a local 6 295 -- then
one-step paths, `$this->m()` 3 425 and `$x->m()` 4 120. A bare local is position-dependent by
definition. A bare `$this` is not, except through `narrowed_this`, and a profile of the held file
puts that subtree near a tenth of the time: `narrowing_at` is the largest self time, and its callers
split `local_binding` ~850 samples, `slot_value` ~730, `narrowed_this` ~560. A prefilter ("does this
body contain anything that can narrow `$this`") is the `NeverReturns` shape and would buy about that
tenth -- not the 86 -> 30 ms the budget needs. The rest is the per-position walk the inventory above
describes, and closing it is still the fact-keyed memo, now with `coverage --dump` to prove it
changes no answer.

## The `narrowed_this` prefilter was built, measured, and bought nothing

The section above priced a prefilter for `narrowed_this` at about a tenth of the worst Laravel file,
from a profile whose `narrowing_at` callers split `local_binding` ~850 samples, `slot_value` ~730,
`narrowed_this` ~560. It was built on 2026-09-26: a per-body flag in `BodyFacts` -- does any guard
condition mention `$this`, is `$this` passed to a call, does an `assert()`, a `match` or an
`instanceof` name it -- and an early `None` in `narrowed_this` when no body on the scope chain holds
one. It was exact: `coverage --dump` and `check` identical on five projects, answer for answer.

And it skipped **10 066 of 10 066** walks on `DatabaseQueryBuilderTest.php` for no change at all:
`perf`'s median-of-seven slowest-file figure read 105.8 / 109.6 ms before and 106.3 / 104.6 ms
after, inside the noise. The walks were cheap -- a test method has a handful of statements before
any read, so each one was a few comparisons -- and the profile had charged `narrowed_this` for time
spent in the shared `narrowing_at` machinery that the other two callers also pay. Not committed.

Two things to keep. **A sample count is not a cost you can remove**: the subtree a profiler charges
to a caller includes work the fix does not touch, and the only price is the one measured after
building. And the remaining corpus gap is not a prefilter's to take -- nothing cheap is left in
front of the per-position walk, which is the conclusion the inventory above already reached.

## One receiver asked from two places was two resolutions

What the prefilter above did not find, reading which checks each pay: `UndefinedMember` cost 24-26
ms on the worst Laravel file even though it runs after the call checks that had already resolved the
same receivers. They had -- from `call.range.start()`, which for a chain
`$builder->select('*')->from('users')` is where the _chain_ begins, while the member-access reader
asks from the member name. The memo is keyed by position, so every prefix of every chain was
resolved twice. The receiver is evaluated whole before its name, so the two points see one value;
`CallSite::receiver_at()` is the member name for a method call and every call reader goes through it
now. Answers identical on five projects (`coverage --dump`, `check`), and measured:
`UndefinedMember` 24-26 -> 9 ms, the file 113-120 -> 104-106 ms, the reference project's slowest
file 13.9 -> 13.1 ms and its diagnostics p95 12.7 -> 12.0 ms.

The shape is worth naming for the next one: **a memo keyed by position is only shared by readers
that agree on the position**, and two readers of one expression each picking a reasonable offset is
enough to halve it without anything failing.

## A word a class's own declaration writes is in every file of its tree

`callHierarchy/incomingCalls` on `PHPUnit\Framework\TestCase::__construct` took **7.2 s** on Symfony
to find one call, and every constructor in the project cost 180-240 ms however few callers it had.
The sweep grepped for `__construct`, for the class, and for every subclass that inherits the
constructor -- correct, since `new Child()` runs it and writes neither word -- and two of those
words are in files that construct nothing: a test's own `class FooTest extends TestCase` holds two
of the names and its `function __construct` the third. 5 713 files were analysed.

The fix is a reading of the text rather than a smaller list: a name counts where it can construct,
and `class Child`, `extends Child`, `use Child` and `function __construct` count only alongside the
thing that would make them construct (`new self`, `new class`, `new parent`, a class the file names
before `::__construct`). The rules live with the reader, `lsp/src/workspace/candidates.rs`, and the
query that feeds it is data, `SweepQuery`, since the analysis crate does not scan text. 407 files
instead of 5 713; TestCase **7232 -> 170 ms**, Exception 1515 -> 471 ms, a project's own constructor
~240 -> ~60 ms; summed over fifteen subjects on Symfony 12.3 -> 2.0 s and over seven on Laravel 3.4
-> 0.7 s, with every answer identical. References and parameter renames at 24 constructors on
Symfony and 36 on the reference project were byte-identical too.

On Laravel one answer was not, and that one was the old bug:
`new class extends AbstractPaginator {}` was listed as calling `Collection::__construct`, because
`AbstractPaginator` is `@mixin Collection` and the member table took the mixin's constructor in. The
filter dropped the file only because the walk that lists subclasses does not follow a mixin. The
same table entry was what every `new Pager()` was checked against, so a class with a `@mixin` and no
constructor of its own drew `missing-argument`, `incompatible-argument` and `extra-argument` on
lines PHP runs without a word; `merge_mixins` leaves `__construct` out now. **When a narrower
prefilter changes an answer, decide which side was right before deciding the filter is wrong.**

## The classes a trait is composed into, held rather than walked

Reading `$this` in a trait as its hosts first found them by walking the class table, once per level
of composition, memoized for one request. On the reference Drupal checkout that took hover inside a
trait from **0.08 ms to 10.6 ms** median and 23.5 ms p95 -- past the 12 ms budget, every request,
with every corpus answer correct. The walk is a pass over 31 000 files' classes for an answer that
is a handful of them. `ProjectSkeleton` keeps `trait_users` now -- a trait's folded name to the
files composing it -- written by `install` and `retract` like `dispatches`, carried by `adopt`, and
shown in the dump so `incremental == from-scratch` holds it (a test that leaves a withdrawal out
fails there). Hover in the same traits: **0.10 ms** median, 0.42 p95. Completion rose from 0.09 to
1.5 ms median (4.0 p95) because it now lists the host's members, which is the answer being bigger
rather than the lookup being slower. Answers identical to the walk on all six corpora.

## A model's property list asked what every method returns, and Laravel asks only the hint

`phrost perf` on Monica read completion at **16.9-17.2 ms p95 against a 12 ms budget**, on the build
before any of this and the one after -- over budget for as long as nobody swept that project. The
cost was flat at 17-19 ms per request across a 1 217-line file, on receivers as plain as
`$this->author->`, while hover and goto at the same sites read 0.2 ms: a whole-class question asked
per request. `--hold-on completion`, added for this, loops one request at the slowest completion
site the sweep found, and `sample` on it put 90 % of the time in `undeclared_properties` -- the
model's magic properties, listed by asking every method whether it is an accessor or a relation,
each by resolving what the method returns, bodies and all.

The accessor half asked a question Laravel does not. `hasAttributeMutator` reads reflection's return
type: a single named type, exactly `Attribute`. A docblock saying so, a body handing one back or a
union holding one make no accessor at runtime, so reading the native hint is the runtime's rule and
costs a table lookup -- 63 % of the request. The relation half genuinely needs the return, since
`getRelationshipFromMethod` calls the method and checks what comes out; but a native hint PHP
enforces settles it whenever it rules a relation out -- `: string`, `: void`, `: array`, `: static`,
a class that is no `Relation` -- and the rule's class was being re-climbed twice per method where
once per class answers it.

17.2 -> 4.8 ms p95, worst site 20 -> 4.9 ms. Every member access's answer byte-identical on Monica,
BookStack, Laravel.io and Laravel's own source, and every one of 3 693 completion lists identical on
the first three -- the change is to how an answer is reached, and the one place the rule narrowed
(an `Attribute` a docblock alone states) is pinned by a test. Left: what is still resolved per
request is the return of each unhinted method, and `ProjectSkeleton::rank` re-sorting the declarers
of a model that `_ide_helper_models.php` declares a second time on every `class_header` -- about 4 %
of a diagnostics run, measured and not worth a cache yet.

## Every literal was one value, so a file of `define()` calls was quadratic

`perf` over this repository itself -- whose "user code" is the two copies of the stub set -- failed
on 2026-11-10 with one file far out: **145 ms** of diagnostics for `php-prelude-upstream/swoole.php`
on the Mac, 308-415 ms on the workspace VM, 97 % of it `IncompatibleArgument`. Not a regression: the
binary from before that day's commits measured the same.

The file ends in 393 lines of `define('SWOOLE_X', 1);`. Bisected with generated files, since nothing
on that machine profiles: 400 calls of `define('C', 1)` took 135 ms and 800 took 485; 400 calls
passing a _string_ literal took 15. The stage was `resolve_path` on the argument, inside it
`held_after_steps`, and inside that `contract_narrowed` -- the reader of what a statement call
asserted about a value it was handed.

`PassedPaths` is keyed by the value's path, and a literal's path is `PathRoot::Builtin` of the type
it produces. So every integer written in a file was one key, every call that took any of them was
filed under it, and resolving the next literal read all the calls before it. §8.1's _two spellings
collapsing into one_, in the index built to remove the quadratic the note above records: a string
literal escaped only because its root carries the expression. `ValuePath::names_a_value` says which
paths are worth recording -- anything but a bare builtin -- and the index leaves the rest out.

Measured: 800 `define()` calls 461 -> 30 ms, `swoole.php` alone 180 -> 44 ms, the sweep's worst
diagnostics 308-415 -> 12.4-12.8 ms. No answer moved: every finding on Symfony, WordPress, Drupal
and Laravel is the same line (124 027 of them), and `coverage --dump` is byte-identical on Laravel
(243 550 lines) and WordPress (110 826). Serial `check` is inside the noise.

What it left was the same walk asked about a _named_ value: a local handed to four hundred statement
calls in one body was still read four hundred times over, 100 ms on the same generated shape. The
note after the next one is that.

## An import was offered out of the symbol picker, once per unresolved name

The same sweep's two code-action rows stayed over budget after the note above: on the workspace VM
p95 88-120 ms for a whole file and 55-123 ms at a cursor (29 and 17 ms on the Mac), all on
`swoole.php`. Timing each offer named `source.fixAll`, 27 ms at every cursor, with `ImportClass`
under it.

The first reading was wrong and is worth keeping for that. `actions/subject.rs` says in its own
header that every sort of subject is answered by walking the whole file, and the fix-all asks about
the whole file for every cursor -- so the cost was read as `name_references` over 3 317 lines, and
written down as that. A three-way cut of the file refuted it in one run: without its 28
`class_alias(X::class, bare_name::class)` lines the same file answers in 4 ms, with them in 28. A
cause that fits a comment already in the code is still a guess until something is taken out.

What cost was `search_symbols`, asked once per unresolved bare name and then filtered to the classes
of exactly that name: the picker's search, with its loose pass and its walk over every member the
project declares, about 0.85 ms a time on that root and 4 ms on Laravel. §8.1's _catch-all answering
a question its inputs could answer better_: the catalog is ordered by short name, so the classes of
one name are one range of it. `IndexView::class_likes_named` reads that range, ranks and locates the
way a search hit is, and is what the import offer asks now -- the same candidates in the same order,
and complete where the search's cap of 200 could cut a class off behind unrelated hits.

Measured on the VM. This repository: whole file p95 88-120 -> 11.1-11.3 ms, cursor 55-123 -> 4.7 ms,
and the sweep passes. Laravel's own source, cloned without its `vendor/` -- so every class a test
takes from PHPUnit or Mockery is one nothing declares, 388 of them in the largest file, which is
what a project looks like before `composer install`: whole file p95 **1 745 -> 62 ms**, cursor p95
**1 489 -> 18.6 ms** (p50 116 -> 7.6). Its other rows did not move; the four it fails besides are
that corpus on that machine, as the 2026-09-25 note records. `search_symbols` itself is unchanged,
4.7-4.9 ms a query on Laravel from either binary.

What it still is: the fix-all asks about the whole file for every cursor, so a cursor's cost keeps a
term in how many fixable problems the file has. That term is cheap now and is not gone.

## What a callee says about a value is the statement's, and asking it of the whole file overflowed the stack

The named half of the literal note above. `m($v);` four hundred times in one body, where `m`
declares nothing about what it takes, cost 100 ms: every read of `$v` was handed every call before
it, and `contract_narrowed` looked each callee up to learn that it has no by-reference parameter, no
`@assert` and no contract -- a function's declaration is not memoized at all, so that was a name
resolution and two clones per call per read.

Whether a callee has anything to say about a path it was handed is §8.1's _property of the tree
recomputed per position_: `CalledDeclaration::speaks_of` is the superset of what the three readers
accept, and the answer is kept per statement, path and budget. The walk asks it before reading
anything else off a statement, so a call that says nothing costs a lookup.

**The first version was wrong in a way no test saw.** It settled the question once per _path_, for
every statement the index holds under it, and that looked like the better shape: 800 calls in 39 ms.
All 2 490 tests of the three crates passed. Run over the corpora it aborted on Symfony, Drupal and
Laravel with a stack overflow, and only WordPress finished. A method's callee is found by resolving
its receiver _where the call is written_; asked of a statement at or after the read -- which the
index holds, and the old walk never reached -- that resolution arrives back at the same value one
budget lower, and nothing ends it but the budget running out, twenty-four levels of the deepest
frames in the crate. Before the position the recursion runs backwards through the file, which is why
it was always safe. So the rule is in the walk's signature now: `speaks` is asked only of a
statement that precedes the position. An answer that is per statement is not thereby safe to compute
for every statement.

And it is the finding-list A/B that caught it, not the suite. The comparison was there to show that
no answer moved; what it showed first was three empty files.

Measured with the version that is in: 400 calls 98-104 -> 29 ms, 800 calls 335-350 -> 53 ms -- a
lookup per earlier call per read is still a quadratic, at a fifteenth of the constant. A body whose
every call _does_ speak, four hundred `@assert`s about one variable, is unchanged at 102-108 ms and
is the walk doing what it is for. No answer moved: the 124 027 findings on four corpora are the same
lines and `coverage --dump` is byte-identical on Laravel and WordPress. Serial `check` is inside the
noise on both.
