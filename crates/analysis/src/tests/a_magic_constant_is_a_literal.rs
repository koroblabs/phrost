//! `__LINE__` is an `int` and the other nine are `string`s.
//!
//! The compiler substitutes a literal, so this is the same kind of fact as a quoted string and not
//! a name anything resolves. Left unmodelled they said nothing at all, and the cost is not the
//! assignment nobody writes -- it is what a *call* around one then answers:
//! `str_replace('\\', '.', __CLASS__)` declares `array|string` and narrows to `string` only when the
//! subject is one, so an untyped `__CLASS__` left the union standing and the `string` parameter
//! below it was reported. **153 such findings on Symfony, 4 on Laravel, and none anywhere new.**
//!
//! Found by a sweep: a scratch file that assigns one of every expression shape PHP has to a local
//! and passes each to a parameter of an incompatible type, so anything not reported is a shape
//! lowering says nothing about.
//!
//! The `str_replace` half is deliberately **not** pinned here. Two attempts at a fixture for it
//! were silent for the wrong reason -- the hand-written stub's conditional return does not decide
//! the way the vendored prelude's does -- and an `expect![""]` that passes without the rule running
//! is the one snapshot that is worse than none. What that half is measured by is the corpora, where
//! the count moved by 157 and nothing appeared.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_ten_magic_constants_say_what_php_substitutes() {
    check(
        r#"
//- /src/Takes.php
<?php

class Takes
{
    public function only(array $value): void
    {
    }
}

//- /src/app.php
<?php

function run(Takes $takes): void
{
    $directory = __DIR__;
    $line = __LINE__;
    $class = __CLASS__;
    $namespace = __NAMESPACE__;

    $takes->only($directory);
    $takes->only($line);
    $takes->only($class);
    $takes->only($namespace);
}
"#,
        expect![[r#"
            10:18-10:28 error [incompatible-argument] $value takes array, and this is non-falsy-string
            11:18-11:23 error [incompatible-argument] $value takes array, and this is int
            12:18-12:24 error [incompatible-argument] $value takes array, and this is string
            13:18-13:28 error [incompatible-argument] $value takes array, and this is string
        "#]],
    );
}
