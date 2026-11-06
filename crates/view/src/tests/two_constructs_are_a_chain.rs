//! Two constructs that may each have written a name, with a guard between them.
//!
//! Read as one join, the value before *everything* is what the name was initialised with, and any
//! guard written between the two constructs sits where nothing looks: the reported shape is a
//! `NULL` init filled in a loop, a `throw` ruling the null out, and a branch replacing it -- which
//! answered `Doc|Doc|null` for a value that is a `Doc` on every path that reaches it.
//!
//! Read as a chain, the second construct joins what it wrote with whatever the name held *just
//! before it*, which is the first join with the guard applied.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_guard_between_two_constructs_is_not_skipped_over() {
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Runner
{
    private bool $flag = FALSE;

    public function make(): Doc
    {
    }

    public function go(array $items): void
    {
        $found = NULL;
        foreach ($items as $item) {
            if ($item instanceof Doc) {
                $found = $item;
                break;
            }
        }
        if (!$found) {
            throw new \RuntimeException('missing');
        }
        if ($this->flag) {
            $found = $this->make();
        }
        $fo@after@und;
    }
}
"#,
        &[Query::variable_type("after both constructs", "after")],
        expect![[r#"
            after both constructs
            - Doc
        "#]],
    );
}

/// The other side of the chain: with nothing ruling the null out, it is still one of the answers,
/// and so is what the *earlier* construct wrote.
///
/// Without this the fixture above would pass just as well if the join had stopped reading what the
/// name held before the last branch at all. Two different classes on purpose -- operands that all
/// name one class collapse to it, which would hide both halves of what this is asking.
#[test]
fn what_the_name_held_before_the_last_construct_is_still_joined() {
    check_view(
        r#"
//- /src/Runner.php
<?php
namespace App;

class Doc {}

class Other {}

class Runner
{
    private bool $flag = FALSE;

    public function other(): Other
    {
    }

    public function go(array $items): void
    {
        $found = NULL;
        foreach ($items as $item) {
            if ($item instanceof Doc) {
                $found = $item;
                break;
            }
        }
        if ($this->flag) {
            $found = $this->other();
        }
        $fo@after@und;
    }
}
"#,
        &[Query::variable_type("no guard between them", "after")],
        expect![[r#"
            no guard between them
            - Other|Doc|null
        "#]],
    );
}
