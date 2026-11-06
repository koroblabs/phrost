//! `non-empty-array<K, V>` and `non-empty-list<V>`, which used to be read as the plain array.
//!
//! Folded into `array<K, V>` at lowering, the one thing these declarations say was gone before any
//! reader existed: `reset()` on a `non-empty-list` was reported as possibly `false`, and `[]` handed
//! to one was silent. A list that may be empty keeps its finding, which is what says the rule reads
//! the declaration rather than the call.

use expect_test::expect;

use super::diagnostics::check;

const STUBS: &str = r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return ($array is non-empty-array ? V : V|false)
 */
function reset(array &$array): mixed {}
"#;

#[test]
fn a_declaration_that_rules_the_empty_array_out_is_read() {
    check(
        &format!(
            r#"{STUBS}
//- /src/Docs.php
<?php
namespace App;

final class Doc {{}}

/** @param non-empty-list<Doc> $docs */
function first(array $docs): Doc
{{
    $first = reset($docs);
    return $first;
}}

/** @param non-empty-array<string, Doc> $docs */
function keyed(array $docs): Doc
{{
    $first = reset($docs);
    return $first;
}}

/** @param list<Doc> $docs */
function maybe(array $docs): Doc
{{
    $first = reset($docs);
    return $first;
}}

/** @param non-empty-list<int> $ids */
function load(array $ids): void {{}}

function run(): void
{{
    load([]);
    load([1]);
}}
"#
        ),
        expect![[r#"
            24:12-24:18 warning [possibly-false-return] Doc|false returned where \App\Doc is declared
            32:10-32:12 error [incompatible-argument] $ids takes non-empty-array<int, int>, and this is array
        "#]],
    );
}
