//! Reprinting a tree of files, so a coding standard's own executable can judge the result.
//!
//! The formatter cannot be argued with about a house style: the standard has a program, and for
//! Drupal that program is `phpcs` with the `Drupal` standard. What it needs is two trees -- the
//! files as written and the same files reprinted -- and the errors that appear only in the second
//! are the formatter's. See `tools/drupal-style.sh`, which is the instrument this half serves.
//!
//! Nothing here is a formatting decision. It walks, reads, reprints and writes.

use std::path::{Path, PathBuf};

use phrost_syntax_features::format::{FormatStyle, FormatTarget, format};

/// What the two trees are called under the output directory.
const TREES: [&str; 2] = ["before", "after"];

pub(crate) fn run(root: PathBuf, out: PathBuf, style: Option<String>) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("{}: {error}", root.display()))?;
    // The project's own target, with the style overridden when one was named: the PHP it has to run
    // on is the project's whatever it is laid out like.
    let mut target = configured_target(&root);
    if let Some(name) = style.as_deref() {
        target.style =
            FormatStyle::named(name).ok_or_else(|| format!("no style called {name:?}"))?;
    }

    let mut files = Vec::new();
    php_files(&root, &mut files);
    files.sort();

    let mut reprinted = 0usize;
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let formatted = match format(&text, target) {
            None => text.clone(),
            Some((range, replacement)) => {
                let mut held = text[..usize::from(range.start())].to_owned();
                held.push_str(&replacement);
                held.push_str(&text[usize::from(range.end())..]);
                held
            }
        };
        if formatted != text {
            reprinted += 1;
        }
        let relative = path.strip_prefix(&root).unwrap_or(path);
        for (tree, body) in TREES.iter().zip([&text, &formatted]) {
            let target = out.join(tree).join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            std::fs::write(&target, body)
                .map_err(|error| format!("{}: {error}", target.display()))?;
        }
    }

    println!(
        "{} files, {reprinted} reprinted, in {}/{{{}}}",
        files.len(),
        out.display(),
        TREES.join(",")
    );
    Ok(())
}

/// The house style this project prints in, resolved the way the server resolves it.
///
/// Through `Workspace::set_root` and not by reading `phrost.toml` here, because the file is only
/// half the answer: a framework says how its own projects are printed, and Drupal's provider is why
/// a Drupal checkout formats to two-space indent without anyone configuring anything. Measured on
/// one before this: 29 of 30 sampled files came back different from what the server produces for
/// the same buffer, which is an instrument reporting on a formatter nobody runs.
///
/// Searched upwards, unlike the server's read of the same file, because this command is pointed at
/// a *directory of user code* rather than at a project: `tools/drupal-style.sh` gives it
/// `docroot/modules/custom`, four levels below the `composer.json` that says what the project is.
///
/// No stubs and no indexing. `set_root` reads composer, the generated autoload map and whatever the
/// frameworks contribute, which is the whole of what decides this; the files themselves are never
/// opened.
fn configured_target(from: &Path) -> FormatTarget {
    let Some(root) = project_root_above(from) else {
        return FormatTarget::default();
    };
    let mut workspace = phrost_lsp::Workspace::new(phrost_stubs::StubSet::empty());
    workspace.set_root(root);
    workspace.format_target()
}

/// The project directory a walked path belongs to.
///
/// The nearest ancestor holding what makes a directory a project: composer's own file, or this
/// server's. `None` where neither is anywhere above, which is a directory of loose scripts.
fn project_root_above(from: &Path) -> Option<PathBuf> {
    from.ancestors()
        .find(|directory| {
            directory.join("composer.json").is_file()
                || directory.join(phrost_project::CONFIG_FILE).is_file()
        })
        .map(Path::to_path_buf)
}

/// Every file PHP would load, by the extensions a Drupal project uses.
fn php_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            php_files(&path, into);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("php" | "module" | "inc" | "install" | "theme" | "profile" | "engine")
        ) {
            into.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use phrost_syntax_features::format::FormatStyle;
    use phrost_testkit::MaterializedFixture;

    use super::configured_target;

    /// A project that names its style, and a directory of its code four levels below the file.
    ///
    /// Which is where `tools/drupal-style.sh` points this command. Reading the style from the
    /// walked directory alone would find nothing there, and the command would reprint in a style
    /// the project's own editor changes back on the next save.
    #[test]
    fn the_style_comes_from_the_nearest_configuration_above_the_walked_directory() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /phrost.toml
[format]
style = "drupal"
//- /docroot/modules/custom/mine/src/Widget.php
<?php

namespace App;

class Widget {}
"#,
        );
        let root = fixture.root();

        assert_eq!(
            configured_target(&root.join("docroot/modules/custom/mine/src")).style,
            FormatStyle::Drupal
        );
        assert_eq!(configured_target(root).style, FormatStyle::Drupal);
    }

    /// A framework saying how its own projects are printed, which no configuration file mentions.
    ///
    /// The half that was actually costing something: a Drupal checkout formats to two-space indent
    /// because the provider says so, and a command reading `phrost.toml` alone would print it in
    /// PER-CS -- 29 of 30 sampled files different from what the editor does to the same buffer.
    #[test]
    fn a_framework_decides_the_style_where_the_project_names_none() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /composer.json
{"type": "project"}
//- /docroot/core/lib/Drupal.php
<?php

class Drupal {}
//- /docroot/modules/custom/mine/src/Widget.php
<?php

namespace App;

class Widget {}
"#,
        );

        assert_eq!(
            configured_target(&fixture.root().join("docroot/modules/custom")).style,
            FormatStyle::Drupal
        );
    }

    /// And a project that says nothing gets the built-in style rather than an ancestor's.
    #[test]
    fn a_project_that_configures_no_style_gets_the_built_in_one() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /composer.json
{"type": "project"}
//- /src/Widget.php
<?php

class Widget {}
"#,
        );

        assert_eq!(
            configured_target(fixture.root()).style,
            FormatStyle::Default
        );
    }
}
