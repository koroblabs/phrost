//! Hover for a docblock that proves something beyond the return type.
//!
//! `@phpstan-assert-if-true \App\Special $this` is how a codebase writes what PHP itself cannot:
//! a boolean method whose `true` answer is also a type test. `crates/body/src/narrowing.rs`
//! already reads the parsed tag to narrow a guard; this is the same tag read for a human instead,
//! through the docblock text every other tag already renders from -- so it is exactly as good on
//! a closed file as an open one, for the same reason `@param` and `@return` already are.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_receiver_assertion_gets_its_own_section() {
    // Declaration and hover are in different files on purpose: under `with_open_scopes`'s
    // cursor-file-only pass, `Application.php` stays closed while `Use.php` is open, so this
    // exercises the docblock-from-disk path and not just the in-buffer one.
    check(
        r#"
//- /src/Application.php
<?php
namespace App;

class Special extends Application {}

class Application
{
    /**
     * Whether this is the special case.
     *
     * @return bool
     * @phpstan-assert-if-true \App\Special $this
     */
    public function isSpecial(): bool { return false; }
}

//- /src/Use.php
<?php
namespace App;

function run(Application $application): void
{
    $application->isSpe@here@cial();
}
"#,
        &[Query::hover("hover", "here")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Application
            {
                public function isSpecial(): bool
            }
            - ---
            - doc: Whether this is the special case.
            - tag: Returns bool 
            - tag: Asserts if true $this \App\Special
        "#]],
    );
}

#[test]
fn an_unconditional_argument_assertion_gets_its_own_section() {
    check(
        r#"
//- /src/Assert.php
<?php
namespace App;

final class Assert
{
    /**
     * Fails unless the value is not null.
     *
     * @phpstan-assert !null $actual
     */
    public static function notNull(mixed $actual): void {}
}

//- /src/Use.php
<?php
namespace App;

function run(): void
{
    Assert::not@here@Null(1);
}
"#,
        &[Query::hover("hover", "here")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Assert
            {
                public static function notNull(mixed $actual): void
            }
            - ---
            - doc: Fails unless the value is not null.
            - tag: Asserts $actual !null
        "#]],
    );
}
