//! A static method handing back the class named after the one it was called on: Laravel's
//! `Contact::factory()`, which is the factory `Factory::resolveFactoryName()` finds by the model's
//! name -- where nothing written fills `HasFactory`'s template and nothing the model declares names
//! the factory itself.

use expect_test::expect;

use phrost_harness::World;
use phrost_index::NamedAfterRule;
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

const FIXTURE: &str = r#"
//- /vendor/Orm.php
<?php
namespace Orm;

/** @template TModel */
abstract class Factory
{
    /** @return TModel */
    public function make() {}
}

/** @template TFactory of Factory */
trait HasFactory
{
    /** @return TFactory */
    public static function factory() {}

    /** @return TFactory|null */
    protected static function newFactory() {}
}

abstract class Model {}

//- /database/factories/ContactFactory.php
<?php
namespace Database\Factories;

/** @extends \Orm\Factory<\App\Models\Contact> */
class ContactFactory extends \Orm\Factory {}

//- /database/factories/Billing/InvoiceFactory.php
<?php
namespace Database\Factories\Billing;

/** @extends \Orm\Factory<\App\Models\Billing\Invoice> */
class InvoiceFactory extends \Orm\Factory {}

//- /database/factories/SpecialFactory.php
<?php
namespace Database\Factories;

/** @extends \Orm\Factory<\App\Models\Contact> */
class SpecialFactory extends \Orm\Factory {}

//- /database/factories/WrittenFactory.php
<?php
namespace Database\Factories;

/** @extends \Orm\Factory<\App\Models\Written> */
class WrittenFactory extends \Orm\Factory {}

//- /database/factories/OverriddenFactory.php
<?php
namespace Database\Factories;

/** @extends \Orm\Factory<\App\Models\Overridden> */
class OverriddenFactory extends \Orm\Factory {}

//- /src/Models.php
<?php
namespace App\Models;

class Contact extends \Orm\Model
{
    use \Orm\HasFactory;
}

namespace App\Models\Billing;

class Invoice extends \Orm\Model
{
    use \Orm\HasFactory;
}

namespace App\Models;

class Written extends \Orm\Model
{
    /** @use \Orm\HasFactory<\Database\Factories\SpecialFactory> */
    use \Orm\HasFactory;
}

class Overridden extends \Orm\Model
{
    use \Orm\HasFactory;

    protected static function newFactory() { return null; }
}

class Lonely extends \Orm\Model
{
    use \Orm\HasFactory;
}

function run(): void
{
    $c@contact@ontact = Contact::factory();
    $m@made@ade = Contact::factory()->make();
    $i@invoice@nvoice = \App\Models\Billing\Invoice::factory();
    $w@written@ritten = Written::factory();
    $o@overridden@verridden = Overridden::factory();
    $l@lonely@onely = Lonely::factory();
}
"#;

#[test]
fn a_model_hands_back_the_factory_named_after_it() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, |world| {
        world.contracts_mut().bind_named_after(NamedAfterRule {
            class: "Orm\\HasFactory".into(),
            method: "factory".into(),
            unless: vec!["newFactory".into(), "factory".into()],
            app_directory: "app".into(),
            default_namespace: "App\\".into(),
            name: |model, app| {
                let in_models = format!("{app}Models\\");
                let name = model
                    .strip_prefix(&in_models)
                    .or_else(|| model.strip_prefix(app))
                    .unwrap_or(model);
                format!("Database\\Factories\\{name}Factory")
            },
        });
    });
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("the factory named after the model", "contact"),
            Query::variable_type("and what it makes", "made"),
            Query::variable_type("below Models, in a namespace of its own", "invoice"),
            Query::variable_type("a model whose @use says which", "written"),
            Query::variable_type("a model that names its factory itself", "overridden"),
            Query::variable_type("a model whose factory does not exist", "lonely"),
        ],
        expect![[r#"
            the factory named after the model
            - ContactFactory
            and what it makes
            - Contact
            below Models, in a namespace of its own
            - InvoiceFactory
            a model whose @use says which
            - SpecialFactory
            a model that names its factory itself
            - Factory
            a model whose factory does not exist
            - Factory
        "#]],
    );
}
