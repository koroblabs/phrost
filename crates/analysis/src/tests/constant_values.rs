//! What a constant or a case was set to, which is most of what either one means.
//!
//! The stored signature is composed from index rows and holds a name and a declared type, so a
//! constant used to hover as `const int LIMIT` -- the half a reader is asking about missing. The
//! value comes back out of the source, over the range the index already keeps, and the awkward
//! shapes are all here because the ranges are not one-to-one with the members.

use expect_test::expect;

use super::{Query, check};

const FIXTURE: &str = r#"
//- /src/Constants.php
<?php
namespace App;

enum Stage: string
{
    case Draft = 'draft';
}

enum Kind
{
    case Plain;
}

class Limits
{
    public const int LIMIT = 10;

    const FIRST = 1, SECOND = 2;

    const SHAPE = ['a' => 1, 'b' => 2];

    const SENTENCE = 'one, two, three';

    const WIDE = [
        'a' => 1,
        'b' => 2,
    ];
}
"#;

/// The shape that started this: a backed case shows what it is backed by.
#[test]
fn a_backed_case_shows_its_value() {
    check(
        &format!(
            "{FIXTURE}\n//- /src/Read.php\n<?php\nnamespace App;\n\n$a = Stage::Dra@here@ft;\n"
        ),
        &[Query::hover("a backed case", "here")],
        expect![[r#"
            a backed case
            - code: <?php
            namespace App;

            enum Stage
            {
                case Draft = 'draft'
            }
        "#]],
    );
}

/// A case with nothing behind it is written `case Plain;` and has nothing to add.
#[test]
fn a_unit_case_shows_no_value() {
    check(
        &format!(
            "{FIXTURE}\n//- /src/Read.php\n<?php\nnamespace App;\n\n$a = Kind::Pla@here@in;\n"
        ),
        &[Query::hover("a unit case", "here")],
        expect![[r#"
            a unit case
            - code: <?php
            namespace App;

            enum Kind
            {
                case Plain
            }
        "#]],
    );
}

/// A typed constant keeps its type and gains its value.
#[test]
fn a_typed_constant_shows_both_its_type_and_its_value() {
    check(
        &format!(
            "{FIXTURE}\n//- /src/Read.php\n<?php\nnamespace App;\n\n$a = Limits::LIM@here@IT;\n"
        ),
        &[Query::hover("a typed constant", "here")],
        expect![[r#"
            a typed constant
            - code: <?php
            namespace App;

            class Limits
            {
                public const int LIMIT = 10
            }
        "#]],
    );
}

/// Two constants in one statement share a `full_range` -- the statement's -- so the first one has
/// to stop before the second begins, and the second has to start after its own name.
#[test]
fn two_constants_in_one_statement_each_show_only_their_own_value() {
    check(
        &format!(
            "{FIXTURE}\n//- /src/Read.php\n<?php\nnamespace App;\n\n$a = Limits::FIR@first@ST;\n$b = Limits::SECO@second@ND;\n"
        ),
        &[
            Query::hover("the first", "first"),
            Query::hover("the second", "second"),
        ],
        expect![[r#"
            the first
            - code: <?php
            namespace App;

            class Limits
            {
                public const FIRST = 1
            }
            the second
            - code: <?php
            namespace App;

            class Limits
            {
                public const SECOND = 2
            }
        "#]],
    );
}

/// A comma inside the value is not the one that ends it -- in brackets or in a string.
#[test]
fn a_comma_inside_the_value_does_not_end_it() {
    check(
        &format!(
            "{FIXTURE}\n//- /src/Read.php\n<?php\nnamespace App;\n\n$a = Limits::SHA@shape@PE;\n$b = Limits::SENTEN@sentence@CE;\n"
        ),
        &[
            Query::hover("an array", "shape"),
            Query::hover("a string holding commas", "sentence"),
        ],
        expect![[r#"
            an array
            - code: <?php
            namespace App;

            class Limits
            {
                public const SHAPE = ['a' => 1, 'b' => 2]
            }
            a string holding commas
            - code: <?php
            namespace App;

            class Limits
            {
                public const SENTENCE = 'one, two, three'
            }
        "#]],
    );
}

/// A value written across lines is left out. Pasting it into a popup buries the declaration it is
/// supposed to be describing, and the name alone is the better answer.
#[test]
fn a_value_written_across_lines_is_left_out() {
    check(
        &format!(
            "{FIXTURE}\n//- /src/Read.php\n<?php\nnamespace App;\n\n$a = Limits::WID@here@E;\n"
        ),
        &[Query::hover("a multi-line array", "here")],
        expect![[r#"
            a multi-line array
            - code: <?php
            namespace App;

            class Limits
            {
                public const WIDE
            }
        "#]],
    );
}
