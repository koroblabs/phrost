//! A docblock with a line comment between it and what it documents.
//!
//! ```php
//! /**
//!  * The entity for this result.
//!  *
//!  * @var \Drupal\Core\Entity\EntityInterface
//!  */
//! // phpcs:ignore Drupal.Classes.PropertyDeclaration
//! public $_entity = NULL;
//! ```
//!
//! That is `Drupal\views\ResultRow`, and the comment is there *because* the property is documented
//! -- the coding standard objects to the leading underscore and the author silenced it. Read as a
//! separator, the `@var` reached nothing, so every `$row->_entity` in a views field plugin had no
//! type and everything through it was dead.
//!
//! PHP's own rule is the other way: the compiler keeps the last doc comment it saw and an ordinary
//! comment does not clear it, which is why `getDocComment()` answers across one and why PhpStorm,
//! PHPStan and Psalm all read it. 215 properties on one real checkout are written this way.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_line_comment_between_them_separates_nothing() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Entity
{
    public function id(): int
    {
        return 1;
    }
}

class Row
{
    /**
     * The entity for this result.
     *
     * @var \App\Entity
     */
    // phpcs:ignore App.Classes.PropertyDeclaration
    public $silenced = NULL;

    /**
     * @var \App\Entity
     */
    # a hash comment
    public $hashed = NULL;

    /**
     * @var \App\Entity
     */
    public $adjacent = NULL;
}

class Reader
{
    public function run(Row $row): void
    {
        $si@silenced@lenced = $row->silenced;
        $ha@hashed@shed = $row->hashed;
        $ad@adjacent@jacent = $row->adjacent;
    }
}
"#,
        &[
            Query::hover("a phpcs:ignore under the block", "silenced"),
            Query::hover("a hash comment under it", "hashed"),
            Query::hover("nothing under it", "adjacent"),
        ],
        expect![[r#"
            a phpcs:ignore under the block
            - code: Entity $silenced
            - note: assigned
            a hash comment under it
            - code: Entity $hashed
            - note: assigned
            nothing under it
            - code: Entity $adjacent
            - note: assigned
        "#]],
    );
}
