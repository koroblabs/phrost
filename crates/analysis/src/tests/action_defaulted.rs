//! Giving a read that may find nothing something to find.
//!
//! The sibling of the nullsafe fix and the same argument: `?? null` moves the null to whatever
//! consumes the expression rather than hiding it, and the next check answers there.
//!
//! Which default depends on which sentinel, and the two are not interchangeable: `??` tests null
//! only, so a `preg_split()` returning `false` walks straight through it.

use expect_test::expect;

use super::actions::check;

#[test]
fn a_read_that_may_find_no_array_is_given_a_null() {
    // Parenthesised, and the fixture is written to show why: `. '!'` binds tighter than `??`, so an
    // unparenthesised edit would read `$row['vin'] ?? (null . '!')` and change what the line means.
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array{vin: string}|null */
    public function row(): ?array
    {
        return NULL;
    }

    public function read(): string
    {
        $row = $this->row();

        return $row['v@here@in'] . '!';
    }
}
"#,
        "here",
        expect![[r#"
            - Extract 'vin' into a constant
                leaves ``
                leaves `return $row[self::VIN] . '!';`
            - Extract $value
                leaves `return $row['vin'] . '!';`
                leaves `return $value . '!';`
            - Read 'vin' as null where there is nothing to read  [preferred]
                leaves `return ($row['vin'] ?? null) . '!';`
        "#]],
    );
}

#[test]
fn a_loop_over_nothing_is_given_an_empty_array() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return string[]|false */
    public function orFalse()
    {
        return FALSE;
    }

    public function walk(): void
    {
        $rows = $this->orFalse();
        foreach ($r@here@ows as $row) {
            echo $row;
        }
    }
}
"#,
        "here",
        expect![[r#"
            - Inline $rows
                leaves `foreach ($rows as $row) {`
                leaves `foreach ($this->orFalse() as $row) {`
            - Walk nothing where there is nothing to walk  [preferred]
                leaves `foreach ($rows ?: [] as $row) {`
        "#]],
    );
}
