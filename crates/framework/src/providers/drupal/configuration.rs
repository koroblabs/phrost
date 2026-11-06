//! What a Drupal YAML file names, and where each name lives.
//!
//! The half of a Drupal project that is not PHP and is full of references to PHP. Measured on one
//! real checkout, in the project's **own** modules alone: 744 container ids written as `'@id'`, 277
//! classes, 144 controllers and forms, 81 permissions, 131 route names, 37 library dependencies --
//! 1 414 references, and not one of them navigable before this.
//!
//! Every one of them names something already held: the service, route, permission and library
//! catalogs are filled at bootstrap by [`services`](super::services) and [`places`](super::places),
//! and a class name is a class name. So nothing new is indexed here -- what was missing is the
//! statement of which key means which table.
//!
//! # Why the file suffix and not the directory
//!
//! Drupal names a configuration file after the extension that owns it: `app_core.routing.yml`,
//! `app_core.services.yml`. The suffix is the whole of what says which kind of file it is, and it is
//! the same rule [`places::catalog_places`](super::places) already reads them by.

use crate::rule::{YamlReference, YamlTarget};

use super::{places, services};

pub(super) static REFERENCES: &[YamlReference] = &[
    // A container id, written the way Symfony's container writes one. Recognised by the `@` rather
    // than by a key, because it appears as a bare list item under `arguments:`, under `calls:`, and
    // under `tags:` -- and naming all the places it can appear would be a list that goes stale
    // against a file the framework keeps adding keys to.
    //
    // `'@?id'` is the same reference to a service that may not exist -- the container passes null
    // where nothing registers it, and core writes it for a module that may not be enabled
    // (`'@?workspaces.tracker'`). Read with the `?` in the name it was an error on every one.
    YamlReference {
        in_files: "services.yml",
        under: "",
        prefix: "@",
        optional: "?",
        names: YamlTarget::CatalogKey(services::CATALOG),
    },
    // What a service is. `factory` too, whose value is a class when it is written alone -- and a
    // `['@id', 'method']` pair when it is not, where the `@id` is read by the rule above.
    YamlReference {
        in_files: "services.yml",
        under: "class",
        prefix: "",
        optional: "",
        names: YamlTarget::Class,
    },
    YamlReference {
        in_files: "services.yml",
        under: "factory",
        prefix: "",
        optional: "",
        names: YamlTarget::Factory,
    },
    // `calls: [[addExportType, ['@a']], ...]` -- the container's own way of assembling a
    // collector, and the only caller those methods have. Counted on one real checkout: 33 methods
    // named this way and reached by no PHP at all, so reading the key is the difference between a
    // report of dead code and a report of how that project wires its services. Read by the same
    // rule as `factory`: the first element is a bare method name and the arguments beside it are
    // `@id`s the container-id rule already claims.
    YamlReference {
        in_files: "services.yml",
        under: "calls",
        prefix: "",
        optional: "",
        names: YamlTarget::Factory,
    },
    // A route's own PHP: the controller it dispatches to, and the form class it builds.
    YamlReference {
        in_files: "routing.yml",
        under: "_controller",
        prefix: "",
        optional: "",
        names: YamlTarget::Callable,
    },
    YamlReference {
        in_files: "routing.yml",
        under: "_form",
        prefix: "",
        optional: "",
        names: YamlTarget::Class,
    },
    // The other two callables a route writes, and they were missing while `_controller` was here
    // -- which is the whole of why they are worth a note. Counted on one real checkout:
    // `_custom_access` 80 and `_title_callback` 30, against `_controller`'s 642, so a fifth of the
    // methods a route names had no goto, appeared in no reference list, and were left behind by a
    // rename. A key is not covered by the key beside it.
    YamlReference {
        in_files: "routing.yml",
        under: "_custom_access",
        prefix: "",
        optional: "",
        names: YamlTarget::Callable,
    },
    YamlReference {
        in_files: "routing.yml",
        under: "_title_callback",
        prefix: "",
        optional: "",
        names: YamlTarget::Callable,
    },
    // `+` is Drupal's *or* and `,` its *and*, and a reader on one of them wants that one.
    YamlReference {
        in_files: "routing.yml",
        under: "_permission",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKeys {
            catalog: places::PERMISSIONS,
            separators: "+,",
        },
    },
    // The three link files, which are the same two keys under three names. `base_route` is a task's
    // parent tab and `appears_on` an action's list of routes; both are route names like `route_name`
    // itself, so all three read the same table.
    YamlReference {
        in_files: "links.menu.yml",
        under: "route_name",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(places::ROUTES),
    },
    YamlReference {
        in_files: "links.task.yml",
        under: "route_name",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(places::ROUTES),
    },
    YamlReference {
        in_files: "links.task.yml",
        under: "base_route",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(places::ROUTES),
    },
    YamlReference {
        in_files: "links.action.yml",
        under: "route_name",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(places::ROUTES),
    },
    YamlReference {
        in_files: "links.action.yml",
        under: "appears_on",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(places::ROUTES),
    },
    // A library names its dependencies as `<extension>/<library>`, which is the spelling the
    // catalog is keyed by -- see `places::read_into` for why the prefix is added there and not here.
    YamlReference {
        in_files: "libraries.yml",
        under: "dependencies",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(places::LIBRARIES),
    },
    // What a configuration object depends on: `dependencies: config: [field.storage.node.body]` in
    // every exported file and every one a module ships. Any YAML file, because a configuration file
    // is named after the object it holds rather than by a suffix -- and nothing else Drupal reads
    // writes a list of names under a key called `config`.
    YamlReference {
        in_files: "yml",
        under: "config",
        prefix: "",
        optional: "",
        names: YamlTarget::CatalogKey(super::config::CONFIG),
    },
];

#[cfg(test)]
mod tests {
    use super::REFERENCES;
    use crate::rule::YamlTarget;

    /// A rule recognised by neither a key nor a prefix would match every scalar in the file.
    ///
    /// The one way this table can be wrong that nothing else would catch: an empty `under` is
    /// deliberate for the `@` form and meaningless without something to recognise the value by, so
    /// a rule with neither would claim every string in a services file as a service id.
    #[test]
    fn every_rule_is_recognised_by_a_key_or_by_a_prefix() {
        for rule in REFERENCES {
            assert!(
                !rule.under.is_empty() || !rule.prefix.is_empty(),
                "a rule for {} recognises nothing and would match every scalar in the file",
                rule.in_files
            );
        }
    }

    /// The suffix is matched against the end of a file's name, and written without a leading dot:
    /// `routing.yml` for one kind of file, `yml` for a rule about every one.
    #[test]
    fn a_file_suffix_is_written_the_way_a_name_ends() {
        for rule in REFERENCES {
            assert!(!rule.in_files.starts_with('.'), "{}", rule.in_files);
            assert!(rule.in_files.ends_with("yml"), "{}", rule.in_files);
        }
    }

    /// Every catalog named here is one this provider actually fills.
    ///
    /// A typo is silence: the rule matches, the catalog is empty, and the reference answers nothing
    /// -- which reads exactly like the feature not being built.
    #[test]
    fn every_catalog_named_is_one_this_provider_fills() {
        let filled = [
            super::services::CATALOG,
            super::places::ROUTES,
            super::places::PERMISSIONS,
            super::places::LIBRARIES,
            super::super::config::CONFIG,
        ];
        for rule in REFERENCES {
            let named = match rule.names {
                YamlTarget::CatalogKey(catalog) | YamlTarget::CatalogKeys { catalog, .. } => {
                    catalog
                }
                YamlTarget::Class | YamlTarget::Callable | YamlTarget::Factory => continue,
            };
            assert!(
                filled.contains(&named),
                "`{named}` is named by a rule for {} and filled by nothing",
                rule.in_files
            );
        }
    }
}
