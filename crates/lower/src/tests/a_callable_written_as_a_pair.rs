//! `[$this, 'refresh']` — how PHP wrote a first-class callable before it had `(...)`.
//!
//! It is still how a framework is handed one: a Twig filter, an event subscriber, a `usort`
//! comparator. Left as an array literal the method name is a *string*, so `references` on the
//! method finds nothing, `rename` leaves the string spelling a method that no longer exists, and
//! goto on it goes nowhere. Counted on one real checkout: 245 written with `$this` and 9 with
//! `Foo::class`, none of them reachable by any of the three.
//!
//! So it lowers to the shape `(...)` already lowers to, which says the same thing — a method is
//! named and not called — and every reader of a member site has understood it since it existed.
//!
//! The whole difficulty is telling the construct from an array that merely looks like one, which
//! is what `presumed` is for and what the last two tests here pin.

use expect_test::expect;

use super::check_bodies;

#[test]
fn a_pair_of_this_and_a_name_is_the_method_it_names() {
    check_bodies(
        r#"
//- /src/Filters.php
<?php
class Filters
{
    public function build(): array
    {
        return [$this, 'refresh'];
    }

    public function refresh(): void {}
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
            Filters::build
              body @ 5:5-7:6
                return @ 6:9-6:35
                  callable ->refresh(...) @ 6:16-6:34 (name @ 6:25-6:32)
                    receiver
                      $this @ 6:17-6:22
            Filters::refresh
              body @ 9:37-9:39
        "#]],
    );
}

#[test]
fn a_pair_of_a_named_class_and_a_name_is_presumed_rather_than_settled() {
    // `[Foo::class, 'bar']` is a callable and is equally a list of two class names -- Symfony
    // writes `setAllowedTypes('active_at', [\DateTimeInterface::class, 'null'])`. The site is
    // kept and marked, and the view drops it unless it resolves to a member that exists.
    check_bodies(
        r#"
//- /src/Filters.php
<?php
class Filters
{
    public function build(): array
    {
        return [Filters::class, 'refresh'];
    }

    public function refresh(): void {}
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
            Filters::build
              body @ 5:5-7:6
                return @ 6:9-6:44
                  callable presumed Filters::refresh(...) @ 6:16-6:43 (name @ 6:34-6:41)
            Filters::refresh
              body @ 9:37-9:39
        "#]],
    );
}

#[test]
fn a_pair_of_two_strings_is_an_array_and_stays_one() {
    // `['Foo', 'bar']` is a callable to PHP and a pair of words to everything else, and there is
    // no reading of the syntax that separates them.
    check_bodies(
        r#"
//- /src/Filters.php
<?php
class Filters
{
    public function build(): array
    {
        return ['Filters', 'refresh'];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Filters::build
              body @ 5:5-7:6
                return @ 6:9-6:39
                  array @ 6:16-6:38
                    string @ 6:17-6:26
                    string @ 6:28-6:37
        "#]],
    );
}

#[test]
fn a_pair_whose_second_half_is_not_a_name_is_an_array() {
    // The quotes are all these two shapes have in common: a method name has no spaces in it.
    check_bodies(
        r#"
//- /src/Filters.php
<?php
class Filters
{
    public function build(): array
    {
        return [$this, 'not a method'];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Filters::build
              body @ 5:5-7:6
                return @ 6:9-6:40
                  array @ 6:16-6:39
                    $this @ 6:17-6:22
                    string @ 6:24-6:38
        "#]],
    );
}

#[test]
fn three_elements_are_an_array_however_the_first_two_are_written() {
    check_bodies(
        r#"
//- /src/Filters.php
<?php
class Filters
{
    public function build(): array
    {
        return [$this, 'refresh', 1];
    }

    public function refresh(): void {}
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
            Filters::build
              body @ 5:5-7:6
                return @ 6:9-6:38
                  array @ 6:16-6:37
                    $this @ 6:17-6:22
                    string @ 6:24-6:33
                    int @ 6:35-6:36
            Filters::refresh
              body @ 9:37-9:39
        "#]],
    );
}
