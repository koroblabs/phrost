//! `$rows = $thing->rows() ?? [];` -- the idiom that turns a nullable list into a list.
//!
//! The union it builds has the element type on one side and a bare `array` on the other, because an
//! empty literal resolves to exactly that. Iterating the union then yielded nothing at all: one
//! operand said `Row`, the other said nothing, and nothing is what a union of the two can promise.
//!
//! An empty array holds nothing, so it contributes no element and the other operand is the whole of
//! what can come out. Only against an operand that *states* its element -- a bare `array` beside a
//! `string` is a real alternative and stays.
//!
//! 65 occurrences of `) ?? []` in one real project, every `foreach` over the result untyped.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn the_element_survives_the_coalesce() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Filter {}

class Report
{
    /** @return Filter[]|null */
    public function filters(): ?array
    {
        return null;
    }
}

class Host
{
    public function read(Report $report): void
    {
        $filters = $report->filters() ?? [];

        foreach ($filters as $filter) {
            $fil@element@ter;
        }
    }
}
"#,
        &[Query::variable_class(
            "what the list holds",
            "element",
            "filter",
        )],
        expect![[r#"
            what the list holds
            - App\Filter
        "#]],
    );
}

/// A bare `array` beside something that is not a container is a real alternative, and stays.
#[test]
fn a_bare_array_beside_a_scalar_is_kept() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Host
{
    /** @return string|array */
    public function either(): string|array
    {
        return '';
    }

    public function read(): void
    {
        $value = $this->either();
        $val@kept@ue;
    }
}
"#,
        &[Query::variable_type("both alternatives", "kept")],
        expect![[r#"
            both alternatives
            - string|array
        "#]],
    );
}
