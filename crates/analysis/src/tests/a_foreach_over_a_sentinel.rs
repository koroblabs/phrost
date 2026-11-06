//! `foreach` over a value that may not be there.
//!
//! The third rule about reading a container, after "does it hold elements" and "is it there". PHP
//! does not stop on any of them: `foreach() argument must be of type array|object, null given` is a
//! warning and the body simply never runs, so every code here is a warning.
//!
//! The subject matters as much as the type. A name the file ever fills a slot of -- `$out = null;
//! $out[] = $pattern;` -- holds an array by the time the loop runs whatever its assignment said, and
//! judging it by the assignment reported code that has shipped in WordPress for fifteen years.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_loop_over_a_value_that_may_be_null_or_false() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return string[]|null */
    public function maybe(): ?array
    {
        return null;
    }

    /** @return string[]|false */
    public function orFalse()
    {
        return false;
    }

    public function run(): void
    {
        foreach ($this->maybe() as $one) {
            echo $one;
        }
        $rows = $this->orFalse();
        foreach ($rows as $row) {
            echo $row;
        }
    }
}
"#,
        expect![[r#"
            20:18-20:32 warning [possibly-null-iteration] Cannot iterate ?string[]
            24:18-24:23 warning [possibly-false-iteration] Cannot iterate string[]|false
        "#]],
    );
}

#[test]
fn a_guard_and_a_name_the_file_builds_are_not_reported() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return string[]|null */
    public function maybe(): ?array
    {
        return null;
    }

    public function guarded(): void
    {
        $rows = $this->maybe();
        if ($rows === NULL) {
            return;
        }
        foreach ($rows as $row) {
            echo $row;
        }
    }

    public function built(): void
    {
        $out = NULL;
        $out[] = 'one';
        foreach ($out as $one) {
            echo $one;
        }
    }
}
"#,
        expect![[r#""#]],
    );
}
