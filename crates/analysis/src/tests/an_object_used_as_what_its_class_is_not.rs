//! An object of a class nothing can extend, invoked, subscripted or turned into a string.
//!
//! Three of PHP's `Error`s, each about what a class declares: `Object of type Foo is not callable`,
//! `Cannot use object of type Foo as array` -- a read, a write, `isset` and `??` alike -- and
//! `Object of class Foo could not be converted to string`, run against 8.5.10. Only a `final` class
//! or an enum is settled; an open class, one declaring the method or the interface, and a runtime
//! class all stay silent.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_closed_class_is_refused_what_it_does_not_declare() {
    check(
        r#"
//- /src/app.php
<?php

final class Plain
{
}

final class Printable
{
    public function __toString(): string
    {
        return '';
    }
}

class Open
{
}

enum Kind
{
    case A;
}

function read(Plain $o): mixed
{
    return $o['k'];
}

function test(Plain $o): bool
{
    return isset($o['k']) || ($o['j'] ?? false);
}

function call(Plain $o): void
{
    $o();
}

function concat(Plain $o, Kind $k): string
{
    return 'a' . $o . "{$k}";
}

function fine(Printable $p, Open $open): string
{
    $open();
    $open['k'];
    return 'a' . $p . $open;
}
"#,
        expect![[r#"
            26:12-26:14 error [impossible-use] an object of Plain cannot be used as an array: it is not an ArrayAccess
            31:18-31:20 error [impossible-use] an object of Plain cannot be used as an array: it is not an ArrayAccess
            31:31-31:33 error [impossible-use] an object of Plain cannot be used as an array: it is not an ArrayAccess
            36:5-36:7 error [impossible-use] an object of Plain is not callable: it declares no __invoke
            41:18-41:20 error [impossible-use] an object of Plain cannot be converted to a string: it declares no __toString
            41:25-41:27 error [impossible-use] an object of Kind cannot be converted to a string: it declares no __toString
        "#]],
    );
}
