//! What a call to a method the class's `__call` reopens hands back.
//!
//! A protected method called from outside is refused by PHP and sent to `__call`, so ordinarily the
//! declaration says nothing about the value. Eloquent's `__call` names `increment()` and runs it, so
//! there the declaration is exactly what comes back -- and an override of it, which `__call` runs
//! the same way.

use expect_test::expect;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public function __call(string $name, array $arguments): mixed {}

    protected function increment(string $column): int {}

    protected function refresh(): static {}
}

//- /src/Post.php
<?php
namespace App;

class Post extends \Orm\Model {}

class Counter extends \Orm\Model
{
    protected function increment(string $column): float {}
}

function run(Post $post, Counter $counter): void
{
    $o@opened@pened = $post->increment('views');
    $r@refused@efused = $post->refresh();
    $c@overridden@verridden = $counter->increment('views');
}
"#;

#[test]
fn a_reopened_method_returns_what_it_declares() {
    check_view_with(
        FIXTURE,
        &[
            Query::variable_type("a method __call reopens", "opened"),
            Query::variable_type("one it does not, which __call answers", "refused"),
            Query::variable_type(
                "an override, which __call reaches the same way",
                "overridden",
            ),
        ],
        |world| {
            world
                .contracts_mut()
                .open_methods("Orm\\Model", &["increment"]);
        },
        expect![[r#"
            a method __call reopens
            - int
            one it does not, which __call answers
            - (no type)
            an override, which __call reaches the same way
            - float
        "#]],
    );
}
