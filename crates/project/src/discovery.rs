use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use phrost_model::SourceOrigin;
use phrost_stubs::StubSet;

use crate::{layout::ProjectLayout, paths::PathRules};

/// Extensions treated as PHP by convention, regardless of framework.
///
/// `.inc` and `.phtml` are plain PHP conventions, not anyone's invention. Framework-specific ones —
/// Drupal's `.module`, `.install`, `.theme` — are deliberately absent: they belong to the framework
/// layer, which contributes them through its own seam. Putting them here would be the first crack
/// in the rule that generic code holds no framework knowledge.
pub const PHP_EXTENSIONS: &[&str] = &["php", "phtml", "inc"];

/// One file worth indexing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub origin: SourceOrigin,
}

/// Walks the project for files worth indexing.
///
/// Two passes, and which pass found a file is what decides whose code it is.
///
/// The first pass walks the project root honouring `.gitignore`. Whatever it finds is code the
/// project tracks — the developer's own.
///
/// The second pass walks every path the project named — the directories composer registered, the
/// vendor directory, and whatever a framework said also holds PHP — ignoring ignore files entirely.
/// Anything it finds that the first pass did not is a dependency.
///
/// That rule exists because `.gitignore` cannot be trusted to find code, only to say who owns it.
/// A composer-installed package need not live under `vendor/`: Drupal puts its core under
/// `docroot/core` and its contrib modules under `docroot/modules/contrib`, all gitignored, none of
/// it under `vendor/`. A single ignore-respecting walk finds none of it, and then Drupal's whole
/// core looks undeclared — while a single ignore-free walk would index every build artefact and
/// call it the developer's own.
///
/// A named path may be a single file rather than a directory. Composer names directories because a
/// PSR-4 rule is a directory; a framework naming the files it loads by hand has no directory to
/// point at that is only those files, so it names them one at a time and this walks each.
///
/// Anything under the vendor directory is a dependency regardless of which pass found it, for the
/// project that does not gitignore its vendor tree.
///
/// Results are sorted by path. Discovery order decides nothing downstream, but a stable order makes
/// a bootstrap reproducible and its logs comparable.
pub fn discover_php_files(
    layout: &ProjectLayout,
    extensions: &[&str],
    dependency_paths: &[PathBuf],
    rules: &PathRules,
    stubs: &StubSet,
) -> Vec<DiscoveredFile> {
    let mut found: BTreeMap<PathBuf, SourceOrigin> = BTreeMap::new();

    // First, and unconditionally. `strlen` and `\Exception` exist in every project, and a required
    // argument is what stops a second caller from setting up a project without them and then
    // reporting half the standard library as undeclared.
    for path in stubs.files() {
        found.insert(path.clone(), SourceOrigin::Stub);
    }

    for path in walk(
        &[layout.root().to_path_buf()],
        IgnoreFiles::Respect,
        extensions,
        rules,
        layout,
    ) {
        let origin = layout.source_origin(&path);
        found.insert(path, origin);
    }

    // Everything under `vendor/` is reached by walking `vendor/` itself, with ignore files off, so
    // a root composer put there -- and most of the 849 on a real checkout are `vendor/<pkg>/src` --
    // would only be walked a second time. The roots that matter here are the ones *outside* it:
    // Drupal's core and its modules, which composer does not install under `vendor/`.
    let vendor_dir = layout.vendor_dir();
    let mut dependency_roots: Vec<PathBuf> = dependency_paths
        .iter()
        .filter(|root| !root.starts_with(vendor_dir))
        .cloned()
        .collect();
    dependency_roots.push(vendor_dir.to_path_buf());
    dependency_roots.sort();
    dependency_roots.dedup();

    // A framework names the files it loads by hand one at a time, and on a real checkout that is
    // over a thousand of them. Each is a `stat` here; as a root of its own walk each was a walk,
    // and a parallel walk starts its threads per call.
    let (files, directories): (Vec<PathBuf>, Vec<PathBuf>) = dependency_roots
        .into_iter()
        .partition(|root| root.is_file());
    for file in files {
        if wanted(&file, extensions) {
            found.entry(file).or_insert(SourceOrigin::Vendor);
        }
    }
    // One walk over every directory root rather than one per root: the roots outside `vendor/`
    // are a hundred and more on a Drupal checkout, and the walker's threads are started once for
    // all of them. A root that does not exist yields one error, skipped like any other.
    for path in walk(&directories, IgnoreFiles::Skip, extensions, rules, layout) {
        // Already tracked, so already the developer's: the root package's own `src/` is in
        // composer's map too, and it is not a dependency.
        found.entry(path).or_insert(SourceOrigin::Vendor);
    }

    // Last, and with ignore files off: a directory the project named as its own code, whatever
    // else said otherwise. Its own walk rather than a wider main one, because the thing it has to
    // outrank is the `.gitignore` the walker applies before any predicate here can see the entry --
    // and because an excluded parent would otherwise prune the walk before it reached this. See
    // `PathRules`.
    //
    // `or_insert` for the same reason the dependency pass uses one: a file both walks reach keeps
    // the origin the first gave it, and the first is the one that read the layout.
    for path in walk(
        rules.included(),
        IgnoreFiles::Skip,
        extensions,
        rules,
        layout,
    ) {
        let origin = layout.source_origin(&path);
        found.entry(path).or_insert(origin);
    }

    found
        .into_iter()
        .map(|(path, origin)| DiscoveredFile { path, origin })
        .collect()
}

/// Whether a walk honours `.gitignore` and friends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IgnoreFiles {
    Respect,
    Skip,
}

/// Whether a path carries one of the extensions being looked for, and is code rather than data.
///
/// A file named `*.json.php` is data by convention: AWS's SDK ships its API models as `<?php return
/// [...]` files named that way -- 2 197 of them on the reference checkout, declaring nothing -- and
/// parsing each buys a row that answers no question. The name is the whole signal, and it is one
/// no class file carries.
fn wanted(path: &Path, extensions: &[impl AsRef<str>]) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".json.php"))
    {
        return false;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|candidate| candidate.as_ref().eq_ignore_ascii_case(extension))
        })
}

fn walk(
    roots: &[PathBuf],
    ignore_files: IgnoreFiles,
    extensions: &[&str],
    rules: &PathRules,
    layout: &ProjectLayout,
) -> Vec<PathBuf> {
    let respect = ignore_files == IgnoreFiles::Respect;
    let vendor_dir = layout.vendor_dir().to_path_buf();
    let rules = rules.clone();
    let extensions: Vec<String> = extensions.iter().map(|ext| (*ext).to_owned()).collect();

    // `lspkit_walk` walks on every thread -- one thread doing a thirty-thousand-file tree's
    // directory reads was 0.3 s of every start -- reads a `.gitignore` without a `.git` (a PHP
    // project not in git still has one; composer writes it), and never the user's global excludes,
    // which differ from one machine to the next.
    lspkit_walk::Walk::new(roots.iter().cloned())
        .ignore_files(respect)
        // A dot-prefixed source directory is a real pattern, and skipping it silently loses
        // whatever it declares.
        .skip_hidden(false)
        .prune(move |entry| {
            let path = entry.path;
            // Never worth walking, and all of them can be enormous: a version store, a JavaScript
            // dependency tree, and the caches PHP's own tools keep -- PHPStan's result cache is a
            // directory of ten thousand `<?php return [...]` files on one real checkout, and only a
            // `.gitignore` line stood between it and the index.
            // Except where the project named this directory as its own code, which outranks a
            // convention about what directories of that name usually hold -- see `PathRules`.
            if !rules.claims(path)
                && matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some(".git" | "node_modules" | ".phpstan" | ".phpunit.cache" | ".psalm")
                )
            {
                return true;
            }
            // What a framework said is not code, and what the project added to it -- see
            // `FrameworkProject::excluded_paths` and `ProjectConfig::files`.
            if rules.skips(path) {
                return true;
            }
            // The dependency pass covers it, without ignore files in the way.
            respect && path == vendor_dir
        })
        .keep(move |path| wanted(path, &extensions))
        // Unordered: every caller puts the files in an ordered map, and sorting them here as well
        // was 10 ms of every start.
        .files_unordered()
}

/// The extensions a template written beside PHP is spelled with.
///
/// One, and it is a *language* rather than a framework: `.twig` is Twig wherever it is found, the
/// way `.php` is PHP. Drupal and Symfony both render with it, and a project with neither still
/// reads it as one.
pub const TEMPLATE_EXTENSIONS: &[&str] = &["twig"];

/// The templates the project's own code writes.
///
/// Not indexed and not parsed -- what a reader wants from one is the *names* it writes, since a
/// template calls a method nothing in PHP ever names. Walked here rather than by whoever asks, for
/// the reason §9 records about the measurement tools: an instrument that samples a checkout has to
/// agree with the server about what the project is, and the rules for that live on this side.
///
/// The project's own only. A dependency's templates call a dependency's methods, and neither is
/// something a report about *this* project's dead code has anything to say about.
pub fn discover_templates(
    layout: &ProjectLayout,
    dependency_paths: &[PathBuf],
    rules: &PathRules,
) -> Vec<PathBuf> {
    walk(
        &[layout.root().to_path_buf()],
        IgnoreFiles::Respect,
        TEMPLATE_EXTENSIONS,
        rules,
        layout,
    )
    .into_iter()
    .filter(|path| layout.source_origin(path) == SourceOrigin::User)
    .filter(|path| !under_any(path, dependency_paths))
    .collect()
}

/// Whether a path lies inside one of these roots.
fn under_any(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use super::*;

    #[test]
    fn vendor_is_found_even_though_gitignore_hides_it() {
        // The failure this guards against is silent and total: a single ignore-respecting walk finds
        // no dependency at all, and every vendor class then looks undeclared.
        let fixture = MaterializedFixture::parse(
            r#"
//- /.gitignore
/vendor
/ignored

//- /src/App.php
<?php class App {}

//- /ignored/Skipped.php
<?php class Skipped {}

//- /vendor/acme/src/Model.php
<?php class Model {}
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let found = discover_php_files(
            &layout,
            PHP_EXTENSIONS,
            &[],
            &PathRules::default(),
            &StubSet::empty(),
        );

        let relative = found
            .iter()
            .map(|file| {
                (
                    file.path
                        .strip_prefix(fixture.root())
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    file.origin,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            relative,
            [
                ("src/App.php".to_owned(), SourceOrigin::User),
                ("vendor/acme/src/Model.php".to_owned(), SourceOrigin::Vendor),
            ]
        );
    }

    /// What the project takes back, against the three things that would otherwise skip it.
    ///
    /// One test for all three because they are one claim -- `include` is a statement about *this*
    /// directory and everything else here is a rule written about something else. The `.gitignore`
    /// arm is the one that needs the second walk: the walker applies an ignore file before any
    /// predicate can see the entry, so an included root is walked again with ignore files off.
    #[test]
    fn what_the_project_includes_outranks_every_reason_to_skip_it() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /.gitignore
generated/
node_modules/

//- /src/App.php
<?php class App {}

//- /generated/Kept.php
<?php class Kept {}

//- /generated/dropped/Dropped.php
<?php class Dropped {}

//- /node_modules/pkg/Bundled.php
<?php class Bundled {}

//- /swept/Swept.php
<?php class Swept {}
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let rules = PathRules::new(
            vec![
                // Excluded by the project, and by a framework: one list to the reader.
                fixture.root().join("swept"),
                fixture.root().join("generated"),
            ],
            vec![
                // Gitignored *and* excluded above -- both outranked.
                fixture.root().join("generated/Kept.php"),
                // Gitignored and skipped by name, which is the built-in rule.
                fixture.root().join("node_modules/pkg"),
            ],
        );
        let found = discover_php_files(&layout, PHP_EXTENSIONS, &[], &rules, &StubSet::empty());
        let mut relative = found
            .iter()
            .filter(|file| file.origin != SourceOrigin::Stub)
            .map(|file| {
                file.path
                    .strip_prefix(fixture.root())
                    .expect("under the root")
                    .display()
                    .to_string()
            })
            .collect::<Vec<_>>();
        relative.sort();

        assert_eq!(
            relative,
            [
                "generated/Kept.php",
                "node_modules/pkg/Bundled.php",
                "src/App.php",
            ],
            "the two included paths and the project's own code -- and neither `generated/dropped` \
             nor `swept`, which nothing took back"
        );
    }

    /// What a framework names as not code is skipped whether or not the project ignores it: a
    /// directory whole, a file by itself, and a tool's cache by convention.
    #[test]
    fn an_excluded_path_is_skipped_even_when_nothing_ignores_it() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /src/App.php
<?php class App {}

//- /web/sites/default/settings.php
<?php $databases = [];

//- /web/sites/default/files/php/twig/x.php
<?php class __TwigTemplate_x {}

//- /web/sites/default/modules/mine/mine.module
<?php function mine_help() {}

//- /.phpstan/resultCache.php
<?php return [];
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let excluded = vec![
            fixture.root().join("web/sites/default/settings.php"),
            fixture.root().join("web/sites/default/files"),
        ];
        let extensions = ["php", "module"];
        let found = discover_php_files(
            &layout,
            &extensions,
            &[],
            &PathRules::new(excluded, Vec::new()),
            &StubSet::empty(),
        )
        .into_iter()
        .map(|file| {
            file.path
                .strip_prefix(fixture.root())
                .expect("under the root")
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
        assert_eq!(
            found,
            ["src/App.php", "web/sites/default/modules/mine/mine.module"]
        );
    }

    /// `*.json.php` is data wearing PHP's extension, and is left out wherever it sits.
    #[test]
    fn a_json_php_data_file_is_not_indexed() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /src/App.php
<?php class App {}

//- /vendor/aws/aws-sdk-php/src/data/s3/2006-03-01/api-2.json.php
<?php return ['metadata' => []];
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let found = discover_php_files(
            &layout,
            PHP_EXTENSIONS,
            &[],
            &PathRules::default(),
            &StubSet::empty(),
        )
        .into_iter()
        .map(|file| {
            file.path
                .strip_prefix(fixture.root())
                .expect("under the root")
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
        assert_eq!(found, ["src/App.php"]);
    }

    #[test]
    fn conventional_php_extensions_are_included_and_others_are_not() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /a.php
<?php

//- /b.phtml
<?php

//- /c.inc
<?php

//- /d.module
<?php

//- /e.txt
plain
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let names = discover_php_files(
            &layout,
            PHP_EXTENSIONS,
            &[],
            &PathRules::default(),
            &StubSet::empty(),
        )
        .iter()
        .map(|file| {
            file.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();

        // `.module` is Drupal's, and generic discovery does not know about Drupal. The framework
        // layer contributes it later.
        assert_eq!(names, ["a.php", "b.phtml", "c.inc"]);
    }

    #[test]
    fn a_gitignored_composer_directory_outside_vendor_is_still_found() {
        // The Drupal layout, and the reason discovery cannot trust `.gitignore` to find code:
        // composer installs core under `docroot/core`, which is gitignored and is not under
        // `vendor/`. Missing it makes the entire framework look undeclared.
        let fixture = MaterializedFixture::parse(
            r#"
//- /.gitignore
/vendor
/docroot/core

//- /src/App.php
<?php class App {}

//- /docroot/core/lib/Drupal/Core/Database.php
<?php class Database {}

//- /docroot/modules/custom/mine/src/Service.php
<?php class Service {}
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let composer_roots = vec![
            fixture.path("src"),
            fixture.path("docroot/core/lib/Drupal/Core"),
        ];

        let found = discover_php_files(
            &layout,
            PHP_EXTENSIONS,
            &composer_roots,
            &PathRules::default(),
            &StubSet::empty(),
        )
        .iter()
        .map(|file| {
            (
                file.path
                    .strip_prefix(fixture.root())
                    .expect("under root")
                    .to_string_lossy()
                    .replace('\\', "/"),
                file.origin,
            )
        })
        .collect::<Vec<_>>();

        assert_eq!(
            found,
            [
                // Gitignored, known only to composer: a dependency.
                (
                    "docroot/core/lib/Drupal/Core/Database.php".to_owned(),
                    SourceOrigin::Vendor
                ),
                // Tracked, so the developer's own even though nothing registered it.
                (
                    "docroot/modules/custom/mine/src/Service.php".to_owned(),
                    SourceOrigin::User
                ),
                // Tracked *and* registered by composer: still the developer's own.
                ("src/App.php".to_owned(), SourceOrigin::User),
            ]
        );
    }

    /// A framework names the files it loads by hand, and they are in no composer rule at all.
    ///
    /// The failure without this is total and quiet: Drupal's `t()` is declared in a gitignored
    /// `core/includes/bootstrap.inc` that no PSR-4 prefix covers, so both passes miss it and every
    /// call to the most-used function in the framework resolves to nothing.
    #[test]
    fn a_named_file_outside_every_composer_rule_is_found_and_is_a_dependency() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /.gitignore
/core

//- /src/App.php
<?php class App {}

//- /core/lib/Drupal/Core/Site.php
<?php namespace Drupal\Core; class Site {}

//- /core/includes/bootstrap.inc
<?php function t($string) {}
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        // What composer knows, and then the one file it does not.
        let named = vec![
            fixture.path("core/lib/Drupal/Core"),
            fixture.path("core/includes/bootstrap.inc"),
        ];

        let found = discover_php_files(
            &layout,
            PHP_EXTENSIONS,
            &named,
            &PathRules::default(),
            &StubSet::empty(),
        )
        .iter()
        .map(|file| {
            (
                file.path
                    .strip_prefix(fixture.root())
                    .expect("under root")
                    .to_string_lossy()
                    .replace('\\', "/"),
                file.origin,
            )
        })
        .collect::<Vec<_>>();

        assert_eq!(
            found,
            [
                (
                    "core/includes/bootstrap.inc".to_owned(),
                    SourceOrigin::Vendor
                ),
                (
                    "core/lib/Drupal/Core/Site.php".to_owned(),
                    SourceOrigin::Vendor
                ),
                ("src/App.php".to_owned(), SourceOrigin::User),
            ]
        );
    }

    /// Naming a file the project already tracks must not demote it to a dependency: a site's own
    /// `.module` is found by the first pass, and a framework naming it again says nothing about who
    /// wrote it.
    #[test]
    fn a_named_file_the_project_tracks_stays_the_developers_own() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /modules/custom/mine/mine.module
<?php function mine_help() {}
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let mut extensions = PHP_EXTENSIONS.to_vec();
        extensions.push("module");
        let named = vec![fixture.path("modules/custom/mine/mine.module")];

        let found = discover_php_files(
            &layout,
            &extensions,
            &named,
            &PathRules::default(),
            &StubSet::empty(),
        );

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].origin, SourceOrigin::User);
    }

    #[test]
    fn an_extra_extension_can_be_asked_for() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /a.php
<?php

//- /b.module
<?php
"#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let mut extensions = PHP_EXTENSIONS.to_vec();
        extensions.push("module");

        assert_eq!(
            discover_php_files(
                &layout,
                &extensions,
                &[],
                &PathRules::default(),
                &StubSet::empty()
            )
            .len(),
            2
        );
    }
}
