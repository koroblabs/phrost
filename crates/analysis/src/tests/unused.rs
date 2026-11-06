//! A local nothing ever reads.
//!
//! Almost entirely about the silences. Every one of them is a way PHP can read a variable that a
//! naive walk would not see, and each was found by running the check over code that demonstrably
//! works -- Symfony's own source, where every finding is a statement about the check.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_assignment_nothing_reads() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): int
    {
        $used = 1;
        $unused = $this->work();
        return $used;
    }

    private function work(): int
    {
        return 2;
    }
}
"#,
        expect![[r#"
            7:9-7:16 warning [unused-variable] $unused is assigned and never read
        "#]],
    );
}

#[test]
fn every_way_php_reads_one() {
    // Inside a string, as a closure capture, through an arrow function, and as the name of a member
    // computed at runtime. Miss any of them and the check reports working code.
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function count(\Countable|array $value): int {}

//- /app.php
<?php
class Service
{
    public function run(): void
    {
        $interpolated = 1;
        $captured = 2;
        $inherited = 3;
        $method = 'work';
        $element = [];

        echo "value $interpolated";
        $closure = function () use ($captured) {
            return $captured;
        };
        $arrow = fn () => $inherited;
        $this->{$method}();
        $element['key'] = 1;
        echo $closure() + $arrow() + count($element);
    }

    private function work(): void {}
}
"#,
        expect![""],
    );
}

#[test]
fn a_parameter_is_never_reported() {
    // Sometimes it is not even true that it is unused: `&$value` exists to be written, and the
    // caller is the reader.
    check(
        r#"
//- /app.php
<?php
function alter(float &$value, int $ignored): void
{
    $value = 1.0;
}
"#,
        expect![""],
    );
}

#[test]
fn an_alias_is_the_whole_point_of_itself() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    private array $rows = [];

    public function run(): void
    {
        $shared = &$this->rows;
        $shared = [1];
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_scope_that_could_be_reading_a_name_it_never_wrote() {
    // Four escape hatches, each in its own function so one silence cannot hide another. `compact`
    // and `extract` name a variable with a string, an `include` hands the whole scope to another
    // file, and `$$name` reaches a variable whose name is a value.
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function compact(array|string $var_name): array {}
function extract(array &$array): int {}

//- /app.php
<?php
class Service
{
    public function by_compact(): array
    {
        $rows = [1];
        return compact('rows');
    }

    public function by_extract(array $data): void
    {
        $rows = [1];
        extract($data);
    }

    public function by_include(): void
    {
        $rows = [1];
        include 'template.php';
    }

    public function by_indirect(string $which): void
    {
        $rows = [1];
        echo $$which;
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_files_own_top_level_belongs_to_whoever_included_it() {
    // Drupal's `settings.php` is nothing but this shape.
    check(
        r#"
//- /settings.php
<?php
$databases = [];
$config['x'] = 1;
"#,
        expect![""],
    );
}

#[test]
fn a_closure_writing_a_captured_name_is_the_outer_variable() {
    // `use (&$seen)` makes the closure's `$seen` the enclosing scope's, so the write belongs to the
    // outer variable -- and asking about it inside the closure would see the write and none of the
    // reads.
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): array
    {
        $seen = [];
        $note = function (int $row) use (&$seen): void {
            $seen[] = $row;
        };
        $note(1);
        return $seen;
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_compact_that_spells_its_names_reaches_only_those() {
    // Every string is an occurrence of its variable, so a scope whose `compact()` names everything
    // it collects hides nothing -- and the local it leaves out is as unused as anywhere else. A
    // name `compact()` is handed as a value is still a silence.
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function compact(array|string $var_name, array|string ...$var_names): array {}

//- /app.php
<?php
class Service
{
    public function spelled(): array
    {
        $rows = [1];
        $count = 2;
        $left = 3;
        return compact('rows', 'count');
    }

    public function named_by_a_value(string $which): array
    {
        $rows = [1];
        return compact($which);
    }
}
"#,
        expect![[r#"
            8:9-8:14 warning [unused-variable] $left is assigned and never read
        "#]],
    );
}
