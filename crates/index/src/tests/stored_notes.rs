//! Every stored fact the dump prints beside a signature.
//!
//! Four fields of a member or a function row are carried by the codec, derived at lowering, and
//! say nothing that `MemberRow::signature` prints -- so until this rendering existed, the two
//! invariants that compare *the dump* could not see any of them: `incremental == from-scratch`
//! (`edit_fuzz`, `incremental`) and `cold == warm` (`cache`, whose dump half goes through `adopt`).
//! The byte half of the cache test covered the codec and nothing else, so a field derived
//! differently on a re-ingest, or dropped by a future `adopt` or compaction path, was invisible.
//! That is the exact shape §8 records for `ProjectSkeleton::claim` against `adopt`, which lost one
//! of three kinds of catalog claim and was found only because somebody looked.
//!
//! This test is what makes the rendering itself checkable: a note the dump stops printing is a
//! field those invariants stop seeing, and nothing else would fail.
//!
//! `docblock_range` is deliberately absent -- see the comment in `dump.rs` for why the one field
//! that would add a note to every documented member is left out.

use expect_test::expect;

use super::check_index;

#[test]
fn the_dump_prints_what_a_signature_does_not() {
    check_index(
        r#"
//- /src/Facts.php
<?php

namespace App;

class Thing {}

class Facts
{
    /** A constant whose type is read off its value rather than written. */
    public const AUTO = 3;

    /**
     * @param array $matches
     * @param-out array<int, string> $matches
     * @param callable(Thing, int): void $each
     */
    public function captured(string $pattern, array &$matches, callable $each): bool
    {
        $rest = func_get_args();

        return $rest !== [];
    }

    /** Nothing stored beyond the signature, so nothing beside it. */
    public function plain(int $id): void {}
}

function loose(int $count): int
{
    return $count + func_num_args();
}
"#,
        expect![[r#"
            state ready
            file src/Facts.php [User]
              class App\Thing @ 5:7-5:12
              class App\Facts @ 7:7-7:12
              function App\loose @ 28:10-28:15
              function App\loose(int $count): int  reads own arguments
              App\Thing is class
              App\Facts is class
              App\Facts::const AUTO  public type from value  @ 10:18-10:22 (full 10:5-10:27)
              App\Facts::method captured(string $pattern, array &$matches, callable(...): void $each): bool  public reads own arguments $matches -> array<int, string> $each calls(\App\Thing, int)  @ 17:21-17:29 (full 17:5-22:6)
              App\Facts::method plain(int $id): void  public  @ 25:21-25:26 (full 25:5-25:44)
            catalog
              [class] app\facts -> src/Facts.php (App\Facts)
              [class] app\thing -> src/Facts.php (App\Thing)
              [function] app\loose -> src/Facts.php (App\loose)
            namespaces
              app -> App x3
            declared keys
            dispatched keys
        "#]],
    );
}
