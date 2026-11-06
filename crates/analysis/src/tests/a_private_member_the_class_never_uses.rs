//! `unused-member`: a private method or constant the class that declares it never reaches.
//!
//! The one dead member a *file* can settle, because `private` reaches exactly as far as the class
//! and a class is one file. Off unless a project asks for it, and by measurement rather than
//! caution: over five real projects it finds nothing on four and, on Symfony, test fixtures
//! declaring a private member so a component can prove reflection reaches it.
//!
//! What is tested here is mostly the stopping. A missed use is what makes a false finding, so the
//! check refuses to judge a file that can reach a member by a name it does not write.

use expect_test::expect;

use super::diagnostics::check_asked_for;

#[test]
fn a_private_method_and_constant_nothing_calls_are_reported_and_the_used_ones_are_not() {
    check_asked_for(
        r#"
//- /app.php
<?php
namespace App;

class Repo
{
    private const KEPT = 1;
    private const DEAD = 2;

    public function run(): int
    {
        return $this->used() + self::KEPT;
    }

    private function used(): int { return 1; }

    private function dead(): int { return 2; }

    private function __toString(): string { return 'x'; }
}
"#,
        &["unused-member"],
        expect![[r#"
            7:19-7:23 warning [unused-member] private constant Repo::DEAD is never used
            16:22-16:26 warning [unused-member] private method Repo::dead is never used
        "#]],
    );
}

#[test]
fn a_name_written_only_inside_a_string_counts_as_a_use() {
    // How a callable is passed and how a handler map is written. The call is real and the file
    // never writes `->refresh` anywhere.
    check_asked_for(
        r#"
//- /app.php
<?php
namespace App;

class Repo
{
    public function boot(): array
    {
        return [$this, 'refresh'];
    }

    private function refresh(): void {}
}
"#,
        &["unused-member"],
        expect![""],
    );
}

#[test]
fn a_file_that_reaches_a_member_by_a_computed_name_is_not_judged_at_all() {
    check_asked_for(
        r#"
//- /app.php
<?php
namespace App;

class Repo
{
    public function call(string $name): void
    {
        $this->$name();
    }

    private function refresh(): void {}
}
"#,
        &["unused-member"],
        expect![""],
    );
}

#[test]
fn a_class_that_uses_a_trait_is_not_judged_and_a_trait_is_not_either() {
    // Both halves of the same fact: a trait's code runs in the using class's scope, so
    // `$this->refresh()` written in the trait calls the class's private method -- from another
    // file. Neither side of that can be settled by reading one of them.
    check_asked_for(
        r#"
//- /app.php
<?php
namespace App;

trait Helps
{
    private function alsoUnjudged(): void {}
}

class Repo
{
    use Helps;

    private function refresh(): void {}
}
"#,
        &["unused-member"],
        expect![""],
    );
}

#[test]
fn a_class_a_closure_is_bound_to_anywhere_in_the_project_is_not_judged() {
    // The one thing outside the file that reaches in. `Closure::bind($fn, null, Item::class)` in
    // another file gives that closure `Item`'s scope, so its private methods are callable from a
    // file that does not declare them -- which is what this check would otherwise assume away.
    // Symfony's Cache component is written this way and was the whole of why it was wrong there.
    check_asked_for(
        r#"
//- /adapter.php
<?php
namespace App;

class Adapter
{
    public function make(): \Closure
    {
        return \Closure::bind(static function () {}, null, Item::class);
    }
}
//- /item.php
<?php
namespace App;

class Item
{
    private function pack(): void {}
}
"#,
        &["unused-member"],
        expect![""],
    );
}

#[test]
fn a_class_no_other_file_mentions_is_reported_and_its_members_are_not_repeated() {
    // `unused-declaration`, the project-wide half. The word sets answer "is this name written in
    // any other file" without a sweep, which is what makes it affordable here; what it costs is
    // that a mention in a comment counts, and that is the direction it has to be wrong in.
    check_asked_for(
        r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Used.php
<?php
namespace App;

class Used
{
    public function run(): int { return 1; }
}
//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function go(Used $used): int { return $used->run(); }
}
//- /src/Orphan.php
<?php
namespace App;

class Orphan
{
    public function neverCalled(): int { return 1; }
}
"#,
        &["unused-declaration"],
        expect![[r#"
            4:7-4:13 warning [unused-declaration] class Orphan is reached from nowhere in this project
        "#]],
    );
}
