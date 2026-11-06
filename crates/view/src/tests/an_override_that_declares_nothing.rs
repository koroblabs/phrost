//! A method overriding one that declares a type inherits that declaration, whatever its body does.
//!
//! A declaration with neither a hint nor a tag takes its type from what its `return` statements
//! agree on, and that answer was stored in the same field a docblock's would be -- so it *was* the
//! declaration as far as every reader was concerned, and the climb to the interface above never
//! happened. Drupal's `Select::execute()` is the shape: `SelectInterface::execute()` declares
//! `@return StatementInterface|null` and the concrete class overrides it with no docblock, so
//! reading the body answered the non-nullable thing it happens to hand back.
//!
//! PHP's own rule is the other way round: the inherited claim is the contract and the body is an
//! implementation detail of it. So the climb happens first for a type read off a body, and an
//! ancestor whose type came off *its* body is skipped on the way -- `Extender::execute()` calling
//! `parent::execute()` found the parent's body and stopped one interface short.

use expect_test::expect;

use super::{Query, check_view};

const KIT: &str = r#"
//- /src/kit.php
<?php
namespace App;

class Statement {}

class Other {}

interface Selects
{
    /** @return Statement|null */
    public function execute();
}

class Base implements Selects
{
    public function execute()
    {
        return new Other();
    }
}

class Extender extends Base
{
    public function execute()
    {
        return parent::execute();
    }
}
"#;

#[test]
fn the_inherited_declaration_wins_over_the_body() {
    check_view(
        &format!(
            "{KIT}{}",
            r#"
//- /src/app.php
<?php
namespace App;

class Host
{
    public function direct(Base $query): void
    {
        $row = $query->execute();
        $r@base@ow;
    }

    public function twoDeep(Extender $query): void
    {
        $row = $query->execute();
        $r@extender@ow;
    }
}
"#
        ),
        &[
            Query::variable_type("the interface above it", "base"),
            Query::variable_type(
                "past an ancestor whose own type came off a body",
                "extender",
            ),
        ],
        expect![[r#"
            the interface above it
            - ?Statement
            past an ancestor whose own type came off a body
            - ?Statement
        "#]],
    );
}

/// And a method nothing above declares still reads its own body, which is what that reading is for.
#[test]
fn a_body_still_answers_where_nothing_above_declares() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Row {}

class Maker
{
    public function make()
    {
        return new Row();
    }

    public function use(): void
    {
        $row = $this->make();
        $r@own@ow;
    }
}
"#,
        &[Query::variable_type("read off its own body", "own")],
        expect![[r#"
            read off its own body
            - Row
        "#]],
    );
}

/// The pairing that types a callback's parameter from a container passed beside it describes the
/// standard library -- `array_map($cb, $rows)` and its neighbours -- and it was applied to any
/// callee at all. A *method* taking a callback beside an array is not that shape: Laravel's
/// `$v->sometimes('x', ['Foo', 'Bar'], fn ($i) => $i->x)` passes rules, not elements, and reading
/// them as the callback's parameter gave `$i` a `string` and reported the property access on it.
#[test]
fn a_method_taking_a_callback_beside_an_array_pairs_nothing() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Rules
{
    public function sometimes(string $attribute, array $rules, callable $callback): void {}
}

class Host
{
    public function run(Rules $rules): void
    {
        $rules->sometimes('x', ['Foo', 'Bar'], function ($item) {
            $it@paired@em;
        });
    }
}
"#,
        &[Query::variable_type(
            "not an element of the rules",
            "paired",
        )],
        expect![[r#"
            not an element of the rules
            - (no type)
        "#]],
    );
}
