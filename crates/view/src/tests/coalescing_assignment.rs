//! What `$x ??= $y` leaves the target holding.
//!
//! `??=` is the one compound assignment whose result the right-hand side describes, and it does not
//! describe it alone: the target keeps what it already held *unless* that was null. So the answer is
//! the union of the two with the null taken off the first, which is exactly what `??` already
//! builds -- and the fixture that matters is the one where the old value was null and nothing
//! survives it, because that is the shape every `?T $x = NULL` parameter is written in.
//!
//! At this layer rather than the body's because only a catalog can say whether the surviving
//! operand names a class, and only the union constructor can say what is left after the null goes.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_null_that_the_operator_replaces_does_not_survive_it() {
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(): void
    {
        $limit = NULL;
        $limit ??= 5;
        $li@after@mit;
    }
}
"#,
        &[Query::variable_type("after the operator", "after")],
        expect![[r#"
            after the operator
            - int
        "#]],
    );
}

#[test]
fn a_nullable_parameter_loses_only_its_null() {
    // The shape the whole feature is for, and the one that removed six false positives from the
    // reference project: a `?T` defaulted to null, defaulted again on the first line, then passed
    // to something that takes a `T`.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function title(): string {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(?Row $row = NULL): void
    {
        $row ??= new Row();
        $r@after@ow;
    }
}
"#,
        &[
            Query::variable_type("rendered", "after"),
            Query::variable_class("still a Row", "after", "row"),
        ],
        expect![[r#"
            rendered
            - Row
            still a Row
            - App\Row
        "#]],
    );
}

#[test]
fn a_target_that_could_not_be_null_keeps_both_alternatives() {
    // Nothing is removed when there was no null to remove, so the value really is one of the two.
    // Pointless code, and the union is what it means -- reading it as either operand alone would
    // offer members a caller cannot rely on.
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(int $n): void
    {
        $n ??= 'fallback';
        $@after@n;
    }
}
"#,
        &[Query::variable_type("both", "after")],
        expect![[r#"
            both
            - int|non-falsy-string
        "#]],
    );
}

#[test]
fn a_memoised_property_is_not_null_once_the_operator_has_filled_it() {
    // `$this->cached ??= new Row()` is the commonest spelling in real code -- a memoised property,
    // filled on first use -- and after it the property cannot be null. This used to answer `?Row`
    // and pinned that as the limit of `??=`, because a property's type came from its declaration
    // and nothing narrowed one. It is `slot_value` that moved the limit: the operator writes the
    // slot like any other assignment, and what it writes is the union with the null taken off.
    check_view(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function title(): string {}
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    private ?Row $cached = NULL;

    public function go(): void
    {
        $this->cached ??= new Row();
        $r@after@ow = $this->cached;
    }
}
"#,
        &[Query::variable_type("the memoised property", "after")],
        expect![[r#"
            the memoised property
            - Row
        "#]],
    );
}
