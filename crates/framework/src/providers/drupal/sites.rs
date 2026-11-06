//! What under `sites/` is configuration and files rather than code.
//!
//! A Drupal docroot keeps two things under `sites/` that a walk for PHP finds and should not:
//! `sites/*/files`, the site's public files directory, where the twig cache writes a compiled
//! `.php` class per template -- thousands on a live site, named `__TwigTemplate_<hash>`, declaring
//! nothing anybody calls; and the settings -- `settings.php`, `settings.local.php`,
//! `default.settings.php`, `sites.php` -- which are PHP by extension and configuration by nature:
//! arrays of database credentials and `$settings` keys, with no declaration to index and every
//! security lint firing on the passwords they exist to hold. Both are usually gitignored, and
//! discovery honours that; naming them here is what makes the answer not depend on it.
//!
//! `sites/simpletest` goes too: it is where the test runner materialises throwaway sites.

use std::path::{Path, PathBuf};

/// Every path under `sites/` that is not code, as it stands on disk.
///
/// Read off the directory rather than written as patterns, because the caller compares paths and a
/// glob would need a second vocabulary. Only what exists is listed, so a docroot with no `sites/`
/// contributes nothing.
pub(super) fn excluded_paths(docroot: &Path) -> Vec<PathBuf> {
    let sites = docroot.join("sites");
    let mut excluded = Vec::new();
    let Ok(entries) = std::fs::read_dir(&sites) else {
        return excluded;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "simpletest" {
                excluded.push(path);
                continue;
            }
            // `sites/<site>/files`, and the site's own settings beside it.
            let files = path.join("files");
            if files.is_dir() {
                excluded.push(files);
            }
            if let Ok(inside) = std::fs::read_dir(&path) {
                for file in inside.flatten() {
                    let file = file.path();
                    if is_settings(&file) {
                        excluded.push(file);
                    }
                }
            }
        } else if is_settings(&path) {
            // `sites/sites.php` and `sites/example.sites.php`: the multi-site map.
            excluded.push(path);
        }
    }
    excluded.sort();
    excluded
}

/// `settings.php`, `settings.local.php`, `default.settings.php`, `example.settings.local.php`,
/// `sites.php`, `example.sites.php` -- a PHP file whose stem is or ends in `settings` or `sites`.
fn is_settings(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(stem) = name.strip_suffix(".php") else {
        return false;
    };
    stem.split('.')
        .any(|part| matches!(part, "settings" | "sites"))
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use super::excluded_paths;

    #[test]
    fn the_files_directory_the_settings_and_the_multisite_map_are_excluded() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/sites/sites.php
<?php $sites['a'] = 'default';
//- /docroot/sites/example.sites.php
<?php
//- /docroot/sites/default/settings.php
<?php $databases['default']['default'] = ['password' => 'secret'];
//- /docroot/sites/default/settings.local.php
<?php
//- /docroot/sites/default/default.settings.php
<?php
//- /docroot/sites/default/services.yml
parameters: {}
//- /docroot/sites/default/files/php/twig/abc/x.php
<?php class __TwigTemplate_abc {}
//- /docroot/sites/default/modules/custom/mine/mine.module
<?php function mine_help() {}
//- /docroot/sites/simpletest/1/settings.php
<?php
"#,
        );
        let docroot = fixture.root().join("docroot");
        let found = excluded_paths(&docroot)
            .into_iter()
            .map(|path| {
                path.strip_prefix(&docroot)
                    .expect("under the docroot")
                    .display()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            found,
            [
                "sites/default/default.settings.php",
                "sites/default/files",
                "sites/default/settings.local.php",
                "sites/default/settings.php",
                "sites/example.sites.php",
                "sites/simpletest",
                "sites/sites.php",
            ]
        );
    }
}
