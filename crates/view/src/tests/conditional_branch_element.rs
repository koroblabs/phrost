//! A conditional return branch that names what an array parameter holds.
//!
//! The third spelling of one claim, and the last of the three to be read. A conditional whose
//! branches agree is `agreed_return`'s, a plain `@return V` is `element_return`'s, and a branch of a
//! conditional whose test an argument *does* settle was reaching neither -- so `current($rows)`
//! answered `mixed` where `reset($rows)` answered `?Row`, for stubs that bind `V` to the same
//! elements. The difference was that `current`'s claim is written inside a branch:
//! `($array is object ? mixed : (V|false))`.
//!
//! The other half of what these hold is that a branch naming a real class is left alone. Reading it
//! as an argument's elements would be more precise where the call site's array is typed and a miss
//! where it is not, which is trading an answer for a guess.

use expect_test::expect;

use super::{Query, check_view};

/// Two functions written the way PHP's array API is: a test on the argument's *kind*, with one
/// branch saying nothing and the other naming the element.
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
 * @template V
 * @param object|array<array-key, V> $array
 * @return ($array is object ? mixed : (V|false))
 */
function head(object|array $array): mixed {}

/**
 * @template V
 * @param object|array<array-key, V> $array
 * @return ($array is object ? mixed : Row)
 */
function always_a_row(object|array $array): mixed {}

/**
 * @template K
 * @template V
 * @param \Traversable<K, V>|array<K, V> $it
 * @return ($keyed is true ? array<K, V> : list<V>)
 */
function to_array(\Traversable|array $it, bool $keyed = true): array {}
"#;

#[test]
fn a_template_inside_a_settled_branch_is_the_arrays_element() {
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
        $first = head($rows);
        $fi@first@rst;
    }}
}}
"#
        ),
        &[Query::variable_class(
            "the branch element",
            "first",
            "first",
        )],
        expect![[r#"
            the branch element
            - App\Row
        "#]],
    );
}

#[test]
fn a_branch_naming_a_class_stays_that_class_rather_than_the_arrays_element() {
    check_view(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    /** @param string[] $names */
    public function run(array $names): void
    {{
        $row = always_a_row($names);
        $r@row@ow;
    }}
}}
"#
        ),
        &[Query::variable_class("the written class", "row", "row")],
        expect![[r#"
            the written class
            - App\Row
        "#]],
    );
}

/// A branch written as a *container* of the template, which is the same claim one level out.
///
/// `iterator_to_array` is the shape, and it wins its conditional by the declaration's own default:
/// `($keyed is true ? array<K, V> : list<V>)` with `$keyed = true`. Lowered as written the branch
/// becomes a generic over two class names nothing declares, so the result read as a bare `array` --
/// an answer shaped right and empty.
#[test]
fn a_container_of_a_template_inside_a_settled_branch_is_the_arrays_elements() {
    check_view(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

class Caller
{{
    /** @param \Traversable<string, Row> $rows */
    public function run(\Traversable $rows): void
    {{
        $all = to_array($rows);
        $a@all@ll;
        foreach ($all as $row) {{
            $r@row@ow;
        }}
    }}
}}
"#
        ),
        &[
            Query::variable_type("the container", "all"),
            Query::variable_class("its element", "row", "row"),
        ],
        expect![[r#"
            the container
            - Row[]
            its element
            - App\Row
        "#]],
    );
}
