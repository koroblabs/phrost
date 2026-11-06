//! A variable bound by destructuring, or by a call's argument, is in scope after the statement that
//! binds it.
//!
//! `[$loader, $tui] = $this->createLoaderWithTui();` -- Symfony's tests write it constantly -- and
//! `$tu` on the next line offered nothing, while goto on the same `$tui` went to the binding. The
//! list of locals read only assignments to a whole variable.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn every_shape_of_destructuring_binds_its_names() {
    check(
        r#"
//- /src/Loader.php
<?php
namespace App;

function load(array $pair, array $row, array $rows, array $nested): void
{
    [$loader, $tui] = $pair;
    ['id' => $id, 'label' => $label] = $row;
    [[$first], $second] = $nested;
    list($old, , $style) = $pair;
    foreach ($rows as [$code, $name]) {
        $n@foreach@
    }
    $t@positional@
}

function more(array $row, array $nested, array $pair): void
{
    ['id' => $id, 'label' => $label] = $row;
    [[$first], $second] = $nested;
    list($old, , $style) = $pair;
    $l@keyed@
}

function deeper(array $nested, array $pair): void
{
    [[$first], $second] = $nested;
    list($old, , $style) = $pair;
    $f@nested@
}

function legacy(array $pair): void
{
    list($old, , $style) = $pair;
    $s@list@
}
"#,
        &[
            Query::completion("positional", "positional"),
            Query::completion("keyed", "keyed"),
            Query::completion("nested", "nested"),
            Query::completion("list()", "list"),
            Query::completion("foreach", "foreach"),
        ],
        expect![[r#"
            positional
            - $tui [variable]
            - ... and the client must ask again
            - accepting the first leaves `$tui`
            keyed
            - $label [variable]
            - ... and the client must ask again
            - accepting the first leaves `$label`
            nested
            - $first [variable]
            - ... and the client must ask again
            - accepting the first leaves `$first`
            list()
            - $style [variable]
            - ... and the client must ask again
            - accepting the first leaves `$style`
            foreach
            - $nested [variable]
            - $name [variable]
            - ... and the client must ask again
            - accepting the first leaves `$nested`
        "#]],
    );
}

/// `preg_match($p, $s, $matches)` creates `$matches` through a by-reference parameter, which nothing
/// at the call site says -- so any bare variable passed to a call may be one it fills. The rule the
/// early-read check already followed; completion offered every local but the one just filled.
#[test]
fn a_variable_a_call_fills_is_in_scope_after_the_call() {
    check(
        r#"
//- /src/Parse.php
<?php
namespace App;

function parse(string $line): string
{
    preg_match('/(\d+)/', $line, $matches);
    return $mat@here@
}
"#,
        &[Query::completion("by reference", "here")],
        expect![[r#"
            by reference
            - $matches [variable]
            - ... and the client must ask again
            - accepting the first leaves `return $matches`
        "#]],
    );
}

/// A string `compact()` reads is a variable written without its `$`, and goto and rename already
/// read it that way -- so what can go there is the names in scope.
#[test]
fn a_compact_string_offers_the_locals_by_name() {
    check(
        r#"
//- /src/Range.php
<?php
namespace App;

function range_of(array $value): array
{
    $from = $value['from'];
    $to = $value['to'];
    return compact('from', 't@here@');
}
"#,
        &[Query::completion("compact", "here")],
        expect![[r#"
            compact
            - to [variable]
            - ... and the client must ask again
            - accepting the first leaves `return compact('from', 'to');`
        "#]],
    );
}
