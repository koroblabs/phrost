//! Goto on a variable that a write created rather than declared.
//!
//! Which write goto lands on is a claim about which one brought the variable into being. The
//! comparison against another server caught this the hard way: taking the most recent write of any
//! kind sent goto to a `$rows[] = ...` mutation while a plain `$rows = [...]` sat above it.

use expect_test::expect;

use super::{Query, check};

#[test]
fn goto_prefers_the_assignment_over_a_later_element_write() {
    check(
        r#"
//- /src/Build.php
<?php
namespace App;

class Build
{
    public function rows(bool $extra): array
    {
        $rows = ['first'];
        if ($extra) {
            $rows[] = 'second';
        }
        return $ro@use@ws;
    }
}
"#,
        &[Query::goto("declared, not mutated", "use")],
        expect![[r#"
            declared, not mutated
            - variable rows @ src/Build.php 8:9-8:14 (full 8:9-8:14)
        "#]],
    );
}

#[test]
fn goto_reaches_a_variable_only_an_element_write_created() {
    // No plain assignment anywhere, so the element write is where it came from -- and the first one
    // is the write that created it.
    check(
        r#"
//- /src/Props.php
<?php
namespace App;

class Props
{
    public function build(): array
    {
        $props['type'] = 'string';
        $props['label'] = 'Label';
        return $pro@use@ps;
    }
}
"#,
        &[Query::goto("created by the first write", "use")],
        expect![[r#"
            created by the first write
            - variable props @ src/Props.php 8:9-8:23 (full 8:9-8:23)
        "#]],
    );
}

#[test]
fn goto_reaches_a_loop_variable_bound_by_reference() {
    check(
        r#"
//- /src/Touch.php
<?php
namespace App;

class Touch
{
    public function all(array $rows): void
    {
        foreach ($rows as &$row) {
            $total = $r@use@ow;
        }
    }
}
"#,
        &[Query::goto("bound through a reference", "use")],
        expect![[r#"
            bound through a reference
            - variable row @ src/Touch.php 8:27-8:32 (full 8:27-8:32)
        "#]],
    );
}

#[test]
fn goto_reaches_a_variable_a_static_or_global_declared() {
    // Neither is a write, so every binding search was blind to them: the declaration itself had no
    // node at all, and a read above the first write -- the `empty($x)` guard that is the whole
    // idiom -- had nothing to resolve to.
    check(
        r#"
//- /src/Options.php
<?php
namespace App;

class Options
{
    public function all(): array
    {
        static $op@declaration@tions;
        if (empty($op@guard@tions)) {
            $options['a'] = 1;
        }
        return $op@after@tions;
    }

    public function url(): string
    {
        global $base@global@_url;
        return (string) $base_url;
    }
}
"#,
        &[
            Query::goto("the declaration itself", "declaration"),
            Query::goto("a read above the first write", "guard"),
            Query::goto("a read after it", "after"),
            Query::goto("a global", "global"),
        ],
        expect![[r#"
            the declaration itself
            - variable options @ src/Options.php 8:16-8:24 (full 8:16-8:24)
            a read above the first write
            - variable options @ src/Options.php 8:16-8:24 (full 8:16-8:24)
            a read after it
            - variable options @ src/Options.php 8:16-8:24 (full 8:16-8:24)
            a global
            - variable base_url @ src/Options.php 17:16-17:25 (full 17:16-17:25)
        "#]],
    );
}

#[test]
fn goto_reaches_the_first_binding_and_not_the_last_write_before_it() {
    // PHP has no block scope, so the write inside the loop is a reassignment rather than a second
    // declaration -- and "where does this variable come from" is answered by where it came into
    // being. Both servers phrost is compared against say the same, and disagreed with us on twenty
    // positions until this.
    check(
        r#"
//- /src/Pick.php
<?php
namespace App;

class Pick
{
    public function run(array $terms): ?int
    {
        $eligible = NULL;
        foreach ($terms as $option) {
            $eligible = $option;
        }
        return $eli@use@gible;
    }
}
"#,
        &[Query::goto("the first binding", "use")],
        expect![[r#"
            the first binding
            - variable eligible @ src/Pick.php 8:9-8:18 (full 8:9-8:18)
        "#]],
    );
}

#[test]
fn each_catch_clause_binds_its_own_variable() {
    // What makes the first-binding rule safe: the candidates are filtered before they are ranked,
    // so a `$e` in the second clause does not resolve to the first clause's.
    check(
        r#"
//- /src/Two.php
<?php
namespace App;

class Two
{
    public function run(): void
    {
        try {
            $a = 1;
        } catch (\RuntimeException $e) {
            echo $e@first@->getMessage();
        }
        try {
            $b = 2;
        } catch (\LogicException $e) {
            echo $e@second@->getMessage();
        }
    }
}
"#,
        &[
            Query::goto("the first clause", "first"),
            Query::goto("the second clause", "second"),
        ],
        expect![[r#"
            the first clause
            - variable e @ src/Two.php 10:36-10:38 (full 10:36-10:38)
            the second clause
            - variable e @ src/Two.php 15:34-15:36 (full 15:34-15:36)
        "#]],
    );
}
