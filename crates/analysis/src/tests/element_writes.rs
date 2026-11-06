//! What was written into one slot of an array.
//!
//! An array built a slot at a time is the one shape no declaration describes: `$rows = []` says only
//! that it is an array, and every word about what it holds is in the writes that follow. Which is
//! most arrays in PHP.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_append_says_what_any_slot_holds() {
    check(
        r#"
//- /src/append.php
<?php
namespace App;

class User
{
    public function getName(): string {}
}

class Runner
{
    public function run(User $user): void
    {
        $rows = [];
        $rows[] = $user;
        $rows[0]->@appended@
    }
}
"#,
        &[Query::completion("read back by index", "appended")],
        expect![[r#"
            read back by index
            - getName [method]  : string  writes `();`
            - accepting the first leaves `$rows[0]->getName`
        "#]],
    );
}

#[test]
fn an_append_inside_a_finished_loop_still_counts() {
    check(
        r#"
//- /src/loop.php
<?php
namespace App;

class User
{
    public function getName(): string {}
}

class Runner
{
    /** @param User[] $users */
    public function run(array $users): void
    {
        $filtered = [];
        foreach ($users as $user) {
            $filtered[] = $user;
        }
        $filtered[0]->@filtered@
    }
}
"#,
        &[Query::completion("filled inside a loop", "filtered")],
        expect![[r#"
            filled inside a loop
            - getName [method]  : string  writes `();`
            - accepting the first leaves `$filtered[0]->getName`
        "#]],
    );
}

#[test]
fn a_keyed_write_says_what_that_key_holds_and_no_other() {
    check(
        r#"
//- /src/keyed.php
<?php
namespace App;

class Config
{
    public function getName(): string {}
}

class Runner
{
    public function run(?object $node): void
    {
        $options = [
            'name' => 'value',
        ];

        if ($node !== null) {
            $options['config'] = new Config();
        }

        $options['config']->@same@
        $options['other']->@other@
    }
}
"#,
        &[
            Query::completion("the key that was written", "same"),
            Query::completion("a key nothing wrote", "other"),
        ],
        expect![[r#"
            the key that was written
            - getName [method]  : string  writes `();`
            - accepting the first leaves `$options['config']->getName`
            a key nothing wrote
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_variable_key_fills_whichever_slot_is_read() {
    check(
        r#"
//- /src/variable.php
<?php
namespace App;

class Result
{
    public function getName(): string {}
}

class Runner
{
    /** @param string[] $files */
    public function run(array $files): void
    {
        $results = [];
        foreach ($files as $file) {
            $results[$file] = new Result();
        }
        $results['foo']->@any@
    }
}
"#,
        &[Query::completion("a slot nothing can name", "any")],
        expect![[r#"
            a slot nothing can name
            - getName [method]  : string  writes `();`
            - accepting the first leaves `$results['foo']->getName`
        "#]],
    );
}

#[test]
fn a_write_into_one_array_says_nothing_about_another() {
    check(
        r#"
//- /src/other.php
<?php
namespace App;

class User
{
    public function getName(): string {}
}

class Runner
{
    public function run(User $user): void
    {
        $rows = [];
        $rows[] = $user;
        $other = [];
        $other[0]->@other@
    }
}
"#,
        &[Query::completion("a different container", "other")],
        expect![[r#"
            a different container
            - (nothing offered)
        "#]],
    );
}

/// A container is often a slot itself, so the writes are read at every step rather than at the last.
#[test]
fn a_slot_that_holds_a_shape_is_read_through() {
    check(
        r#"
//- /src/nested.php
<?php
namespace App;

class Decimal
{
    public function add(Decimal $other): Decimal {}
}

class Aggregator
{
    /** @return array{paid: Decimal, earned: Decimal} */
    private function structure(): array {}

    /** @param list<int> $ids */
    public function run(array $ids): void
    {
        $sums = [];
        foreach ($ids as $id) {
            $sums[$id] = $this->structure();
            $sums[$id]['paid']->@nested@
        }
    }
}
"#,
        &[Query::completion("a slot inside a slot", "nested")],
        expect![[r#"
            a slot inside a slot
            - add [method]  : Decimal  writes `(|);`
            - accepting the first leaves `$sums[$id]['paid']->add`
        "#]],
    );
}

/// A key nothing here can name reaches every slot at once, so a shape answers with every value it
/// declares -- and with the one value when they all declare the same.
#[test]
fn a_computed_key_on_a_shape_is_one_of_its_values() {
    check(
        r#"
//- /src/kit.php
<?php
namespace App;

class Pen
{
    public function write(): void {}
}

class Pencil
{
    public function sketch(): void {}
}

class Kit
{
    /** @return array{pen: Pen, pencil: Pencil} */
    private function kit(): array {}

    public function pick(string $which): void
    {
        $kit = $this->kit();
        $to@tool@ol = $kit[$which];
    }
}
"#,
        &[Query::hover("one of the values", "tool")],
        expect![[r#"
            one of the values
            - code: Pen|Pencil $tool
            - note: assigned
        "#]],
    );
}

/// A write describes exactly one path and no prefix of itself, so a slot several levels down is
/// answered by the whole path rather than by walking to it.
#[test]
fn a_write_several_levels_down_is_read_back_by_the_same_path() {
    check(
        r#"
//- /src/deep.php
<?php
namespace App;

class Decimal
{
    public function add(Decimal $other): Decimal {}
}

class Report
{
    /** @param list<Decimal> $prices */
    public function build(array $prices): void
    {
        $return = [];
        $count = 0;
        foreach ($prices as $price) {
            $return['data'][$count]['earnings'] = $price;
            $sum = $return['data'][$count]['earnings'];
            $sum->@deep@
            $count++;
        }
    }
}
"#,
        &[Query::completion("three levels down", "deep")],
        expect![[r#"
            three levels down
            - add [method]  : Decimal  writes `(|);`
            - accepting the first leaves `$sum->add`
        "#]],
    );
}
