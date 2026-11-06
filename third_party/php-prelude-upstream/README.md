# PHP prelude stubs, as vendored

`mago-prelude` **1.58.2** exactly as it is published, and the answer to one question:
*what did this project change?*

Nothing here is edited, ever. The set beside it, `third_party/php-prelude`, carries around ten
thousand changed lines across 28 files — 2 144 prose summaries, 138 examples, some sixty
corrections to types and tags, and one upstream improvement deliberately declined — and until this
directory existed that delta lived only as prose in that set's README, written from memory and
already incomplete: four of its seven recorded categories had nothing that would fail if a re-vendor
reverted them.

```
just stubs-diff     the whole local delta, file by file
just stubs-accept   re-record it after a deliberate edit
```

`a_re_vendor_that_reverts_a_local_edit_fails_here` holds the recorded shape of the delta, so an edit
that disappears fails the suite and names the file it went missing from. It is structural and
deliberately dumb; the tripwires beside it (`the_added_type_tags_survive`,
`the_documented_names_still_carry_their_prose`, `no_declaration_carries_two_docblocks`) are
semantic, and say that a particular tag is still attached to a particular declaration. Both are
worth having: the first covers everything including the edits nobody wrote a test for, the second
says *which* edit and *where*.

## Getting it

The crate ships the stubs as assets, so no clone is needed:

```
curl -sSLO https://static.crates.io/crates/mago-prelude/mago-prelude-<version>.crate
tar xzf mago-prelude-<version>.crate
```

`assets/mago.php` and `assets/extensions/*.php` are flattened into one directory here, which is the
single structural difference from upstream and the reason the diff beside it is file-to-file. 139
files here, and 138 beside it: `swoole.php` is the one this project leaves out.

## Re-vendoring

The point of this directory. The order that works:

1. Fetch the new version and flatten it over this directory.
2. `just stubs-diff` now shows the local delta *and* the upstream step mixed together — so take the
   upstream step on its own first (`diff` the old pristine against the new one) and read it. It is
   usually small: 1.48.0 to 1.50.0 was **17 lines across two files**, and one of them removed a
   template from `array_push`, which is a narrowing that can put new findings on a corpus. Read a
   *new file* as closely as the diff: 1.52.0's whole hazard was in `swoole.php`, which declares two
   global functions a framework declares too, and a diff of the old pristine against the new one
   shows a new file only as "Only in".
3. Re-apply the local delta, resolving where upstream has since fixed something we fixed — and
   delete our version where it has, recording that in the other README.
4. `just stubs-accept`, then measure: `phrost check third_party/php-prelude` for the set's own
   defects, and the ratchet over all five corpora for what moved in real code.
