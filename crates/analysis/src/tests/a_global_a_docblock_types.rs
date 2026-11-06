//! `@global wpdb $wpdb` above `global $wpdb;`.
//!
//! phpDocumentor's tag for the one binding PHP has no syntax to type: a `global` statement names a
//! variable the whole program shares, and nothing in the function says what it holds. WordPress
//! writes the tag **599 times** for exactly that, and `$wpdb`, `$post` and `$comment` are the three
//! commonest untyped receivers in that entire checkout -- `$wpdb->prepare` alone is 333 accesses.
//!
//! It says precisely what `@var` says in the same place, and mago's docblock parser has no
//! `@global`: the tag is phpDocumentor's rather than one of the static-analysis vocabularies it
//! models, so it arrived as text and nothing read it. It is rewritten into the `@var` it means,
//! padded to the same length so that every range downstream -- a goto on the type, an
//! `undefined-type` report -- still points where it did.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_global_the_docblock_types_answers_like_any_other_binding() {
    check(
        r#"
//- /src/db.php
<?php

class wpdb
{
    public function prepare(string $query): string
    {
        return $query;
    }
}

//- /src/run.php
<?php

/**
 * @global wpdb $wpdb WordPress database abstraction object.
 */
function above(): void
{
    global $wpdb;
    $a@above@bove = $wpdb->prepare('SELECT 1');
}

function inline(): void
{
    /**
     * @global wpdb $wpdb WordPress database abstraction object.
     */
    global $wpdb;
    $i@inline@nside = $wpdb->prepare('SELECT 1');
}

function untagged(): void
{
    global $other;
    $n@none@othing = $other;
}
"#,
        &[
            Query::hover("the tag above the function", "above"),
            Query::hover("the tag above the statement", "inline"),
            Query::hover("a global nothing types", "none"),
        ],
        expect![[r#"
            the tag above the function
            - code: string $above
            - note: assigned
            the tag above the statement
            - code: string $inside
            - note: assigned
            a global nothing types
            - code: $nothing
            - note: assigned
            - note: no declared type
        "#]],
    );
}

/// `@globally` is a different word, and the substitution is length-preserving: the ranges a type's
/// name reference carries are byte offsets into the docblock's own text.
#[test]
fn a_word_beginning_with_the_tag_is_left_alone() {
    check(
        r#"
//- /src/run.php
<?php

class Thing {}

/**
 * Applies @globally, which is prose and not a tag.
 *
 * @var Thing $kept
 */
function prose(): void
{
    global $kept;
    $t@thing@hing = $kept;
}
"#,
        &[Query::hover("the @var beside the prose", "thing")],
        expect![[r#"
            the @var beside the prose
            - code: Thing $thing
            - note: assigned
        "#]],
    );
}

/// A method's docblock, which is where WordPress writes the tag inside a class. A function's reached
/// the binding only because a top-level function is a statement of the file, and the file's
/// annotations are what `global` reads -- a class member is not one, so every `global $wpdb;` in a
/// method stayed untyped: a percentage point of the whole WordPress checkout.
#[test]
fn a_global_a_method_docblock_types_answers_too() {
    check(
        r#"
//- /src/db.php
<?php

class wpdb
{
    public function prepare(string $query): string
    {
        return $query;
    }
}

//- /src/run.php
<?php

class Debug
{
    /**
     * Returns the value of a variable.
     *
     * @global wpdb $wpdb WordPress database abstraction object.
     *
     * @param string $name Name of the variable.
     */
    public static function variable(string $name): void
    {
        global $wpdb;
        $m@method@ethod = $wpdb->prepare($name);
    }
}
"#,
        &[Query::hover("the tag above a method", "method")],
        expect![[r#"
            the tag above a method
            - code: string $method
            - note: assigned
        "#]],
    );
}
