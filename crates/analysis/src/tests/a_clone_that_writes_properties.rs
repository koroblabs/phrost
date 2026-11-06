//! PHP 8.5's `clone($object, ['x' => 2])`, whose entries are property writes on the copy.
//!
//! Asked of PHP 8.5.10: an undeclared name creates a dynamic property, a value of the wrong type is
//! a `TypeError` and a private property is an `Error` -- each exactly what `$copy->x = 2` gives from
//! the same scope. So the entries are judged by the checks a property write already has, and the
//! name inside the quotes is the member the way `$copy->x` names it.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn each_entry_is_judged_as_the_write_it_is() {
    check(
        r#"
//- /stubs/core.php  stub
<?php
/**
 * @template T of object
 * @param T $object
 * @return T
 */
function clone(object $object, array $withProperties = []): object {}

//- /src/Point.php
<?php
declare(strict_types=1);
namespace App;

final class Point
{
    public function __construct(
        public int $x,
        public string $label,
        private int $secret = 0,
    ) {}
}

function move(Point $point): array
{
    return [
        clone($point, ['x' => 2, 'label' => 'moved']),
        clone($point, ['nope' => 1]),
        clone($point, ['x' => 'two']),
        clone($point, ['secret' => 5]),
    ];
}
"#,
        expect![[r#"
            18:25-18:29 error [undefined-property] Undefined property App\Point::$nope
            19:31-19:36 error [incompatible-assignment] App\Point::$x takes int, and this is non-falsy-string
            20:25-20:31 error [inaccessible-member] Cannot access private property App\Point::$secret
        "#]],
    );
}
