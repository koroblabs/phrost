//! The `;` accepting a method writes, and the many places it must not.
//!
//! A call somebody then has to terminate is half an answer -- but a `;` written where one does not
//! belong is worse than none at all, because it makes the buffer invalid and is deleted by hand
//! every single time. So the interesting tests here are the refusals: an assignment, a condition, a
//! `return`, an argument, a chain that carries on, and a line already terminated.
//!
//! Rendered by the harness as the trailing `;` of `writes `();``, because the decision is this
//! layer's and only the encoding is the protocol's.

use expect_test::expect;

use super::{Query, check};

#[test]
fn an_access_that_is_the_whole_statement_writes_the_semicolon() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function run(): void {}
    public function find(int $id): void {}
    public int $size = 0;
}

class Caller
{
    public function go(Repo $repo): void
    {
        $repo->@here@
    }
}
"#,
        &[Query::completion("a statement of its own", "here")],
        expect![[r#"
            a statement of its own
            - find [method]  : void  writes `(|);`
            - run [method]  : void  writes `();`
            - size [property]  int
            - accepting the first leaves `$repo->find`
        "#]],
    );
}

#[test]
fn an_assignment_writes_no_semicolon() {
    // `$x = $repo->run();` is a statement, but the `=` means the access is not the whole of it --
    // and the `;` this would add lands in the middle of the line the person is still writing.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function run(): void {}
}

class Caller
{
    public function go(Repo $repo): void
    {
        $result = $repo->@here@
    }
}
"#,
        &[Query::completion("the right of an assignment", "here")],
        expect![[r#"
            the right of an assignment
            - run [method]  : void  writes `()`
            - accepting the first leaves `$result = $repo->run`
        "#]],
    );
}

#[test]
fn a_condition_writes_no_semicolon() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function ready(): bool { return true; }
}

class Caller
{
    public function go(Repo $repo): void
    {
        if ($repo->@here@) {
        }
    }
}
"#,
        &[Query::completion("inside an if", "here")],
        expect![[r#"
            inside an if
            - ready [method]  : bool  writes `()`
            - accepting the first leaves `if ($repo->ready) {`
        "#]],
    );
}

#[test]
fn a_return_writes_no_semicolon() {
    // The keyword left of the receiver is what says so. `return` is not a place a statement begins,
    // it is a place one is already under way.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function name(): string { return ''; }
}

class Caller
{
    public function go(Repo $repo): string
    {
        return $repo->@here@
    }
}
"#,
        &[Query::completion("after return", "here")],
        expect![[r#"
            after return
            - name [method]  : string  writes `()`
            - accepting the first leaves `return $repo->name`
        "#]],
    );
}

#[test]
fn an_argument_writes_no_semicolon() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function name(): string { return ''; }
}

class Caller
{
    public function go(Repo $repo): void
    {
        \strlen($repo->@here@);
    }
}
"#,
        &[Query::completion("inside a call", "here")],
        expect![[r#"
            inside a call
            - name [method]  : string  writes `()`
            - accepting the first leaves `\strlen($repo->name);`
        "#]],
    );
}

#[test]
fn a_chain_carried_onto_the_next_line_writes_no_semicolon() {
    // The line under the cursor is blank to its end, so only what comes after settles it: an
    // operator continues the expression, and a `;` in front of one is a syntax error.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function self(): Repo { return $this; }
}

class Caller
{
    public function go(Repo $repo): void
    {
        $repo->@here@
            ->self();
    }
}
"#,
        &[Query::completion("mid-chain", "here")],
        expect![[r#"
            mid-chain
            - self [method]  : Repo  writes `()`
            - accepting the first leaves `$repo->self`
        "#]],
    );
}

#[test]
fn a_line_already_terminated_writes_no_second_semicolon() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function run(): void {}
}

class Caller
{
    public function go(Repo $repo): void
    {
        $repo->@here@;
    }
}
"#,
        &[Query::completion("a semicolon is already there", "here")],
        expect![[r#"
            a semicolon is already there
            - run [method]  : void  writes `()`
            - accepting the first leaves `$repo->run;`
        "#]],
    );
}

#[test]
fn a_word_operator_after_the_cursor_writes_no_semicolon() {
    // `$repo->ready() or throw ...` reads as a new statement to anything that only looks at the
    // first character, and PHP's word-spelled operators are the one thing that carries an
    // expression across a line break.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function ready(): bool { return true; }
}

class Caller
{
    public function go(Repo $repo): void
    {
        $repo->@here@
        or \exit(1);
    }
}
"#,
        &[Query::completion("an or follows", "here")],
        expect![[r#"
            an or follows
            - ready [method]  : bool  writes `()`
            - accepting the first leaves `$repo->ready`
        "#]],
    );
}

#[test]
fn a_trailing_comment_writes_no_semicolon() {
    // The `;` belongs before the comment and this scanner cannot say where that is, so it says
    // nothing -- the refusal a half-read line always gets here.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function run(): void {}
}

class Caller
{
    public function go(Repo $repo): void
    {
        $repo->@here@ // finish this
    }
}
"#,
        &[Query::completion("a comment follows", "here")],
        expect![[r#"
            a comment follows
            - run [method]  : void  writes `()`
            - accepting the first leaves `$repo->run // finish this`
        "#]],
    );
}

#[test]
fn a_static_call_that_is_the_whole_statement_writes_the_semicolon() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public static function boot(): void {}
    public const string VERSION = '1';
}

class Caller
{
    public function go(): void
    {
        Repo::@here@
    }
}
"#,
        &[Query::completion("through the double colon", "here")],
        expect![[r#"
            through the double colon
            - boot [method]  : void  writes `();`
            - VERSION [const]  string
            - class [const]  Repo
            - accepting the first leaves `Repo::boot`
        "#]],
    );
}

#[test]
fn a_receiver_nothing_can_read_offers_nothing_to_terminate() {
    // The head this decision reads is the text left of the receiver's *root*, and an expression with
    // no root readable here has none. Nothing is offered either way, which is the point: the two
    // refusals have the same cause.
    //
    // A parenthesised expression, because a function call stopped being one of these: `make()->` is
    // a root now, looked up in the function table. What is left unreadable is an expression whose
    // type is written nowhere near the cursor.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function run(): void {}
}

class Caller
{
    private Repo $repo;

    public function go(): void
    {
        ($this->repo ?? $this->repo)->@here@
    }
}
"#,
        &[Query::completion("an unreadable receiver", "here")],
        expect![[r#"
            an unreadable receiver
            - (nothing offered)
        "#]],
    );
}
