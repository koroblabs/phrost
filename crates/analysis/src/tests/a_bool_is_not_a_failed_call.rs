//! A value declared plainly `bool` is a boolean, not a call nobody checked.
//!
//! The `false` sentinel is about a function that hands back `false` to mean *not there* --
//! `strstr`, a finder, a docblock's `object|false`. A parameter declared `bool` is not that, and
//! passing one where an `int` or a `string` is expected is ordinary weak-mode PHP: `takesInt($flag)`
//! in a file without `strict_types` coerces, which PHP 8.5.10 was asked along with the whole
//! coercion table.
//!
//! The same word means the sentinel in two other places, and both have to keep it: a **union**
//! operand -- `object|bool` is how a docblock spells `object|false` -- and the **expectation** side,
//! where a `bool` parameter does accept a `false`.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_bool_passed_where_a_scalar_is_expected_is_weak_mode_php() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Takes
{
    public function anInt(int $x): void {}

    public function aString(string $x): void {}

    public function run(bool $flag): void
    {
        $this->anInt($flag);
        $this->aString($flag);
    }
}
"#,
        expect![""],
    );
}

/// With `strict_types` the same two calls are `TypeError`s, and the message names the kind rather
/// than a sentinel nobody wrote.
#[test]
fn under_strict_types_the_same_calls_are_errors() {
    check(
        r#"
//- /src/app.php
<?php

declare(strict_types=1);

namespace App;

class Takes
{
    public function anInt(int $x): void {}

    public function run(bool $flag): void
    {
        $this->anInt($flag);
    }
}
"#,
        expect![[r#"
            13:22-13:27 error [incompatible-argument] $x takes int, and this is bool
        "#]],
    );
}

/// And the two places `bool` still means `false`: an operand of a union, and a parameter being
/// asked whether it admits one.
#[test]
fn a_union_operand_and_an_accepting_parameter_still_read_it_as_false() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    /** @return object|bool */
    public function find() {}

    public function takeObject(object $it): void {}

    public function takeBool(bool $it): void {}

    public function run(): void
    {
        $this->takeObject($this->find());
        $this->takeBool($this->find());
    }
}
"#,
        expect![[r#"
            15:27-15:40 warning [possibly-false-argument] $it takes object, and this may be false
        "#]],
    );
}
