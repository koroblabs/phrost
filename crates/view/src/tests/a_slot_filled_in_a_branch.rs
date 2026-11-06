//! A property written in a branch the position is not in, beside what it held before.
//!
//! `if ($this->cache === NULL) { $this->cache = $this->build(); } return $this->cache;` is the
//! commonest way a property is filled in PHP, and it is exactly a local's shape with `$this->cache`
//! for `$v` -- so it gets the local's answer: the write joined with what the slot held before the
//! branch, minus what the guard that let control in ruled out. Held as two constructors, the slot
//! half *withdrew* its claim wherever the local half joined, and every lazily filled property read
//! as the `?T` its declaration says -- which the nullable-return check then reported, twelve times
//! on the reference project, on code that cannot return null.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_property_filled_under_a_null_guard_is_not_null_after_it() {
    check_view(
        r#"
//- /src/app.php
<?php
class Registry
{
    private ?array $definitions = NULL;

    public function all(): array
    {
        if ($this->definitions === NULL) {
            $this->definitions = $this->build();
        }
        $d = $this->definitions;
        $@here@d;
    }

    private function build(): array { return []; }
}
"#,
        &[Query::variable_type("after the guarded fill", "here")],
        expect![[r#"
            after the guarded fill
            - array
        "#]],
    );
}

#[test]
fn a_static_property_is_a_slot_too() {
    check_view(
        r#"
//- /src/app.php
<?php
class Rule {}
class Config
{
    private static ?Rule $cache = NULL;

    public static function rule(): Rule
    {
        if (self::$cache === NULL) {
            self::$cache = new Rule();
        }
        $r = self::$cache;
        $@here@r;
    }
}
"#,
        &[Query::variable_type("after the guarded fill", "here")],
        expect![[r#"
            after the guarded fill
            - Rule
        "#]],
    );
}

#[test]
fn a_fill_under_an_unrelated_guard_keeps_the_null() {
    // The control: the branch may not have run and nothing about the slot decided that, so the
    // declaration is the answer -- null included. Deliberately the declaration rather than a join
    // of it with the write: the two say the same thing, and the join of a class with its nullable
    // collapses to the class, which would lose the null.
    check_view(
        r#"
//- /src/app.php
<?php
class Registry
{
    private ?array $definitions = NULL;

    public function all(bool $eager): array
    {
        if ($eager) {
            $this->definitions = $this->build();
        }
        $d = $this->definitions;
        $@here@d;
    }

    private function build(): array { return []; }
}
"#,
        &[Query::variable_type("after the conditional fill", "here")],
        expect![[r#"
            after the conditional fill
            - ?array
        "#]],
    );
}

#[test]
fn a_write_through_the_receiver_after_the_fill_withdraws_the_claim() {
    check_view(
        r#"
//- /src/app.php
<?php
class Holder
{
    public ?array $rows = NULL;
}
class Reader
{
    public function run(Holder $h): void
    {
        if ($h->rows === NULL) {
            $h->rows = [1];
        }
        $h = new Holder();
        $d = $h->rows;
        $@here@d;
    }
}
"#,
        &[Query::variable_type(
            "after the receiver was replaced",
            "here",
        )],
        expect![[r#"
            after the receiver was replaced
            - ?array
        "#]],
    );
}

/// A guard above the construct, and a fill under a guard that says nothing about the slot. The path
/// that skipped the write still has the guard above it in effect, so the join is the write beside a
/// value that is not null -- where withdrawing read the declaration, and its null, back.
#[test]
fn a_guard_above_an_unrelated_fill_still_holds_on_the_path_that_skipped_it() {
    check_view(
        r#"
//- /src/app.php
<?php
class Sleep
{
    private int|float|null $pending = NULL;

    public function pull(): void
    {
        if ($this->pending === NULL) {
            throw new \RuntimeException('No duration specified.');
        }
        if ($this->pending < 0) {
            $this->pending = 0;
        }
        $d = $this->pending;
        $@here@d;
    }
}
"#,
        &[Query::variable_type("past the guard and the fill", "here")],
        expect![[r#"
            past the guard and the fill
            - int|float
        "#]],
    );
}

/// The guard is about the value *before* a write that may have run since: `isset` fails, the method
/// leaves, and a branch then writes the slot. What the guard proved may no longer be there, so it
/// does not answer past the write -- WordPress's `WP_Theme::get()` read the null that a failed `isset`
/// proves and reported returning it on every path.
///
/// The element reads what was last written into it, which is `element_written`'s rule for a slot
/// filled in a branch; the property gets the join.
#[test]
fn a_guard_a_later_write_may_have_replaced_does_not_answer() {
    check_view(
        r#"
//- /src/app.php
<?php
class Theme
{
    private ?string $title = NULL;

    /** @var array<string, string> */
    private array $headers = [];

    public function title(bool $cached): void
    {
        if (isset($this->title)) {
            return;
        }
        if ($cached) {
            $this->other();
        } else {
            $this->title = 'Untitled';
        }
        $d = $this->title;
        $@property@d;
    }

    public function header(string $name, bool $all): void
    {
        if (isset($this->headers[$name])) {
            return;
        }
        if ($all) {
            $this->headers['other'] = 'x';
        } else {
            $this->headers[$name] = 'y';
        }
        $e = $this->headers[$name];
        $@element@e;
    }

    private function other(): void {}
}
"#,
        &[
            Query::variable_type("a property written in one arm", "property"),
            Query::variable_type("an element written in one arm", "element"),
        ],
        expect![[r#"
            a property written in one arm
            - ?non-falsy-string
            an element written in one arm
            - string
        "#]],
    );
}
