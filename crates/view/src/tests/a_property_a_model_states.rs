//! A property a class states in an array it writes: Eloquent's `$casts`, `casts()` and `$fillable`.
//!
//! Nothing declares `published_at`; the model's `__get` reads a database row, and the only statement
//! anywhere about what comes back is the cast. Strict about the null, every one: a null column is a
//! null whatever the cast, and whether a column can be null is in a migration, which no test here
//! writes -- `a_column_a_migration_writes` is where one does.
//! A listed name with no cast is there and nothing more -- `mixed` -- and the two every model has,
//! `created_at` and `updated_at`, come from the rule rather than from anything the class wrote.
//!
//! And PHP's own rule about redeclaring: a subclass's `$casts` replaces the parent's array whole, so
//! a cast only the parent's lists is gone there, while the parent's `casts()` method still runs.

use expect_test::expect;

use phrost_harness::World;
use phrost_index::{StatedKind, StatedPropertiesRule};
use phrost_model::BuiltinTy;
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

abstract class Model
{
    public function __get(string $key): mixed {}
}

class Date {}

/**
 * @template TGet
 */
interface CastsAttributes
{
    /** @return TGet|null */
    public function get(Model $model, string $key, mixed $value, array $attributes);
}

//- /src/Status.php
<?php
namespace App;

enum Status: string
{
    case Draft = 'draft';
}

//- /src/Money.php
<?php
namespace App;

use Orm\CastsAttributes;
use Orm\Model;

final class Money {}

/** @implements CastsAttributes<Money> */
final class AsMoney implements CastsAttributes
{
    public function get(Model $model, string $key, mixed $value, array $attributes): Money
    {
        return new Money();
    }
}

/** @implements CastsAttributes<Money> */
final class AsMaybeMoney implements CastsAttributes
{
    public function get(Model $model, string $key, mixed $value, array $attributes) {}
}

//- /src/Member.php
<?php
namespace App;

use Orm\Model;

#[Fillable(['email', 'name'])]
class Member extends Model {}

#[\Orm\Fillable('nickname')]
class Guest extends Model {}

trait UsesStrings
{
    public function getKeyType(): string { return 'string'; }
}

class Token extends Model
{
    use UsesStrings;
}

class Code extends Model
{
    protected $keyType = 'string';
}

//- /src/Post.php
<?php
namespace App;

use Orm\Model;

class Post extends Model
{
    protected $fillable = ['title', 'published'];

    protected $casts = [
        'published' => 'boolean',
        'published_at' => 'datetime:Y-m-d',
        'secret' => 'encrypted:array',
    ];

    protected function casts(): array
    {
        return ['status' => Status::class, 'price' => AsMoney::class, 'cost' => AsMaybeMoney::class];
    }
}

class Draft extends Post
{
    protected $casts = ['published' => 'integer'];
}

function run(Post $post, Draft $draft, Member $member, Guest $guest, Token $token, Code $code): void
{
    $k@key@ey = $post->id;
    $u@uuid@uid = $token->id;
    $x@keytype@eytype = $code->id;
    $l@listed@isted = $member->email;
    $v@variadic@ariadic = $guest->nickname;
    $p@published@ublished = $post->published;
    $a@at@t = $post->published_at;
    $s@secret@ecret = $post->secret;
    $e@enum@num = $post->status;
    $t@title@itle = $post->title;
    $c@created@reated = $post->created_at;
    $o@overridden@verridden = $draft->published;
    $i@inherited@nherited = $draft->published_at;
    $m@method@ethod = $draft->status;
    $p@price@rice = $post->price;
    $k@cost@ost = $post->cost;
}
"#;

#[test]
fn a_cast_says_what_the_property_holds_and_that_it_may_be_null() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, |world| {
        // Before the files, because lowering keeps only the defaults it was told to.
        world.set_kept_defaults(&["casts", "fillable"], &["Fillable"]);
        world
            .contracts_mut()
            .bind_stated_properties(StatedPropertiesRule {
                class: "Orm\\Model".into(),
                typed: vec!["casts".into()],
                listed: vec!["fillable".into()],
                listed_by_attribute: vec!["Orm\\Fillable".into()],
                spellings: vec![
                    ("boolean".into(), StatedKind::Builtin(BuiltinTy::Bool)),
                    ("integer".into(), StatedKind::Builtin(BuiltinTy::Int)),
                    ("array".into(), StatedKind::Builtin(BuiltinTy::Array)),
                    ("datetime".into(), StatedKind::Class("Orm\\Date".into())),
                ],
                wrapping: vec!["encrypted:".into()],
                classes: Vec::new(),
                custom: Some(("Orm\\CastsAttributes".into(), "get".into())),
                implicit: vec![("created_at".into(), StatedKind::Class("Orm\\Date".into()))],
                key: Some(phrost_index::StatedKeyRule {
                    name: "id".into(),
                    kind: StatedKind::Builtin(BuiltinTy::Int),
                    unless: vec!["keyType".into(), "getKeyType".into()],
                }),
                nullable: true,
            });
    });
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("a cast", "published"),
            Query::variable_type("a cast written with its format", "at"),
            Query::variable_type("a wrapped cast", "secret"),
            Query::variable_type("an enum, from casts()", "enum"),
            Query::variable_type("listed and nothing more", "title"),
            Query::variable_type("one every model has", "created"),
            Query::variable_type("a subclass's own cast", "overridden"),
            Query::variable_type(
                "a parent's $casts, which the subclass's replaces",
                "inherited",
            ),
            Query::variable_type("a parent's casts(), which it does not", "method"),
            Query::variable_type(
                "a cast of the project's own, by what its get() declares",
                "price",
            ),
            Query::variable_type("one that leaves it to the interface", "cost"),
            Query::property_goto("goto, the cast", "App\\Post", "published_at"),
            Query::property_goto("goto, the listing", "App\\Post", "title"),
            Query::variable_type("the key every model has", "key"),
            Query::variable_type("a trait that changes the key", "uuid"),
            Query::variable_type("a model that declares another key type", "keytype"),
            Query::variable_type("listed by #[Fillable([...])]", "listed"),
            Query::variable_type(
                "listed by #[Fillable(...)] written as arguments",
                "variadic",
            ),
            Query::property_goto("goto, the attribute's entry", "App\\Member", "name"),
        ],
        expect![[r#"
            a cast
            - ?bool
            a cast written with its format
            - ?Date
            a wrapped cast
            - ?array
            an enum, from casts()
            - ?Status
            listed and nothing more
            - mixed
            one every model has
            - ?Date
            a subclass's own cast
            - ?int
            a parent's $casts, which the subclass's replaces
            - mixed
            a parent's casts(), which it does not
            - ?Status
            a cast of the project's own, by what its get() declares
            - Money
            one that leaves it to the interface
            - ?Money
            goto, the cast
            - property App\Post::published_at @ src/Post.php 12:10-12:22
            goto, the listing
            - property App\Post::title @ src/Post.php 8:29-8:34
            the key every model has
            - ?int
            a trait that changes the key
            - mixed
            a model that declares another key type
            - mixed
            listed by #[Fillable([...])]
            - mixed
            listed by #[Fillable(...)] written as arguments
            - mixed
            goto, the attribute's entry
            - property App\Member::name @ src/Member.php 6:23-6:27
        "#]],
    );
}
