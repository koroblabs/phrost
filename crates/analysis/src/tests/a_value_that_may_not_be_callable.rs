//! Invoking a value that may be a sentinel.
//!
//! The fourth site in the family, beside the receiver, the subscript and the `foreach` subject.
//! PHP answers `Error: Value of type null is not callable` and, for the other half, `Value of type
//! bool is not callable` -- both `Error`, whatever the mode, which is why the `false` codes here
//! are errors where `possibly-false-argument` is a warning: weak mode converts an argument and
//! cannot convert a callee.
//!
//! Found against PHPStan on Laravel's own source, where 88 of 107 `callable.nonCallable` findings
//! are a `?callable` invoked without a guard -- which is how `when()` and `unless()` are written.
//!
//! What is *certainly* a sentinel is `impossible-use`, which owns that half and says which kind it
//! found; the third case below is here to show the two do not both report.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_call_through_a_value_that_may_be_a_sentinel() {
    check(
        r#"
//- /src/Invoke.php
<?php
namespace App;

class Invoke
{
    public function nullableCallable(?callable $cb): mixed
    {
        return $cb();
    }

    public function throughAProperty(): mixed
    {
        return ($this->maybe)();
    }

    /** @var callable|null */
    private $maybe;

    public function falseCallable(): mixed
    {
        $cb = $this->find();
        return $cb();
    }

    /** @return callable|false */
    private function find()
    {
        return false;
    }
}
"#,
        expect![[r#"
            8:16-8:19 warning [possibly-null-callee] Cannot call ?callable
            13:17-13:29 warning [possibly-null-callee] Cannot call ?callable
            22:16-22:19 warning [possibly-false-callee] Cannot call callable|false
        "#]],
    );
}

/// What a guard takes off, which is the whole of what keeps the check liveable.
///
/// The same narrowing the receiver check reads: a value tested before it is used is not the value
/// the declaration described. Without this every `?callable` in a project would be a finding, and
/// a project's answer to that is to switch the check off.
#[test]
fn a_guarded_call_through_a_value_says_nothing() {
    check(
        r#"
//- /src/Guarded.php
<?php
namespace App;

class Guarded
{
    public function ternary(?callable $cb): mixed
    {
        return $cb === null ? null : $cb();
    }

    public function earlyReturn(?callable $cb): mixed
    {
        if (!$cb) {
            return null;
        }
        return $cb();
    }

    /// Certainly null, and `impossible-use`'s to report rather than this check's.
    public function certain(): mixed
    {
        $cb = null;
        return $cb();
    }
}
"#,
        expect![[r#"
            23:16-23:19 error [impossible-use] a value of type null is not callable
        "#]],
    );
}
