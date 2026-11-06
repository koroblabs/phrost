//! `@see` as a name reference.
//!
//! The tag points at a class, a function or a constant, and it is the tag a framework's own source
//! uses to say "this only makes sense beside that one" -- Drupal writes it 11 435 times in code a
//! reader jumps into. Like `@throws` it is the type *of* nothing, so it shows up in no declaration
//! snapshot and what it must produce is a reference: goto lands on that, find-references counts it,
//! and the unused-import check reads it as a mention.
//!
//! # Why the assertions are about ranges
//!
//! Because most of the rules here are about where a reference stops. `@see Foo::bar()` names the
//! class and must not cover the member; a URL is not a name at all; and the description after the
//! reference is prose. Each of those is a range, so the snapshot renders both ends.

use expect_test::expect;

use super::check_name_refs;

#[test]
fn the_class_a_see_names_is_recorded() {
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

/**
 * @see \App\Ledger
 */
class Repo
{
    /**
     * @see Nearby the one next door.
     */
    public function find(): void {}
}
"#,
        expect![[r#"
            App\Ledger  [ClassLike] @ 4:9-4:20  [doc]
            Nearby  [ClassLike] @ 9:13-9:19  [doc]
        "#]],
    );
}

#[test]
fn a_member_reference_stops_before_the_member() {
    // The range is what a cursor is tested against. Covering `::id()` would send a click on the
    // member to the class, which is an answer to a question nobody asked.
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

/**
 * @see \App\Ledger::id()
 */
class Repo
{
}
"#,
        expect![[r#"
            App\Ledger  [ClassLike] @ 4:9-4:20  [doc]
        "#]],
    );
}

#[test]
fn parentheses_alone_name_a_function_and_shouting_names_a_constant() {
    // Three tables, and the kind has to be chosen here because a lookup is asked for one of them.
    // The underscore is what separates a constant from a class somebody chose to call `URL`.
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

/**
 * @see helper()
 * @see JSON_ERROR_NONE
 * @see URL
 */
class Repo
{
}
"#,
        expect![[r#"
            helper  [Function] @ 4:9-4:15  [doc]
            JSON_ERROR_NONE  [Constant] @ 5:9-5:24  [doc]
            URL  [ClassLike] @ 6:9-6:12  [doc]
        "#]],
    );
}

#[test]
fn what_is_not_a_reference_records_nothing() {
    // A URL is the other thing this tag holds -- 2 358 of Drupal's own -- and prose written where a
    // reference goes is indistinguishable from a class name without resolving, so a token PHP could
    // not spell is what gets refused rather than a token that names nothing.
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

/**
 * @see https://www.drupal.org/node/1884796
 * @see $someVariable
 * @see 2things
 */
class Repo
{
}
"#,
        expect![[r#"
            <no name references>
        "#]],
    );
}

#[test]
fn an_inline_see_is_read_in_prose_and_inside_another_tags_description() {
    // Sixty per cent of the inline form is written inside a tag's description, where mago hands the
    // text back inside whichever of its sixty tag values that tag is. Reading the value back through
    // the parser reaches every one of them; a match over the variants that carry a description
    // would answer for the twenty-five that have one today and silently miss the next.
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

class Repo
{
    /**
     * Encodes, see {@see Ledger} for the reverse.
     *
     * @param int $flags
     *   Bitmask of options. Default: {@see JSON_ERROR_NONE}
     */
    public function encode(int $flags): void {}
}
"#,
        expect![[r#"
            Ledger  [ClassLike] @ 6:27-6:33  [doc]
            JSON_ERROR_NONE  [Constant] @ 9:45-9:60  [doc]
        "#]],
    );
}
