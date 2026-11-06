//! `@param \int $value` is the keyword `int` with a slash in front, not a class in the root
//! namespace. Read as a class it named nothing, and every argument for it went unjudged; a slash on
//! a real class -- `\DateTime` -- is still a fully-qualified name.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_slashed_keyword_is_the_keyword() {
    check(
        r#"
//- /src/DateTime.php
<?php
class DateTime {}

//- /src/Run.php
<?php
namespace App;

/** @param \int $value */
function slashed($value): void {}

/** @param \DateTime $value */
function dated($value): void {}

function go(): void
{
    slashed(1);
    slashed('x');
    dated(new \DateTime());
}
"#,
        expect![[r#"
            13:13-13:16 error [incompatible-argument] $value takes int, and this is non-falsy-string
        "#]],
    );
}
