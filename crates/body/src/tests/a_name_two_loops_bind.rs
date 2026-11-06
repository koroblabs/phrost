//! Two `foreach` loops in one function, both binding the same name.
//!
//! `$handler`, `$item`, `$row` -- a method that loops over two collections usually reuses the name,
//! and the binding was taken from the *first* loop found rather than the latest one at or before the
//! offset. So a method looping over `LeaveHandler[]` and then over `EnterHandler[]` gave both bodies
//! the first subject's element type, and every call in the second loop reported
//! `undefined-method LeaveHandler::onEnter()`.
//!
//! What made it read as a scoping rule rather than a bug is that renaming one of the two variables
//! makes it go away: the report says "a loop variable is bound once per name in a scope", which is
//! not how PHP works and was not what the code meant to do.
//!
//! Position and nothing else, which is the rule `local_binding` already applies across its four
//! kinds of binding -- and it gets a nested loop right without a case of its own, since an inner
//! `foreach` is written after the one containing it.

use expect_test::expect;

use crate::tests::check;

#[test]
fn the_later_loop_binds_the_name_inside_its_own_body() {
    check(
        r#"
<?php
class Registry {
    /** @var LeaveHandler[] */
    private array $leave = [];

    /** @var EnterHandler[] */
    private array $enter = [];

    public function run(): void {
        foreach ($this->leave as $handler) {
            $handler->onLeave();
        }
        foreach ($this->enter as $hand$0ler) {
            $handler->onEnter();
        }
    }
}
"#,
        "handler",
        expect!["$this->$enter[element]"],
    );
}

#[test]
fn the_earlier_loop_still_binds_it_inside_its_own_body() {
    // The other direction, and the one a fix by "take the last" alone would break: the loop written
    // *after* the offset must not reach backwards into the one the offset is in.
    check(
        r#"
<?php
class Registry {
    /** @var LeaveHandler[] */
    private array $leave = [];

    /** @var EnterHandler[] */
    private array $enter = [];

    public function run(): void {
        foreach ($this->leave as $handler) {
            $hand$0ler->onLeave();
        }
        foreach ($this->enter as $handler) {
            $handler->onEnter();
        }
    }
}
"#,
        "handler",
        expect!["$this->$leave[element]"],
    );
}

#[test]
fn a_nested_loop_binds_it_over_the_one_around_it() {
    check(
        r#"
<?php
class Registry {
    /** @var LeaveHandler[] */
    private array $leave = [];

    /** @var EnterHandler[] */
    private array $enter = [];

    public function run(): void {
        foreach ($this->leave as $handler) {
            foreach ($this->enter as $handler) {
                $hand$0ler->onEnter();
            }
        }
    }
}
"#,
        "handler",
        expect!["$this->$enter[element]"],
    );
}

#[test]
fn a_later_loop_whose_subject_says_nothing_does_not_fall_back_to_an_earlier_one() {
    // The load-bearing half. A loop binds its name whether or not anything can say what the subject
    // holds, so the answer here has to be *nothing* -- answering the earlier loop's element would
    // name a type the variable does not have, which is how a wrong answer gets reported as an error.
    check(
        r#"
<?php
class Registry {
    /** @var LeaveHandler[] */
    private array $leave = [];

    public function run($mystery): void {
        foreach ($this->leave as $handler) {
            $handler->onLeave();
        }
        foreach ($mystery->whatever() as $handler) {
            $hand$0ler->onEnter();
        }
    }
}
"#,
        "handler",
        expect!["$mystery->whatever()[element]"],
    );
}
