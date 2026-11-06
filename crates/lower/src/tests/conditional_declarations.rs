//! Declarations PHP reaches only when the code around them runs.
//!
//! One fixture per shape rather than one big one, because what matters is that *every* place a
//! statement can nest is covered: a shape left out is a name in no index, which reads as a typo to
//! the check that looks for one. The `[conditional]` marker in each snapshot is the fact ranking
//! reads -- see `precedence` in `phrost-index`, and `conditional.rs` beside this for why it is
//! recorded rather than the declaration simply being dropped.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_function_exists_guard_declares_the_function_it_guards() {
    check_hir(
        r#"
//- /src/polyfill.php
<?php
if (!function_exists('str_starts_with')) {
    function str_starts_with(?string $haystack, ?string $needle): bool {}
}
"#,
        expect![[r#"
            function str_starts_with(?string $haystack, ?string $needle): bool [conditional] @ 3:14-3:29
        "#]],
    );
}

#[test]
fn a_class_exists_guard_declares_the_class_it_guards() {
    check_hir(
        r#"
//- /src/Conditional.php
<?php
if (!class_exists('Legacy')) {
    class Legacy {}
}
"#,
        expect![[r#"
            class Legacy [conditional] @ 3:11-3:17
        "#]],
    );
}

#[test]
fn every_arm_of_an_if_declares_what_it_holds() {
    check_hir(
        r#"
//- /src/Arms.php
<?php
if (PHP_VERSION_ID >= 80000) {
    class Modern {}
} elseif (PHP_VERSION_ID >= 70400) {
    class Recent {}
} else {
    class Legacy {}
}

if (PHP_VERSION_ID >= 80000):
    class ModernAlt {}
elseif (PHP_VERSION_ID >= 70400):
    class RecentAlt {}
else:
    class LegacyAlt {}
endif;
"#,
        expect![[r#"
            class Modern [conditional] @ 3:11-3:17
            class Recent [conditional] @ 5:11-5:17
            class Legacy [conditional] @ 7:11-7:17
            class ModernAlt [conditional] @ 11:11-11:20
            class RecentAlt [conditional] @ 13:11-13:20
            class LegacyAlt [conditional] @ 15:11-15:20
        "#]],
    );
}

#[test]
fn a_switch_case_a_try_and_the_four_loops_all_declare() {
    check_hir(
        r#"
//- /src/Nested.php
<?php
switch (PHP_VERSION_ID) {
    case 80000:
        function in_a_case(): void {}
        break;
    default:
        function in_the_default(): void {}
}

try {
    function in_a_try(): void {}
} catch (\Throwable $error) {
    function in_a_catch(): void {}
} finally {
    function in_a_finally(): void {}
}

while (false) {
    function in_a_while(): void {}
}

do {
    function in_a_do_while(): void {}
} while (false);

for ($i = 0; $i < 1; $i++) {
    function in_a_for(): void {}
}

foreach ([] as $each) {
    function in_a_foreach(): void {}
}
"#,
        expect![[r#"
            function in_a_case(): void [conditional] @ 4:18-4:27
            function in_the_default(): void [conditional] @ 7:18-7:32
            function in_a_try(): void [conditional] @ 11:14-11:22
            function in_a_catch(): void [conditional] @ 13:14-13:24
            function in_a_finally(): void [conditional] @ 15:14-15:26
            function in_a_while(): void [conditional] @ 19:14-19:24
            function in_a_do_while(): void [conditional] @ 23:14-23:27
            function in_a_for(): void [conditional] @ 27:14-27:22
            function in_a_foreach(): void [conditional] @ 31:14-31:26
        "#]],
    );
}

#[test]
fn a_function_and_a_method_body_declare_what_they_hold() {
    // The other half of the same fact: PHP hoists neither, so `inner` and `from_a_method` exist only
    // once the code that wraps them has run. A body is not a statement, so neither is reached by the
    // statement walk -- see `Lowerer::body_declarations`.
    check_hir(
        r#"
//- /src/Bodies.php
<?php
function outer(): void
{
    function inner(): void {}
    class InnerClass {}
}

class Service
{
    public function make(): void
    {
        function from_a_method(): void {}
    }
}
"#,
        expect![[r#"
            class InnerClass [conditional] @ 5:11-5:21
            class Service @ 8:7-8:14
              method make(): void  public @ 10:21-10:25
            function outer(): void @ 2:10-2:15
            function inner(): void [conditional] @ 4:14-4:19
            function from_a_method(): void [conditional] @ 12:18-12:31
        "#]],
    );
}

#[test]
fn a_declaration_after_a_guard_is_not_itself_conditional() {
    // The flag is saved and restored around each nested walk, so the marker stops at the closing
    // brace. Getting this wrong would rank a project's own unguarded classes below the stubs, which
    // is the failure this whole mechanism exists to avoid -- and nothing else in a snapshot would
    // show it.
    check_hir(
        r#"
//- /src/After.php
<?php
if (!class_exists('Guarded')) {
    class Guarded {}
}

class Plain {}

function plain(): void {}

const PLAIN = 1;
"#,
        expect![[r#"
            class Guarded [conditional] @ 3:11-3:18
            class Plain @ 6:7-6:12
            function plain(): void @ 8:10-8:15
            const PLAIN @ 10:7-10:12
        "#]],
    );
}

#[test]
fn a_guard_inside_a_namespace_keeps_both_facts() {
    check_hir(
        r#"
//- /src/Namespaced.php
<?php
namespace App\Compat;

if (!function_exists('App\Compat\helper')) {
    function helper(): void {}
}
"#,
        expect![[r#"
            namespace App\Compat @ 2:11-2:21
              function helper(): void [conditional] @ 5:14-5:20
        "#]],
    );
}
