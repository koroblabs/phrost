//! A closure's untyped parameter, where the closure is called in the same expression it is written.
//!
//! `(static function ($resellers) { ... })($this->user()->getResellers())` builds a form's options in
//! place; the parameter holds the argument, and what the argument holds is read where the call is,
//! since a static closure has no `$this` of its own.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_parameter_holds_what_the_call_passes() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Reseller {}

class Form
{
    /** @return list<Reseller> */
    public function resellers(): array { return []; }

    public function build(): void
    {
        (static function ($r@passed@esellers) {})($this->resellers());
        (fn ($f@arrow@irst, $s@second@econd) => 1)(new Reseller(), 'x');
        (function ($n@named@amed) {})(named: new Reseller());
        (function ($u@unpassed@npassed) {})();
    }
}
"#,
        &[
            Query::variable_type("a static closure called with a method's return", "passed"),
            Query::variable_type("an arrow function's first parameter", "arrow"),
            Query::variable_type("and its second", "second"),
            Query::variable_type("a named argument is not read by position", "named"),
            Query::variable_type("a parameter nothing is passed to stays untyped", "unpassed"),
        ],
        expect![[r#"
            a static closure called with a method's return
            - array<int, Reseller>
            an arrow function's first parameter
            - Reseller
            and its second
            - non-falsy-string
            a named argument is not read by position
            - (no type)
            a parameter nothing is passed to stays untyped
            - (no type)
        "#]],
    );
}
