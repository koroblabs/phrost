//! A `false` where the parameter cannot take one.
//!
//! The other half of the sentinel argument rule, and the half the check family was missing: null had
//! four codes and `false` had none, though `strpos()`, `reset()`, `ob_get_clean()` and their
//! neighbours hand back a `false` to mean *not there* exactly as the other half of the library hands
//! back null.
//!
//! What makes it a report rather than a `TypeError` is the thing worth stating: weak mode converts a
//! `false` to `""` for a `string` parameter and to `0` for an `int` one, with no error at all. That
//! silence is the reason to report it -- `base64_encode(false)` encodes the empty string -- and it is
//! why the compatibility judgement cannot answer here, since it asks what PHP rejects.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_value_that_may_be_false_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function take(string $name): void {}
    public function takeBool(bool $flag): void {}
    public function takeMixed(mixed $any): void {}

    /** @return string|false */
    public function maybe() { return 'x'; }
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->take($repo->maybe());
        $repo->take(FALSE);
        $repo->takeBool($repo->maybe());
        $repo->takeMixed($repo->maybe());
    }
}
"#,
        expect![[r#"
            16:21-16:35 warning [possibly-false-argument] $name takes string, and this may be false
            17:21-17:26 warning [false-argument] Cannot pass false as $name, which takes string
        "#]],
    );
}

/// The same guards the null half relies on, and the layer below is what applies them.
#[test]
fn a_guarded_value_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function take(string $name): void {}

    /** @return string|false */
    public function maybe() { return 'x'; }
}

class Caller
{
    public function run(Repo $repo): void
    {
        $name = $repo->maybe();
        if ($name !== FALSE) {
            $repo->take($name);
        }
        if ($repo->maybe()) {
            $repo->take($repo->maybe());
        }
        $repo->take($name);
    }
}
"#,
        // The unguarded read at the end is the control: without it an empty snapshot would pass
        // just as well with the whole check switched off.
        expect![[r#"
            21:21-21:26 warning [possibly-false-argument] $name takes string, and this may be false
        "#]],
    );
}

/// A value that may be both is one mistake, and the null report is the one that describes it.
#[test]
fn a_value_that_may_be_either_is_reported_once() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function take(string $name): void {}

    /** @return string|false|null */
    public function maybe() { return 'x'; }
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->take($repo->maybe());
    }
}
"#,
        expect![[r#"
            14:21-14:35 warning [possibly-null-argument] $name takes string, and this may be null
        "#]],
    );
}

/// A parameter with a default is still a parameter, and every argument after one was unchecked.
///
/// The pairing stopped at the first optional parameter -- and stopped there only here, since
/// `incompatible_arguments` pairs the same arguments against the same list and never has. So
/// `date('F jS', strtotime($when))` said nothing: `strtotime` gives `int|false` and `date`'s second
/// parameter is `?int $timestamp = null`. Measured against PHPStan on the reference checkout, 13 of
/// its `argument.type` findings are that pair, and phrost answered none of them.
#[test]
fn an_argument_past_an_optional_parameter_is_still_checked() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    /** @return string|false */
    public function maybe() { return 'x'; }

    public function optional(string $first, ?int $stamp = null, int $count = 0): void {}

    /** The pre-8.4 spelling, which PHP still makes implicitly nullable. */
    public function implicitlyNullable(int $stamp = null): void {}

    public function run(): void
    {
        $value = $this->maybe();
        $this->optional('a', $value);
        $this->optional('a', 1, $value);
        // Null is what the default says this takes, however the hint is written.
        $this->implicitlyNullable(null);
    }
}
"#,
        expect![[r#"
            15:30-15:36 warning [possibly-false-argument] $stamp takes ?int, and this may be false
            16:33-16:39 warning [possibly-false-argument] $count takes int, and this may be false
        "#]],
    );
}
