//! `@return V` naming what an array parameter holds.
//!
//! The plain half of a claim the conditional path already read. `reset`'s stub writes
//! `($array is non-empty-list ? V : … V|false)` and reached the element branch; `array_find`'s
//! writes `@return V|null` and reached nothing — so one answered `?Row` and the other `mixed`, for
//! the same claim written two ways.
//!
//! What the tests hold, beyond the answer itself: a docblock naming two different templates keeps
//! the union its declaration wrote, and a docblock with no `@template` is untouched, because a
//! `@param Row[]` beside a `@return Row` already lowers by the ordinary path and a second route to
//! one answer is how two routes come to disagree.

use expect_test::expect;

use super::{Query, check_view};

/// One class to hand back, so a wrong answer is a different type rather than nothing.
const FIXTURE: &str = r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function title(): string
    {
        return '';
    }
}

//- /src/helpers.php
<?php
namespace App;

/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return V|null
 */
function pick(array $array) {}

/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return V|K
 */
function either(array $array) {}
"#;

#[test]
fn a_template_an_array_parameter_binds_is_that_arrays_element() {
    check_view(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    /** @param Row[] $rows */
    public function run(array $rows): void
    {{
        $one = pick($rows);
        $o@one@ne;
    }}
}}
"#
        ),
        &[Query::variable_class("the element", "one", "one")],
        expect![[r#"
            the element
            - App\Row
        "#]],
    );
}

#[test]
fn two_different_templates_keep_the_union_the_declaration_wrote() {
    check_view(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    /** @param Row[] $rows */
    public function run(array $rows): void
    {{
        $both = either($rows);
        $b@both@oth;
    }}
}}
"#
        ),
        &[Query::variable_class("two templates", "both", "both")],
        expect![[r#"
            two templates
            - (type not known here)
        "#]],
    );
}
