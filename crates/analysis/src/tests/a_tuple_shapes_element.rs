//! `array{Thing, string}` indexed by position, and the same nested in a generic.
//!
//! A keyless shape entry takes PHP's next free integer key, so a tuple is readable by `[0]` the way
//! any other shape is readable by its key. The whole stack already did that -- `$row[0]` on an
//! `array{0: Thing}` resolved before this -- and what was missing was the numbering at lowering,
//! which dropped keyless entries and took the shape with them.
//!
//! Here as well as at the lowering layer because the two answer different questions. That one says
//! the key is invented; this says a reader finds it, through a `Held::Stored` shape from the index
//! rather than a written one -- and the nested case is the shape the borrowed corpus asks for,
//! where the tuple is the element type of an `array<...>`.
//!
//! `undefined-method` on the element, because that is the instrument that separates "the element is
//! known" from "the element is unknown": an unknown receiver is silent.

use expect_test::expect;

use super::diagnostics::check;

const THING: &str = r#"
//- /thing.php
<?php
class Thing
{
    public function name(): string
    {
        return '';
    }
}
"#;

/// A tuple's own position, read straight off the declaration.
#[test]
fn a_positional_shape_is_readable_by_its_position() {
    check(
        &[
            THING,
            r#"
//- /app.php
<?php
class Rows
{
    /**
     * @return array{Thing, string}
     */
    public function one()
    {
        return [new Thing(), ''];
    }

    public function read(): void
    {
        $pair = $this->one();
        $pair[0]->name();
        $pair[0]->nope();
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            16:19-16:23 error [undefined-method] Undefined method Thing::nope()
        "#]],
    );
}

/// The same tuple as the element type of a generic, which is the shape a callback receives.
#[test]
fn a_positional_shape_survives_being_an_element_type() {
    check(
        &[
            THING,
            r#"
//- /nested.php
<?php
class Cases
{
    /**
     * @return array<array{Thing, string}>
     */
    public function all()
    {
        return [];
    }

    public function read(): void
    {
        foreach ($this->all() as $case) {
            $case[0]->name();
            $case[0]->nope();
        }
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            16:23-16:27 error [undefined-method] Undefined method Thing::nope()
        "#]],
    );
}

/// A named key beside a keyless one claims no integer, so the entry after it is still 1.
///
/// The one part of the numbering a reader could get wrong without a snapshot noticing, checked
/// here as well as at lowering because getting it wrong types the *wrong position* -- which reads
/// as the feature working until somebody indexes the other one.
#[test]
fn a_named_key_does_not_consume_a_position() {
    check(
        &[
            THING,
            r#"
//- /mixed.php
<?php
class Mixed_
{
    /**
     * @return array{Thing, label: string, Thing}
     */
    public function three()
    {
        return [new Thing(), 'label' => '', new Thing()];
    }

    public function read(): void
    {
        $row = $this->three();
        $row[0]->name();
        $row[1]->nope();
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            16:18-16:22 error [undefined-method] Undefined method Thing::nope()
        "#]],
    );
}
