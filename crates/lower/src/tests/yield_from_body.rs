//! What a generator's `yield`s say the declaration produces.
//!
//! PHP has no syntax for writing the element type in the signature -- `: \Generator` is all a
//! declaration can say -- so the body is the only place it is written down. And the correction that
//! comes with it: a `return` inside a generator hands its value to `Generator::getReturn()`, so
//! reading one as what the declaration produces types the call as something it can never be.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_yielded_class_becomes_the_element_of_what_the_signature_named() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function generating(): \Generator
    {
        yield new Row();
    }

    public function iterating(): iterable
    {
        yield new Row();
    }

    public function undeclared()
    {
        yield new Row();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method generating(): \Generator doc: \Generator<int, Row>  public @ 4:21-4:31
              method iterating(): iterable doc: iterable<int, Row>  public @ 9:21-9:30
              method undeclared() doc: iterable<int, Row>  public @ 14:21-14:31
        "#]],
    );
}

#[test]
fn a_return_inside_a_generator_is_not_what_the_declaration_produces() {
    // The correction. `getReturn()` is the only thing that ever sees this value, and typing the
    // call `Row` made every member reached through it resolve against a class it can never be.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function mixedUp()
    {
        yield new Item();
        return new Row();
    }

    public function unreadable(array $rows)
    {
        foreach ($rows as $row) {
            yield $row;
        }
        return new Row();
    }

    public function plain()
    {
        return new Row();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method mixedUp() doc: iterable<int, Item>  public @ 4:21-4:28
              method unreadable(array $rows)  public @ 10:21-10:31
              method plain() doc: Row  public @ 18:21-18:26
        "#]],
    );
}

#[test]
fn a_key_is_read_only_when_every_yield_writes_one() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function keyed()
    {
        yield new Key() => new Row();
    }

    public function halfKeyed()
    {
        yield new Key() => new Row();
        yield new Row();
    }

    public function keyedByAVariable(array $rows)
    {
        foreach ($rows as $at => $row) {
            yield $at => new Row();
        }
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method keyed() doc: iterable<Key, Row>  public @ 4:21-4:26
              method halfKeyed() doc: iterable<Row>  public @ 9:21-9:30
              method keyedByAVariable(array $rows) doc: iterable<Row>  public @ 15:21-15:37
        "#]],
    );
}

#[test]
fn yields_that_disagree_settle_nothing() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function two()
    {
        yield new Row();
        yield new Other();
    }

    public function delegating(array $rows)
    {
        yield from $rows;
    }

    public function bare()
    {
        yield;
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method two()  public @ 4:21-4:24
              method delegating(array $rows)  public @ 10:21-10:31
              method bare()  public @ 15:21-15:25
        "#]],
    );
}

#[test]
fn a_closure_that_yields_leaves_the_method_around_it_reading_its_own_returns() {
    // The scope rule `generators.rs` states, asked of the type rather than of the flag: the
    // closure is the generator, so the method still hands back what its own `return` says.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function make()
    {
        $rows = function () {
            yield new Row();
        };
        return new Wrapper($rows);
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method make() doc: Wrapper  public @ 4:21-4:25
        "#]],
    );
}

#[test]
fn a_docblock_that_says_what_is_yielded_is_not_second_guessed() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    /** @return \Generator<string, Row> */
    public function documented(): \Generator
    {
        yield new Other();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method documented(): \Generator doc: \Generator<string, Row>  public @ 5:21-5:31  [doc]
        "#]],
    );
}

#[test]
fn a_literal_key_is_the_type_php_hands_over() {
    // A generator keeps its key as written, where an array would store `'1'` as the integer `1`:
    // PHP 8.5.10 iterates `yield '1' => x` with the string key `"1"`. So `'1'` is a string here, and
    // keys of two kinds are two key domains that settle nothing.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function named()
    {
        yield 'first' => new Row();
        yield '1' => new Row();
    }

    public function numbered()
    {
        yield 10 => new Row();
    }

    public function mixed()
    {
        yield 'a' => new Row();
        yield 2 => new Row();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method named() doc: iterable<string, Row>  public @ 4:21-4:26
              method numbered() doc: iterable<int, Row>  public @ 10:21-10:29
              method mixed() doc: iterable<Row>  public @ 15:21-15:26
        "#]],
    );
}
