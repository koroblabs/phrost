mod lexer;
mod parser;

use std::{
    fs,
    path::{Path, PathBuf},
};

use phrost_model::Interner;
use phrost_vfs::SourceText;

use self::parser::PathValue;
use crate::{AutoloadMap, ProjectLayout};

/// Parses one composer-generated `return array(...)` file into its raw `(key, values)` entries.
///
/// Reads bytes, never text: `vendor/composer/*.php` can contain a stray non-UTF-8 byte (composer
/// does not guarantee its own trailing comments are ASCII), and a strict decode would turn that
/// into an error that silently drops the *entire* file rather than the one bad byte.
/// `SourceText::from_bytes` already replaces invalid bytes instead of failing on them; reusing it
/// here keeps that decision made in exactly one place in the workspace.
fn parse_generated_file(bytes: &[u8]) -> Vec<(String, Vec<PathValue>)> {
    let source = SourceText::from_bytes(bytes);
    parser::parse_entries(lexer::tokenize(source.text()))
}

fn read_prefix_map(
    bytes: &[u8],
    vendor_dir: &Path,
    base_dir: &Path,
) -> Vec<(String, Vec<PathBuf>)> {
    parse_generated_file(bytes)
        .into_iter()
        .map(|(prefix, values)| {
            let dirs = values
                .iter()
                .map(|value| value.resolve(vendor_dir, base_dir))
                .collect();
            (prefix, dirs)
        })
        .collect()
}

fn read_classmap(bytes: &[u8], vendor_dir: &Path, base_dir: &Path) -> Vec<(String, PathBuf)> {
    parse_generated_file(bytes)
        .into_iter()
        .filter_map(|(fqn, values)| {
            values
                .first()
                .map(|value| (fqn, value.resolve(vendor_dir, base_dir)))
        })
        .collect()
}

fn read_files_list(bytes: &[u8], vendor_dir: &Path, base_dir: &Path) -> Vec<PathBuf> {
    parse_generated_file(bytes)
        .into_iter()
        .filter_map(|(_, values)| {
            values
                .first()
                .map(|value| value.resolve(vendor_dir, base_dir))
        })
        .collect()
}

/// Loads composer's own generated autoload data into `map`: `vendor/composer/autoload_psr4.php`,
/// `autoload_namespaces.php` (PSR-0), `autoload_classmap.php`, and `autoload_files.php`.
///
/// This is the authoritative source for vendor code — the merged result across every installed
/// package, already resolved by composer itself at `install`/`dump-autoload` time, and far more
/// reliable than re-deriving the same information from `installed.json` or walking `vendor/` on
/// disk by hand. A project that has not run `composer install` yet, or is simply missing one of
/// the four files, contributes nothing from that file: that is a normal state for a partially set
/// up project, not a broken one, so nothing here treats it as an error.
pub fn load_generated_autoload(
    interner: &mut Interner,
    map: &mut AutoloadMap,
    layout: &ProjectLayout,
) {
    let composer_dir = layout.vendor_dir().join("composer");
    let vendor_dir = layout.vendor_dir();
    let base_dir = layout.root();

    if let Ok(bytes) = fs::read(composer_dir.join("autoload_psr4.php")) {
        for (prefix, dirs) in read_prefix_map(&bytes, vendor_dir, base_dir) {
            map.add_psr4(interner, &prefix, dirs);
        }
    }
    if let Ok(bytes) = fs::read(composer_dir.join("autoload_namespaces.php")) {
        for (prefix, dirs) in read_prefix_map(&bytes, vendor_dir, base_dir) {
            map.add_psr0(interner, &prefix, dirs);
        }
    }
    if let Ok(bytes) = fs::read(composer_dir.join("autoload_classmap.php")) {
        for (fqn, path) in read_classmap(&bytes, vendor_dir, base_dir) {
            map.add_classmap_entry(interner, &fqn, path);
        }
    }
    if let Ok(bytes) = fs::read(composer_dir.join("autoload_files.php")) {
        for path in read_files_list(&bytes, vendor_dir, base_dir) {
            map.add_files_entry(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use super::*;

    #[test]
    fn a_classmap_with_an_invalid_utf8_byte_in_a_comment_still_loads_every_entry() {
        // A stray Latin-1 byte in a trailing comment is enough to make a strict UTF-8 decode of
        // the whole file fail; a `read_to_string` + `let Ok(..) else` guard would then silently
        // drop the entire classmap, and every class in it — including root-level ones like
        // `\Drupal` that resolve no other way — would stop working with no error anywhere.
        let fixture = MaterializedFixture::parse(
            r#"
            //- /composer.json
            {}
            "#,
        );

        let mut bytes = b"<?php\n// \xa9 copyright\nreturn array(\n".to_vec();
        bytes.extend_from_slice(b"    'Drupal' => $baseDir . '/web/core/lib/Drupal.php',\n");
        bytes.extend_from_slice(b");\n");

        let composer_dir = fixture.path("vendor/composer");
        fs::create_dir_all(&composer_dir).expect("vendor/composer should be creatable");
        fs::write(composer_dir.join("autoload_classmap.php"), &bytes)
            .expect("classmap fixture file should be writable");

        let layout = ProjectLayout::new(fixture.root());
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        load_generated_autoload(&mut interner, &mut map, &layout);

        assert_eq!(map.stats().classmap_entries, 1);
        assert_eq!(
            map.candidate_files("Drupal"),
            vec![fixture.root().join("web/core/lib/Drupal.php")]
        );
    }

    #[test]
    fn a_project_with_no_vendor_composer_directory_contributes_nothing() {
        let fixture = MaterializedFixture::parse(
            r#"
            //- /composer.json
            {}
            "#,
        );
        let layout = ProjectLayout::new(fixture.root());
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();

        load_generated_autoload(&mut interner, &mut map, &layout);

        let stats = map.stats();
        assert_eq!(stats.psr4_rules, 0);
        assert_eq!(stats.psr0_rules, 0);
        assert_eq!(stats.classmap_entries, 0);
    }
}
