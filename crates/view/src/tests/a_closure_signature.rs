//! A closure's rendered type keeps its parameters.
//!
//! `Closure(): string` on a closure that takes two arguments does not read as "the parameters are
//! unknown" -- it reads as "it takes none", which is a claim about the value and a false one. The
//! inference was never wrong here; only the rendering dropped half the signature.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_closure_renders_with_the_parameters_it_declares() {
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

class Maker
{
    public function probe(): void
    {
        $long = function (int $n, ?string $label = null): Row { return new Row(); };
        $lo@long@ng;
        $short = fn (Row $row): string => 'x';
        $sh@short@ort;
        $none = function (): Row { return new Row(); };
        $no@none@ne;
    }
}
"#,
        &[
            Query::variable_type("two parameters, one defaulted", "long"),
            Query::variable_type("an arrow function", "short"),
            Query::variable_type("genuinely no parameters", "none"),
        ],
        expect![[r#"
            two parameters, one defaulted
            - Closure(int $n, ?string $label = ...): Row
            an arrow function
            - Closure(Row $row): string
            genuinely no parameters
            - Closure(): Row
        "#]],
    );
}

/// Invoking it still gives back what it returns, which is the one thing anything but the renderer
/// asks a callable.
#[test]
fn invoking_a_closure_still_gives_back_what_it_returns() {
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row {}

//- /src/Use.php
<?php
namespace App;

class Maker
{
    public function probe(): void
    {
        $make = function (int $n): Row { return new Row(); };
        $row = $make(1);
        $r@here@ow;
    }
}
"#,
        &[Query::variable_class("invoked", "here", "row")],
        expect![[r#"
            invoked
            - App\Row
        "#]],
    );
}
