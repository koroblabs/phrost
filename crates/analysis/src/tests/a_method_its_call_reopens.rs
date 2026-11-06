//! A protected method the class's `__call` hands straight back to its declaration.
//!
//! Eloquent's `increment()`: PHP refuses it outside the model, sends the call to `__call`, and
//! `__call` runs the declaration -- through `::` as well, by way of `__callStatic`. So it is offered
//! wherever it runs, and a protected method nothing reopens is offered nowhere it does not.

use expect_test::expect;

use super::{Query, check_with};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public static function __callStatic(string $name, array $arguments): mixed {}
    public function __call(string $name, array $arguments): mixed {}

    protected function increment(string $column, int $amount = 1): int {}

    protected function incrementOrCreate(): int {}
}

//- /src/Post.php
<?php
namespace App;

class Post extends \Orm\Model {}

//- /src/Use.php
<?php
namespace App;

function onAnInstance(Post $post): void
{
    $post->incr@instance@
}

function throughTheClass(): void
{
    Post::incr@static@
}

function onTheCall(Post $post): void
{
    $post->incr@call@ement('views');
}
"#;

#[test]
fn a_reopened_method_is_offered_and_answered_outside_its_class() {
    check_with(
        FIXTURE,
        &[
            Query::completion(
                "on an instance, and not the one nothing reopens",
                "instance",
            ),
            Query::completion("through the class, which __callStatic runs", "static"),
            Query::goto("goto, the declaration", "call"),
        ],
        |world| {
            world
                .contracts_mut()
                .open_methods("Orm\\Model", &["increment"]);
        },
        expect![[r#"
            on an instance, and not the one nothing reopens
            - increment [method]  : int  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$post->increment`
            through the class, which __callStatic runs
            - increment [method]  : int  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `Post::increment`
            goto, the declaration
            - method Orm\Model::increment @ vendor/Orm.php 9:24-9:33 (full 9:5-9:74)
        "#]],
    );
}
