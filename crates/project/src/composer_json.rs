use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use phrost_model::{Interner, PhpVersion};
use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::{AutoloadMap, VersionConstraint};

/// The root `composer.json`'s autoload configuration and PHP version requirement.
///
/// Only the fields phrost actually needs are modelled; everything else in the file (`require`
/// entries for other extensions, `scripts`, `authors`, ...) is silently ignored by serde rather
/// than rejected, which is exactly what "a minimal local struct" should do with a file this crate
/// does not own the schema of.
#[derive(Debug)]
pub struct ComposerJson {
    psr4: Vec<(String, Vec<PathBuf>)>,
    psr0: Vec<(String, Vec<PathBuf>)>,
    /// Directories and files composer.json's own `classmap` autoload entries name.
    ///
    /// These are *scan roots*, not a name-to-file map: composer turns them into one at
    /// `dump-autoload` time by parsing every PHP file under them for class declarations, which
    /// this crate is not allowed to do (it must not parse PHP). The authoritative, already-parsed
    /// result of that scan is `vendor/composer/autoload_classmap.php`, read by
    /// [`crate::load_generated_autoload`] instead. These roots are kept here, unresolved into
    /// names, for a caller that does own a class scanner and would otherwise have to re-read
    /// `composer.json` to find them.
    classmap_roots: Vec<PathBuf>,
    files: Vec<PathBuf>,
    php_requirement: Option<String>,
    /// What composer calls this package, which decides whether it has callers outside itself.
    ///
    /// `project` is a leaf -- an application, a site -- whose code is called by its own code and
    /// nothing else. Anything else is exported: composer's default is `library`, and a library's
    /// public API is called by consumers a checkout does not contain. A report of what nothing
    /// calls means opposite things in the two cases, which is why it is read at all.
    package_type: Option<String>,
}

impl ComposerJson {
    /// Reads and parses `path`.
    ///
    /// A missing or unreadable file and malformed JSON are both reported as
    /// [`ComposerJsonError`], never a panic — a project without a `composer.json`, or with a
    /// temporarily broken one mid-edit, is a normal condition a caller needs to keep going past.
    pub fn load(path: &Path) -> Result<Self, ComposerJsonError> {
        let bytes = std::fs::read(path).map_err(ComposerJsonError::Io)?;
        let base_dir = path.parent().unwrap_or(Path::new("."));
        Self::parse(&bytes, base_dir)
    }

    /// Parses already-read bytes, resolving every relative autoload path against `base_dir`.
    pub fn parse(bytes: &[u8], base_dir: &Path) -> Result<Self, ComposerJsonError> {
        let raw: RawComposerJson =
            serde_json::from_slice(bytes).map_err(ComposerJsonError::Json)?;
        Ok(Self::from_raw(raw, base_dir))
    }

    fn from_raw(raw: RawComposerJson, base_dir: &Path) -> Self {
        let mut psr4 = Vec::new();
        let mut psr0 = Vec::new();
        let mut classmap_roots = Vec::new();
        let mut files = Vec::new();

        // `autoload` and `autoload-dev` are merged: phrost analyzes a project's tests the same
        // way it analyzes its source, so there is no reason to keep the two sections apart once
        // their paths are resolved.
        for autoload in [raw.autoload, raw.autoload_dev] {
            merge_prefix_map(&mut psr4, autoload.psr4, base_dir);
            merge_prefix_map(&mut psr0, autoload.psr0, base_dir);
            classmap_roots.extend(
                autoload
                    .classmap
                    .into_iter()
                    .map(|entry| base_dir.join(entry)),
            );
            files.extend(autoload.files.into_iter().map(|entry| base_dir.join(entry)));
        }

        // Iterating the two merged hash maps above does not promise any particular order for
        // entries that came from different prefixes; sort so this type's own behavior does not
        // depend on hash bucket layout.
        psr4.sort_by(|a, b| a.0.cmp(&b.0));
        psr0.sort_by(|a, b| a.0.cmp(&b.0));

        Self {
            psr4,
            psr0,
            classmap_roots,
            files,
            php_requirement: raw.require.php,
            package_type: raw.package_type,
        }
    }

    /// Feeds every autoload rule this file declared into `map`.
    ///
    /// Composer.json's own `classmap` entries are deliberately not included here — see
    /// [`Self::classmap_roots`] for why — so a caller relying only on this method still needs
    /// [`crate::load_generated_autoload`] for classmap coverage.
    pub fn apply_to(&self, interner: &mut Interner, map: &mut AutoloadMap) {
        for (prefix, dirs) in &self.psr4 {
            map.add_psr4(interner, prefix, dirs.iter().cloned());
        }
        for (prefix, dirs) in &self.psr0 {
            map.add_psr0(interner, prefix, dirs.iter().cloned());
        }
        for path in &self.files {
            map.add_files_entry(path.clone());
        }
    }

    pub fn classmap_roots(&self) -> &[PathBuf] {
        &self.classmap_roots
    }

    /// The minimum PHP version `require.php` expresses, or `None` if it names none, or if there
    /// is no `require.php` at all.
    /// Whether this package is a leaf: an application, rather than something with consumers.
    ///
    /// `true` only for composer's `project`. Absent means `library`, which is composer's own
    /// default and the right reading of silence -- a package that says nothing about itself is
    /// one that may be depended on.
    pub fn is_an_application(&self) -> bool {
        self.package_type.as_deref() == Some("project")
    }

    pub fn php_version(&self) -> Option<PhpVersion> {
        self.php_requirement
            .as_deref()
            .and_then(|constraint| VersionConstraint::new(constraint).minimum_php_version())
    }
}

fn merge_prefix_map(
    out: &mut Vec<(String, Vec<PathBuf>)>,
    map: FxHashMap<String, StringOrVec>,
    base_dir: &Path,
) {
    for (prefix, dirs) in map {
        let dirs = dirs
            .into_vec()
            .into_iter()
            .map(|dir| base_dir.join(dir))
            .collect();
        out.push((prefix, dirs));
    }
}

/// Why [`ComposerJson::load`] could not produce a value.
#[derive(Debug)]
pub enum ComposerJsonError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for ComposerJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not read composer.json: {error}"),
            Self::Json(error) => write!(f, "composer.json is not valid JSON: {error}"),
        }
    }
}

impl std::error::Error for ComposerJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawComposerJson {
    #[serde(default)]
    autoload: RawAutoload,
    #[serde(default, rename = "autoload-dev")]
    autoload_dev: RawAutoload,
    #[serde(default)]
    require: RawRequire,
    #[serde(default, rename = "type")]
    package_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawAutoload {
    #[serde(default, rename = "psr-4")]
    psr4: FxHashMap<String, StringOrVec>,
    #[serde(default, rename = "psr-0")]
    psr0: FxHashMap<String, StringOrVec>,
    #[serde(default)]
    classmap: Vec<String>,
    #[serde(default)]
    files: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawRequire {
    php: Option<String>,
}

/// A composer.json autoload value: either one directory, or several.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    One(String),
    Many(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use super::*;

    #[test]
    fn a_psr4_value_written_as_a_bare_string_resolves_one_directory() {
        let composer_json = ComposerJson::parse(
            br#"{"autoload": {"psr-4": {"App\\": "src"}}}"#,
            Path::new("/proj"),
        )
        .expect("valid composer.json should parse");

        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        composer_json.apply_to(&mut interner, &mut map);

        assert_eq!(
            map.candidate_files("App\\Repo"),
            vec![PathBuf::from("/proj/src/Repo.php")]
        );
    }

    #[test]
    fn a_psr4_value_written_as_an_array_resolves_every_directory_in_order() {
        let composer_json = ComposerJson::parse(
            br#"{"autoload": {"psr-4": {"App\\": ["src", "src2"]}}}"#,
            Path::new("/proj"),
        )
        .expect("valid composer.json should parse");

        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        composer_json.apply_to(&mut interner, &mut map);

        assert_eq!(
            map.candidate_files("App\\Repo"),
            vec![
                PathBuf::from("/proj/src/Repo.php"),
                PathBuf::from("/proj/src2/Repo.php"),
            ]
        );
    }

    #[test]
    fn autoload_dev_entries_are_merged_in_alongside_autoload() {
        // phrost analyzes a project's tests the same way it analyzes its source, so a class only
        // reachable through `autoload-dev` must resolve exactly like one from `autoload`.
        let composer_json = ComposerJson::parse(
            br#"{"autoload-dev": {"psr-4": {"Tests\\": "tests"}}}"#,
            Path::new("/proj"),
        )
        .expect("valid composer.json should parse");

        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        composer_json.apply_to(&mut interner, &mut map);

        assert_eq!(
            map.candidate_files("Tests\\RepoTest"),
            vec![PathBuf::from("/proj/tests/RepoTest.php")]
        );
    }

    #[test]
    fn classmap_and_files_entries_resolve_relative_to_composer_jsons_own_directory() {
        let composer_json = ComposerJson::parse(
            br#"{"autoload": {"classmap": ["src/Legacy"], "files": ["src/bootstrap.php"]}}"#,
            Path::new("/proj"),
        )
        .expect("valid composer.json should parse");

        assert_eq!(
            composer_json.classmap_roots(),
            &[PathBuf::from("/proj/src/Legacy")]
        );

        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        composer_json.apply_to(&mut interner, &mut map);
        assert_eq!(map.files(), &[PathBuf::from("/proj/src/bootstrap.php")]);
    }

    #[test]
    fn require_php_becomes_the_minimum_php_version() {
        let composer_json =
            ComposerJson::parse(br#"{"require": {"php": "^8.3"}}"#, Path::new("/proj"))
                .expect("valid composer.json should parse");

        assert_eq!(composer_json.php_version(), Some(PhpVersion::new(8, 3)));
    }

    #[test]
    fn a_missing_composer_json_is_a_reported_error_not_a_panic() {
        let fixture = MaterializedFixture::parse(
            r#"
            //- /README.md
            no composer.json in this fixture
            "#,
        );

        let error = ComposerJson::load(&fixture.path("composer.json"))
            .expect_err("a file that was never written should not parse");
        assert!(matches!(error, ComposerJsonError::Io(_)));
    }

    #[test]
    fn malformed_json_is_a_reported_error_not_a_panic() {
        let error = ComposerJson::parse(b"{ this is not json", Path::new("/proj"))
            .expect_err("malformed JSON should not parse");
        assert!(matches!(error, ComposerJsonError::Json(_)));
    }
}
