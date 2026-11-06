//! Which factory a model's `factory()` hands back: `Factory::resolveFactoryName()`, restated.
//!
//! `Database\Factories\` and the model's name without the application's namespace -- and without
//! its `Models\` below it, which is where a model usually lives -- then `Factory`. So
//! `App\Models\Contact` is `Database\Factories\ContactFactory` and BookStack's
//! `BookStack\Entities\Models\Page` is `Database\Factories\Entities\Models\PageFactory`.

/// The factory class `model`'s `factory()` resolves to, given the application's namespace with its
/// trailing separator.
pub(super) fn factory_for(model: &str, app_namespace: &str) -> String {
    let in_models = format!("{app_namespace}Models\\");
    let name = model
        .strip_prefix(&in_models)
        .or_else(|| model.strip_prefix(app_namespace))
        .unwrap_or(model);
    format!("Database\\Factories\\{name}Factory")
}

/// The models a factory makes when it does not say, in the order `Factory::modelName()` tries them:
/// below the application's `Models\`, then directly below the application. `UserFactory` is
/// `App\Models\User`, or `App\User` where there is no such model.
pub(super) fn model_for(factory: &str, app_namespace: &str) -> Vec<String> {
    let Some(named) = factory.strip_suffix("Factory") else {
        return Vec::new();
    };
    let namespaced = named.strip_prefix("Database\\Factories\\").unwrap_or(named);
    let short = phrost_model::short_name(named);
    vec![
        format!("{app_namespace}Models\\{namespaced}"),
        format!("{app_namespace}{short}"),
    ]
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    /// The two `Factory::modelName()` tries, first the one below `Models\`.
    #[test]
    fn a_factory_names_the_models_laravel_tries() {
        let rendered = [
            ("Database\\Factories\\UserFactory", "App\\"),
            ("Database\\Factories\\Billing\\InvoiceFactory", "App\\"),
            (
                "Database\\Factories\\Entities\\Models\\PageFactory",
                "BookStack\\",
            ),
        ]
        .iter()
        .map(|(factory, app)| format!("{factory} -> {:?}\n", super::model_for(factory, app)))
        .collect::<String>();
        expect![[r#"
            Database\Factories\UserFactory -> ["App\\Models\\User", "App\\User"]
            Database\Factories\Billing\InvoiceFactory -> ["App\\Models\\Billing\\Invoice", "App\\Invoice"]
            Database\Factories\Entities\Models\PageFactory -> ["BookStack\\Models\\Entities\\Models\\Page", "BookStack\\Page"]
        "#]]
        .assert_eq(&rendered);
    }

    /// Each as `Factory::resolveFactoryName()` answers on Laravel 12.
    #[test]
    fn a_model_names_its_factory_the_way_laravel_finds_it() {
        let rendered = [
            ("App\\Models\\Contact", "App\\"),
            ("App\\Models\\Billing\\Invoice", "App\\"),
            ("App\\Contact", "App\\"),
            ("BookStack\\Entities\\Models\\Page", "BookStack\\"),
            ("Vendor\\Package\\Thing", "App\\"),
        ]
        .iter()
        .map(|(model, app)| format!("{model} -> {}\n", super::factory_for(model, app)))
        .collect::<String>();
        expect![[r#"
            App\Models\Contact -> Database\Factories\ContactFactory
            App\Models\Billing\Invoice -> Database\Factories\Billing\InvoiceFactory
            App\Contact -> Database\Factories\ContactFactory
            BookStack\Entities\Models\Page -> Database\Factories\Entities\Models\PageFactory
            Vendor\Package\Thing -> Database\Factories\Vendor\Package\ThingFactory
        "#]]
        .assert_eq(&rendered);
    }
}
