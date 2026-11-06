//! Symfony, as much of it as indexing needs.
//!
//! Deliberately shallow, for the reason the Laravel provider gives: what a walk of a Symfony project
//! gets wrong without being told is that the framework compiles PHP into a directory, and a walk for
//! `*.php` finds it.
//!
//! `var/cache/<env>` is where, and Symfony's own `Kernel::getCacheDir()` says so -- it returns
//! `$this->getProjectDir().'/var/cache/'.$this->environment`, and `getBuildDir()` returns the same.
//! What lands there is the compiled dependency-injection container: one `<?php class ...` per
//! service graph, generated, hashed, and large. `var/log` beside it holds no PHP at all, so it is
//! not named -- only what a walk for code would otherwise pick up.
//!
//! The whole of `var/cache` rather than one environment, because a project has as many as it has
//! environments and `dev`, `test` and `prod` are only the conventional three.
//!
//! Usually gitignored, and discovery honours that; naming it is what makes the answer not depend on
//! this checkout's `.gitignore`.

use std::path::{Path, PathBuf};

use crate::FrameworkProject;

#[derive(Debug)]
pub struct Symfony;

impl crate::FrameworkProvider for Symfony {
    fn id(&self) -> &'static str {
        "symfony"
    }

    /// The console entry point and the bundle list, and neither alone.
    ///
    /// `bin/console` is a path other frameworks use too, and `config/bundles.php` is the one file
    /// only a Symfony application has -- Flex writes it and every bundle installed edits it. Asking
    /// for both keeps a project that merely *depends* on a Symfony component from being read as an
    /// application, which matters because the exclusion below is about an application's layout.
    ///
    /// A checkout of `symfony/symfony` itself has neither at its root, which is right: the monorepo
    /// is the library and has no `var/cache` to keep out.
    fn detect(&self, root: &Path) -> bool {
        root.join("bin/console").is_file() && root.join("config/bundles.php").is_file()
    }

    fn project(&self, root: &Path) -> Box<dyn FrameworkProject> {
        Box::new(SymfonyProject {
            root: root.to_path_buf(),
        })
    }
}

#[derive(Debug)]
struct SymfonyProject {
    root: PathBuf,
}

impl FrameworkProject for SymfonyProject {
    /// Only if it exists, so a project that has never been booted contributes nothing.
    fn excluded_paths(&self) -> Vec<PathBuf> {
        let cache = self.root.join("var/cache");
        match cache.is_dir() {
            true => vec![cache],
            false => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FrameworkProvider;

    /// Both markers, for the reason the doc gives: a project merely depending on a Symfony component
    /// is not an application and has no `var/cache` layout to speak of.
    #[test]
    fn an_application_is_detected_and_a_library_is_not() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        assert!(!Symfony.detect(root), "an empty directory is not Symfony");

        std::fs::create_dir_all(root.join("bin")).expect("bin");
        std::fs::write(root.join("bin/console"), "#!/usr/bin/env php\n").expect("console");
        assert!(
            !Symfony.detect(root),
            "a console entry point alone is not enough -- other frameworks use the path"
        );

        std::fs::create_dir_all(root.join("config")).expect("config");
        std::fs::write(root.join("config/bundles.php"), "<?php return [];\n").expect("bundles");
        assert!(
            Symfony.detect(root),
            "both markers together are an application"
        );
    }

    /// The whole of `var/cache` and not one environment, and only once it is there.
    #[test]
    fn the_compiled_cache_is_excluded_once_it_exists() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        assert!(
            Symfony.project(root).excluded_paths().is_empty(),
            "a project that has never been booted compiled nothing"
        );

        std::fs::create_dir_all(root.join("var/cache/dev")).expect("dev cache");
        std::fs::create_dir_all(root.join("var/cache/prod")).expect("prod cache");
        // `var/log` holds no PHP, so it is deliberately not named.
        std::fs::create_dir_all(root.join("var/log")).expect("log");

        assert_eq!(
            Symfony.project(root).excluded_paths(),
            vec![root.join("var/cache")],
            "the directory above every environment, once"
        );
    }
}
