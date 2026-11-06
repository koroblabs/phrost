//! The model `config/auth.php` says a user is, when it says one.
//!
//! `$request->user()` declares `mixed` and a guard's `user()` an `Authenticatable`, and which class
//! comes back is configuration: each provider names its model. A project with one -- the skeleton's
//! `App\Models\User` -- has one answer for every guard; a project whose providers name several, or
//! whose provider reads a table rather than a model (`GenericUser`), has none this can give. So the
//! entry is made only where every provider names the same Eloquent model.
//!
//! Read as text: a config file is a `return [...]` and nothing here parses PHP. Comments are taken
//! out first -- the skeleton keeps a commented `database` provider beside the real one -- and a
//! name is resolved against the file's own `use` lines, which is how the skeleton writes it.

use std::path::Path;

use super::config_text::{Scope, without_comments, written_class};
use crate::{CatalogEntry, CatalogName};

/// The catalog the user model is filed under. A name this provider chose.
pub(super) const CATALOG: &str = "laravel.auth-user";
/// Its one key.
pub(super) const KEY: &str = "user";

/// The entry, where every provider in the project's `config/auth.php` names one model.
///
/// Since Laravel 11 the framework's own `config/auth.php` is merged beneath the project's, and its
/// providers with it: a project whose file writes no `'providers'` -- or has no file -- has the
/// framework's one `users` provider, which is what the skeleton names.
pub(super) fn user_model(root: &Path) -> Option<CatalogEntry> {
    let own = root.join("config").join("auth.php");
    let (path, code) = match std::fs::read_to_string(&own) {
        Ok(text) if without_comments(&text).contains("'providers'") => {
            (own, without_comments(&text))
        }
        _ => {
            let base = root
                .join("vendor/laravel/framework/config")
                .join("auth.php");
            let text = std::fs::read_to_string(&base).ok()?;
            (base, without_comments(&text))
        }
    };
    if names_a_table_provider(&code) {
        return None;
    }
    let scope = Scope::of(&code);
    let mut models: Vec<String> = Vec::new();
    for at in code.match_indices("'model'").map(|(at, _)| at) {
        let rest = code[at + "'model'".len()..].trim_start();
        let Some(rest) = rest.strip_prefix("=>") else {
            continue;
        };
        let model = written_class(rest.trim_start())?;
        let model = scope.resolved(&model);
        if !models.contains(&model) {
            models.push(model);
        }
    }
    let [model] = models.as_slice() else {
        return None;
    };
    Some(CatalogEntry {
        catalog: CATALOG,
        key: KEY.to_owned(),
        names: Some(CatalogName::Class(model.clone())),
        declared_in: Some(path),
    })
}

/// Whether a provider reads users from a table, which hands back a `GenericUser`, not a model.
fn names_a_table_provider(code: &str) -> bool {
    code.match_indices("'driver'").any(|(at, _)| {
        code[at + "'driver'".len()..]
            .trim_start()
            .strip_prefix("=>")
            .is_some_and(|value| value.trim_start().starts_with("'database'"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_of(config: &str) -> Option<String> {
        model_beside(config, None)
    }

    /// The model a project names, with the framework's own file beneath it when there is one.
    fn model_beside(config: &str, framework: Option<&str>) -> Option<String> {
        let dir = std::env::temp_dir().join(format!(
            "phrost-auth-{}-{}",
            config.len(),
            framework.map_or(0, str::len)
        ));
        std::fs::create_dir_all(dir.join("config")).ok()?;
        std::fs::write(dir.join("config").join("auth.php"), config).ok()?;
        if let Some(framework) = framework {
            let base = dir.join("vendor/laravel/framework/config");
            std::fs::create_dir_all(&base).ok()?;
            std::fs::write(base.join("auth.php"), framework).ok()?;
        }
        let found = user_model(&dir).and_then(|entry| match entry.names {
            Some(CatalogName::Class(class)) => Some(class),
            _ => None,
        });
        let _ = std::fs::remove_dir_all(&dir);
        found
    }

    #[test]
    fn the_skeleton_names_its_model_through_an_import_and_env() {
        let config = r"<?php

use App\Models\User;

return [
    'providers' => [
        'users' => [
            'driver' => 'eloquent',
            'model' => env('AUTH_MODEL', User::class),
        ],

        // 'users' => [
        //     'driver' => 'database',
        //     'table' => 'users',
        // ],
    ],
];
";
        assert_eq!(model_of(config).as_deref(), Some("App\\Models\\User"));
    }

    #[test]
    fn a_project_writing_no_providers_has_the_frameworks() {
        let project = r"<?php
return ['guards' => ['api' => ['driver' => 'token', 'provider' => 'users']]];
";
        let framework = r"<?php
return ['providers' => [
    'users' => ['driver' => 'eloquent', 'model' => env('AUTH_MODEL', App\Models\User::class)],
]];
";
        assert_eq!(
            model_beside(project, Some(framework)).as_deref(),
            Some("App\\Models\\User")
        );
        assert_eq!(model_of(project), None);
    }

    #[test]
    fn two_models_or_a_table_provider_name_no_one_user() {
        let two = r"<?php
return ['providers' => [
    'users' => ['driver' => 'eloquent', 'model' => App\Models\User::class],
    'admins' => ['driver' => 'eloquent', 'model' => App\Models\Admin::class],
]];
";
        let table = r"<?php
return ['providers' => [
    'users' => ['driver' => 'eloquent', 'model' => App\Models\User::class],
    'legacy' => ['driver' => 'database', 'table' => 'members'],
]];
";
        assert_eq!(model_of(two), None);
        assert_eq!(model_of(table), None);
    }
}
