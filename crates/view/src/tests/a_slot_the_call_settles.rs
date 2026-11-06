//! A conditional written as one slot of a returned class, settled by the call.
//!
//! Laravel's `Collection::groupBy()` returns
//! `static<($groupBy is string ? array-key : ... TGroupKey), static<($preserveKeys is true ? TKey :
//! int), ($groupBy is array ? mixed : TValue)>>`: groups keyed by any key for a column name and by
//! what the callback returns for a callback, each group a collection of the same elements. A slot
//! was read as one type, the template in a branch refused the whole contract, and every
//! `groupBy()` held nothing. `keyBy()` is the one-slot form.
//!
//! Laravel 12 writes the key slot with tests this cannot read -- `($groupBy is (array|string) ? ...`
//! and `TGroupKey is \UnitEnum`, a template only the callback binds -- and the slot is then left to
//! its bound: any key. The groups beside it are what matter, and are read.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_conditional_slot_is_settled_by_the_call() {
    check_view(
        r#"
//- /src/Coll.php
<?php
namespace App;

class Page {}

/**
 * @template TKey of array-key
 * @template TValue
 */
class Coll
{
    /**
     * @template TGroupKey of array-key
     * @param (callable(TValue, TKey): TGroupKey)|array|string $groupBy
     * @param bool $preserveKeys
     * @return static<($groupBy is string ? array-key : ($groupBy is array ? array-key : TGroupKey)), static<($preserveKeys is true ? TKey : int), ($groupBy is array ? mixed : TValue)>>
     */
    public function groupBy($groupBy, $preserveKeys = false) {}

    /**
     * @template TGroupKey of array-key|\UnitEnum|\Stringable
     * @param (callable(TValue, TKey): TGroupKey)|array|string $groupBy
     * @param bool $preserveKeys
     * @return static<
     *  ($groupBy is (array|string)
     *      ? array-key
     *      : (TGroupKey is \UnitEnum ? array-key : (TGroupKey is \Stringable ? string : TGroupKey))),
     *  static<($preserveKeys is true ? TKey : int), ($groupBy is array ? mixed : TValue)>
     * >
     */
    public function groupByNewer($groupBy, $preserveKeys = false) {}

    /**
     * @template TNewKey of array-key
     * @param (callable(TValue, TKey): TNewKey)|array|string $keyBy
     * @return static<($keyBy is string ? array-key : ($keyBy is array ? array-key : TNewKey)), TValue>
     */
    public function keyBy($keyBy) {}
}

//- /src/run.php
<?php
namespace App;

/** @param Coll<int, Page> $pages */
function run(Coll $pages): void
{
    $byColumn = $pages->groupBy('chapter_id');
    $c@column@olumn = $byColumn;
    $byCallback = $pages->groupBy(fn (Page $page): int => 1);
    $b@callback@ack = $byCallback;
    $kept = $pages->groupBy('chapter_id', true);
    $k@kept@ept = $kept;
    $newer = $pages->groupByNewer('chapter_id');
    $n@newer@ewer = $newer;
    $keyed = $pages->keyBy('id');
    $y@keyed@ed = $keyed;
}
"#,
        &[
            Query::variable_type("grouped by a column", "column"),
            Query::variable_type("grouped by a callback", "callback"),
            Query::variable_type("keeping the keys", "kept"),
            Query::variable_type("keyed by a column", "keyed"),
            Query::variable_type("Laravel 12's spelling, its key left to the bound", "newer"),
        ],
        expect![[r#"
            grouped by a column
            - Coll<int|string, Coll<int, Page>>
            grouped by a callback
            - Coll<int, Coll<int, Page>>
            keeping the keys
            - Coll<int|string, Coll<int, Page>>
            keyed by a column
            - Coll<int|string, Page>
            Laravel 12's spelling, its key left to the bound
            - Coll<int|string, Coll<int, Page>>
        "#]],
    );
}
