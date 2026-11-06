//! The type a class constant's value gives it, when the declaration writes none.
//!
//! PHP only let a class constant carry a type in 8.3, so the untyped form is not a corner case --
//! it is every constant in the standard library, in `vendor/`, and in a framework's interfaces.
//! What is snapshotted here is the whole list of shapes read and the whole list refused, because
//! the refusals are the load-bearing half: a wrong type on a constant that thousands of call sites
//! pass around is worse than the nothing these declarations had before.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_literal_beside_the_name_types_the_constant() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Kinds
{
    const TEXT = 'draft';
    const COUNT = 1;
    const RATIO = 1.5;
    const ON = true;
    const OFF = false;
    const NOTHING = null;
}
"#,
        expect![[r#"
            class Kinds @ 2:7-2:12
              const TEXT: non-falsy-string  public @ 4:11-4:15
              const COUNT: int  public @ 5:11-5:16
              const RATIO: float  public @ 6:11-6:16
              const ON: true  public @ 7:11-7:13
              const OFF: false  public @ 8:11-8:14
              const NOTHING: null  public @ 9:11-9:18
        "#]],
    );
}

/// A written hint wins, and `ty_from_value` is not set -- which is what keeps the rendered
/// declaration honest. See `phrost_sema::MemberRow::ty_from_value`.
#[test]
fn a_written_hint_is_never_replaced_by_the_value() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Kinds
{
    public const int MAX = 84;
    public const string NAME = 'x';
}
"#,
        expect![[r#"
            class Kinds @ 2:7-2:12
              const MAX: int  public @ 4:22-4:25
              const NAME: string  public @ 5:25-5:29
        "#]],
    );
}

/// One declaration, two values: the type is read per item, not once for the statement.
#[test]
fn each_item_of_one_declaration_is_read_from_its_own_value() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Kinds
{
    const FIRST = 1, SECOND = 'two', THIRD = [];
}
"#,
        expect![[r#"
            class Kinds @ 2:7-2:12
              const FIRST: int  public @ 4:11-4:16
              const SECOND: non-falsy-string  public @ 4:22-4:28
              const THIRD: array  public @ 4:38-4:43
        "#]],
    );
}

/// The wrappers a value arrives inside. A sign is an operator to mago rather than part of the
/// literal, so `const LIMIT = -1;` reaches here as a unary expression and would otherwise be
/// unread -- and it is how half the integer constants in real code are written.
#[test]
fn a_sign_parentheses_and_the_array_forms_are_looked_through() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Kinds
{
    const LIMIT = -1;
    const OFFSET = +2;
    const DRIFT = -1.5;
    const GROUPED = (7);
    const NESTED = -(-3);
    const SHORT = ['a', 'b'];
    const LEGACY = array('a');
}
"#,
        expect![[r#"
            class Kinds @ 2:7-2:12
              const LIMIT: int  public @ 4:11-4:16
              const OFFSET: int  public @ 5:11-5:17
              const DRIFT: float  public @ 6:11-6:16
              const GROUPED: int  public @ 7:11-7:18
              const NESTED: int  public @ 8:11-8:17
              const SHORT: array<int, non-falsy-string>  public @ 9:11-9:16
              const LEGACY: array<int, non-falsy-string>  public @ 10:11-10:17
        "#]],
    );
}

/// `.` produces a string whatever it is fed -- an operand it cannot concatenate is a `TypeError`
/// rather than another type -- so the operands do not have to resolve. This is what types
/// `const PATH = self::ROOT . '/sub';`, which no other rule here reaches.
#[test]
fn concatenation_and_the_string_forms_are_a_string_whatever_the_operands_are() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Kinds
{
    const ROOT = '/var';
    const PATH = self::ROOT . '/sub';
    const DOC = <<<TXT
        a line
        TXT;
}
"#,
        expect![[r#"
            class Kinds @ 2:7-2:12
              const ROOT: non-falsy-string  public @ 4:11-4:15
              const PATH: non-falsy-string  public @ 5:11-5:15
              const DOC: non-falsy-string  public @ 6:11-6:14
        "#]],
    );
}

/// The refusals, and the reason each one is a refusal rather than an omission. Every constant here
/// has a type at runtime; none of them has one the *syntax* settles, so reading it would mean
/// resolving a name -- which is the def-map's job -- or guessing.
#[test]
fn a_value_whose_type_depends_on_an_operand_leaves_the_constant_untyped() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Kinds
{
    const A = 1;
    const B = 2;
    // `int` for integer operands and `string` for string ones.
    const FLAGS = self::A | self::B;
    // `int`, `float`, or -- for two arrays -- `array`.
    const SUM = self::A + self::B;
    // A name, which only the def-map can resolve.
    const ALIAS = self::A;
    const ELSEWHERE = Other::THING;
    const WHERE = __DIR__;
    const SHAPE = new Kinds();
}
"#,
        expect![[r#"
            class Kinds @ 2:7-2:12
              const A: int  public @ 4:11-4:12
              const B: int  public @ 5:11-5:12
              const FLAGS  public @ 7:11-7:16
              const SUM  public @ 9:11-9:14
              const ALIAS  public @ 11:11-11:16
              const ELSEWHERE  public @ 12:11-12:20
              const WHERE  public @ 13:11-13:16
              const SHAPE  public @ 14:11-14:16
        "#]],
    );
}

/// An enum case is not a constant and has no value type to read: its type is the enum. Snapshotted
/// beside the constants because both arrive through the same member lookup, and a change that
/// started typing cases would be a silent behaviour change.
#[test]
fn an_enum_case_is_still_typed_by_its_enum_and_not_by_its_backing_value() {
    check_hir(
        r#"
//- /src/T.php
<?php
enum Stage: string
{
    case Draft = 'draft';
    const DEFAULT = 'draft';
}
"#,
        expect![[r#"
            enum Stage: string @ 2:6-2:11
              case Draft  public @ 4:10-4:15
              const DEFAULT: non-falsy-string  public @ 5:11-5:18
        "#]],
    );
}

/// A lookup table: an array literal whose values are all one builtin states what it holds, and its
/// keys when they are all of one kind -- a canonical decimal string is stored as the integer it
/// spells, and `'01'` is not one. Anything this cannot settle leaves the literal a plain `array`.
#[test]
fn a_table_of_one_kind_states_its_element() {
    check_hir(
        r#"
//- /src/T.php
<?php
class Tables
{
    const SIZES = ['thumb' => 200, 'full' => 1200];
    const NAMES = ['a', 'b'];
    const FLAGS = ['x' => true, 'y' => false];
    const NUMERIC = ['1' => 'a', '2' => 'b'];
    const PADDED = ['01' => 'a', 2 => 'b'];
    const MIXED = ['a' => 1, 'b' => 'x'];
    const NULLS = [null, null];
    const EMPTY = [];
    const SPREAD = [...self::NAMES];
}
"#,
        expect![[r#"
            class Tables @ 2:7-2:13
              const SIZES: array<non-falsy-string, int>  public @ 4:11-4:16
              const NAMES: array<int, non-falsy-string>  public @ 5:11-5:16
              const FLAGS: array<non-falsy-string, bool>  public @ 6:11-6:16
              const NUMERIC: array<int, non-falsy-string>  public @ 7:11-7:18
              const PADDED: non-falsy-string[]  public @ 8:11-8:17
              const MIXED: array{a: int, b: non-falsy-string}  public @ 9:11-9:16
              const NULLS: array  public @ 10:11-10:16
              const EMPTY: array  public @ 11:11-11:16
              const SPREAD: array  public @ 12:11-12:17
        "#]],
    );
}
