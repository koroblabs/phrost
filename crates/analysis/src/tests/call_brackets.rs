//! The brackets accepting a method writes.
//!
//! A name somebody then has to add brackets to is half an answer, and the cursor's place is the other
//! half: between them when there is an argument to type, after them when there is not.
//!
//! The decision is here and the encoding is the protocol's, which is why these tests render the
//! decision. A test that showed only the label would pass while every method completed to a bare
//! name -- which is how it was until somebody said so.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_method_with_parameters_puts_the_cursor_between_the_brackets() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
    public function all(): array
    {
        return [];
    }
    public int $size = 0;
    public const string VERSION = '1';
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->@here@
    }
}
"#,
        &[Query::completion("what each one writes", "here")],
        expect![[r#"
            what each one writes
            - all [method]  : array  writes `();`
            - find [method]  : void  writes `(|);`
            - size [property]  int
            - accepting the first leaves `$repo->all`
        "#]],
    );
}

#[test]
fn brackets_already_written_are_not_written_again() {
    // Re-typing the name in `$repo->find(1)` must not leave `find()(1)`. The one shape where adding
    // what is missing produces something worse than what was there.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->fi@here@nd(1);
    }
}
"#,
        &[Query::completion("already a call", "here")],
        expect![[r#"
            already a call
            - find [method]  : void  writes `(|)`
            - ... and the client must ask again
            - accepting the first leaves `$repo->findnd(1);`
        "#]],
    );
}

#[test]
fn a_property_and_a_constant_write_no_brackets() {
    // Not a call, so there is nothing to write -- and `$repo->size()` would be a different thing
    // entirely.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public int $size = 0;
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->si@here@
    }
}
"#,
        &[Query::completion("a property", "here")],
        expect![[r#"
            a property
            - size [property]  int
            - ... and the client must ask again
            - accepting the first leaves `$repo->size`
        "#]],
    );
}
