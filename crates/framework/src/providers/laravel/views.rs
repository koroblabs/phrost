//! The templates a project's views directory holds, by the name a call renders each by.
//!
//! `view('posts.show')` and `@include('posts.row')` name a file -- `resources/views/posts/show.blade
//! .php` -- and a reader on the name wants to go there, and to be offered the names that exist while
//! typing one. A catalog of places, the shape a Drupal route takes: the key is the name, and what
//! it names is a file rather than a class.

use std::path::Path;

use crate::PlaceEntry;

/// The catalog the names are filed under. A name this provider chose.
pub(super) const CATALOG: &str = "laravel.views";
/// Where a template a call names by name lives, from the project root.
pub(super) const DIRECTORY: &str = "resources/views";
/// What a Blade template's file name ends in.
pub(super) const SUFFIX: &str = ".blade.php";
/// What a template's name writes between directories.
pub(super) const SEPARATOR: &str = ".";

/// Every template under the views directory, by the name a call renders it by.
///
/// Walked without following a link, so a directory linked into itself cannot loop.
pub(super) fn places(root: &Path) -> Vec<PlaceEntry> {
    let base = root.join(DIRECTORY);
    let mut found = Vec::new();
    let mut directories = vec![base.clone()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                directories.push(path);
                continue;
            }
            let Some(within) = path.strip_prefix(&base).ok().and_then(Path::to_str) else {
                continue;
            };
            let Some(name) = within.strip_suffix(SUFFIX) else {
                continue;
            };
            found.push(PlaceEntry {
                catalog: CATALOG,
                key: name
                    .split(std::path::MAIN_SEPARATOR)
                    .collect::<Vec<_>>()
                    .join(SEPARATOR),
                detail: format!("{DIRECTORY}/{within}"),
                path,
                line: 0,
                character: 0,
            });
        }
    }
    found.sort_by(|left, right| left.key.cmp(&right.key));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_is_filed_by_the_name_a_call_renders_it_by() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let views = dir.path().join(DIRECTORY).join("posts");
        std::fs::create_dir_all(&views).expect("the views directory");
        std::fs::write(views.join("show.blade.php"), "").expect("a template");
        std::fs::write(views.join("notes.txt"), "").expect("something else");
        let keys: Vec<(String, String)> = places(dir.path())
            .into_iter()
            .map(|place| (place.key, place.detail))
            .collect();
        assert_eq!(
            keys,
            [(
                "posts.show".to_owned(),
                "resources/views/posts/show.blade.php".to_owned()
            )]
        );
    }
}
