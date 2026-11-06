//! A docblock type that names a class nobody declares.
//!
//! A warning under its own code rather than `undefined-type`: in a docblock the name is a wrong
//! annotation, not a fatal error, and the docblock type language has names PHP does not. Those are
//! left alone -- a template, `self` and `static`, a PHPStan pseudo-type, a constant, an alias the
//! class declares or imports (in a method body too), and a `@see`, which points somewhere rather
//! than stating a type.

use expect_test::expect;

use super::diagnostics::{check, check_switched_off};

const FIXTURE: &str = r#"
//- /src/Rows.php
<?php
namespace App;

/**
 * @phpstan-import-type RowShape from Shapes
 * @phpstan-type Input string|RowShape
 * @phpstan-type Page array{rows: list<RowShape>}
 * @template TKey of array-key
 * @extends Base<Page>
 * @mixin Missing\Helper
 * @mixin Missing\Builder<RowShape>
 * @property MissingProperty $count
 * @see RfcLogLevel for the levels
 */
final class Rows extends Base
{
    /**
     * @template T
     * @param T $value
     * @param TKey $key
     * @param Input $input
     * @param non-empty-string $name
     * @return static|self|null
     */
    public function put(mixed $value, int|string $key, mixed $input, string $name): static
    {
        return $this;
    }

    /** @return Status */
    public function status(): int
    {
        /** @var Page $page */
        $page = ['rows' => []];
        return $page['rows'] === [] ? 0 : 1;
    }

    /**
     * @phpstan-type Param (callable(int): mixed)|int
     * @psalm-type Legacy = array{id: int}
     * @param Param|Legacy $param
     */
    public function each(mixed $param): void
    {
    }

    /** @return ($ratio is INF ? string : int) */
    public function ratio(float $ratio): int|string
    {
        return 0;
    }
}

/** @template T */
abstract class Base
{
}
"#;

#[test]
fn only_a_name_that_could_be_a_class_and_is_none_is_reported() {
    check(
        FIXTURE,
        expect![[r#"
            10:11-10:25 warning [doc-undefined-type] Undefined type Missing\Helper in a docblock
            11:11-11:36 warning [doc-undefined-type] Undefined type Missing\Builder in a docblock
            12:14-12:29 warning [doc-undefined-type] Undefined type MissingProperty in a docblock
            30:17-30:23 warning [doc-undefined-type] Undefined type Status in a docblock
        "#]],
    );
}

#[test]
fn a_project_can_switch_the_code_off() {
    check_switched_off(FIXTURE, &["doc-undefined-type"], expect![""]);
}
