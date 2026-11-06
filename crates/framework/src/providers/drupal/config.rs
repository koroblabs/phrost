//! Which configuration objects exist, and where each one is written down.
//!
//! `\Drupal::config('system.site')`, `$this->config('mine.settings')` in a settings form,
//! `$configFactory->getEditable(...)`: a configuration object is named by a string, and the only
//! statement that it exists is a YAML file named after it. 215 such calls in the measured project's
//! own modules, and until this each was a string with no meaning at all -- no way from the call to
//! the values it reads, and no list of the names there are to write.
//!
//! The name is the file's own name, so reading the catalog opens no file: a directory listing per
//! extension, which is what the field reader beside this already walks.
//!
//! # Why this catalog is open
//!
//! A configuration object can be created at runtime -- `getEditable('new.name')->save()` is how a
//! module writes one -- and a project that has not exported its configuration has nothing on disk for
//! the objects a site's administrators made. So a name missing from every directory read here is not
//! a mistake this reader can see, and nothing is reported; goto, hover and completion are what it
//! gives.

use std::path::{Path, PathBuf};

use crate::rule::{ArgumentBinding, PlaceEntry};

/// Where a configuration object's name is a key. A word this provider chose.
pub(super) const CONFIG: &str = "config";

/// The calls whose first argument names a configuration object.
///
/// `FormBase::config()` covers `ConfigFormBase`, which every settings form extends and whose own
/// `config()` comes from a trait, because a binding is matched against the receiver's ancestors.
pub(super) static BINDINGS: &[ArgumentBinding] = &[
    ArgumentBinding {
        class: "Drupal",
        method: "config",
        at: 0,
        suffix: "",
        catalog: CONFIG,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Config\\ConfigFactoryInterface",
        method: "get",
        at: 0,
        suffix: "",
        catalog: CONFIG,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Config\\ConfigFactoryInterface",
        method: "getEditable",
        at: 0,
        suffix: "",
        catalog: CONFIG,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Form\\FormBase",
        method: "config",
        at: 0,
        suffix: "",
        catalog: CONFIG,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Controller\\ControllerBase",
        method: "config",
        at: 0,
        suffix: "",
        catalog: CONFIG,
    },
];

/// Every configuration object core, the extensions and the exported configuration declare.
///
/// The exported copy *last*, because a later entry for the same name wins and it is the one holding
/// the site's real values: `system.site` is shipped by core's system module and exported with the
/// site's own name in it, and a reader going to the definition wants the second.
pub(super) fn catalog_places(
    root: &Path,
    core: &[PathBuf],
    extensions: &[PathBuf],
) -> Vec<PlaceEntry> {
    let mut found = Vec::new();

    let shipped = core.iter().cloned().chain(
        extensions
            .iter()
            .filter_map(|info| info.parent().map(Path::to_path_buf)),
    );
    for extension in shipped {
        let owner = extension
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (directory, detail) in [
            (
                extension.join("config/install"),
                format!("shipped by {owner}"),
            ),
            (
                extension.join("config/optional"),
                format!("shipped by {owner}, optional"),
            ),
        ] {
            read_into(&mut found, &directory, &detail);
        }
    }
    for directory in exported_directories(root) {
        read_into(&mut found, &directory, "exported");
    }

    found
}

/// One entry per `*.yml` in `directory`, in name order so that a rerun reads the same.
fn read_into(into: &mut Vec<PlaceEntry>, directory: &Path, detail: &str) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "yml"))
        .collect::<Vec<_>>();
    files.sort();
    for path in files {
        let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        into.push(PlaceEntry {
            catalog: CONFIG,
            key: key.to_owned(),
            path: path.clone(),
            line: 0,
            character: 0,
            detail: detail.to_owned(),
        });
    }
}

/// Where the project keeps its exported configuration.
///
/// Found by the file Drupal requires in it -- `core.extension.yml` -- and not by reading
/// `$settings['config_sync_directory']`, which is an expression: one real project writes
/// `dirname(DRUPAL_ROOT) . '/config/' . basename($site_path)`, and evaluating that means running PHP.
/// The marker is exact and needs nothing evaluated.
pub(super) fn exported_directories(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();

    // Two levels, which is where every convention puts it: `config/sync`, `config/default`,
    // `config/<site>`. Deeper would mean walking the whole checkout for a file that is never there.
    for depth in [root.to_path_buf(), root.join("config")] {
        let Ok(entries) = std::fs::read_dir(&depth) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("core.extension.yml").is_file() {
                found.push(path);
            }
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use phrost_testkit::MaterializedFixture;

    use super::{BINDINGS, CONFIG};
    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    /// Which calls read a configuration name, as a whole list: what matters is what is missing.
    #[test]
    fn the_calls_that_read_a_configuration_name() {
        let rendered = BINDINGS
            .iter()
            .map(|binding| {
                format!(
                    "{}::{} #{} -> {}\n",
                    binding.class, binding.method, binding.at, binding.catalog
                )
            })
            .collect::<String>();

        expect![[r#"
            Drupal::config #0 -> config
            Drupal\Core\Config\ConfigFactoryInterface::get #0 -> config
            Drupal\Core\Config\ConfigFactoryInterface::getEditable #0 -> config
            Drupal\Core\Form\FormBase::config #0 -> config
            Drupal\Core\Controller\ControllerBase::config #0 -> config
        "#]]
        .assert_eq(&rendered);
    }

    /// Core's shipped configuration, an extension's, and the exported copy that wins for a name
    /// all three declare.
    #[test]
    fn shipped_and_exported_configuration_are_named_by_their_files() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/modules/system/config/install/system.site.yml
name: Drupal

//- /docroot/core/modules/system/config/optional/system.theme.yml
default: olivero

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/config/install/mine.settings.yml
enabled: true

//- /config/default/core.extension.yml
module: {}

//- /config/default/system.site.yml
name: 'The site'

//- /config/default/notes.txt
not configuration
"#,
        );

        let rendered = Drupal
            .project(fixture.root())
            .catalog_places()
            .into_iter()
            .filter(|place| place.catalog == CONFIG)
            .map(|place| {
                format!(
                    "{} at {} -- {}\n",
                    place.key,
                    place
                        .path
                        .strip_prefix(fixture.root())
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    place.detail,
                )
            })
            .collect::<String>();

        expect![[r#"
            system.site at docroot/core/modules/system/config/install/system.site.yml -- shipped by system
            system.theme at docroot/core/modules/system/config/optional/system.theme.yml -- shipped by system, optional
            mine.settings at docroot/modules/custom/mine/config/install/mine.settings.yml -- shipped by mine
            core.extension at config/default/core.extension.yml -- exported
            system.site at config/default/system.site.yml -- exported
        "#]].assert_eq(&rendered);
    }
}
