//! `$double = new class implements Contract { ... };` -- a test double written without a library.
//!
//! The class was lowered all along, so what is *inside* one worked: its members, its supertypes,
//! the locals in its methods. What did not was the value: the expression stayed unmodelled, so
//! `$double` had no type, every call on it answered nothing, and a call to a method it does not
//! have was not reported either.
//!
//! The name it is claimed under is synthesised from where its `class` keyword sits, and nobody can
//! write it -- which is why it is not offered by completion and is skipped as a *reference*. It is
//! a perfectly good name to resolve through, though, which is all the value needs.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_member_it_declares_resolves_and_a_member_it_does_not_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

interface Greets
{
    public function greet(): string;
}

class Host
{
    public function run(): string
    {
        $double = new class implements Greets {
            public function greet(): string
            {
                return 'hi';
            }
        };

        $double->missing();

        return $double->greet();
    }
}
"#,
        expect![[r#"
            20:18-20:25 error [undefined-method] Undefined method App\class@anonymous::missing()
        "#]],
    );
}

/// The constructor arguments are written outside the body and are judged like any other call's.
#[test]
fn its_constructor_is_the_one_it_declares() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Host
{
    public function run(): string
    {
        $held = new class('hi') {
            public function __construct(private string $message) {}

            public function message(): string
            {
                return $this->message;
            }
        };

        return $held->message();
    }
}
"#,
        expect![""],
    );
}
