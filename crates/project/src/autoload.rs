use std::path::{Path, PathBuf};

use phrost_model::{Interner, NameKey};
use rustc_hash::{FxHashMap, FxHashSet};

/// Resolves a fully-qualified PHP name to the files that could declare it.
///
/// Three independent resolution sources feed [`Self::candidate_files`], tried in the order that
/// matches how composer itself prioritizes them: an exact classmap entry beats any PSR-4 rule,
/// which beats any PSR-0 rule. None of this touches the filesystem — a caller checks the returned
/// candidates against whatever index or disk state it has, in order, until one exists.
#[derive(Debug, Default)]
pub struct AutoloadMap {
    classmap: FxHashMap<NameKey, Vec<PathBuf>>,
    psr4: Vec<PrefixRule>,
    psr0: Vec<PrefixRule>,
    files: Vec<PathBuf>,
}

/// One `prefix => [dirs]` rule. The same shape serves PSR-4 and PSR-0 — composer's own config
/// format uses it for both — only how a match becomes a file path differs, which is why that part
/// lives in two separate methods below rather than a shared one.
#[derive(Debug)]
struct PrefixRule {
    prefix: NameKey,
    /// The prefix as composer wrote it, case and all.
    ///
    /// Beside the folded key rather than instead of it: resolution compares names the way PHP does,
    /// which is case-insensitively, and a reader that *writes a namespace back* needs the case the
    /// project uses. `Drupal\app_core\` suggested as `drupal\app_core\` is a suggestion nobody
    /// would accept.
    written: Box<str>,
    dirs: Vec<PathBuf>,
}

impl PrefixRule {
    /// Whether `fqn` starts with this rule's prefix, ASCII-case-insensitively.
    ///
    /// Slicing `fqn` to the prefix's byte length and comparing that slice, rather than folding
    /// `fqn` itself: ASCII folding never changes a string's byte length or the position of any
    /// byte, so this is always a valid comparison, and avoids allocating a folded copy of `fqn`
    /// for every rule checked.
    fn matches(&self, fqn: &str) -> bool {
        fqn.get(..self.prefix.as_str().len())
            .is_some_and(|head| head.eq_ignore_ascii_case(self.prefix.as_str()))
    }

    /// PSR-4 candidates: one per registered directory, built from the part of `fqn` *after* this
    /// rule's prefix — the directory already stands in for the prefix itself.
    fn psr4_candidates<'a>(&'a self, fqn: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
        let remainder = &fqn[self.prefix.as_str().len()..];
        self.dirs
            .iter()
            .map(move |dir| psr4_class_path(dir, remainder))
    }

    /// PSR-0 candidates: one per registered directory, built from the *whole* name. Composer's
    /// own runtime loader does not strip the matched prefix before building a PSR-0 path — unlike
    /// PSR-4, where the directory already covers the prefix — so this must not strip it either.
    fn psr0_candidates<'a>(&'a self, fqn: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
        self.dirs.iter().map(move |dir| psr0_class_path(dir, fqn))
    }
}

impl AutoloadMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one exact-name resolution.
    ///
    /// This is composer's generated `autoload_classmap.php` in miniature: the only way a
    /// non-namespaced, non-PSR-4 class (Drupal's root `\Drupal` is the standing example) ever
    /// resolves at all, since no prefix rule can name it. Multiple registrations of the same name
    /// accumulate rather than overwrite, so a caller merging composer.json's own classmap with
    /// vendor's generated one never silently loses an entry to the other.
    pub fn add_classmap_entry(&mut self, interner: &mut Interner, fqn: &str, path: PathBuf) {
        self.classmap
            .entry(interner.key(fqn))
            .or_default()
            .push(path);
    }

    pub fn add_psr4(
        &mut self,
        interner: &mut Interner,
        prefix: &str,
        dirs: impl IntoIterator<Item = PathBuf>,
    ) {
        Self::register_prefix_rule(&mut self.psr4, interner, prefix, dirs);
    }

    pub fn add_psr0(
        &mut self,
        interner: &mut Interner,
        prefix: &str,
        dirs: impl IntoIterator<Item = PathBuf>,
    ) {
        Self::register_prefix_rule(&mut self.psr0, interner, prefix, dirs);
    }

    /// One eagerly-included file. Carried without interpretation: nothing here decides when or
    /// whether it runs, only that composer said to include it.
    pub fn add_files_entry(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Every directory composer registered as holding code.
    ///
    /// This is what discovery walks, and the reason it has to. A composer-installed package does
    /// not have to live under `vendor/`: Drupal puts its core under `docroot/core` and its contrib
    /// modules under `docroot/modules/contrib`, all of it gitignored, none of it under `vendor/`. A
    /// walk that trusted `.gitignore` to find code would miss every one of them, and then Drupal's
    /// entire core would look undeclared.
    ///
    /// Classmap entries contribute their parent directory, since composer records a file per class.
    /// Sorted and deduplicated, so walking them is stable and does not repeat work.
    /// The directories a namespace prefix is mapped onto, and only those.
    ///
    /// Apart from [`Self::source_roots`], which also takes the parent of every classmap entry so a
    /// discovery walk reaches them. That is the wrong set for a question about *territory*: a
    /// classmap names one file, and its directory holds files the autoloader knows nothing about.
    pub fn namespace_roots(&self) -> Vec<PathBuf> {
        let mut roots = self
            .psr4
            .iter()
            .chain(&self.psr0)
            .flat_map(|rule| rule.dirs.iter().cloned())
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        roots.dedup_by(|inner, outer| inner.starts_with(&*outer));
        roots
    }

    /// Every PSR-4 rule, as the prefix composer wrote and one directory it maps onto.
    ///
    /// Flattened to a pair per directory, because that is the shape the question is asked in: given
    /// a file's path, which prefix does the directory holding it stand for.
    pub fn psr4_rules(&self) -> impl Iterator<Item = (&str, &PathBuf)> {
        self.psr4
            .iter()
            .flat_map(|rule| rule.dirs.iter().map(move |dir| (&*rule.written, dir)))
    }

    pub fn source_roots(&self) -> Vec<PathBuf> {
        let mut roots = self
            .psr4
            .iter()
            .chain(&self.psr0)
            .flat_map(|rule| rule.dirs.iter().cloned())
            .chain(
                self.classmap
                    .values()
                    .flatten()
                    .filter_map(|path| path.parent().map(Path::to_path_buf)),
            )
            .collect::<Vec<_>>();

        roots.sort();
        roots.dedup();
        // A nested root adds nothing: walking the outer one covers it.
        roots.dedup_by(|inner, outer| inner.starts_with(&*outer));
        roots
    }

    fn register_prefix_rule(
        rules: &mut Vec<PrefixRule>,
        interner: &mut Interner,
        prefix: &str,
        dirs: impl IntoIterator<Item = PathBuf>,
    ) {
        let key = interner.key(prefix);
        match rules.iter_mut().find(|rule| rule.prefix == key) {
            // Two sources can register the same namespace prefix with different directories — a
            // split package, or a root composer.json re-declaring a vendor namespace. Compose
            // rather than let the second registration shadow the first outright.
            Some(rule) => rule.dirs.extend(dirs),
            None => rules.push(PrefixRule {
                prefix: key,
                written: prefix.into(),
                dirs: dirs.into_iter().collect(),
            }),
        }
    }

    /// Every file that could declare `fqn`, most likely first: classmap, then the longest
    /// matching PSR-4 prefix, then every matching PSR-0 prefix.
    ///
    /// Never touches the filesystem. A caller checks each candidate — against an index, against
    /// disk, whatever it has — in order, and stops at the first that exists.
    pub fn candidate_files(&self, fqn: &str) -> Vec<PathBuf> {
        // `\App\Foo` and `App\Foo` name the same class; normalize before matching either kind of
        // rule so a caller does not have to agree with us on which spelling to use.
        let fqn = fqn.strip_prefix('\\').unwrap_or(fqn);

        let mut candidates = Vec::new();

        // classmap: exact, case-insensitive match. `NameKey` borrows as `str`, so the folded query
        // can be looked up directly -- no interner, and no allocation at all for a name that is
        // already lowercase, which most of a Drupal classmap is. Folding is ASCII-only, matching
        // PHP's `zend_str_tolower`.
        let folded = fold_ascii(fqn);
        if let Some(paths) = self.classmap.get(folded.as_ref()) {
            candidates.extend(paths.iter().cloned());
        }

        // PSR-4: only the longest matching prefix contributes. Without that, a project overriding
        // one sub-namespace of a vendor package (`App\Vendor\Patched\` living in `src/`) would
        // also get a second, wrong candidate from the vendor's own broader `App\Vendor\` rule.
        if let Some(rule) = self
            .psr4
            .iter()
            .filter(|rule| rule.matches(fqn))
            .max_by_key(|rule| rule.prefix.as_str().len())
        {
            candidates.extend(rule.psr4_candidates(fqn));
        }

        // PSR-0: legacy, and every matching prefix contributes. Composer's own runtime loader
        // tries each registered prefix in turn rather than picking one winner, and nothing in the
        // PSR-0 spec suggests narrowing that the way PSR-4 does.
        for rule in self.psr0.iter().filter(|rule| rule.matches(fqn)) {
            candidates.extend(rule.psr0_candidates(fqn));
        }

        dedup_preserving_order(candidates)
    }

    /// Rule and entry counts, for diagnostics — not the number of candidate paths, which can be
    /// larger when one prefix or name has several registered directories.
    pub fn stats(&self) -> AutoloadStats {
        AutoloadStats {
            psr4_rules: self.psr4.len(),
            psr0_rules: self.psr0.len(),
            classmap_entries: self.classmap.len(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AutoloadStats {
    pub psr4_rules: usize,
    pub psr0_rules: usize,
    pub classmap_entries: usize,
}

/// Builds a PSR-4 candidate: `remainder`'s namespace separators become nested directories, and
/// the final segment gets a `.php` extension.
fn psr4_class_path(dir: &Path, remainder: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    let mut segments = remainder.split('\\').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_some() {
            path.push(segment);
        } else {
            path.push(format!("{segment}.php"));
        }
    }
    path
}

/// Builds a PSR-0 candidate for the whole name. Only the final segment — the class name itself,
/// after the last namespace separator — has its underscores translated into directories too;
/// namespace segments before it do not. This mirrors the historical `SplClassLoader` algorithm
/// exactly, asymmetry included: `Vendor_Ns\Bar_Baz` becomes `Vendor_Ns/Bar/Baz.php`, keeping the
/// namespace segment's underscore intact, rather than `Vendor/Ns/Bar/Baz.php`.
fn psr0_class_path(dir: &Path, fqn: &str) -> PathBuf {
    let (namespace, class_name) = fqn.rsplit_once('\\').unwrap_or(("", fqn));

    let mut path = dir.to_path_buf();
    for segment in namespace.split('\\').filter(|segment| !segment.is_empty()) {
        path.push(segment);
    }

    let mut parts = class_name.split('_').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_some() {
            path.push(part);
        } else {
            path.push(format!("{part}.php"));
        }
    }
    path
}

/// Removes duplicate candidates while keeping the first occurrence's position — a plain sort
/// would destroy the priority order that is the entire point of [`AutoloadMap::candidate_files`].
fn dedup_preserving_order(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = FxHashSet::default();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

/// Lowercases ASCII letters, borrowing when there is nothing to change.
///
/// This sits on the lazy-resolution path, which runs once per unresolved class name, so the common
/// all-lowercase case must not allocate.
fn fold_ascii(text: &str) -> std::borrow::Cow<'_, str> {
    if text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        std::borrow::Cow::Owned(text.to_ascii_lowercase())
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psr4_longest_matching_prefix_wins_over_a_shorter_one() {
        // `App\Sub\` is a more specific rule than `App\`; if both match, only the more specific
        // one should contribute, or a project could never override one sub-namespace of a
        // dependency without the dependency's own broader rule also offering a wrong candidate.
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr4(&mut interner, "App\\", [PathBuf::from("/proj/src")]);
        map.add_psr4(
            &mut interner,
            "App\\Sub\\",
            [PathBuf::from("/proj/special")],
        );

        assert_eq!(
            map.candidate_files("App\\Sub\\Thing"),
            vec![PathBuf::from("/proj/special/Thing.php")]
        );
    }

    #[test]
    fn psr4_with_multiple_directories_yields_one_candidate_per_directory_in_order() {
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr4(
            &mut interner,
            "App\\",
            [PathBuf::from("/proj/src"), PathBuf::from("/proj/src2")],
        );

        assert_eq!(
            map.candidate_files("App\\Sub\\Thing"),
            vec![
                PathBuf::from("/proj/src/Sub/Thing.php"),
                PathBuf::from("/proj/src2/Sub/Thing.php"),
            ]
        );
    }

    #[test]
    fn psr0_translates_underscores_in_a_legacy_pear_style_class_name() {
        // The historical convention this exists for: no namespace at all, the whole name uses
        // underscores as a stand-in for directory separators.
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr0(&mut interner, "Zend_", [PathBuf::from("/proj/vendor/zend")]);

        assert_eq!(
            map.candidate_files("Zend_Db_Adapter"),
            vec![PathBuf::from("/proj/vendor/zend/Zend/Db/Adapter.php")]
        );
    }

    #[test]
    fn psr0_only_translates_underscores_in_the_final_segment() {
        // A namespaced PSR-0 class only translates underscores in the class name after the last
        // `\`; earlier namespace segments are left alone even if they contain an underscore.
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr0(&mut interner, "Vendor_Ns\\", [PathBuf::from("/proj/lib")]);

        assert_eq!(
            map.candidate_files("Vendor_Ns\\Bar_Baz"),
            vec![PathBuf::from("/proj/lib/Vendor_Ns/Bar/Baz.php")]
        );
    }

    #[test]
    fn classmap_hit_is_returned_before_any_psr4_candidate() {
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_classmap_entry(
            &mut interner,
            "App\\Foo",
            PathBuf::from("/proj/generated/Foo.php"),
        );
        map.add_psr4(&mut interner, "App\\", [PathBuf::from("/proj/src")]);

        assert_eq!(
            map.candidate_files("App\\Foo"),
            vec![
                PathBuf::from("/proj/generated/Foo.php"),
                PathBuf::from("/proj/src/Foo.php"),
            ]
        );
    }

    #[test]
    fn classmap_lookup_folds_ascii_case_like_phps_own_class_table() {
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_classmap_entry(
            &mut interner,
            "App\\Foo",
            PathBuf::from("/proj/generated/Foo.php"),
        );

        assert_eq!(
            map.candidate_files("APP\\foo"),
            vec![PathBuf::from("/proj/generated/Foo.php")]
        );
    }

    #[test]
    fn psr4_prefix_matching_folds_case_but_the_remainder_keeps_its_written_casing() {
        // The registered prefix matches regardless of how the caller cased it, but the remainder
        // — which becomes part of a real filename — must keep exactly the casing the caller
        // wrote, because the actual file on disk may live on a case-sensitive filesystem.
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr4(&mut interner, "App\\", [PathBuf::from("/proj/src")]);

        assert_eq!(
            map.candidate_files("app\\Sub\\Thing"),
            vec![PathBuf::from("/proj/src/Sub/Thing.php")]
        );
    }

    #[test]
    fn candidates_are_deduplicated_without_disturbing_priority_order() {
        // The same file can legitimately be reachable two ways (a classmap entry pointing at
        // exactly what a PSR-4 rule would also compute); the duplicate should disappear, but the
        // survivor must be the earlier, higher-priority occurrence.
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_classmap_entry(
            &mut interner,
            "App\\Foo",
            PathBuf::from("/proj/src/Foo.php"),
        );
        map.add_psr4(&mut interner, "App\\", [PathBuf::from("/proj/src")]);

        assert_eq!(
            map.candidate_files("App\\Foo"),
            vec![PathBuf::from("/proj/src/Foo.php")]
        );
    }

    #[test]
    fn a_leading_backslash_names_the_same_class_as_without_one() {
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr4(&mut interner, "App\\", [PathBuf::from("/proj/src")]);

        assert_eq!(
            map.candidate_files("\\App\\Foo"),
            map.candidate_files("App\\Foo")
        );
    }

    #[test]
    fn a_name_with_no_matching_rule_resolves_to_nothing() {
        let map = AutoloadMap::new();
        assert_eq!(
            map.candidate_files("Unregistered\\Thing"),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn stats_count_rules_and_names_not_the_candidate_paths_they_expand_to() {
        let mut interner = Interner::new();
        let mut map = AutoloadMap::new();
        map.add_psr4(
            &mut interner,
            "App\\",
            [PathBuf::from("/proj/src"), PathBuf::from("/proj/src2")],
        );
        map.add_classmap_entry(
            &mut interner,
            "Drupal",
            PathBuf::from("/proj/web/core/Drupal.php"),
        );

        let stats = map.stats();
        assert_eq!(stats.psr4_rules, 1);
        assert_eq!(stats.psr0_rules, 0);
        assert_eq!(stats.classmap_entries, 1);
    }
}
