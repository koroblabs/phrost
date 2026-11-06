//! A property the class answers for and no declaration writes.
//!
//! `__get` is what makes a name legal that nothing declared, and it is why nothing could say what
//! reading such a name gives back. A framework can, two ways: by naming the property outright, or by
//! saying that every key of one of its catalogs is one -- which is how a Drupal field, declared in
//! YAML and mentioned in no PHP anywhere, becomes a type.
//!
//! Owned at this layer because this is where a value's class is decided, and because the answer has
//! to hold for both halves of it: what the property holds, and where a jump on it goes.

use expect_test::expect;
use phrost_index::PropertyNames;

use super::{Query, check_view_with};

/// A receiver with an accessor, an interface for what one hands back, and a declared property to
/// prove the accessor does not get asked about it.
const FIXTURE: &str = r#"
//- /src/Dep.php
<?php
namespace Dep;

interface ItemsInterface
{
    public function getValue();
}

interface Referenced
{
    public function label(): string;
}

interface HasFieldsInterface
{
    /** @return \Dep\ItemsInterface */
    public function get($name);
}

class Record implements HasFieldsInterface
{
    public string $name = '';

    public function get($name) {}

    public function __get($property) {}
}
"#;

fn with_fields(world: &mut phrost_harness::World) {
    world.contracts_mut().bind_property(
        "Dep\\HasFieldsInterface",
        PropertyNames::KeyOf("field".into()),
        "Dep\\ItemsInterface",
        false,
    );
    world.contracts_mut().bind_property(
        "Dep\\ItemsInterface",
        PropertyNames::Exactly("target".into()),
        "Dep\\Referenced",
        true,
    );
    // Registered and nothing more, which is the shape a key read out of a configuration file has:
    // the catalog knows the name exists and the class comes from the contract, not the key.
    world.contracts_mut().catalog_entry("field", "amount", None);
    // A key spelled like the receiver's own declared property, for the test that PHP's order holds.
    world.contracts_mut().catalog_entry("field", "name", None);
}

#[test]
fn a_property_named_like_a_catalog_key_holds_what_the_contract_said() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $list = $record->amount;
        $li@one@st;
    }}
}}
"#
        ),
        &[Query::variable_class("read as a property", "one", "list")],
        with_fields,
        expect![[r#"
            read as a property
            - Dep\ItemsInterface
        "#]],
    );
}

#[test]
fn a_name_the_catalog_does_not_hold_stays_as_dynamic_as_it_was() {
    // The half that keeps this from being a claim about every name. A class with `__get` answers to
    // anything, and saying what two of those names hold must not turn the rest into answers.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $list = $record->nothing_registers_this;
        $li@one@st;
    }}
}}
"#
        ),
        &[Query::variable_class("an unregistered name", "one", "list")],
        with_fields,
        expect![[r#"
            an unregistered name
            - (type not known here)
        "#]],
    );
}

#[test]
fn a_declared_property_of_the_same_name_wins() {
    // PHP's own order, and the reason the member table is asked first: `__get` runs only where no
    // accessible declaration answered, so a catalog key spelled like a real property changes nothing.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $record->na@one@me;
    }}
}}
"#
        ),
        &[Query::goto("the declaration, not the accessor", "one")],
        with_fields,
        expect![[r#"
            the declaration, not the accessor
            - property Dep\Record::name @ src/Dep.php 22:19-22:24
        "#]],
    );
}

#[test]
fn a_method_call_of_the_same_name_is_untouched() {
    // The contract is about a property read. A call spelled the same way is a different member table
    // and a different question, and answering it here would invent a method.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $record->amo@one@unt();
    }}
}}
"#
        ),
        &[Query::goto("called rather than read", "one")],
        with_fields,
        expect![[r#"
            called rather than read
            - (not found)
        "#]],
    );
}

#[test]
fn the_chain_carries_on_through_one() {
    // Both contracts at once, and the shape that made this worth building: the property gives a
    // list, and a name the list's own accessor answers for gives what it points at.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $found = $record->amount->target;
        $fou@one@nd;
    }}
}}
"#
        ),
        &[Query::variable_class("two accessors deep", "one", "found")],
        with_fields,
        expect![[r#"
            two accessors deep
            - Dep\Referenced
        "#]],
    );
}

#[test]
fn a_call_on_a_declared_return_reaches_the_same_accessor() {
    // The spelling that already worked, kept working. `get()` declares the list, and the property
    // read on it is the same contract the property spelling reaches.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $found = $record->get('amount')->target;
        $fou@one@nd;
    }}
}}
"#
        ),
        &[Query::variable_class("through the call", "one", "found")],
        with_fields,
        expect![[r#"
            through the call
            - Dep\Referenced
        "#]],
    );
}

#[test]
fn a_jump_lands_on_the_class_the_property_holds() {
    // There is nowhere else to land. The property is declared in no PHP at all, so the class it
    // hands back is the only thing at that position with a definition to read.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\Record;

class Caller
{{
    public function run(Record $record): void
    {{
        $record->am@one@ount;
    }}
}}
"#
        ),
        &[Query::goto("where a field read goes", "one")],
        with_fields,
        expect![[r#"
            where a field read goes
            - interface Dep\ItemsInterface @ src/Dep.php 4:11-4:25
        "#]],
    );
}

#[test]
fn a_receiver_below_no_contracted_class_is_left_alone() {
    // The hierarchy gate. A class with an accessor that no contract names answers for nothing here,
    // however the name is spelled.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Other.php
<?php
namespace App;

class Bag
{{
    public function __get($property) {{}}
}}

class Caller
{{
    public function run(Bag $bag): void
    {{
        $list = $bag->amount;
        $li@one@st;
    }}
}}
"#
        ),
        &[Query::variable_class(
            "an unrelated accessor",
            "one",
            "list",
        )],
        with_fields,
        expect![[r#"
            an unrelated accessor
            - (type not known here)
        "#]],
    );
}
