//! The strings Drupal code writes that name something declared in YAML.
//!
//! A route name and a permission id are the two of these a real project writes most: 106 and 141
//! respectively on the one measured here, against five uses of every plugin-manager id put together.
//! Neither is a name PHP has, so nothing but a reader of the declaring file can say whether one
//! exists -- and until something does, the string is the one thing at that position with no meaning
//! at all.
//!
//! A library name is the third, and the odd one out: it is not read by a call at all, only written
//! as `'#attached' => ['library' => ['a/b']]`, so [`ArrayKeyBinding`] carries it rather than
//! [`ArgumentBinding`]. Filled the same way as the other two -- one file per extension, named after
//! it, with the id at the top level -- so it costs the same nothing: the walk over installed
//! extensions already runs for routes and permissions, and this reads a third file while it is
//! there.
//!
//! # Why this catalog is closed
//!
//! Unlike routes and permissions, which core can generate at request time from data this reader
//! never sees, every library id a real project's PHP names is declared in a `*.libraries.yml` this
//! reader loads in full: 18 of 18 distinct names on the checkout measured here, including core's
//! own and one read off a service's `getAttachments()` rather than a literal `#attached`. The one
//! escape hatch, `hook_library_info_build()`, defines a library with no YAML entry at all -- but the
//! one implementation on that checkout uses `hook_library_info_alter()` instead, which edits an
//! entry already declared, not one this reader would otherwise miss. That is a real risk for a
//! project that leans on the runtime hook harder than this one does; it is not the risk `#type`
//! carries, where an old-style annotation is a second *declaration* spelling this reader never
//! reads at all.

use std::path::{Path, PathBuf};

use crate::{
    keys::keys_in,
    rule::{ArgumentBinding, ArrayKeyBinding, PlaceEntry},
};

/// Names this provider chose. Nothing below the seam knows what either means.
pub(super) const ROUTES: &str = "route";
pub(super) const PERMISSIONS: &str = "permission";
pub(super) const LIBRARIES: &str = "library";

/// The one array key that names a library: `'#attached' => ['library' => ['a/b']]`.
pub(super) static ARRAY_KEYS: &[ArrayKeyBinding] = &[ArrayKeyBinding {
    key: "library",
    catalog: LIBRARIES,
}];

/// The calls whose argument at a stated position is a route name or a permission id.
///
/// Nearly always the first, because a method that looks a value up by a string puts the string
/// first. `AccessResult::allowedIfHasPermission($account, $permission)` is the one that is not, and
/// it is the canonical way to write an access check in a controller or an access handler -- so the
/// binding states `1`, and binding it without a position would have bound the account.
///
/// `allowedIfHasPermissions($account, array $permissions)` beside it is a *list* of them, which is a
/// different shape and not one this rule can say: the keys are entries of an array literal rather
/// than an argument. `ArrayKeyBinding` is the mechanism for that shape and it is keyed by the array
/// key rather than by the parameter, so there is nothing to hang it on here.
pub(super) static BINDINGS: &[ArgumentBinding] = &[
    ArgumentBinding {
        class: "Drupal\\Core\\Url",
        method: "fromRoute",
        at: 0,
        suffix: "",
        catalog: ROUTES,
    },
    // `$this->redirect('route.name')` in every controller.
    ArgumentBinding {
        class: "Drupal\\Core\\Controller\\ControllerBase",
        method: "redirect",
        at: 0,
        suffix: "",
        catalog: ROUTES,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Routing\\UrlGeneratorInterface",
        method: "generateFromRoute",
        at: 0,
        suffix: "",
        catalog: ROUTES,
    },
    // Where a submit handler says what to do next, and on the measured project the second-commonest
    // route name in the codebase: 63 literals against 99 for `Url::fromRoute` and 3 for the
    // generator. Declared on the interface rather than on `FormState`, because
    // `FormStateDecoratorBase` implements it too and every subform in a project goes through one.
    ArgumentBinding {
        class: "Drupal\\Core\\Form\\FormStateInterface",
        method: "setRedirect",
        at: 0,
        suffix: "",
        catalog: ROUTES,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Session\\AccountInterface",
        method: "hasPermission",
        at: 0,
        suffix: "",
        catalog: PERMISSIONS,
    },
    // The same catalog reached from the other side. Declared on `AccessResult` itself because it is
    // a static factory rather than something an interface promises.
    ArgumentBinding {
        class: "Drupal\\Core\\Access\\AccessResult",
        method: "allowedIfHasPermission",
        at: 1,
        suffix: "",
        catalog: PERMISSIONS,
    },
];

/// Every route, permission and library id the installed extensions declare.
///
/// One file per extension for each, named after it, which is Drupal's own rule -- the same rule the
/// service reader follows, and the reason none of the three costs a walk of its own: the extensions
/// are the ones already found for their namespaces.
///
/// A route's `path`, a permission's `title` and a library's `version` come back with them, because a
/// list of a hundred route names is unreadable and a list of a hundred names each beside its URL is
/// not.
pub(super) fn catalog_places(docroot: &Path, extensions: &[PathBuf]) -> Vec<PlaceEntry> {
    let mut found = Vec::new();

    // Core declares both too, and a project's own code links to core's routes constantly --
    // `entity.node.canonical` is in every template and half the controllers.
    for module in core_modules(docroot) {
        read_into(&mut found, &module, &module_name(&module));
    }
    for info in extensions {
        let Some(directory) = info.parent() else {
            continue;
        };
        let Some(name) = super::extensions::machine_name(info) else {
            continue;
        };
        read_into(&mut found, directory, name);
    }

    found
}

/// Both files an extension may declare, if they are there.
fn read_into(into: &mut Vec<PlaceEntry>, directory: &Path, name: &str) {
    for (catalog, suffix, field) in [
        (ROUTES, "routing.yml", "path"),
        (PERMISSIONS, "permissions.yml", "title"),
        (LIBRARIES, "libraries.yml", "version"),
    ] {
        let path = directory.join(format!("{name}.{suffix}"));
        // Bytes rather than a string, for the reason the service reader gives: a hand-edited
        // configuration file can carry a stray byte in a comment, and `read_to_string` would fail on
        // it and silently drop every key the file declares.
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        for entry in keys_in(&text, field) {
            // A library's own file names it bare -- `editor:` in `app_core.libraries.yml` -- but
            // every reference to it, in `#attached` and in Drupal's own runtime lookup, is namespaced
            // by the extension that declared it: `app_core/editor`. A route and a permission carry no
            // such prefix; their own ids are already the whole dotted name a call writes.
            let key = if catalog == LIBRARIES {
                format!("{name}/{}", entry.key)
            } else {
                entry.key
            };
            into.push(PlaceEntry {
                catalog,
                key,
                path: path.clone(),
                line: entry.line,
                character: entry.character,
                detail: entry.detail.unwrap_or_default(),
            });
        }
    }
}

/// Every directory under `core/modules`, which is where core's own extensions live.
///
/// Listed rather than discovered through `*.info.yml` like the rest: core's modules are found for
/// their namespaces already, but that list is the *installed* ones, and a route in an uninstalled
/// core module is still a route somebody writes a link to.
pub(super) fn core_directories(docroot: &Path) -> Vec<PathBuf> {
    core_modules(docroot)
}

fn core_modules(docroot: &Path) -> Vec<PathBuf> {
    let mut found = vec![docroot.join("core/lib/Drupal/Core"), docroot.join("core")];
    let Ok(entries) = std::fs::read_dir(docroot.join("core/modules")) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.push(path);
        }
    }
    found
}

/// The machine name a directory's files are named after.
fn module_name(directory: &Path) -> String {
    directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use phrost_testkit::MaterializedFixture;

    use super::{ARRAY_KEYS, BINDINGS, LIBRARIES, PERMISSIONS, ROUTES};
    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    /// Which calls read one of these strings, as a whole list.
    ///
    /// A whole list and not a search for one entry: what matters about this set is what is *missing*
    /// from it, and a test that asks whether a particular binding is present can never say that. Each
    /// line here is a place a route name can be written and be understood; every other spelling in a
    /// project is a string with no meaning at that position.
    #[test]
    fn the_calls_that_read_a_route_or_a_permission() {
        let rendered = BINDINGS
            .iter()
            .map(|binding| {
                // The position too, because it is part of the rule and not a detail of it: reading
                // the wrong argument binds the account rather than the permission it guards.
                format!(
                    "{}::{} #{} -> {}\n",
                    binding.class, binding.method, binding.at, binding.catalog
                )
            })
            .collect::<String>();

        expect![[r#"
            Drupal\Core\Url::fromRoute #0 -> route
            Drupal\Core\Controller\ControllerBase::redirect #0 -> route
            Drupal\Core\Routing\UrlGeneratorInterface::generateFromRoute #0 -> route
            Drupal\Core\Form\FormStateInterface::setRedirect #0 -> route
            Drupal\Core\Session\AccountInterface::hasPermission #0 -> permission
            Drupal\Core\Access\AccessResult::allowedIfHasPermission #1 -> permission
        "#]]
        .assert_eq(&rendered);
    }

    /// Core's files and each extension's, with the detail that makes a list of them readable.
    ///
    /// Only two of the three catalogs this module fills: the provider's `catalog_places` also asks
    /// the field reader for its own, and library ids get their own test below because the shape of
    /// their key -- an array entry rather than a call argument -- is a different fact to check.
    #[test]
    fn core_and_every_extension_contribute_their_routes_and_permissions() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/modules/node/node.routing.yml
entity.node.canonical:
  path: '/node/{node}'
  defaults:
    _entity_view: 'node.full'

//- /docroot/core/modules/node/node.permissions.yml
'access content':
  title: 'View published content'

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/mine.routing.yml
mine.settings:
  path: '/admin/mine'

//- /docroot/modules/custom/mine/mine.permissions.yml
administer mine:
  title: 'Administer Mine'
"#,
        );

        let mut rendered = Drupal
            .project(fixture.root())
            .catalog_places()
            .into_iter()
            .filter(|place| place.catalog == ROUTES || place.catalog == PERMISSIONS)
            .map(|place| {
                format!(
                    "{} {} at {}:{}:{} -- {}\n",
                    place.catalog,
                    place.key,
                    place
                        .path
                        .strip_prefix(fixture.root())
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    place.line,
                    place.character,
                    place.detail,
                )
            })
            .collect::<Vec<_>>();
        rendered.sort();

        expect![[r#"
            permission access content at docroot/core/modules/node/node.permissions.yml:0:1 -- View published content
            permission administer mine at docroot/modules/custom/mine/mine.permissions.yml:0:0 -- Administer Mine
            route entity.node.canonical at docroot/core/modules/node/node.routing.yml:0:0 -- /node/{node}
            route mine.settings at docroot/modules/custom/mine/mine.routing.yml:0:0 -- /admin/mine
        "#]]
        .assert_eq(&rendered.concat());
    }

    /// The one array key that names a library, as a whole list for the reason
    /// [`the_calls_that_read_a_route_or_a_permission`] gives: what matters is what is missing.
    #[test]
    fn the_array_key_that_names_a_library() {
        let rendered = ARRAY_KEYS
            .iter()
            .map(|binding| format!("{} -> {}\n", binding.key, binding.catalog))
            .collect::<String>();

        expect![[r#"
            library -> library
        "#]]
        .assert_eq(&rendered);
    }

    /// Core's and each extension's library ids, with the `version` field that makes a list of them
    /// readable -- the same role a route's `path` and a permission's `title` play above.
    #[test]
    fn core_and_every_extension_contribute_their_library_names() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/core.libraries.yml
drupal.dialog.ajax:
  version: VERSION

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/mine.libraries.yml
app:
  version: 1.x
  js:
    app.js: {}
"#,
        );

        let mut rendered = Drupal
            .project(fixture.root())
            .catalog_places()
            .into_iter()
            .filter(|place| place.catalog == LIBRARIES)
            .map(|place| {
                format!(
                    "{} {} at {}:{}:{} -- {}\n",
                    place.catalog,
                    place.key,
                    place
                        .path
                        .strip_prefix(fixture.root())
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    place.line,
                    place.character,
                    place.detail,
                )
            })
            .collect::<Vec<_>>();
        rendered.sort();

        expect![[r#"
            library core/drupal.dialog.ajax at docroot/core/core.libraries.yml:0:0 -- VERSION
            library mine/app at docroot/modules/custom/mine/mine.libraries.yml:0:0 -- 1.x
        "#]]
        .assert_eq(&rendered.concat());
    }

    #[test]
    fn a_project_that_is_not_drupal_declares_no_places() {
        let fixture = MaterializedFixture::parse("//- /index.php\n<?php");
        assert_eq!(Drupal.project(fixture.root()).catalog_places(), Vec::new());
    }
}
