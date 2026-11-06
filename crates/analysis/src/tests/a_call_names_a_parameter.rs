//! `$repo->find(id: 5)`: goto and hover at the label.
//!
//! The label names a parameter of whatever the call reaches. Goto goes to that declaration -- the
//! index keeps a signature's types and not where each parameter is written -- and hover shows the
//! parameter as declared and whose it is. A label naming no parameter of the callee answers nothing:
//! PHP stops on that call, and jumping would hide it.

use expect_test::expect;

use crate::tests::{Query, check};

const FIXTURE: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id, bool $fresh = false): ?string
    {
        return null;
    }
}

function go(Repo $repo): void
{
    $repo->find(1, fre@label@sh: true);
    $repo->find(1, sta@unknown@le: true);
}
"#;

#[test]
fn a_label_goes_to_and_describes_the_parameter_it_names() {
    check(
        FIXTURE,
        &[
            Query::goto("goto on a label", "label"),
            Query::hover("hover on a label", "label"),
            Query::goto("a label naming no parameter", "unknown"),
        ],
        expect![[r#"
            goto on a label
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-9:6)
            hover on a label
            - code: bool $fresh = false
            - note: parameter of App\Repo::find()
            a label naming no parameter
            - (nothing to go to)
        "#]],
    );
}
