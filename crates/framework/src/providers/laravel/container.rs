//! What the container's own alias names resolve to.
//!
//! `$this->app['config']->get('app.timezone')` is how a Laravel project reaches its configuration,
//! and every part of it is untypeable from the declarations alone: `offsetGet` returns `mixed`,
//! `make` returns `mixed`, and the alias is a string. So the receiver of `->get` is nothing, and on
//! the framework's own source that is **575 subscript sites** against 80 written as `make`.
//!
//! # One fact, three spellings
//!
//! `$app['config']` *is* `offsetGet('config')`, whose body is `return $this->make($offset);`, and
//! `make` is what a `Container` contract's `get` forwards to as well. So this is not three rules --
//! it is one, bound to three method names, and the generic layer reaches the same reader for all of
//! them. The subscript needed one arm in `IndexView::indexed_of_held`; the two calls needed nothing
//! at all, because a method with a literal first argument already goes through
//! `returned_from_argument`.
//!
//! The helpers are the fourth spelling and the one an application writes most: `app('router')` and
//! `resolve('cache')` are `make` with the container left out, 180 calls in the framework's own
//! source and 47 in a fresh application skeleton. A function has no class to bind a method on, so
//! they are bound as functions -- the same catalog, read through `FunctionBinding`.
//!
//! # Why the list is static, and why the catalog is open
//!
//! These forty are not a property of a *checkout*. They are written in one array literal in
//! `Application::registerCoreContainerAliases`, they are public API -- `app('config')` is in
//! Laravel's own documentation -- and they do not vary with what a project installs. That makes
//! them the same kind of fact as [`NonNullReturn`](crate::NonNullReturn): read off the framework's
//! source once, stated here, rather than parsed per project.
//!
//! And the catalog must stay **open**. An application binds its own ids constantly --
//! `$app->make(Repo::class)`, `$app['my.service']` -- so a closed catalog would report every one of
//! them as an unknown key. §8 records that exact mistake being made once already, for
//! `getInstanceFromDefinition`: a catalog binding is only safe where the argument is *always* a key
//! of that catalog, and "sometimes a key" needs the open kind.

use crate::CatalogEntry;

/// The catalog these keys live in. Nothing below the seam knows what the name means.
pub(super) const CATALOG: &str = "container";

/// Alias to the concrete class it resolves to, as `registerCoreContainerAliases` writes them.
///
/// The *first* class of each entry, which is the one the container builds; the rest of each array is
/// the contracts it also answers under, and binding one of those would answer an interface where
/// the object is a concrete class -- less than the truth, and less than a person reading the alias
/// expects.
const ALIASES: &[(&str, &str)] = &[
    ("app", "Illuminate\\Foundation\\Application"),
    ("auth", "Illuminate\\Auth\\AuthManager"),
    ("auth.driver", "Illuminate\\Contracts\\Auth\\Guard"),
    (
        "auth.password",
        "Illuminate\\Auth\\Passwords\\PasswordBrokerManager",
    ),
    (
        "auth.password.broker",
        "Illuminate\\Auth\\Passwords\\PasswordBroker",
    ),
    (
        "blade.compiler",
        "Illuminate\\View\\Compilers\\BladeCompiler",
    ),
    ("cache", "Illuminate\\Cache\\CacheManager"),
    ("cache.store", "Illuminate\\Cache\\Repository"),
    (
        "cache.psr6",
        "Symfony\\Component\\Cache\\Adapter\\Psr16Adapter",
    ),
    ("config", "Illuminate\\Config\\Repository"),
    ("cookie", "Illuminate\\Cookie\\CookieJar"),
    ("db", "Illuminate\\Database\\DatabaseManager"),
    ("db.connection", "Illuminate\\Database\\Connection"),
    ("db.schema", "Illuminate\\Database\\Schema\\Builder"),
    ("encrypter", "Illuminate\\Encryption\\Encrypter"),
    ("events", "Illuminate\\Events\\Dispatcher"),
    ("files", "Illuminate\\Filesystem\\Filesystem"),
    ("filesystem", "Illuminate\\Filesystem\\FilesystemManager"),
    (
        "filesystem.disk",
        "Illuminate\\Contracts\\Filesystem\\Filesystem",
    ),
    (
        "filesystem.cloud",
        "Illuminate\\Contracts\\Filesystem\\Cloud",
    ),
    ("hash", "Illuminate\\Hashing\\HashManager"),
    ("hash.driver", "Illuminate\\Contracts\\Hashing\\Hasher"),
    ("image", "Illuminate\\Image\\ImageManager"),
    ("log", "Illuminate\\Log\\LogManager"),
    ("mail.manager", "Illuminate\\Mail\\MailManager"),
    ("mailer", "Illuminate\\Mail\\Mailer"),
    ("queue", "Illuminate\\Queue\\QueueManager"),
    ("queue.connection", "Illuminate\\Contracts\\Queue\\Queue"),
    (
        "queue.failer",
        "Illuminate\\Queue\\Failed\\FailedJobProviderInterface",
    ),
    ("redirect", "Illuminate\\Routing\\Redirector"),
    ("redis", "Illuminate\\Redis\\RedisManager"),
    (
        "redis.connection",
        "Illuminate\\Redis\\Connections\\Connection",
    ),
    ("request", "Illuminate\\Http\\Request"),
    ("router", "Illuminate\\Routing\\Router"),
    ("session", "Illuminate\\Session\\SessionManager"),
    ("session.store", "Illuminate\\Session\\Store"),
    ("translator", "Illuminate\\Translation\\Translator"),
    ("url", "Illuminate\\Routing\\UrlGenerator"),
    ("validator", "Illuminate\\Validation\\Factory"),
    ("view", "Illuminate\\View\\Factory"),
];

/// The aliases as catalog entries.
pub(super) fn catalog_entries() -> Vec<CatalogEntry> {
    ALIASES
        .iter()
        .map(|(key, class)| CatalogEntry {
            catalog: CATALOG,
            key: (*key).to_owned(),
            names: Some(crate::CatalogName::Class((*class).to_owned())),
            // The framework's own bindings, declared in no file of the project's.
            declared_in: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every alias names a class the way the index stores one, and none of them repeats.
    ///
    /// A leading `\` or a single-quoted separator matches nothing, so the entry would simply do
    /// nothing -- the silent failure the workspace-wide contract check exists for, which cannot see
    /// these because a catalog entry carries its class as data rather than as a contract field.
    #[test]
    fn every_alias_names_a_class_the_index_could_hold() {
        let mut seen = Vec::new();
        for (key, class) in ALIASES {
            assert!(
                !class.starts_with('\\') && !class.contains('/'),
                "{key} names `{class}`, which is not how the index spells a class"
            );
            assert!(
                class.contains('\\'),
                "{key} names `{class}`, which has no namespace -- every one of these is namespaced"
            );
            assert!(!seen.contains(key), "{key} is listed twice");
            seen.push(key);
        }
        assert_eq!(seen.len(), 40, "the table Laravel writes has forty entries");
    }
}
