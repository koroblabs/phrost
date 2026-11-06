//! Goto on a name `[$a, $b] = $row` brought into being.
//!
//! A destructuring target writes through no variable of its own, so `bound_by` -- which answers
//! *what does this write write through* -- has nothing to say about it. The type half of this was
//! built when destructuring was modelled, and the other place a local lives was not: every name
//! one of these binds had no declaration at all, so goto answered nothing on the use **and** on
//! the binding, and `documentHighlight`, `references`, `rename` and `typeDefinition` were dead
//! with it. §8.1 calls the form "a local exists in two places"; this is its fifth instance.
//!
//! Found by the comparison against another server, which is what that instrument is for: three of
//! its six remaining goto losses were one `['item_id' => $id, ...] = $row->getSource();` in a
//! contrib module.

use expect_test::expect;

use super::{Query, check};

#[test]
fn goto_on_a_positionally_destructured_name_lands_on_its_element() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    public function run(array $row): void
    {
        [$first, $second] = $row;
        echo $sec@use@ond;
    }
}
"#,
        &[Query::goto("the second name", "use")],
        expect![[r#"
            the second name
            - variable second @ src/Rows.php 8:18-8:25 (full 8:18-8:25)
        "#]],
    );
}

#[test]
fn goto_on_a_keyed_destructured_name_lands_on_its_own_element_and_not_the_first() {
    // The shape a real migration writes, over several lines and keyed. Each name takes its own
    // slot, so each one's binding is its own element -- landing on the target as a whole would be
    // four names sharing one declaration.
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    public function run(array $row): void
    {
        [
            'id' => $id,
            'name' => $name,
        ] = $row;
        echo $na@use@me;
    }
}
"#,
        &[Query::goto("the keyed name", "use")],
        expect![[r#"
            the keyed name
            - variable name @ src/Rows.php 10:23-10:28 (full 10:23-10:28)
        "#]],
    );
}

#[test]
fn a_target_inside_a_target_binds_every_name_in_it() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    public function run(array $rows): void
    {
        [[$left, $right], $rest] = $rows;
        echo $rig@use@ht;
    }
}
"#,
        &[Query::goto("the nested name", "use")],
        expect![[r#"
            the nested name
            - variable right @ src/Rows.php 8:18-8:24 (full 8:18-8:24)
        "#]],
    );
}

/// A `foreach` writes its target the same way: `foreach ($cases as [$handler, $matching])` binds
/// through no variable of its own, so hover named the loop variable and goto went nowhere. Found by
/// the comparison against another server on the reference project's own tests.
#[test]
fn a_foreach_destructuring_binds_each_name() {
    check(
        r#"
//- /src/Cases.php
<?php
namespace App;

function run(array $cases): void
{
    foreach ($cases as [$handler, $matching]) {
        $matching;
        $hand@here@ler;
    }
}
"#,
        &[Query::goto("the second of two", "here")],
        expect![[r#"
            the second of two
            - variable handler @ src/Cases.php 6:25-6:33 (full 6:25-6:33)
        "#]],
    );
}
