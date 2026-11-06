//! A conditional return whose branches are containers of a template a parameter is declared as.
//!
//! `array_fill`'s shape: `@param T $value` beside `($start_index is 0 ? ($count is int<1, max> ?
//! non-empty-list<T> : list<T>) : (...))`. Nothing read a container of the argument itself, and the
//! inner test is a range nothing settles, so the contract was refused whole. Held at the index
//! because the contract is stored, and both invariants compare the dump.

use expect_test::expect;

use super::check_index;

#[test]
fn a_nested_conditional_that_agrees_is_its_container_of_the_value() {
    check_index(
        r#"
//- /src/Filler.php
<?php
namespace App;

class Filler
{
    /**
     * @template T
     * @param T $value
     * @return (
     *   $start is 0 ?
     *   ($count is int<1, max> ? non-empty-list<T> : list<T>) :
     *   ($count is int<1, max> ? non-empty-array<int, T> : array<int, T>)
     * )
     */
    public function fill(int $start, int $count, mixed $value): array {}
}
"#,
        expect![[r#"
            state ready
            file src/Filler.php [User]
              class App\Filler @ 4:7-4:13
              App\Filler is class
              App\Filler::method fill(int $start, int $count, mixed $value): array<int, mixed>  public returns, if argument 0 is 0, a container of whatever argument 2 is and otherwise a container of whatever argument 2 is  @ 15:21-15:25 (full 15:5-15:73)
            catalog
              [class] app\filler -> src/Filler.php (App\Filler)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}
