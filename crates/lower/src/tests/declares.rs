//! `declare(...)` at file scope.
//!
//! Two separate facts come out of one statement: whether the file is strict, and whatever a
//! block-bodied `declare` wraps. They are tested together because the same lowering arm produces
//! both, and dropping the body was the bug that hid the second one.

use expect_test::expect;

use super::check_hir;

#[test]
fn strict_types_is_recorded() {
    check_hir(
        r#"
//- /src/Strict.php
<?php
declare(strict_types=1);

class Payment {}
"#,
        expect![[r#"
            declare(strict_types=1)
            class Payment @ 4:7-4:14
        "#]],
    );
}

#[test]
fn a_file_without_the_directive_is_not_strict() {
    check_hir(
        r#"
//- /src/Lenient.php
<?php

class Payment {}
"#,
        expect![[r#"
            class Payment @ 3:7-3:14
        "#]],
    );
}

#[test]
fn strict_types_zero_is_not_strict() {
    // The explicit off switch. `=0` is what the engine already assumes, and reading it as "the
    // directive is present, therefore strict" would make every lenient file that spells its
    // choice out report the opposite of what it says.
    check_hir(
        r#"
//- /src/Off.php
<?php
declare(strict_types=0);

class Payment {}
"#,
        expect![[r#"
            class Payment @ 4:7-4:14
        "#]],
    );
}

#[test]
fn an_unrelated_directive_leaves_the_file_lenient() {
    check_hir(
        r#"
//- /src/Ticks.php
<?php
declare(ticks=1);

class Payment {}
"#,
        expect![[r#"
            class Payment @ 4:7-4:14
        "#]],
    );
}

#[test]
fn the_directive_name_is_case_insensitive() {
    // PHP compares the directive name case-insensitively, so a file spelling it in capitals is
    // strict and has to be checked as one.
    check_hir(
        r#"
//- /src/Shouty.php
<?php
declare(STRICT_TYPES=1);

class Payment {}
"#,
        expect![[r#"
            declare(strict_types=1)
            class Payment @ 4:7-4:14
        "#]],
    );
}

#[test]
fn a_block_bodied_declare_still_declares_its_contents() {
    // The reason the body is walked at all. `declare(ticks=1) { class Foo {} }` declares `Foo` as
    // visibly as the top level does; before the body was walked the class simply vanished.
    check_hir(
        r#"
//- /src/Block.php
<?php
declare(ticks=1) {
    class Payment {}
}
"#,
        expect![[r#"
            class Payment @ 3:11-3:18
        "#]],
    );
}

#[test]
fn several_directives_in_one_statement() {
    check_hir(
        r#"
//- /src/Both.php
<?php
declare(ticks=1, strict_types=1);

class Payment {}
"#,
        expect![[r#"
            declare(strict_types=1)
            class Payment @ 4:7-4:14
        "#]],
    );
}
