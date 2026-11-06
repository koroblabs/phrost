//! Which container spellings state how they are keyed, and which only look as though they do.
//!
//! `list<V>` and `V[]` interned to the same one-argument type, and they do not say the same thing:
//! brackets name a container and nothing about its keys, where a list names `int` keys by definition.
//! Nothing needed the difference until a judgement started comparing keys, and then the two had to be
//! told apart before either could be read -- so `list<V>` is lowered with the key it means.
//!
//! `array-key` is the other half of the same care. Read as `string` -- which it was, sitting at the
//! end of a long list of string-ish spellings -- it makes an int-keyed array a finding against a key
//! that is explicitly *either*, which is the one thing such a key must never do.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_list_states_its_int_key_and_the_bracket_spelling_states_none() {
    // Read through `@property` tags because that is where a docblock type reaches the rendered
    // declaration: a `@param list<V>` beside a native `array` hint renders as the hint.
    //
    // `list<Row>` and `array<int, Row>` land on the same rendering, and that is the truth about them
    // -- nothing here models a list's contiguity, so the two really are one type. The rendering is
    // the weaker of the two claims on purpose: `array<int, Node>` keyed by entity id is the commonest
    // int-keyed array in a framework project and is emphatically not a list.
    check_hir(
        r#"
//- /src/Store.php
<?php
/**
 * @property list<Row> $listed
 * @property non-empty-list<Row> $nonEmpty
 * @property array<int, Row> $intKeyed
 * @property array<string, Row> $stringKeyed
 * @property Row[] $bracketed
 * @property array<Row> $oneArgument
 * @property list $bareList
 * @property iterable<string, Row> $stringKeyedIterable
 */
class Store {}
"#,
        expect![[r#"
            class Store @ 12:7-12:12  [doc]
              property $listed: array<int, Row>  public @ 3:24-3:31
              property $nonEmpty: non-empty-array<int, Row>  public @ 4:34-4:43
              property $intKeyed: array<int, Row>  public @ 5:30-5:39
              property $stringKeyed: array<string, Row>  public @ 6:33-6:45
              property $bracketed: Row[]  public @ 7:20-7:30
              property $oneArgument: Row[]  public @ 8:25-8:37
              property $bareList: array  public @ 9:19-9:28
              property $stringKeyedIterable: iterable<string, Row>  public @ 10:36-10:56
        "#]],
    );
}

#[test]
fn array_key_is_the_union_it_stands_for() {
    check_hir(
        r#"
//- /src/Keys.php
<?php
/**
 * @property array<array-key, Row> $anyKeyed
 * @property array-key $bare
 * @property array<int|string, Row> $spelledOut
 */
class Keys {}
"#,
        expect![[r#"
            class Keys @ 7:7-7:11  [doc]
              property $anyKeyed: array<int|string, Row>  public @ 3:36-3:45
              property $bare: int|string  public @ 4:24-4:29
              property $spelledOut: array<int|string, Row>  public @ 5:37-5:48
        "#]],
    );
}

/// `numeric` and `scalar` are the sets they name, not `mixed`.
///
/// Both sat with `mixed` in one arm, which was inert until a stub started writing the first:
/// mago-prelude 1.48.0 respells `array_sum` as `@param array<numeric>`, and folding that to
/// `array<mixed>` accepted every array there is.
///
/// **The old precision is not recoverable and this does not pretend to it.** `numeric` is
/// `int|float|numeric-string`, and `numeric-string` folds to plain `string` two arms up with every
/// other way the docblock language narrows one -- so `array_sum(['a'])` is accepted here where the
/// previous spelling `@param array<int|float>` rejected it. What the union does buy is the honest
/// weaker claim: `array_sum([true])` is rejected in a `strict_types` file, and a reader hovering
/// the parameter is shown `int|float|string` rather than `mixed`.
///
/// **The parentheses on the last line are this renderer's and nothing else's.** mago nests a union
/// as pairs, and the docblock lowering keeps the nesting where the native-hint lowering flattens it
/// (`types.rs::flatten_union`) -- so a hand-written `int|float|string` arrives as
/// `Union[int, Union[float, string]]` and prints with a bracket, while the union built here is
/// flat. It reads exactly like §8.1's degenerate-form hazard and is not one: `TypeTable::union`
/// folds the nesting out when the type is interned, so hover shows `int|float|string` and a guard
/// narrows a three-operand union as readily as a two-operand one. Both were checked before this
/// note was written.
#[test]
fn numeric_and_scalar_are_the_sets_they_stand_for() {
    check_hir(
        r#"
//- /src/Loose.php
<?php
/**
 * @property numeric $n
 * @property scalar $s
 * @property array<numeric> $sums
 * @property int|float|string $spelledOut
 */
class Loose {}
"#,
        expect![[r#"
            class Loose @ 8:7-8:12  [doc]
              property $n: int|float|string  public @ 3:22-3:24
              property $s: int|float|string|bool  public @ 4:21-4:23
              property $sums: array<int|float|string>  public @ 5:29-5:34
              property $spelledOut: int|(float|string)  public @ 6:31-6:42
        "#]],
    );
}
