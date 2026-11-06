//! One walk of the docroot, for everything that is read off its directory tree.
//!
//! Three questions used to walk it separately -- which extensions are installed, which files hold
//! procedural PHP, and which directories hold the catalogs -- and on a 30 965-file checkout each
//! walk was about 120 ms, before discovery took a fourth. They prune the same directories and want
//! the same files sorted into two lists, so they are one walk that sorts as it goes.

use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

use ignore::{WalkBuilder, WalkState};

use super::{extensions, procedural, templates};

/// Directories the walk never enters, by name.
pub(super) const NEVER_ENTERED: &[&str] = &[".git", "node_modules", "vendor", "files"];

/// Directories the walk enters only inside a directory of templates or components, by name.
pub(super) const ENTERED_FOR_TEMPLATES: &[&str] = &["src", "assets", "css", "js", "images", "misc"];

/// What one look at a docroot found.
#[derive(Debug, Default)]
pub(super) struct Survey {
    /// Every `*.info.yml`: one per installed extension.
    pub(super) descriptors: Vec<PathBuf>,
    /// Every file holding PHP that no autoload rule names -- see `procedural`.
    pub(super) procedural: Vec<PathBuf>,
    /// Every Twig template and every single-directory component's description -- see `templates`.
    ///
    /// Collected here rather than by walking each extension's `templates/` again: this walk already
    /// reads every one of those directories, and the second walk was 1 500 directory reads at every
    /// start for files the first had just listed.
    pub(super) templates: Vec<PathBuf>,
}

impl Survey {
    /// Walks the docroot once.
    ///
    /// Ignore files are not honoured: core and contrib are both gitignored on a normal Drupal
    /// project, and they are exactly what this is looking for. Directories that cannot hold an
    /// extension are pruned, because the walk is otherwise the slowest part of startup on a large
    /// site.
    ///
    /// Across threads, for the reason discovery's walk is: a docroot is tens of thousands of
    /// directory reads and `stat`s the kernel serves concurrently, and one thread doing them was
    /// most of the quarter second `set_root` took on every start of a large site. The threads hand
    /// back what they find in whatever order they reach it, so both lists are sorted before anyone
    /// reads them -- a module's rules and a catalog's claims must not depend on which thread got
    /// to which directory first.
    pub(super) fn of(docroot: &Path) -> Self {
        let (send, receive) = mpsc::channel::<Found>();
        WalkBuilder::new(docroot)
            .standard_filters(false)
            .hidden(false)
            .filter_entry(|entry| {
                let name = entry.path().file_name().and_then(|name| name.to_str());
                // `node_modules` and `.git` are large and hold nothing. `vendor` is composer's, and
                // a package there registers itself through composer, not through an info file.
                // `files` is the site's public upload directory: no code, and on a live site it can
                // be larger than everything else put together.
                if name.is_some_and(|name| NEVER_ENTERED.contains(&name)) {
                    return false;
                }
                // Directories Drupal's own `ExtensionDiscovery` never descends into, less the ones
                // that hold what this walk also wants: `includes` has the procedural core, `lib` a
                // dozen `*.api.php` documenting hooks, `tests` and `fixtures` the test modules the
                // catalogs count, and `templates` a contrib module's `theme.inc`. What is left is a
                // module's `src` -- its PSR-4 classes, which composer's rules cover and which is
                // where three files in four of a docroot live -- and the asset directories.
                // Measured on a 25 404-file docroot: 19 741 files under `src` alone, and not one
                // descriptor or procedural file under any of these.
                //
                // Except inside a directory of templates, where these are ordinary names: a theme
                // keeps `templates/misc/status-messages.html.twig`, and fifteen of one checkout's
                // templates are in a `misc` that the rule above would have skipped.
                !(entry.file_type().is_some_and(|kind| kind.is_dir())
                    && name.is_some_and(|name| ENTERED_FOR_TEMPLATES.contains(&name))
                    && !templates::holds_templates(entry.path()))
            })
            .build_parallel()
            .run(|| {
                let send = send.clone();
                Box::new(move |entry| {
                    let Ok(entry) = entry else {
                        return WalkState::Continue;
                    };
                    if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                        return WalkState::Continue;
                    }
                    let path = entry.into_path();
                    let found = if extensions::is_descriptor(&path) {
                        Found::Descriptor(path)
                    } else if procedural::is_source(&path) {
                        Found::Procedural(path)
                    } else if templates::is_template(&path) {
                        Found::Template(path)
                    } else {
                        return WalkState::Continue;
                    };
                    // A closed channel means the collector is gone, which it cannot be while this
                    // runs; stopping is the only sensible answer if it ever is.
                    match send.send(found) {
                        Ok(()) => WalkState::Continue,
                        Err(_) => WalkState::Quit,
                    }
                })
            });
        drop(send);

        let mut survey = Self::default();
        for found in receive {
            match found {
                Found::Descriptor(path) => survey.descriptors.push(path),
                Found::Procedural(path) => survey.procedural.push(path),
                Found::Template(path) => survey.templates.push(path),
            }
        }
        survey.descriptors.sort();
        survey.procedural.sort();
        survey.templates.sort();
        survey
    }
}

/// One file a walker thread sorted into a list, on its way to the collector.
enum Found {
    Descriptor(PathBuf),
    Procedural(PathBuf),
    Template(PathBuf),
}
