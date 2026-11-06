//! `Closure(int): string` in a docblock is a signature *and* the class `Closure`.
//!
//! Read as a bare `callable`, the name was nobody's reference, so `use Closure;` above it was
//! reported unused. And as the PHPDoc keyword it is, it means `\Closure` in any namespace -- so a
//! file that does not import it has written nothing wrong either.

use expect_test::expect;

use super::diagnostics::check_asked_for;

#[test]
fn an_imported_closure_is_used_by_its_signature() {
    check_asked_for(
        r#"
//- /stubs/Closure.php
<?php
final class Closure {}

//- /src/Run.php
<?php
namespace App;

use Closure;

/** @param Closure(int): string $format */
function run(callable $format): string
{
    return $format(1);
}
"#,
        &["unused-import"],
        expect![[r#""#]],
    );
}

#[test]
fn an_unimported_closure_signature_names_the_global_class() {
    check_asked_for(
        r#"
//- /src/Run.php
<?php
namespace App;

/** @return iterable<array{0: int, 1: (Closure(int): int)}> */
function cases(): iterable
{
    return [];
}
"#,
        &["unused-import"],
        expect![[r#""#]],
    );
}
