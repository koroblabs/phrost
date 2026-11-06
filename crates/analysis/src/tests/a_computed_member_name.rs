//! `$x->{$name}` and `$x->{$name}()`, where `$name` is a value PHP cannot use as the name.
//!
//! Two rules, run against PHP 8.5. A property's name is converted to a string, so only an object
//! with no `__toString` fails -- and only one of a class nothing can extend, since a subclass may add
//! one. A method's name has to be a string already: an `int` fails, and so does a `Stringable`.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_method_name_has_to_be_a_string_and_a_property_name_has_to_convert_to_one() {
    check(
        r#"
//- /src/Names.php
<?php
namespace App;

final class Name
{
    public function __toString(): string
    {
        return 'id';
    }
}

final class Plain {}

final class Target
{
    public int $id = 1;

    public function id(): int
    {
        return 1;
    }
}

function run(Name $name, Plain $plain, Target $target, int $position, string $method): void
{
    echo $target->{$name};
    echo $target->{$plain};
    echo $target->{$position};
    echo $target->{$method}();
    echo $target->{$name}();
    echo $target->{$plain}();
    echo $target->{$position}();
    echo Target::{$name}();
}
"#,
        expect![[r#"
            27:20-27:26 error [impossible-use] an object of App\Plain cannot be converted to a string: it declares no __toString
            30:20-30:25 error [impossible-use] a method name has to be a string, and this is an object of App\Name
            31:20-31:26 error [impossible-use] a method name has to be a string, and this is an object of App\Plain
            32:20-32:29 error [impossible-use] a method name has to be a string, and this is int
            33:19-33:24 error [impossible-use] a method name has to be a string, and this is an object of App\Name
        "#]],
    );
}
