//! Every template an extension ships, by each name Twig loads it by.
//!
//! Drupal configures Twig with one namespace per installed extension, pointing at its `templates/`
//! directory, so `{% include '@my_module/parts/thumb.html.twig' %}` is
//! `<my_module>/templates/parts/thumb.html.twig` and nothing else. A typo there is an exception while
//! the page renders -- and a string no compiler, linter or type checker reads. 35 such names are
//! written in one real project's own templates, and 70 more in the core and contributed ones beside
//! them.
//!
//! # Three spellings, one walk
//!
//! - `@extension/path` -- the namespace an extension's directory is registered under.
//! - `block.html.twig` -- a bare file name, which Drupal's theme registry loader resolves to the
//!   template of the hook it names. So a bare name is a key exactly when a template file is called
//!   that, which is what the theme hook catalog is read from as well.
//! - `extension:component` -- a single-directory component, read from the `*.component.yml` beside
//!   its template.
//!
//! The namespaces are what the first spelling can be checked against. The loader reads a namespace's
//! directory and no other, so a name under a namespace this walked whole is a template that is there
//! or one that is not. The other two are open: a hook may name a template anywhere through its
//! `path`, and components can come from elsewhere.

use std::path::{Path, PathBuf};

use crate::rule::{PlaceEntry, TwigLoader};

/// The catalog holding every template by the names Twig loads it by. A name this provider chose.
pub(super) const TEMPLATES: &str = "template";

/// The catalog of every other name a `.twig` file under an extension is loaded by: its path from the
/// extension's root, where Drupal registers the namespace a second time.
pub(super) const ALIASES: &str = "template path";

/// The catalog of extension namespaces whose directories were walked.
pub(super) const NAMESPACES: &str = "template namespace";

/// The directories the docroot walk does not enter, so a name passing through one is not one the
/// catalogs can rule out. Both of the walk's lists, which a test holds this to.
const UNWALKED: &[&str] = &[
    ".git",
    "node_modules",
    "vendor",
    "files",
    "src",
    "assets",
    "css",
    "js",
    "images",
    "misc",
];

pub(super) static TWIG_LOADERS: &[TwigLoader] = &[TwigLoader {
    templates: TEMPLATES,
    aliases: ALIASES,
    namespaces: NAMESPACES,
    unwalked: UNWALKED,
    globals: GLOBALS,
}];

/// What the theme manager hands every template before anything renders it: its default variables,
/// with the three attribute arrays made `Attribute` objects -- `ThemeManager::getDefaultTemplateVariables`
/// and the loop over them in `render()`.
const GLOBALS: &[(&str, &str)] = &[
    ("attributes", "Drupal\\Core\\Template\\Attribute"),
    ("title_attributes", "Drupal\\Core\\Template\\Attribute"),
    ("content_attributes", "Drupal\\Core\\Template\\Attribute"),
    ("title_prefix", "array"),
    ("title_suffix", "array"),
    ("db_is_active", "bool"),
    ("is_admin", "bool"),
    ("logged_in", "bool"),
    ("directory", "string"),
];

/// What a Twig template is called, whatever it renders: `.html.twig`, `.svg.twig`, `.txt.twig`.
const TWIG: &str = ".twig";

/// What a component's description is called, and so where its id comes from.
const COMPONENT: &str = ".component.yml";

/// Whether a file the docroot walk meets is one this reads: a template, or a component's
/// description.
pub(super) fn is_template(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(TWIG) || name.ends_with(COMPONENT))
}

/// Whether a directory is inside one an extension keeps templates or components in, where every
/// name is a template's own business and none of them is skipped.
pub(super) fn holds_templates(directory: &Path) -> bool {
    directory.ancestors().skip(1).any(|ancestor| {
        ancestor
            .file_name()
            .is_some_and(|name| name == "templates" || name == "components")
    })
}

/// An installed extension: the directory its descriptor is in, and its machine name.
#[derive(Debug)]
pub(super) struct Extension {
    pub(super) directory: PathBuf,
    /// From the descriptor's file name, which is what Drupal registers the namespace under.
    pub(super) name: String,
    pub(super) descriptor: PathBuf,
}

impl Extension {
    /// One per descriptor.
    pub(super) fn all(descriptors: &[PathBuf]) -> Vec<Self> {
        descriptors
            .iter()
            .filter_map(|descriptor| {
                Some(Self {
                    directory: descriptor.parent()?.to_path_buf(),
                    name: descriptor
                        .file_name()?
                        .to_str()?
                        .strip_suffix(".info.yml")?
                        .to_owned(),
                    descriptor: descriptor.clone(),
                })
            })
            .collect()
    }

    /// What to show as the extension providing something: its directory's name.
    pub(super) fn owner(&self) -> &str {
        self.directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }
}

/// One Twig file under an extension's `templates/`.
#[derive(Debug)]
pub(super) struct TemplateFile {
    pub(super) path: PathBuf,
    /// Its path under `templates/`, with `/` between directories as a template writes it.
    pub(super) within: String,
    /// Which of the extensions it belongs to.
    pub(super) extension: usize,
}

/// Every Twig file under an extension's `templates/` directory, out of what the docroot walk found.
///
/// Core's modules *last*, which is the order a later entry winning needs: a theme shipping its own
/// `table.html.twig` overrides how one theme draws `table` without redefining it, so the module's
/// own template is the answer for the hook and for the bare name alike. Otherwise in the walk's
/// order, which is sorted, so that which file a name ends up at does not depend on which thread
/// reached it first.
pub(super) fn template_files(
    docroot: &Path,
    extensions: &[Extension],
    found: &[PathBuf],
) -> Vec<TemplateFile> {
    let by_directory = extensions
        .iter()
        .enumerate()
        .map(|(at, extension)| (extension.directory.as_path(), at))
        .collect::<std::collections::HashMap<_, _>>();
    let core_modules = docroot.join("core").join("modules");
    let mut files = found
        .iter()
        .filter(|path| path.extension().is_some_and(|ending| ending == "twig"))
        .filter_map(|path| {
            let (extension, base) = owned_under(path, "templates", &by_directory)?;
            let within = path.strip_prefix(base).ok()?.to_str()?;
            Some(TemplateFile {
                path: path.clone(),
                within: within.replace(std::path::MAIN_SEPARATOR, "/"),
                extension,
            })
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|file| file.path.starts_with(&core_modules));
    files
}

/// The extension a file belongs to through a directory of this name right under it, and that
/// directory: `<extension>/templates/parts/thumb.html.twig` is the extension's, under `templates`.
///
/// The outermost such directory, because that is the one the loader is given: a `templates/`
/// inside `templates/` is a directory like any other.
fn owned_under<'a>(
    path: &'a Path,
    directory: &str,
    extensions: &std::collections::HashMap<&Path, usize>,
) -> Option<(usize, &'a Path)> {
    path.ancestors()
        .filter(|ancestor| ancestor.file_name().is_some_and(|name| name == directory))
        .find_map(|ancestor| Some((*extensions.get(ancestor.parent()?)?, ancestor)))
}

/// Every name Twig loads a template by, and every namespace that holds them.
pub(super) fn catalog_places(
    docroot: &Path,
    extensions: &[Extension],
    files: &[TemplateFile],
    found: &[PathBuf],
) -> Vec<PlaceEntry> {
    let shown = |path: &Path| {
        path.strip_prefix(docroot)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    };
    let place = |catalog: &'static str, key: String, path: &Path, detail: String| PlaceEntry {
        catalog,
        key,
        path: path.to_path_buf(),
        // The file is the declaration, so a reader following the name lands at its top.
        line: 0,
        character: 0,
        detail,
    };

    let mut entries = Vec::new();
    let mut namespaces = vec![false; extensions.len()];
    for file in files {
        let extension = &extensions[file.extension];
        namespaces[file.extension] = true;
        entries.push(place(
            TEMPLATES,
            format!("@{}/{}", extension.name, file.within),
            &file.path,
            shown(&file.path),
        ));
        // The theme registry's spelling, which is a file name and never a path.
        if file.within.ends_with(".html.twig")
            && let Some(name) = file.path.file_name().and_then(|name| name.to_str())
        {
            entries.push(place(
                TEMPLATES,
                name.to_owned(),
                &file.path,
                shown(&file.path),
            ));
        }
    }

    // A namespace whose directory holds a template, which is one this walked: an empty one holds
    // nothing to be wrong about.
    for (extension, walked) in extensions.iter().zip(namespaces) {
        if walked {
            entries.push(place(
                NAMESPACES,
                extension.name.clone(),
                &extension.descriptor,
                shown(&extension.directory.join("templates")),
            ));
        }
    }

    let by_directory = extensions
        .iter()
        .enumerate()
        .map(|(at, extension)| (extension.directory.as_path(), at))
        .collect::<std::collections::HashMap<_, _>>();

    // Every `.twig` file under an extension, by its path from the extension's root -- and from the
    // root of every extension it is nested in, which a test module inside its parent is. No detail:
    // these are answered and never listed, and the template's own name already carries one.
    for path in found
        .iter()
        .filter(|path| path.extension().is_some_and(|ending| ending == "twig"))
    {
        for ancestor in path.ancestors().skip(1) {
            let Some(&extension) = by_directory.get(ancestor) else {
                continue;
            };
            let Some(within) = path.strip_prefix(ancestor).ok().and_then(Path::to_str) else {
                continue;
            };
            entries.push(place(
                ALIASES,
                format!(
                    "@{}/{}",
                    extensions[extension].name,
                    within.replace(std::path::MAIN_SEPARATOR, "/")
                ),
                path,
                String::new(),
            ));
        }
    }

    let present = found
        .iter()
        .map(PathBuf::as_path)
        .collect::<std::collections::HashSet<_>>();
    for description in found {
        let Some(component) = description
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(COMPONENT))
        else {
            continue;
        };
        let Some((extension, _)) = owned_under(description, "components", &by_directory) else {
            continue;
        };
        let template = description.with_file_name(format!("{component}{TWIG}"));
        let at = match present.contains(template.as_path()) {
            true => template.as_path(),
            false => description.as_path(),
        };
        entries.push(place(
            TEMPLATES,
            format!("{}:{component}", extensions[extension].name),
            at,
            shown(at),
        ));
    }
    entries
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use phrost_testkit::MaterializedFixture;

    use super::{ALIASES, NAMESPACES, TEMPLATES, UNWALKED};

    /// What the loader says the walk skipped is what the walk skips, or a name through a directory
    /// added to one list and not the other is reported missing when nothing looked.
    #[test]
    fn the_directories_called_unwalked_are_the_ones_the_walk_skips() {
        let skipped = super::super::survey::NEVER_ENTERED
            .iter()
            .chain(super::super::survey::ENTERED_FOR_TEMPLATES)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(UNWALKED, skipped.as_slice());
    }
    use crate::{FrameworkProvider as _, providers::drupal::Drupal};

    /// Every name a template is loaded by, and every namespace walked whole.
    ///
    /// The whole set, because an extra key matters as much as a missing one: a namespace in the list
    /// is one whose missing names are reported.
    #[test]
    fn every_template_is_filed_under_each_name_twig_loads_it_by() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/modules/system/system.info.yml
name: System

//- /docroot/core/modules/system/templates/item-list.html.twig
<ul></ul>

//- /docroot/core/modules/navigation/navigation.info.yml
name: Navigation

//- /docroot/core/modules/navigation/components/toolbar-button/toolbar-button.component.yml
name: Toolbar button

//- /docroot/core/modules/navigation/components/toolbar-button/toolbar-button.twig
<button></button>

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/templates/parts/thumb.html.twig
<img>

//- /docroot/modules/custom/mine/templates/icons/logo.svg.twig
<svg></svg>

//- /docroot/modules/custom/mine/templates/misc/status.html.twig
<div></div>

//- /docroot/modules/custom/mine/templates/notes.md
not a template

//- /docroot/modules/custom/bare/bare.info.yml
name: Bare
"#,
        );

        let mut rendered = Drupal
            .project(fixture.root())
            .catalog_places()
            .into_iter()
            .filter(|place| matches!(place.catalog, TEMPLATES | ALIASES | NAMESPACES))
            .map(|place| match place.detail.is_empty() {
                true => format!("{}: {}\n", place.catalog, place.key),
                false => format!("{}: {} -- {}\n", place.catalog, place.key, place.detail),
            })
            .collect::<Vec<_>>();
        rendered.sort();
        rendered.dedup();

        expect![[r#"
            template namespace: mine -- modules/custom/mine/templates
            template namespace: system -- core/modules/system/templates
            template path: @mine/templates/icons/logo.svg.twig
            template path: @mine/templates/misc/status.html.twig
            template path: @mine/templates/parts/thumb.html.twig
            template path: @navigation/components/toolbar-button/toolbar-button.twig
            template path: @system/templates/item-list.html.twig
            template: @mine/icons/logo.svg.twig -- modules/custom/mine/templates/icons/logo.svg.twig
            template: @mine/misc/status.html.twig -- modules/custom/mine/templates/misc/status.html.twig
            template: @mine/parts/thumb.html.twig -- modules/custom/mine/templates/parts/thumb.html.twig
            template: @system/item-list.html.twig -- core/modules/system/templates/item-list.html.twig
            template: item-list.html.twig -- core/modules/system/templates/item-list.html.twig
            template: navigation:toolbar-button -- core/modules/navigation/components/toolbar-button/toolbar-button.twig
            template: status.html.twig -- modules/custom/mine/templates/misc/status.html.twig
            template: thumb.html.twig -- modules/custom/mine/templates/parts/thumb.html.twig
        "#]]
        .assert_eq(&rendered.concat());
    }
}
