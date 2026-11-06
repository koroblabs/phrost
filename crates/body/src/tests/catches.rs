//! What a `catch` clause gives its variable, and for how long.
//!
//! Scoped to the clause, unlike a parameter: treating it as scope-wide would answer about it from
//! code that runs before the `try`.

use expect_test::expect;

use super::check;

#[test]
fn a_catch_clause_binds_its_variable_for_its_own_block() {
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        try {
            $this->go();
        } catch (\RuntimeException $error) {
            $err$0or;
        }
    }
}
"#,
        "error",
        // Without the leading separator: the HIR stores the written path with it stripped, and the
        // anchor -- fully qualified or not -- is a separate fact this rendering does not show.
        expect!["RuntimeException"],
    );
}

#[test]
fn a_catch_variable_does_not_leak_out_of_its_block() {
    // PHP leaves it defined after the block, but its type there is whatever the last binding gave
    // it -- and before the try it is not bound at all, which is what this asks about.
    check(
        r#"<?php
class Repo
{
    public function run(): void
    {
        $err$0or;
        try {
        } catch (\RuntimeException $error) {
        }
    }
}
"#,
        "error",
        expect!["<not declared here>"],
    );
}
