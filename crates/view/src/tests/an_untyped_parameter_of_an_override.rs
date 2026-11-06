//! A method parameter with no type of its own holds what the declaration it overrides says.
//!
//! `{@inheritdoc}` over an untyped signature is a house style, not a gap: on a framework the parent
//! is where every type is written, and the child repeats the signature without it. Counted on the
//! reference project, **188 of its 202** untyped parameters are exactly that shape.
//!
//! Strictly a lower bound -- PHP is contravariant on inputs, so an override may accept more than its
//! parent documents. What keeps it safe is the question it answers: what a *receiver* holds, which
//! is what an editor asks, and a declaration that means to widen writes a type rather than leaving
//! the slot empty.

use expect_test::expect;

use super::{Query, check_view};

/// The shape that found it: Drupal's `preRender(&$values)`, whose `@param ResultRow[]` is two
/// levels up because the class in between carries `{@inheritdoc}` too.
const HIERARCHY: &str = r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function id(): int {}
}

//- /src/Handler.php
<?php
namespace App;

interface Handler
{
    /**
     * @param \App\Row[] $values
     */
    public function preRender(&$values);
}

//- /src/Base.php
<?php
namespace App;

abstract class Base implements Handler
{
    /**
     * {@inheritdoc}
     */
    public function preRender(&$values) {}
}
"#;

#[test]
fn it_climbs_past_a_parent_that_states_no_type_either() {
    check_view(
        &format!(
            "{HIERARCHY}{}",
            r#"
//- /src/Field.php
<?php
namespace App;

final class Field extends Base
{
    /**
     * {@inheritdoc}
     */
    public function preRender(&$values): void
    {
        $v@values@alues;
        foreach ($values as $row) {
            $r@row@ow;
        }
    }
}
"#
        ),
        &[
            Query::variable_type("the parameter itself", "values"),
            Query::variable_class("and what it holds", "row", "row"),
        ],
        expect![[r#"
            the parameter itself
            - Row[]
            and what it holds
            - App\Row
        "#]],
    );
}

/// A parameter that overrides nothing is still untyped, and a guess there would be worse than
/// nothing.
#[test]
fn a_parameter_that_overrides_nothing_says_nothing() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Alone
{
    public function run($thing): void
    {
        $t@thing@hing;
    }
}
"#,
        &[Query::variable_type("nothing above it", "thing")],
        expect![[r#"
            nothing above it
            - (no type)
        "#]],
    );
}

/// A type written on the override is the override's own answer, whatever the parent says.
#[test]
fn a_type_written_here_wins() {
    check_view(
        &format!(
            "{HIERARCHY}{}",
            r#"
//- /src/Typed.php
<?php
namespace App;

final class Typed extends Base
{
    /**
     * {@inheritdoc}
     */
    public function preRender(&$values): void
    {
        $v@values@alues;
    }

    public function one(Row $row): void
    {
        $r@row@ow;
    }
}
"#
        ),
        &[
            Query::variable_type("inherited", "values"),
            Query::variable_type("written here", "row"),
        ],
        expect![[r#"
            inherited
            - Row[]
            written here
            - Row
        "#]],
    );
}
