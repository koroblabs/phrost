//! Goto and hover on a parameter's name written inside a docblock.
//!
//! The cursor is inside a *comment*, which every other goto case in this crate is built to miss: a
//! docblock holds no name reference and no declaration range. What makes it answerable anyway is
//! that a docblock names things the signature under it declares, and `@param string $processorType`
//! is a claim about a parameter eight characters away.
//!
//! Anywhere in the block, not only on the tag. The variable is written the same way in the `@param`
//! line, in the prose under it and in an `@throws` description, and all of them mean the same
//! parameter -- answering for one and not the others is the shape a reader reads as unreliability
//! rather than as a rule.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_docblock_parameter_name_reaches_the_parameter_it_is_about() {
    check(
        r#"
//- /src/Reviewer.php
<?php
namespace App;

class Reviewer
{
    /**
     * Whether a document failed.
     *
     * @param string $proc@tag@essorType
     *   The kind of processor, which $proc@ose@essorType selects.
     * @param string $missing
     *   Named here and nowhere in the signature.
     */
    public function failed(string $processorType): bool
    {
        return $processorType === '';
    }
}
"#,
        &[
            Query::goto("on the tag's own name", "tag"),
            Query::goto("and in the prose under it", "ose"),
        ],
        expect![[r#"
            on the tag's own name
            - parameter failed::processorType @ src/Reviewer.php 14:35-14:49 (full 14:35-14:49)
            and in the prose under it
            - parameter failed::processorType @ src/Reviewer.php 14:35-14:49 (full 14:35-14:49)
        "#]],
    );
}

/// Hover describes what goto lands on, at the same `$name`. It answered nothing while goto worked,
/// and a name that goes somewhere and describes nothing reads as broken.
#[test]
fn hover_on_a_docblock_parameter_name_describes_the_parameter() {
    check(
        r#"
//- /src/Reviewer.php
<?php
namespace App;

class Reviewer
{
    /**
     * @param string $proc@tag@essorType
     *   Which processor, as $proc@ose@essorType says.
     * @param string $mis@missing@sing
     */
    public function failed(string $processorType): bool
    {
        return $processorType === '';
    }
}
"#,
        &[
            Query::hover("on the tag's own name", "tag"),
            Query::hover("in the prose", "ose"),
            Query::hover("a name the signature does not declare", "missing"),
        ],
        expect![[r#"
            on the tag's own name
            - code: string $processorType
            in the prose
            - code: string $processorType
            a name the signature does not declare
            - (nothing to describe)
        "#]],
    );
}
