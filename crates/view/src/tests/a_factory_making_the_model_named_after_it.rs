//! A template a subclass fills by its own name: Laravel's `Factory<TModel>`, where a factory that
//! writes neither `@extends Factory<User>` nor `$model` makes the model `Factory::modelName()` finds
//! by the factory's name -- below the application's `Models\`, then directly below the application --
//! and a `$model = User::class` the factory declares is what it makes, before any name.

use expect_test::expect;

use phrost_harness::World;
use phrost_index::NamedArgumentRule;
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/** @template TModel */
abstract class Factory
{
    protected $model;

    /** @return TModel */
    public function make() {}

    /** @return Collection<int, TModel> */
    public function many() {}
}

/** @template TKey @template TValue */
class Collection {}

//- /app/Models.php
<?php
namespace App\Models;

class User {}
class Post {}
class Comment {}
class StringModel {}

namespace App;

class Invoice {}

//- /database/factories/Factories.php
<?php
namespace Database\Factories;

class UserFactory extends \Orm\Factory {}

/** @extends \Orm\Factory<\App\Models\User> */
class PostFactory extends \Orm\Factory {}

class InvoiceFactory extends \Orm\Factory {}

class TeamFactory extends \Orm\Factory {}

class CommentFactory extends \Orm\Factory
{
    protected $model = \App\Models\Post::class;
}

class StringModelFactory extends \Orm\Factory
{
    protected $model = 'App\Models\Post';
}

function run(UserFactory $users, PostFactory $posts, InvoiceFactory $invoices, TeamFactory $teams, CommentFactory $comments, StringModelFactory $strings): void
{
    $u@user@ser = $users->make();
    $m@many@any = $users->many();
    $w@written@ritten = $posts->make();
    $i@invoice@nvoice = $invoices->make();
    $t@team@eam = $teams->make();
    $c@comment@omment = $comments->make();
    $s@string_model@tring = $strings->make();
}
"#;

#[test]
fn a_factory_makes_the_model_named_after_it() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, |world| {
        // Before the files, because lowering keeps only the defaults it was told to.
        world.set_kept_defaults(&["model"], &[]);
        world
            .contracts_mut()
            .bind_named_argument(NamedArgumentRule {
                class: "Orm\\Factory".into(),
                template: "TModel".into(),
                from_property: Some("model".into()),
                unless: vec!["model".into()],
                app_directory: "app".into(),
                default_namespace: "App\\".into(),
                names: |factory, app| {
                    let Some(named) = factory.strip_suffix("Factory") else {
                        return Vec::new();
                    };
                    let namespaced = named.strip_prefix("Database\\Factories\\").unwrap_or(named);
                    let short = named.rsplit('\\').next().unwrap_or(named);
                    vec![
                        format!("{app}Models\\{namespaced}"),
                        format!("{app}{short}"),
                    ]
                },
            });
    });
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("the model below Models", "user"),
            Query::variable_type("inside a container", "many"),
            Query::variable_type("an @extends says which, and wins", "written"),
            Query::variable_type("the model directly below the application", "invoice"),
            Query::variable_type("a model that does not exist", "team"),
            Query::variable_type("a $model the factory declares, before its name", "comment"),
            Query::variable_type(
                "a $model that is no class constant, which it is not guessed past",
                "string_model",
            ),
        ],
        expect![[r#"
            the model below Models
            - User
            inside a container
            - Collection<int, User>
            an @extends says which, and wins
            - User
            the model directly below the application
            - Invoice
            a model that does not exist
            - (no type)
            a $model the factory declares, before its name
            - Post
            a $model that is no class constant, which it is not guessed past
            - (no type)
        "#]],
    );
}
