//! A declaration naming a class the index holds nothing for judges nothing.
//!
//! The report built out of an absence. `@return Row` where `Row` is a type alias the file imported
//! and nothing expanded, `@return T` where nobody wrote the `@template`, a vendor class the
//! checkout does not install -- all three name a class the project does not have, and the value the
//! method really hands back was rejected against it.
//!
//! `ClassHierarchy::relate` already said a class nothing declares says nothing, and it is asked only
//! when *both* sides are classes: a shape, an array or a string against a class never reached it.
//! Two halves of one rule and only one of them knew, which is the form §8.1 opens with.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_return_judged_against_a_class_nothing_declares_is_silent() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Real {}

class Rows
{
    /** @return NoSuchClass */
    public function absent()
    {
        return ['id' => 1];
    }

    /** @return Real */
    public function declared()
    {
        return ['id' => 1];
    }
}
"#,
        expect![[r#"
            8:17-8:28 warning [doc-undefined-type] Undefined type NoSuchClass in a docblock
            17:16-17:27 error [incompatible-return] Cannot return array{id: int} where \App\Real is declared
        "#]],
    );
}

#[test]
fn an_argument_judged_against_a_class_nothing_declares_is_silent_too() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Real {}

class Rows
{
    /** @param NoSuchClass $row */
    public function absent($row): void {}

    /** @param Real $row */
    public function declared($row): void {}

    public function go(): void
    {
        $this->absent('text');
        $this->declared('text');
    }
}
"#,
        expect![[r#"
            8:16-8:27 warning [doc-undefined-type] Undefined type NoSuchClass in a docblock
            17:25-17:31 error [incompatible-argument] $row takes \App\Real, and this is non-falsy-string
        "#]],
    );
}
