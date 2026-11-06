//! What an override keeps of the docblock it did not repeat.
//!
//! `public function getFields(): array` under `{@inheritdoc}` is the whole Drupal idiom, and every
//! implementation of a documented interface looks like it. The hint says nothing about what the array
//! holds; the `@return Field[]` that does is on the interface.
//!
//! Carried when the table is flattened, beside the contract and for the same reason: reading is the
//! hot path, and climbing a hierarchy per member access cost goto p95 0.41 ms -> 0.88 ms last time it
//! was tried.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_override_inherits_the_element_type_its_docblock_did_not_repeat() {
    check(
        r#"
//- /src/Field.php
<?php
namespace App;

class Field
{
    public string $name;

    public function toSchemaProperty(): array {}
}

//- /src/AttachmentTypeInterface.php
<?php
namespace App;

interface AttachmentTypeInterface
{
    /**
     * @return \App\Field[]
     */
    public function getFields(): array;
}

//- /src/AbstractAttachmentType.php
<?php
namespace App;

abstract class AbstractAttachmentType implements AttachmentTypeInterface
{
    /**
     * {@inheritdoc}
     */
    public function getFields(): array {}

    public function buildJsonSchema(): array
    {
        foreach ($this->getFields() as $field) {
            $@one@field;
            $field->$0
        }
    }
}
"#,
        &[
            Query::hover("the loop variable", "one"),
            Query::completion("through it", "0"),
        ],
        expect![[r#"
            the loop variable
            - code: Field $field
            - note: loop variable
            through it
            - toSchemaProperty [method]  : array  writes `();`
            - name [property]  string
            - accepting the first leaves `$field->toSchemaProperty`
        "#]],
    );
}

#[test]
fn an_override_that_wrote_its_own_type_keeps_it() {
    // Only what the override left unsaid is inherited. A docblock of its own is the author saying
    // something different, and `effective_ty` still ranks it against the native hint besides.
    check(
        r#"
//- /src/Row.php
<?php
namespace App;

class Row
{
    public function id(): int {}
}

class Other
{
    public function other(): int {}
}

//- /src/Reads.php
<?php
namespace App;

interface Reads
{
    /** @return \App\Other[] */
    public function all(): array;
}

//- /src/Repo.php
<?php
namespace App;

class Repo implements Reads
{
    /** @return \App\Row[] */
    public function all(): array {}

    public function run(): void
    {
        foreach ($this->all() as $one) {
            $one->$0
        }
    }
}
"#,
        &[Query::completion("its own docblock wins", "0")],
        expect![[r#"
            its own docblock wins
            - id [method]  : int  writes `();`
            - accepting the first leaves `$one->id`
        "#]],
    );
}
