//! The global names Laravel's `AliasLoader` gives a class at start: `DB` for the facade.
//!
//! `use DB;` and a bare `DB::table()` in a file with no namespace are both written against a name
//! no file declares -- the loader `class_alias`es it the first time it is asked for. Which names
//! it holds is configuration: `config/app.php`'s `'aliases'`, which since Laravel 11 a project may
//! leave out, leaving the framework's own -- `Facade::defaultAliases()`, merged into by the
//! project's list where it writes one. That method is read from the installed framework rather
//! than listed here, because the list grows from one release to the next.
//!
//! Packages register aliases too, in their `composer.json`; those are not read.

use std::path::Path;

use super::config_text::{Scope, class_entries, value_of, without_comments};
use crate::ClassAlias;

/// The facade base class, whose `defaultAliases()` is the framework's list.
const FACADE: &str = "vendor/laravel/framework/src/Illuminate/Support/Facades/Facade.php";

/// Every alias the loader registers: the framework's defaults unless the project's `'aliases'` is
/// a list of its own, and what that list adds.
pub(super) fn class_aliases(root: &Path) -> Vec<ClassAlias> {
    let configured = std::fs::read_to_string(root.join("config").join("app.php"))
        .ok()
        .map(|text| without_comments(&text));
    let written = configured
        .as_deref()
        .and_then(|code| value_of(code, "aliases").map(|value| (code, value)));
    let mut aliases = match written {
        // A list that starts from the defaults, or no list at all, has them.
        Some((_, value)) if !value.contains("defaultAliases") => Vec::new(),
        _ => default_aliases(root),
    };
    if let Some((code, value)) = written {
        let scope = Scope::of(code);
        for (alias, class) in class_entries(value) {
            let class = scope.resolved(&class);
            aliases.retain(|known: &ClassAlias| !known.alias.eq_ignore_ascii_case(&alias));
            aliases.push(ClassAlias { alias, class });
        }
    }
    aliases
}

/// The framework's own list, out of `Facade::defaultAliases()`.
fn default_aliases(root: &Path) -> Vec<ClassAlias> {
    let Ok(text) = std::fs::read_to_string(root.join(FACADE)) else {
        return Vec::new();
    };
    let code = without_comments(&text);
    let Some(at) = code.find("function defaultAliases") else {
        return Vec::new();
    };
    let body = &code[at..];
    let body = &body[..body.find("]);").unwrap_or(body.len())];
    let scope = Scope::of(&code);
    class_entries(body)
        .into_iter()
        .map(|(alias, class)| ClassAlias {
            class: scope.resolved(&class),
            alias,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACADE_SOURCE: &str = r"<?php

namespace Illuminate\Support\Facades;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Str;

abstract class Facade
{
    /**
     * Get the application default aliases.
     */
    public static function defaultAliases()
    {
        return new Collection([
            'DB' => DB::class,
            'Eloquent' => Model::class,
            'Str' => Str::class,
        ]);
    }
}
";

    fn aliases_of(config: Option<&str>) -> Vec<(String, String)> {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let facade = dir.path().join(FACADE);
        std::fs::create_dir_all(facade.parent().expect("a parent")).expect("the vendor tree");
        std::fs::write(&facade, FACADE_SOURCE).expect("the facade");
        if let Some(config) = config {
            std::fs::create_dir_all(dir.path().join("config")).expect("the config directory");
            std::fs::write(dir.path().join("config").join("app.php"), config).expect("app.php");
        }
        class_aliases(dir.path())
            .into_iter()
            .map(|alias| (alias.alias, alias.class))
            .collect()
    }

    #[test]
    fn a_project_with_no_list_has_the_frameworks() {
        assert_eq!(
            aliases_of(None),
            [
                ("DB", "Illuminate\\Support\\Facades\\DB"),
                ("Eloquent", "Illuminate\\Database\\Eloquent\\Model"),
                ("Str", "Illuminate\\Support\\Str"),
            ]
            .map(|(alias, class)| (alias.to_owned(), class.to_owned()))
        );
    }

    #[test]
    fn a_list_merged_into_the_defaults_adds_to_them() {
        let config = r"<?php

use Illuminate\Support\Facades\Facade;

return [
    'name' => 'App',
    'aliases' => Facade::defaultAliases()->merge([
        // 'Commented' => Gone::class,
        'Activity' => App\Facades\Activity::class,
        'Theme' => 'App\Facades\Theme',
    ])->toArray(),
    'proxies' => [],
];
";
        let found = aliases_of(Some(config));
        assert_eq!(found.len(), 5);
        assert!(found.contains(&("Activity".to_owned(), "App\\Facades\\Activity".to_owned())));
        assert!(found.contains(&("Theme".to_owned(), "App\\Facades\\Theme".to_owned())));
    }

    #[test]
    fn a_list_of_its_own_replaces_them() {
        let config = r"<?php
return [
    'aliases' => [
        'App' => Illuminate\Support\Facades\App::class,
    ],
];
";
        assert_eq!(
            aliases_of(Some(config)),
            [(
                "App".to_owned(),
                "Illuminate\\Support\\Facades\\App".to_owned()
            )]
        );
    }
}
