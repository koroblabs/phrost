//! `$this` inside a trait that names no other half of itself: the classes the project composes it
//! into, since a trait's body only ever runs as one of them.
//!
//! Laravel's `InteractsWithPivotTable` reaches `$this->parent` and `$this->using`, which its one
//! host, `BelongsToMany`, declares two classes up; read as the trait alone they went nowhere.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn one_host_answers_for_this_whole() {
    check(
        r#"
//- /src/Counts.php
<?php
namespace App;

class Base
{
    public function label(): string { return ''; }
}

class Host extends Base
{
    use Counts;

    public function total(): int { return 1; }
}

trait Counts
{
    public function bump(): int
    {
        $sum = $this->total();
        $s@sum@um;
        return strlen($this->la@label@bel());
    }
}
"#,
        &[
            Query::hover("the host's own method", "sum"),
            Query::goto("a method two classes up", "label"),
        ],
        expect![[r#"
            the host's own method
            - code: int $sum
            - note: assigned
            a method two classes up
            - method App\Base::label @ src/Counts.php 6:21-6:26 (full 6:5-6:51)
        "#]],
    );
}

#[test]
fn several_hosts_answer_for_what_any_of_them_declares() {
    check(
        r#"
//- /src/Counts.php
<?php
namespace App;

class Left
{
    use Counts;

    public function onlyLeft(): int { return 1; }
}

class Right
{
    use Counts;
}

trait Counts
{
    public function bump(): int
    {
        return $this->only@left@Left();
    }
}
"#,
        &[Query::goto("the one host declaring it", "left")],
        expect![[r#"
            the one host declaring it
            - method App\Left::onlyLeft @ src/Counts.php 8:21-8:29 (full 8:5-8:50)
        "#]],
    );
}

#[test]
fn a_host_through_another_trait_is_a_host() {
    check(
        r#"
//- /src/Counts.php
<?php
namespace App;

trait Counts
{
    public function bump(): int
    {
        return $this->tot@total@al();
    }
}

trait CountsTwice
{
    use Counts;
}

class Host
{
    use CountsTwice;

    public function total(): int { return 1; }
}
"#,
        &[Query::goto(
            "the class composing the composing trait",
            "total",
        )],
        expect![[r#"
            the class composing the composing trait
            - method App\Host::total @ src/Counts.php 21:21-21:26 (full 21:5-21:47)
        "#]],
    );
}

#[test]
fn a_guard_on_this_keeps_the_traits_own_members() {
    // `PromptsForMissingInput` writes exactly this: the guard names a contract only some hosts
    // implement, and the call under it is one the trait declares itself.
    check(
        r#"
//- /src/Prompts.php
<?php
namespace App;

interface Prompts
{
    public function prompt(): int;
}

class Quiet
{
    use Asks;
}

class Loud implements Prompts
{
    use Asks;

    public function prompt(): int { return 1; }
}

trait Asks
{
    public function own(): int { return 1; }

    public function ask(): int
    {
        if ($this instanceof Prompts) {
            return $this->o@own@wn() + $this->pro@prompt@mpt();
        }
        return 0;
    }
}
"#,
        &[
            Query::goto("the trait's own method", "own"),
            Query::goto("the one host the guard leaves", "prompt"),
        ],
        expect![[r#"
            the trait's own method
            - method App\Asks::own @ src/Prompts.php 23:21-23:24 (full 23:5-23:45)
            the one host the guard leaves
            - method App\Loud::prompt @ src/Prompts.php 18:21-18:27 (full 18:5-18:48)
        "#]],
    );
}
