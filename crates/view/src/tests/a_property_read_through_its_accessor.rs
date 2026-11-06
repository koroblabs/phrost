//! A property read through the accessor its name spells: Eloquent's `getExcerptAttribute()` and
//! `excerpt(): Attribute`, each `$post->excerpt`.
//!
//! Nothing declares `excerpt`; `__get` finds the method by the name, studly-cased between `get` and
//! `Attribute` for the first spelling and camel-cased, returning an `Attribute`, for the second. The
//! method is what goto lands on and what says the type -- in full for the first, and for the second
//! as far as a docblock writes the attribute's type arguments, which is the only place a declaration
//! says what `get:` hands back.

use expect_test::expect;

use super::{Query, check_view_with};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public function __get(string $key): mixed {}
}

final class Attribute
{
    public static function make(?callable $get = null): static {}
}

//- /src/Post.php
<?php
namespace App;

use Orm\Attribute;
use Orm\Model;

class Post extends Model
{
    public function getExcerptAttribute(): string { return ''; }

    public function getPublishedAtAttribute(): ?\DateTimeImmutable { return null; }

    /** @return Attribute<int, never> */
    protected function wordCount(): Attribute { return Attribute::make(); }

    protected function headline(): Attribute { return Attribute::make(); }

    public function getAttribute(string $key): mixed {}

    /** @return Attribute */
    protected function teaser() { return Attribute::make(); }

    protected function caption(): Attribute|string { return Attribute::make(); }
}

function run(Post $post): void
{
    $e@excerpt@xcerpt = $post->excerpt;
    $p@published@ublished = $post->published_at;
    $w@words@ords = $post->word_count;
    $h@headline@eadline = $post->headline;
    $n@nothing@othing = $post->attribute;
}
"#;

#[test]
fn an_accessor_answers_for_the_property_its_name_spells() {
    check_view_with(
        FIXTURE,
        &[
            Query::variable_type("get...Attribute()", "excerpt"),
            Query::variable_type("a snake-cased name", "published"),
            Query::variable_type("an Attribute whose docblock says", "words"),
            Query::variable_type("an Attribute that does not", "headline"),
            Query::variable_type("getAttribute() spells no property", "nothing"),
            Query::property_goto("goto, the method spelling", "App\\Post", "excerpt"),
            Query::property_goto("goto, the attribute spelling", "App\\Post", "word_count"),
            Query::property_goto("an Attribute a docblock alone says", "App\\Post", "teaser"),
            Query::property_goto("an Attribute in a union", "App\\Post", "caption"),
        ],
        |world| {
            world
                .contracts_mut()
                .bind_accessor("Orm\\Model", "get", "Attribute", "Orm\\Attribute");
        },
        expect![[r#"
            get...Attribute()
            - string
            a snake-cased name
            - ?DateTimeImmutable
            an Attribute whose docblock says
            - int
            an Attribute that does not
            - mixed
            getAttribute() spells no property
            - mixed
            goto, the method spelling
            - method App\Post::getExcerptAttribute @ src/Post.php 9:21-9:40
            goto, the attribute spelling
            - method App\Post::wordCount @ src/Post.php 14:24-14:33
            an Attribute a docblock alone says
            - (not found)
            an Attribute in a union
            - (not found)
        "#]],
    );
}

/// The getter a body hands over, where no docblock writes the attribute's type: the closure is the
/// one statement of what the property holds. Read at ingest, where the body still is, so the rule
/// has to be in place before the files are.
#[test]
fn an_attribute_whose_body_hands_over_a_typed_getter() {
    let spec = phrost_testkit::FixtureSpec::parse(
        r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public function __get(string $key): mixed {}
}

final class Attribute
{
    public static function make(?callable $get = null, ?callable $set = null): static {}
    public static function get(callable $get): static {}
    public static function set(callable $set): static {}
}

//- /src/Post.php
<?php
namespace App;

use Orm\Attribute;
use Orm\Model;

class Post extends Model
{
    protected function headline(): Attribute
    {
        return Attribute::make(get: fn (): string => 'x');
    }

    protected function summary(): Attribute
    {
        return Attribute::get(function (): ?string {
            return null;
        });
    }

    protected function slug(): Attribute
    {
        return Attribute::set(fn (string $value): string => $value);
    }
}

class Draft extends Post {}

function run(Post $post, Draft $draft): void
{
    $h@headline@eadline = $post->headline;
    $s@summary@ummary = $post->summary;
    $l@slug@lug = $post->slug;
    $d@inherited@nherited = $draft->headline;
}
"#,
    );
    let world = phrost_harness::World::build_with(&spec, |world| {
        world
            .contracts_mut()
            .bind_accessor("Orm\\Model", "get", "Attribute", "Orm\\Attribute");
    });
    super::check_world(
        &spec,
        &world,
        &[
            Query::variable_type("make(get: ...)", "headline"),
            Query::variable_type("get(...)", "summary"),
            Query::variable_type("set(...) is no getter", "slug"),
            Query::variable_type("declared on a parent", "inherited"),
        ],
        expect![[r#"
            make(get: ...)
            - string
            get(...)
            - ?string
            set(...) is no getter
            - mixed
            declared on a parent
            - string
        "#]],
    );
}
