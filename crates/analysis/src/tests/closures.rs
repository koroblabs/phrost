//! Variables written inside a closure.
//!
//! A file of its own because the failure was invisible from the outside: the member call resolved,
//! so the receiver's type was clearly known, and only the variable under the cursor found nothing.
//! Two walks reach a body — one for scope rules, which stops at a closure because from outside a
//! closure is not a scope a name resolves against, and one for position lookups, which must not.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn every_variable_written_inside_a_closure_is_reachable() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function each(array $rows, Repo $repo): void
    {
        array_map(function (Repo $one) use ($re@capture@po) {
            $o@param@ne->find();
            $re@captured@po->find();
        }, $rows);

        $fn = fn(Repo $two) => $t@arrow@wo->find();

        usort($rows, function ($a, $b) use ($repo) {
            $held = $re@nested@po;
            return 0;
        });
    }
}
"#,
        &[
            Query::goto("the use () capture", "capture"),
            Query::goto("the closure's own parameter", "param"),
            Query::goto("a captured variable in the body", "captured"),
            Query::goto("an arrow function's parameter", "arrow"),
            Query::goto("a capture two closures down", "nested"),
        ],
        expect![[r#"
            the use () capture
            - parameter repo @ src/Uses.php 6:44-6:49 (full 6:44-6:49)
            the closure's own parameter
            - parameter one @ src/Uses.php 8:34-8:38 (full 8:34-8:38)
            a captured variable in the body
            - parameter repo @ src/Uses.php 6:44-6:49 (full 6:44-6:49)
            an arrow function's parameter
            - parameter two @ src/Uses.php 13:23-13:27 (full 13:23-13:27)
            a capture two closures down
            - parameter repo @ src/Uses.php 6:44-6:49 (full 6:44-6:49)
        "#]],
    );
}

#[test]
fn hover_describes_a_variable_inside_a_closure() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(array $rows): void
    {
        array_map(function (Repo $one) {
            $o@here@ne->find();
        }, $rows);
    }
}
"#,
        &[Query::hover("inside a closure", "here")],
        expect![[r#"
            inside a closure
            - code: Repo $one
            - note: parameter
        "#]],
    );
}
