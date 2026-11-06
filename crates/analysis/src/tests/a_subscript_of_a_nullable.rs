//! `$row['vin']` where `$row` may be null.
//!
//! One mistake with two spellings, and for a long time only one of them had a rule. `$row->vin()`
//! on a `?array`-shaped value was reported by `possibly-null-receiver`; `$row['vin']` was reported
//! by nobody, because the subscript rule steps aside for a possible null on the ground that it
//! "belongs to the null check" -- and that check walks member accesses.
//!
//! What the body layer already excludes is what a presence test is: a write creates the array, and
//! `isset`, `empty` and `??` are all willing to find nothing there. So the guards need no rule of
//! their own here; they were never sites.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_element_read_from_a_value_that_may_be_null() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array{vin: string}|null */
    public function row(): ?array
    {
        return null;
    }

    public function read(): string
    {
        $row = $this->row();

        return $row['vin'];
    }
}
"#,
        expect![[r#"
            16:16-16:20 warning [possibly-null-subscript] Cannot read element 'vin' of ?array{vin: string}
        "#]],
    );
}

#[test]
fn a_guard_and_a_coalesce_are_not_sites_at_all() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array{vin: string}|null */
    public function row(): ?array
    {
        return null;
    }

    public function guarded(): string
    {
        $row = $this->row();
        if ($row === NULL) {
            return '';
        }

        return $row['vin'];
    }

    public function coalesced(): string
    {
        $held = $this->row();

        return $held['vin'] ?? '';
    }
}
"#,
        expect![[r#""#]],
    );
}

/// The value on the left need not be a local, and the two shapes it is usually not are here.
///
/// The base was restricted to a plain variable for cost: a render array is `$form['a']['b']['#x']`
/// a thousand times per file, and stopping at the innermost `[` is one resolution per chain. That
/// exclusion is what the *nested* case buys, and it is kept; restricting it to a variable on top
/// bought nothing and lost the two spellings a nullable container is written in -- a `?array`
/// property, and a method that declares one.
#[test]
fn an_element_read_through_a_property_or_a_call() {
    check(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public ?array $state = null;

    public function trigger(): ?array
    {
        return null;
    }

    public function fromProperty(): mixed
    {
        return $this->state['#name'];
    }

    public function fromCall(): mixed
    {
        return $this->trigger()['#parents'];
    }
}
"#,
        expect![[r#"
            15:16-15:28 warning [possibly-null-subscript] Cannot read element '#name' of ?array
            20:16-20:32 warning [possibly-null-subscript] Cannot read element '#parents' of ?array
        "#]],
    );
}

/// A kind is asked only of a local, and the reason is what an untyped property's initializer is.
///
/// `public $update = false;` on WordPress's `WP_Theme` is written to with an array from outside the
/// class -- whole-program, which this index does not answer -- so the `false` is PHP's starting
/// value rather than a claim about what the property holds. A null is the other way round: it is
/// always declared, by a `?T` or by a docblock, so it can be judged wherever the value came from.
#[test]
fn a_kind_read_off_a_property_initializer_is_not_a_claim() {
    check(
        r#"
//- /src/Theme.php
<?php
namespace App;

class Theme
{
    public $update = false;

    public function version(): mixed
    {
        return $this->update['new_version'];
    }

    public function ownLocal(): mixed
    {
        $flag = false;

        return $flag['new_version'];
    }
}
"#,
        expect![[r#"
            17:16-17:21 warning [false-subscript] Cannot read element 'new_version' of false
        "#]],
    );
}

/// The other sentinel, at the same site. `json_decode($body, true)`, `unpack()`, `stat()` and
/// `wp_get_attachment_image_src()` all hand back `array|false`, and an unguarded `[...]` on one
/// reads an offset off a boolean -- which PHP warns about and evaluates as null, exactly as it does
/// for the null.
#[test]
fn an_element_read_from_a_value_that_may_be_false() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array|false */
    public function maybe()
    {
        return [];
    }

    public function unguarded(): mixed
    {
        $rows = $this->maybe();

        return $rows['id'];
    }

    public function guarded(): mixed
    {
        $rows = $this->maybe();
        if ($rows === FALSE) {
            return NULL;
        }

        return $rows['id'];
    }
}
"#,
        expect![[r#"
            16:16-16:21 warning [possibly-false-subscript] Cannot read element 'id' of array|false
        "#]],
    );
}

/// A value that may be either is one mistake, and the null report is the one that describes it.
#[test]
fn a_value_that_may_be_null_or_false_is_reported_once() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array|false|null */
    public function maybe()
    {
        return [];
    }

    public function read(): mixed
    {
        $rows = $this->maybe();

        return $rows['id'];
    }
}
"#,
        expect![[r#"
            16:16-16:21 warning [possibly-null-subscript] Cannot read element 'id' of array|false|null
        "#]],
    );
}

#[test]
fn a_local_another_method_builds_is_still_a_site_here() {
    // A local is a different value in every scope, and the set of values the file "builds an array
    // out of" was keyed by the name alone -- so a widget that writes `$element['#prefix']` in
    // `formElement` silenced every subscript check on `$element` in every other method of the file.
    // Three real findings on one Drupal widget, and the shape recurs wherever a framework names its
    // arrays the same thing twice; twenty-four more across WordPress, Symfony and Laravel.
    check(
        r#"
//- /src/Widget.php
<?php
namespace App;

class Widget
{
    public function triggering(): ?array
    {
        return NULL;
    }

    public function build(array $element): array
    {
        $element['#prefix'] = '<div>';

        return $element;
    }

    public function callback(): int
    {
        $element = $this->triggering();

        return count($element['#array_parents']);
    }
}
"#,
        expect![[r#"
            22:16-22:21 error [undefined-function] Undefined function count()
            22:22-22:30 warning [possibly-null-subscript] Cannot read element '#array_parents' of ?array
        "#]],
    );
}

#[test]
fn a_property_another_method_builds_is_still_not_one() {
    // The control, and the reason the rule is keyed by the body only for a local: `$this->rows` is
    // the same slot wherever it is written, so a method that fills it says exactly what the
    // file-wide reading always said it did.
    check(
        r#"
//- /src/Widget.php
<?php
namespace App;

class Widget
{
    private ?array $rows = NULL;

    public function fill(): void
    {
        $this->rows['first'] = 1;
    }

    public function read(): int
    {
        return $this->rows['first'];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_guard_on_a_property_reaches_the_local_a_call_on_it_filled() {
    // `resolve_path` asks what a guard settled about the path as a whole, which answers for
    // `$this->subject` and not for the `$this->subject->info()` a line below it. So the *call*
    // resolved -- the chain walk asks the same question of the receiver, which is why goto and
    // hover on it always worked -- and the local it was assigned to came back with no type at all.
    // Two readings of one rule, and the property is the only step a guard can name.
    check(
        r#"
//- /src/Subject.php
<?php
namespace App;

class Base {}

class Special extends Base
{
    /**
     * @return array{owners: array}|null
     */
    public function info(): ?array
    {
        return NULL;
    }
}

class Subject
{
    protected Base $subject;

    public function run(): int
    {
        if (!$this->subject instanceof Special) {
            return 0;
        }

        $info = $this->subject->info();

        return count($info['owners']);
    }
}
"#,
        expect![[r#"
            29:16-29:21 error [undefined-function] Undefined function count()
            29:22-29:27 warning [possibly-null-subscript] Cannot read element 'owners' of ?array{owners: array}
        "#]],
    );
}
