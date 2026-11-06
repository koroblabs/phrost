//! `list($a, $b) = $row`, which is an array literal wearing a keyword.
//!
//! PHP has had one destructuring construct since 7.1: `list()` takes keys, holes and nesting exactly
//! as `[]` does, and neither can be written anywhere but an assignment or a `foreach` target. So the
//! two lower to one shape, and every reader of a destructuring target -- there are four -- knows both
//! for free.
//!
//! Both carry the slot each element takes, which is what
//! [`destructuring_elements`](crate::lowerer::Lowerer::destructuring_elements) writes down and what
//! makes `$first` knowable as `$row[0]`: a hole binds nothing and still takes its place.
//!
//! Left as an unmodelled node it kept its variables as nested expressions, which is the opposite of
//! what they are: `occurrences::collect_bindings` recognises a target by matching `ArrayLiteral`, so
//! the names inside a `list()` read as *uses* of something nothing binds.
//! `list($xlen) = unpack('v', $s); return $xlen;` reported the binding itself, and only where the
//! name was not also passed to a call -- an argument counts as bound, because a by-reference
//! parameter is the callee's business. **127 findings on WordPress**, whose bundled libraries are old
//! enough to write the long spelling throughout.

use expect_test::expect;

use super::check_bodies;

#[test]
fn both_spellings_of_a_destructuring_target_lower_to_one_shape() {
    check_bodies(
        r#"
//- /src/Rows.php
<?php
class Rows
{
    public function run(array $row): void
    {
        list($first, , 'k' => $keyed) = $row;
        [$short] = $row;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Rows::run
              body @ 5:5-8:6
                assign @ 6:9-6:45
                  array @ 6:9-6:38
                    element 0 @ 6:14-6:20 (key @ 6:14-6:20)
                      $first @ 6:14-6:20
                    element k @ 6:24-6:37 (key @ 6:24-6:27)
                      $keyed @ 6:31-6:37
                  =
                    $row @ 6:41-6:45
                assign @ 7:9-7:24
                  array @ 7:9-7:17
                    element 0 @ 7:10-7:16 (key @ 7:10-7:16)
                      $short @ 7:10-7:16
                  =
                    $row @ 7:20-7:24
        "#]],
    );
}
