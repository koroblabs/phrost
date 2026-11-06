//! The words of the language, where one may be written.
//!
//! Offered by what stands before the cursor and by nothing else. A list of keywords in a type
//! position is noise, and working out that `return` cannot go in a class body means reading back to
//! the nearest unmatched brace on every keystroke -- so the rule is the character before, and being
//! occasionally too generous is the price.

use expect_test::expect;

use super::{Query, check};

#[test]
fn keywords_where_a_statement_may_start_and_nowhere_else() {
    // Three files, because each marker leaves a dangling statement and a recovered parse drops
    // everything after the break -- two in one file would silently test only the first.
    //
    // The words are offered by what stands before the cursor, not by what would actually compile:
    // `return` in a class body is one row a person filters out with the next letter, and ruling it
    // out means reading back to the nearest unmatched brace on every keystroke.
    check(
        r#"
//- /body.php
<?php
class Protocol {}

class InAClassBody
{
    pro@in_class@
}
//- /statement.php
<?php
class InAFunction
{
    public function run(): void
    {
        ret@in_function@
    }
}
//- /hint.php
<?php
class InAReturnType
{
    public function typed(): Pro@in_type@
}
"#,
        &[
            Query::completion("in a class body", "in_class"),
            Query::completion("in a function body", "in_function"),
            Query::completion("in a return type", "in_type"),
        ],
        expect![[r#"
            in a class body
            - protected [keyword]
            - Protocol [class]  Protocol
            - ... and the client must ask again
            - accepting the first leaves `protected`
            in a function body
            - return [keyword]
            - InAReturnType [class]  InAReturnType
            - ... and the client must ask again
            - accepting the first leaves `return`
            in a return type
            - Protocol [class]  Protocol
            - ... and the client must ask again
            - accepting the first leaves `public function typed(): Protocol`
        "#]],
    );
}
