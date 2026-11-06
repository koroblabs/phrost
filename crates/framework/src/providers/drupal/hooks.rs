//! The docblock line that is the only documentation a hook implementation has.
//!
//! A hook implementation is a plain function whose *name* is the contract: `app_core_form_alter()`
//! runs whenever any form is built, because it is named `<extension>_form_alter` and the kernel goes
//! looking for exactly that. The standard asks for one line of documentation above it --
//! `Implements hook_form_alter().` -- because the twenty lines that say what the parameters mean are
//! written once, on `hook_form_alter()` in a `*.api.php` file. One real checkout declares 306 hooks
//! across 57 such files, and the index already holds every one of them.
//!
//! So the sentence is a cross-reference, and two things follow from reading it as one. Documentation
//! can be shown where a person is actually standing, and the claim can be *checked* -- because
//! dispatch is by name, a claim written over a function no installed extension could own is a claim
//! the runtime will never honour. The function silently does nothing, which is why nothing catches
//! it today.
//!
//! What the check must not do is report the placeholders. Hook names have variable parts, written in
//! uppercase -- `hook_form_FORM_ID_alter`, `hook_ENTITY_TYPE_presave`, `hook_preprocess_HOOK` -- and
//! `app_core_form_views_exposed_form_alter` implements the first of those perfectly. The rule that
//! an uppercase run is a hole is not a list written from memory: of the 333 hook names one real
//! checkout declares, 43 carry a placeholder, every one of them is an uppercase run, and no hook
//! spells anything else in uppercase at all.

use std::path::PathBuf;

use super::extensions::machine_name;
use crate::rule::{
    ArgumentBinding, CatalogEntry, DerivedName, DispatchedName, DocumentedBy, UserFacingDocblock,
};

/// The names an implementation may be prefixed with: every installed extension's machine name.
///
/// Filled rather than written out, for the reason every catalog here is: which extensions are
/// installed is a fact about the checkout. Read by the name check through the seam, which is handed
/// this name and knows nothing else about it.
pub(super) const CATALOG: &str = "extension";

/// The hooks a project dispatches itself, filled from its own `invokeAll` and `alter` calls.
///
/// Kept apart from [`CATALOG`] because the two answer different questions about one name: that one
/// says whether `app_region` could own a function, this one says whether `app_core_order_reports
/// _requested` is a hook at all.
pub(super) const DISPATCHED: &str = "dispatched hook";

/// Where a hook name that *something implements or documents* is a key.
///
/// The other side of [`DISPATCHED`], and the two answer different questions about one name. That one
/// is filled from the project's own `invokeAll` and `alter` calls, so a typo at such a call registers
/// itself and no report can be made about it -- which is exactly why the near-miss reader was silent
/// on a misspelt hook while it reported a misspelt route.
///
/// This one is filled from the names of the functions that implement hooks, and from the
/// `function hook_x()` declarations that document them, so a typo at a *call* cannot reach it.
/// Measured on one real checkout: **462 names**, and a deliberate typo of each of the six hooks the
/// project dispatches suggests the right one, while the four the catalog does not hold -- a hook
/// nothing has implemented yet -- suggest nothing at all.
///
/// Open, and it can never be closed: a hook nobody has implemented is a perfectly ordinary extension
/// point, and reporting an absence would light up every new one on the day it is written.
pub(super) const IMPLEMENTED: &str = "hook";

/// A hook implementation's name *is* the contract, so the name is where the catalog comes from.
///
/// `app_core_notifications_alter()` implements `notifications_alter` because it is named
/// `<extension>_<hook>`; the extensions are [`CATALOG`], filled from the checkout's own
/// `*.info.yml`. And `function hook_theme()` in an `*.api.php` documents one, which is the same fact
/// written the other way round.
pub(super) static DERIVED_NAMES: &[DerivedName] = &[DerivedName {
    prefixed_by: CATALOG,
    or_prefix: PREFIX,
    catalog: IMPLEMENTED,
}];

/// The dispatching calls, and which argument of each is the hook.
///
/// `invoke($module, $hook)` puts it second, which is why the binding carries a position at all --
/// every other dispatcher puts it first. Bound to [`IMPLEMENTED`] rather than to [`DISPATCHED`],
/// because a call cannot be measured against a catalog it fills.
pub(super) static BINDINGS: &[ArgumentBinding] = &[
    ArgumentBinding {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "invokeAll",
        at: 0,
        suffix: "",
        catalog: IMPLEMENTED,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "invokeAllWith",
        at: 0,
        suffix: "",
        catalog: IMPLEMENTED,
    },
    // `alter($type)` runs `hook_{$type}_alter`, so its argument is a hook name with the suffix
    // taken off -- and reading it as a whole hook name judged `alter('form')` against a key called
    // `form`. What made that look harmless is that both `hook_entity_view` and
    // `hook_entity_view_alter` exist, so a stem often happens to be a hook name too; where it does
    // not, the suggestion for a typo was a name nobody can write at that position.
    ArgumentBinding {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "alter",
        at: 0,
        suffix: "_alter",
        catalog: IMPLEMENTED,
    },
    // The one that puts the module first and the hook second.
    ArgumentBinding {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "invoke",
        at: 1,
        suffix: "",
        catalog: IMPLEMENTED,
    },
];

/// What a hook implementation's name and its documentation both begin with.
///
/// One constant read by both rules below, because a `hook_` written twice is a `hook_` that can be
/// changed once -- and the two halves would then silently stop meeting.
const PREFIX: &str = "hook_";

/// Drupal's own second spelling of `{@inheritdoc}`, and what it binds.
///
/// Two prefixes, because the standard has two. A hook is dispatched by name, so the second half of
/// the rule can check what the claiming function is called; a **callback** is passed as a callable
/// and may be named anything, so nothing about the claimant is checked -- and nothing dispatches it
/// by a literal name either. Both are declared the same way and in the same kind of file:
/// `callback_batch_operation()` and `callback_batch_finished()` live in `core/lib/Drupal/Core/Form/
/// form.api.php` beside the hooks, `callback_allowed_values_function()` in `options.api.php`, and
/// one real checkout writes `Implements callback_batch_operation().` 33 times and
/// `Implements callback_batch_finished().` 21 more.
pub(super) static DOCUMENTED_BY: &[DocumentedBy] = &[
    DocumentedBy {
        lead: "Implements",
        names: PREFIX,
        implementers: Some(CATALOG),
        dispatched: Some(DISPATCHED),
    },
    DocumentedBy {
        lead: "Implements",
        names: "callback_",
        implementers: None,
        dispatched: None,
    },
];

/// The two hooks whose docblock Drupal publishes rather than merely stores.
///
/// `hook_update_N` and `hook_post_update_NAME`. What `update.php` prints beside a pending update,
/// and what `drush updb` lists before asking whether to run it, is the function's own docblock with
/// the stars stripped and the lines run together -- `update_get_update_list()` for the first and
/// `UpdateRegistry::getPendingUpdateInformation()` for the second, both by reflection. So the
/// docblock is required, and a `@return string` generated into it is read aloud to whoever is
/// deciding whether to migrate a production database.
///
/// Neither is discovered by the file it is written in, so neither rule names one: Drupal loads
/// `.install` and `*.post_update.php` and then matches over `get_defined_functions()`.
///
/// `_post_update_` is spelled out rather than left to the rule above it, though every name carrying
/// it carries `_update_` too. The two differ in what may follow -- an update is numbered and a post
/// update is named -- so the numbered rule alone would exempt none of them.
pub(super) static USER_FACING_DOCBLOCKS: &[UserFacingDocblock] = &[
    UserFacingDocblock {
        names: "_update_",
        numbered: true,
    },
    UserFacingDocblock {
        names: "_post_update_",
        numbered: false,
    },
];

/// The module handler, whose four dispatching methods are how a Drupal project declares a hook of
/// its own.
///
/// `invokeAll('notifications_alter')` is the whole declaration of `hook_notifications_alter`: there
/// is no `*.api.php` file, no signature, and nothing else in the checkout that says the name exists.
/// One real project dispatches five such hooks and documents none of them, and every implementation
/// of one references a name the index could not hold.
///
/// `invoke($module, $hook)` names the hook *second* -- the first argument is the extension to run it
/// on -- and `alter($type)` runs `hook_{$type}_alter`, which is why the suffix travels with the
/// method rather than with the framework. `alter` also takes a list, for the documented spelling
/// that dispatches a general and a specific form of one point together.
///
/// The interface and not the implementation, because a service is typed by what it is injected as:
/// every one of those five call sites is written against `ModuleHandlerInterface`, and the seam's
/// reader covers everything below it anyway.
pub(super) static DISPATCHED_NAMES: &[DispatchedName] = &[
    DispatchedName {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "invokeAll",
        argument: 0,
        several: false,
        names: PREFIX,
        suffix: "",
        catalog: DISPATCHED,
    },
    DispatchedName {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "invokeAllWith",
        argument: 0,
        several: false,
        names: PREFIX,
        suffix: "",
        catalog: DISPATCHED,
    },
    DispatchedName {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "invoke",
        argument: 1,
        several: false,
        names: PREFIX,
        suffix: "",
        catalog: DISPATCHED,
    },
    DispatchedName {
        class: "Drupal\\Core\\Extension\\ModuleHandlerInterface",
        method: "alter",
        argument: 0,
        several: true,
        names: PREFIX,
        suffix: "_alter",
        catalog: DISPATCHED,
    },
];

/// Every installed extension, as a key naming nothing.
///
/// Nothing, because there is nothing for the name to be: an extension is a directory and a
/// descriptor, not a class. What the catalog is for is membership -- whether `app_region` is
/// something that could own a function called `app_region_notifications_alter` -- and for that the key
/// is the whole answer.
///
/// Fed the descriptors the provider has already walked for, rather than walking again: the same list
/// fills the service catalog and the place catalog, and a second walk of a real docroot is a second
/// second of bootstrap.
pub(super) fn catalog_entries(extensions: &[PathBuf]) -> Vec<CatalogEntry> {
    extensions
        .iter()
        .filter_map(|info| Some((machine_name(info)?.to_owned(), info)))
        .map(|(key, info)| CatalogEntry {
            catalog: CATALOG,
            key,
            names: None,
            declared_in: Some(info.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    /// The catalog is what makes the name check able to tell an owner from a coincidence, so the
    /// question it has to answer is which extensions exist -- core's, contrib's and the site's own,
    /// themes and profiles included, since all four kinds implement hooks.
    #[test]
    fn every_installed_extension_is_a_name_an_implementation_may_start_with() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/modules/node/node.info.yml
name: Node

//- /docroot/modules/contrib/token/token.info.yml
name: Token

//- /docroot/modules/custom/app_core/app_core.info.yml
name: App Core

//- /docroot/themes/custom/plain/plain.info.yml
name: Plain

//- /docroot/profiles/app/app.info.yml
name: App
"#,
        );

        let mut found = Drupal
            .project(fixture.root())
            .catalog_entries()
            .into_iter()
            .filter(|entry| entry.catalog == super::CATALOG)
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        found.sort();

        assert_eq!(found, ["app", "app_core", "node", "plain", "token"]);
    }

    /// A project that is not Drupal contributes no convention, so nothing above reads a docblock
    /// looking for one.
    #[test]
    fn a_project_that_is_not_drupal_states_no_convention() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /src/App.php
<?php class App {}
"#,
        );

        assert!(!Drupal.detect(fixture.root()));
        assert!(Drupal.project(fixture.root()).catalog_entries().is_empty());
    }

    /// A convention that names a catalog names one this module fills.
    ///
    /// Two halves that have to agree by name and nothing else checks that they do: a convention
    /// naming a catalog nobody fills silently reports every implementation in the project, which is
    /// the expensive direction.
    ///
    /// Naming *no* catalog is a different statement and a legitimate one -- a callback is passed as
    /// a callable and may be called anything, so there is nothing about the claiming declaration's
    /// name to check. What this refuses is a name that means nothing.
    #[test]
    fn a_convention_that_names_a_catalog_names_one_this_module_fills() {
        for rule in Drupal.documented_elsewhere() {
            if let Some(catalog) = rule.implementers {
                assert_eq!(
                    catalog,
                    super::CATALOG,
                    "the convention is checked against a catalog this module does not fill"
                );
            }
        }
    }
}
