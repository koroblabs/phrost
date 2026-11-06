//! The PHP Drupal loads by hand.
//!
//! An autoloader keys a file by the class inside it, so a file that declares only functions has
//! nothing to look up. Drupal has hundreds of them and they hold the names a site's own code uses
//! most: `t()`, `batch_set()` and `base_path()` are in `core/includes/*.inc`, and every extension
//! keeps its hook implementations in a `.module` beside its `*.info.yml`. Core requires the first
//! set when it boots and the second from the extension registry; composer's map mentions neither.
//!
//! Two things then hide them from a generic walk at once, which is why missing them looks like
//! nothing rather than like a gap: `core/` and `modules/contrib/` are gitignored on a normal
//! checkout, and the autoload roots that survive gitignore point at the `src/` directories these
//! files sit *beside*. What is left indexed is the site's own modules and nothing else, so a
//! project resolves its own hooks and not one name core gave it.

use std::path::Path;

/// The suffixes Drupal writes procedural PHP in.
///
/// One list with two readers: discovery is told these count as PHP at all, and the walk below is
/// told which files to hand it. Two lists would drift, and both directions are silent -- a suffix in
/// one and not the other is either a file discovery skips or a file it never hears about.
///
/// `inc` is on it although the generic layer already treats `.inc` as PHP by convention. The walk
/// needs it named here, and what the seam contributes is merged with PHP's own suffixes and
/// deduplicated, so saying it twice costs nothing and leaving it out would cost `t()`.
pub(super) const EXTENSIONS: &[&str] = &["module", "install", "theme", "engine", "profile", "inc"];

/// The suffix of a file that documents hooks and declares nothing anyone calls.
///
/// A `*.api.php` is Drupal's catalogue of what a hook is: `function hook_form_alter(&$form, ...)`
/// with the parameters typed and twenty lines of prose above it, in a file the runtime never loads.
/// It is the only thing that says what a hook takes, and following `Implements hook_X()` to it is
/// what makes that sentence mean something.
///
/// Matched on the whole name and not on an extension, which is the whole difficulty: the extension
/// is `php`, and accepting `php` here would pull every PHP file under the docroot into a walk that
/// deliberately ignores git -- duplicating what the autoloader already found and drowning the index
/// in what it does not.
///
/// Twelve of the fifty-seven on a real checkout sit inside a PSR-4 root and were already indexed;
/// the other forty-five are not, and hold **178 of the 306** hooks the project documents. Before
/// this, `hook_theme()` resolved and `hook_cron()` did not, for no reason a reader could see.
const DOCUMENTED_HOOKS: &str = ".api.php";

/// Whether a file is procedural PHP Drupal loads by hand. The walk that finds them is `Survey::of`.
///
/// These suffixes are procedural PHP wherever Drupal puts them, so a whole-docroot walk needs no
/// list of directories and `core/includes` needs no separate mention.
pub(super) fn is_source(path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str());
    if name.is_some_and(|name| name.to_ascii_lowercase().ends_with(DOCUMENTED_HOOKS)) {
        return true;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    fn named(fixture: &MaterializedFixture) -> Vec<String> {
        let mut found = Drupal
            .project(fixture.root())
            .procedural_sources()
            .into_iter()
            .map(|path| {
                path.strip_prefix(fixture.root())
                    .expect("under root")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();
        found.sort();
        found
    }

    #[test]
    fn cores_own_includes_and_every_extensions_hooks_are_named() {
        // None of these is in composer's autoload map and none of them is tracked by git on a real
        // checkout, so a project that does not name them here resolves no call to `t()` anywhere.
        let fixture = MaterializedFixture::parse(
            r#"
//- /.gitignore
/docroot/core
/docroot/modules/contrib

//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/includes/bootstrap.inc
<?php function t($string) {}

//- /docroot/core/includes/batch.inc
<?php function batch_set($definition) {}

//- /docroot/core/modules/node/node.info.yml
name: Node

//- /docroot/core/modules/node/node.module
<?php function node_help() {}

//- /docroot/core/modules/node/node.install
<?php function node_install() {}

//- /docroot/core/core.api.php
<?php
function hook_cron(): void {}

//- /docroot/core/modules/node/src/NodeStorage.php
<?php namespace Drupal\node; class NodeStorage {}

//- /docroot/modules/contrib/token/token.info.yml
name: Token

//- /docroot/modules/contrib/token/token.module
<?php function token_help() {}

//- /docroot/themes/custom/plain/plain.theme
<?php function plain_preprocess_page() {}

//- /docroot/profiles/app/app.profile
<?php function app_install_tasks() {}

//- /docroot/themes/engines/twig/twig.engine
<?php function twig_init() {}

"#,
        );

        assert_eq!(
            named(&fixture),
            [
                // A hook catalogue: matched on its whole name, because its extension is `php` and
                // accepting that would drag in every PHP file under the docroot.
                "docroot/core/core.api.php",
                "docroot/core/includes/batch.inc",
                "docroot/core/includes/bootstrap.inc",
                "docroot/core/modules/node/node.install",
                "docroot/core/modules/node/node.module",
                "docroot/modules/contrib/token/token.module",
                "docroot/profiles/app/app.profile",
                "docroot/themes/custom/plain/plain.theme",
                "docroot/themes/engines/twig/twig.engine",
            ]
        );
    }

    #[test]
    fn a_class_file_is_not_named_here_because_the_autoloader_already_reaches_it() {
        // Naming one would be harmless but wrong in the direction that grows: the point of this
        // seam is the files no autoload rule can key on, and a `.php` under `src/` has a class in it.
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/src/Service.php
<?php namespace Drupal\mine; class Service {}
"#,
        );

        assert!(named(&fixture).is_empty());
    }

    #[test]
    fn a_project_that_is_not_drupal_names_nothing() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /src/App.php
<?php class App {}

//- /src/helpers.inc
<?php function helper() {}
"#,
        );

        assert!(named(&fixture).is_empty());
    }

    /// The composer-managed dependency tree is reached through the autoloader, and walking it here
    /// would be the whole of `vendor/` for the handful of `.inc` files a package happens to ship.
    #[test]
    fn a_vendor_directory_inside_the_docroot_is_left_to_composer() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/vendor/acme/legacy/functions.inc
<?php function acme_helper() {}

//- /docroot/sites/default/files/cached/render.inc
<?php function leftover() {}

//- /docroot/core/includes/common.inc
<?php function base_path() {}
"#,
        );

        assert_eq!(named(&fixture), ["docroot/core/includes/common.inc"]);
    }
}
