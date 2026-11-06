//! `@return V|false` is not `@return V|null`, and a branch reducing to one template keeps which.
//!
//! Both spellings bind `V` to the same thing and say different things about the result, so the
//! reduction that sets the sentinel aside used to fold them into one flag -- and `reset($rows)`,
//! whose stub says `V|false`, answered `?V`. The damage was not the tooltip: a guard written
//! `=== FALSE`, which is the one PHP makes true on an empty array, left the value nullable and every
//! finding standing, while `=== NULL`, which proves nothing, cleared them.
//!
//! Read here rather than at lowering because this layer is where a sentinel becomes a type: `null`
//! wraps into a nullable and `false` stands as a union operand, which is the same split the interned
//! union constructor makes.

use expect_test::expect;

use super::{Query, check_view};

/// One template bound by an array's elements, said three ways.
const HEADS: &str = r#"
//- /stubs/heads.php  stub
<?php
/**
 * @template V
 * @param array<array-key, V> $array
 * @return ($array is non-empty-array ? V : V|false)
 */
function head_or_false(array $array): mixed {}

/**
 * @template V
 * @param array<array-key, V> $array
 * @return ($array is non-empty-array ? V : V|null)
 */
function head_or_null(array $array): mixed {}

/**
 * @template V
 * @param array<array-key, V> $array
 * @return ($array is non-empty-array ? V : V|null|false)
 */
function head_or_either(array $array): mixed {}
"#;

#[test]
fn a_false_branch_is_a_union_and_not_a_nullable() {
    check_view(
        &[
            HEADS,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    /** @param Doc[] $docs */
    public function run(array $docs): void
    {
        $f@it@irst = head_or_false($docs);
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_type("what `V|false` holds", "it")],
        expect![[r#"
            what `V|false` holds
            - Doc|false
        "#]],
    );
}

#[test]
fn a_null_branch_is_still_a_nullable() {
    check_view(
        &[
            HEADS,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    /** @param Doc[] $docs */
    public function run(array $docs): void
    {
        $f@it@irst = head_or_null($docs);
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_type("what `V|null` holds", "it")],
        expect![[r#"
            what `V|null` holds
            - ?Doc
        "#]],
    );
}

/// The shape that makes one sentinel per branch too few: the DOM, mysqli and intl stubs each write
/// a `null|false` return, and dropping either half is a type narrower than the truth.
///
/// Rendered with the null as an operand rather than as a `?`, because PHP has no `?(A|B)` and
/// `Doc|false|null` is the only spelling of this type anybody can write. The null is still lifted
/// underneath -- `one_of` does that for every union -- so the reach check finds it.
#[test]
fn a_branch_admitting_both_keeps_both() {
    check_view(
        &[
            HEADS,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    /** @param Doc[] $docs */
    public function run(array $docs): void
    {
        $f@it@irst = head_or_either($docs);
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_type("what `V|null|false` holds", "it")],
        expect![[r#"
            what `V|null|false` holds
            - Doc|false|null
        "#]],
    );
}

/// A `false` beside a class still names that class, which is the half a reader asks for members.
///
/// Three readers reduce a union to one class -- the interned one, the written one and the resolved
/// one -- and each held a list with `null` alone in it. Until all three learned `false`, making the
/// type honest took every member on the result of an array-pointer call with it.
#[test]
fn a_false_beside_a_class_still_names_the_class() {
    check_view(
        &[
            HEADS,
            r#"
//- /src/Runner.php
<?php
namespace App;

class Doc
{
    public function title(): string
    {
        return '';
    }
}

class Runner
{
    /** @param Doc[] $docs */
    public function run(array $docs): void
    {
        $first = head_or_false($docs);
        $fi@it@rst;
    }
}
"#,
        ]
        .concat(),
        &[Query::variable_class(
            "the class beside the false",
            "it",
            "first",
        )],
        expect![[r#"
            the class beside the false
            - App\Doc
        "#]],
    );
}
