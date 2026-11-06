//! `defined('Normalizer::NFKC_CF')` -- PHP's only way of asking whether a class constant is there.
//!
//! The constant is in the intl extension and *not* in the polyfill class that stands in for it when
//! the extension is absent, which is exactly what Symfony's `AbstractUnicodeString` guards for:
//!
//! ```php
//! if (!$compat || !\defined('Normalizer::NFKC_CF')) { ... }
//! $str->string = normalizer_normalize($str->string, \Normalizer::NFKC_CF);
//! ```
//!
//! Reporting inside that is reporting the guard. `method_exists` and `property_exists` were already
//! read for the same reason; `defined` is the third and the only one that names a class constant.
//!
//! Both halves of the name are in the one string, so the class is compared too -- a guard about
//! another class licenses nothing here, which is the same care `method_exists` takes by requiring
//! its subject to be the receiver.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_constant_the_file_asks_about_is_not_reported_and_its_neighbours_are() {
    check(
        r#"
//- /src/stubs.php
<?php

function defined(string $constant_name): bool {}

//- /src/Shim.php
<?php

class Shim
{
    public const PRESENT = 1;
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): int
    {
        if (!\defined('Shim::MAYBE')) {
            return 0;
        }

        // Asked about, so the file has already said it may be absent.
        $asked = \Shim::MAYBE;
        // Asked about on another class, which vouches for nothing here.
        $elsewhere = \Shim::ALSO_MAYBE;

        return $asked + $elsewhere + \Shim::PRESENT;
    }
}
"#,
        expect![[r#"
            15:29-15:39 error [undefined-constant] Undefined constant Shim::ALSO_MAYBE
        "#]],
    );
}
