//! What is still worth saying about a file that does not parse.
//!
//! An editor's normal state is a file with a break in it, because that is what typing looks like.
//! The break decides how much of the rest is trustworthy: above it the text is exactly what was
//! written, below it recovery has skipped whatever it had to.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_finding_above_the_break_survives() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $repo->fnid(1);
        $oops = 1
    }
}
"#,
        expect![[r#"
            11:16-11:20 error [undefined-method] Undefined method Repo::fnid()
            13:5-13:6 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `RightBrace`
        "#]],
    );
}

#[test]
fn a_finding_below_the_break_does_not() {
    // Not because it is wrong, but because nothing here knows whether it is: the statement that
    // would have given `$repo` its type is one of the ones recovery skipped.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $oops = 1
        $repo->fnid(1);
    }
}
"#,
        expect![[r#"
            12:9-12:14 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `Variable`
        "#]],
    );
}

#[test]
fn a_member_declared_below_the_break_is_not_reported_missing() {
    // The shape the positional rule could have got wrong, and the reason it was measured rather
    // than argued: a class body's recovery keeps the members after the break, so a call above it
    // still finds them. If it did not, every method a person had not finished typing would report
    // every call to it.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): void
    {
        $this->later();
        $oops = 1
    }

    public function later(): void {}
}
"#,
        expect![[r#"
            8:5-8:6 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `RightBrace`
        "#]],
    );
}

#[test]
fn an_unused_variable_is_not_reported_in_a_broken_file() {
    // A variable is unused because nothing *anywhere* reads it, so skipped text reads as nothing
    // reading it. Position cannot make that safe -- the read may be below the break -- so the check
    // stands down for the whole file. This is the one finding a break can fabricate, and it did.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): void
    {
        $used = 1;
        $oops = 2
        echo $used;
    }
}
"#,
        expect![[r#"
            8:9-8:13 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `Echo`
        "#]],
    );
}
