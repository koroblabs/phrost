//! `$this`, `self`, `static` and `parent` written where the language gives them nothing.
//!
//! Four of PHP's `Error`s, and the interesting half of this check is everything it does *not*
//! report. Run against 8.5.10 one line at a time, the answers are not what the messages suggest:
//!
//! * **everything inside a closure compiles**, however the closure is written and wherever it sits.
//!   `function () { return $this; }` in a static method compiles, `static fn () => $this` compiles,
//!   `fn () => parent::class` in a class with no parent compiles. `Closure::bind` is why, and
//!   Symfony has 79 `$this` and 24 `parent::` of exactly that shape that all run;
//! * the file's **top level** is not an error either: a `.php` file `include`d from inside a method
//!   runs in that object's context, which is what every PHP template is. Symfony's
//!   `error.html.php` opens with `<?= $this->charset ?>`;
//! * a **trait**'s `parent::` is resolved against the using class, which the trait cannot know.
//!   `MicroKernelTrait` writes twenty-two;
//! * `self::` in a class **constant's initialiser** is inside the class, though no method body
//!   holds it. Symfony writes 628, which is why the class test is over ranges rather than over the
//!   walk.
//!
//! What is left is certain: a name written directly in a body nothing can rebind.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_name_written_where_it_cannot_mean_anything() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Holder
{
    public int $x = 1;

    public static function inStatic(): int
    {
        return $this->x;
    }
}

class Orphan
{
    public function m(): string
    {
        return parent::class;
    }
}

function free(): int
{
    return $this->x;
}

function outside(): string
{
    return static::class;
}
"#,
        expect![[r#"
            10:16-10:21 error [out-of-context] $this is written where there is no object
            18:16-18:22 error [out-of-context] parent is written in a class that extends nothing
            24:12-24:17 error [out-of-context] $this is written where there is no object
            29:12-29:18 error [out-of-context] static is written outside a class
        "#]],
    );
}

/// Everything above that PHP compiles, which is most of it.
#[test]
fn what_binding_can_still_settle_is_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

trait UsesParent
{
    public function m(): string
    {
        return parent::class;
    }
}

class Holder
{
    public int $x = 1;

    const NAMES = [self::LABEL => 1];

    const LABEL = 'a';

    public static function inStatic(): callable
    {
        return function () {
            return $this->x;
        };
    }

    public function instance(): int
    {
        $f = fn () => $this->x;

        return $f();
    }
}
"#,
        expect![""],
    );
}
