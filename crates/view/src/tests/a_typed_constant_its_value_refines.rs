//! A typed class constant is what its value says, where the value says more than the hint.
//!
//! `const string CERT = __DIR__ . '/a.crt'` is a `non-falsy-string`, and reading only the hint made
//! every call handing it to a `non-empty-string` parameter a finding -- five in one corpus's tests,
//! none of them real. PHPStan reads the value. An untyped constant already was its value; a hint is
//! PHP 8.3's, and it was winning outright.
//!
//! The hint still bounds what is read: a value saying nothing narrower leaves it as written, a
//! `bool` hint is never read off its value, and a constant a subclass may redeclare keeps its hint,
//! which is the contract the redeclaration keeps -- see `Lowerer::constant_hint_refined`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_string_constant_is_the_string_its_value_is() {
    check_view(
        r#"
//- /src/Paths.php
<?php
namespace App;

final class Paths
{
    public const string CERT = __DIR__ . '/certs/server.crt';
    public const string EMPTY = '';
    public const string ZERO = '0';
    public const UNTYPED = 'id';
    public const bool DEBUG = false;

    public function read(): void
    {
        $c@cert@ert = self::CERT;
        $e@empty@mpty = self::EMPTY;
        $z@zero@ero = self::ZERO;
        $u@untyped@ntyped = self::UNTYPED;
        $d@debug@ebug = self::DEBUG;
    }
}
"#,
        &[
            Query::variable_type("a path built from __DIR__", "cert"),
            Query::variable_type("the empty string says nothing narrower", "empty"),
            Query::variable_type("'0' is not empty but is falsy", "zero"),
            Query::variable_type("an untyped constant, as before", "untyped"),
            Query::variable_type("a bool hint is not read off its value", "debug"),
        ],
        expect![[r#"
            a path built from __DIR__
            - non-falsy-string
            the empty string says nothing narrower
            - string
            '0' is not empty but is falsy
            - non-empty-string
            an untyped constant, as before
            - non-falsy-string
            a bool hint is not read off its value
            - bool
        "#]],
    );
}

/// Through `$object::X` the class is whichever the object is, and a subclass may redeclare a public
/// constant of an open class with any `string` at all. Private and final ones it cannot.
#[test]
fn a_constant_a_subclass_may_redeclare_keeps_its_hint() {
    check_view(
        r#"
//- /src/Status.php
<?php
namespace App;

class Status
{
    public const string OPEN = 'ok';
    final public const string SEALED = 'ok';
    private const string OWN = 'ok';

    public function read(): void
    {
        $o@open@pen = self::OPEN;
        $s@sealed@ealed = self::SEALED;
        $w@own@wn = self::OWN;
    }
}
"#,
        &[
            Query::variable_type("public in an open class", "open"),
            Query::variable_type("final in an open class", "sealed"),
            Query::variable_type("private in an open class", "own"),
        ],
        expect![[r#"
            public in an open class
            - string
            final in an open class
            - non-falsy-string
            private in an open class
            - non-falsy-string
        "#]],
    );
}
