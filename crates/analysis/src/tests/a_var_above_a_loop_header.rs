//! A `/** @var T $x */` above a `while` types the assignment in its condition.
//!
//! `while ($item = $queue->claimItem())` is how a queue is drained, and `claimItem()` is declared
//! `object|bool` -- so the body saw an `object|bool` narrowed by truthiness to nothing narrower, and
//! every use of `$item` as an object was reported. The `@var` above the loop is the author's remedy,
//! written for the same reason as the one above an `if` with an assignment in its condition, and it
//! attached to nothing: the annotation was recorded before the statement and the header's assignment,
//! written later, outranked it. It is the assignment's now, as it is for an `if`.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_annotation_types_what_the_header_assigns() {
    check(
        r#"
//- /app.php
<?php

class Exception {}

class Queue
{
    /** @return object|bool */
    public function claimItem(): object|bool { return FALSE; }

    public function deleteItem(object $item): void {}
}

class Runner
{
    public function drainWithoutHelp(Queue $queue): void
    {
        while ($item = $queue->claimItem()) {
            $queue->deleteItem($item);
        }
    }

    public function drain(Queue $queue): void
    {
        /** @var object|false $item */
        while ($item = $queue->claimItem()) {
            try {
                $queue->deleteItem($item);
            }
            catch (\Exception $e) {
                $queue->deleteItem($item);
            }
        }
    }

    public function drainBottomUp(Queue $queue): void
    {
        /** @var object|false $item */
        do {
            if ($item) {
                $queue->deleteItem($item);
            }
        } while ($item = $queue->claimItem());
    }
}
"#,
        expect![[r#"
            18:32-18:37 error [incompatible-argument] $item takes object, and this is object|true
        "#]],
    );
}
