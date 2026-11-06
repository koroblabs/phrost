//! What a name holds after an `if`/`elseif` chain whose arms leave or write again.
//!
//! An `elseif` is an `if` inside the outer one's `else`, and three rules had each stopped at that
//! nesting. A closed `if` was never descended into, so the leaving guard of an `elseif` -- the
//! whole of what says `$b` is there after `if ($c) { return; } elseif (!$b = find()) { return; }`
//! -- was never read. A write in a condition joined beside the write that replaces it on the one
//! path out. And an arm that is nothing but another `if` never counted as filled, so the path
//! around every write was kept although no such path exists. Symfony's
//! `ResolveAutowireInlineAttributesPass` is all three at once: `elseif (!$attribute =
//! $parameter->getAttributes(X::class)[0] ?? null) { continue; } else { $attribute =
//! $attribute->newInstance(); }`.

use expect_test::expect;

use super::check;

#[test]
fn the_leaving_guard_of_an_elseif_holds_after_the_chain() {
    check(
        r#"<?php
class Repo
{
    public function run(bool $c): void
    {
        if ($c) {
            return;
        } elseif (!$b = $this->find()) {
            return;
        }
        $$0b;
    }
}
"#,
        "b",
        expect!["not null $this->find()"],
    );
}

#[test]
fn a_write_in_a_condition_is_gone_where_every_arm_leaves_or_writes_again() {
    check(
        r#"<?php
class Repo
{
    public function run(array $args, bool $c): void
    {
        foreach ($args as $arg) {
            if ($c) {
                $a = $this->given();
            } elseif (!$a = $this->found()) {
                continue;
            } else {
                $a = $a->made();
            }
            $$0a;
        }
    }
}
"#,
        "a",
        expect!["$this->given() or $a->made()"],
    );
}

/// The control: an arm that neither leaves nor writes keeps the condition's write, because that arm
/// is a way past with the value it wrote.
#[test]
fn a_write_in_a_condition_stays_where_an_arm_falls_through() {
    check(
        r#"<?php
class Repo
{
    public function run(bool $c): void
    {
        if ($c) {
            $a = $this->given();
        } elseif (!$a = $this->found()) {
            $this->log();
        } else {
            $a = $a->made();
        }
        $$0a;
    }
}
"#,
        "a",
        expect!["$this->given() or $this->found() or $a->made()"],
    );
}

/// The path that skipped a write comes out of the other arm, and what that arm proved on its way out
/// is the claim where the two paths meet -- not the complement of the write's condition, which of a
/// `mixed` says only "not an int". The reference project's `LoggerCore::assertSeverityLevel()`.
#[test]
fn the_other_arms_exit_is_what_the_path_around_a_write_proved() {
    check(
        r#"<?php
class Logger
{
    public function run(mixed $level): void
    {
        if (is_int($level)) {
            $level = Level::from($level);
        } elseif (!$level instanceof Level) {
            throw new \InvalidArgumentException();
        }
        $$0level;
    }
}
"#,
        "level",
        expect!["Level->from() or $level before as Level"],
    );
}

/// The control: an other arm that proves nothing on its way out leaves the complement standing.
#[test]
fn an_other_arm_that_proves_nothing_leaves_the_complement() {
    check(
        r#"<?php
class Logger
{
    public function run(mixed $level): void
    {
        if (is_int($level)) {
            $level = Level::from($level);
        } else {
            $this->log();
        }
        $$0level;
    }
}
"#,
        "level",
        expect!["Level->from() or $level before minus int"],
    );
}
