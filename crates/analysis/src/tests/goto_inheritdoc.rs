//! Goto and hover on the `{@inheritdoc}` tag itself.
//!
//! The cursor is inside a *comment* here, which every other goto case in this crate is built to
//! miss -- a docblock holds no name reference and no declaration range. `inherited_docblocks.rs`
//! already proves the hierarchy climb hover uses; this is the same climb, wired to goto instead of
//! to the text hover renders.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn goto_on_the_inheritdoc_tag_reaches_the_ancestor_method() {
    check(
        r#"
//- /src/BaseService.php
<?php
namespace App;

class BaseService
{
    /**
     * Upstream summary.
     */
    public function label(): string { return 'base'; }
}

//- /src/ChildService.php
<?php
namespace App;

class ChildService extends BaseService
{
    /** {\@inherit@tag@doc} */
    public function label(): string { return 'child'; }
}
"#,
        &[Query::goto("the inheritdoc tag", "tag")],
        expect![[r#"
            the inheritdoc tag
            - method App\BaseService::label @ src/BaseService.php 9:21-9:26 (full 9:5-9:55)
        "#]],
    );
}

#[test]
fn goto_on_the_inheritdoc_tag_reaches_an_interface_method_through_two_levels() {
    // `overridden_member` climbs through an already-flattened table, so one step here crosses
    // both the abstract class and the interface it implements -- the same shape Drupal's own
    // interfaces are written in.
    check(
        r#"
//- /src/AttachmentTypeInterface.php
<?php
namespace App;

interface AttachmentTypeInterface
{
    /**
     * Builds the schema.
     */
    public function buildSchema(): array;
}

//- /src/AbstractAttachmentType.php
<?php
namespace App;

abstract class AbstractAttachmentType implements AttachmentTypeInterface
{
}

//- /src/JsonAttachmentType.php
<?php
namespace App;

class JsonAttachmentType extends AbstractAttachmentType
{
    /** {\@inheritd@tag@oc} */
    public function buildSchema(): array { return []; }
}
"#,
        &[Query::goto("through the abstract class", "tag")],
        expect![[r#"
            through the abstract class
            - method App\AttachmentTypeInterface::buildSchema @ src/AttachmentTypeInterface.php 9:21-9:32 (full 9:5-9:42)
        "#]],
    );
}

#[test]
fn a_position_elsewhere_in_the_same_docblock_finds_nothing() {
    // Precision, not proximity: a docblock that *contains* `{@inheritdoc}` is not the same as the
    // cursor sitting on it, and prose beside the tag is not a name or a declaration either.
    check(
        r#"
//- /src/BaseService.php
<?php
namespace App;

class BaseService
{
    /** Upstream summary. */
    public function label(): string { return 'base'; }
}

//- /src/ChildService.php
<?php
namespace App;

class ChildService extends BaseService
{
    /**
     * No@te@te.
     * {@inheritdoc}
     */
    public function label(): string { return 'child'; }
}
"#,
        &[Query::goto("prose beside the tag", "te")],
        expect![[r#"
            prose beside the tag
            - (nothing to go to)
        "#]],
    );
}

/// Hover on the tag, which answers the same search rendered rather than followed.
///
/// Worth having beside goto because the whole point of the tag is that the words are in another
/// file: reading them should not cost leaving this one. The note is what keeps the card honest --
/// it describes a declaration that is not the one under the cursor.
#[test]
fn hover_on_the_inheritdoc_tag_shows_what_it_inherits() {
    check(
        r#"
//- /src/BaseService.php
<?php
namespace App;

class BaseService
{
    /**
     * Upstream summary.
     *
     * @return string The label.
     */
    public function label(): string { return 'base'; }
}

//- /src/ChildService.php
<?php
namespace App;

class ChildService extends BaseService
{
    /** {\@inherit@tag@doc} */
    public function label(): string { return 'child'; }
}
"#,
        &[Query::hover("the inheritdoc tag", "tag")],
        expect![[r#"
            the inheritdoc tag
            - note: inherited from BaseService::label
            - code: <?php
            namespace App;

            class BaseService
            {
                public function label(): string
            }
            - ---
            - doc: Upstream summary.
            - tag: Returns string The label.
        "#]],
    );
}
