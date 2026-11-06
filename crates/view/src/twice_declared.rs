//! A name the project's own code declares in two files.
//!
//! `Cannot redeclare class C` is a fatal the moment both files are loaded, and which one PHP
//! reaches first is an accident of the autoloader -- so a project with one of these has a class
//! that resolves differently depending on the request. Every other answer this index gives about
//! such a name is a coin toss too, which is why it is worth saying out loud: a catalog that holds
//! two claimants for one name ranks them, and the loser is silently invisible.
//!
//! # What is not reported, and why
//!
//! - **Anything outside the project's own code.** A class moved between packages is in both for a
//!   release, a polyfill declares what an extension also declares, and only one of the two is ever
//!   autoloaded. Vendor duplicates are ordinary and none of them is the author's to fix.
//! - **A conditional declaration.** `if (!class_exists(Foo::class)) { class Foo {} }` is how a
//!   project ships a shim, and the guard is the point. The catalog already demotes one -- see
//!   `ProjectSkeleton::rank` -- and what it demotes, this skips.
//! - **A second declaration in the same file**, which PHP refuses for a different reason and which
//!   a file can only have by being unrunnable on its own.
//! - **An anonymous class**, which has no name PHP could redeclare. The catalog gives one a
//!   synthetic `class@anonymous#88` so that `$this` inside it resolves, and two files whose
//!   anonymous classes land on the same counter are not a conflict.
//! - **A declaration the autoloader would never be asked for.** This is the one that matters on
//!   real code: a test fixture, an example script and a dumped container each declare a name and
//!   are included *by path*, so two of them may share a name and PHP never sees a second
//!   declaration. What separates them from a real conflict is PSR-4 -- a class the autoloader
//!   finds by name has exactly one file it can come from, and two files claiming that place is a
//!   conflict whichever one wins. Measured: without this rule, Symfony reported 39 and Laravel 22,
//!   every one of them a fixture or an example.

use phrost_model::{Name, SourceOrigin, TextRange};

use crate::view::IndexView;

/// One class-like name two of the project's own files declare.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TwiceDeclared {
    /// The declaration in *this* file, which is what a report underlines.
    pub range: TextRange,
    pub class: Name,
    /// Where the other one is, as a project-relative path.
    pub other: String,
}

impl IndexView<'_> {
    /// Every class-like in the current file that another of the project's files also declares.
    pub fn twice_declared(&self) -> Vec<TwiceDeclared> {
        let Some(file) = self.current_file() else {
            return Vec::new();
        };
        let Some(skeleton) = self.skeleton.file(file) else {
            return Vec::new();
        };
        if skeleton.origin() != SourceOrigin::User {
            return Vec::new();
        }
        let mut out = Vec::new();

        for class in skeleton.classes() {
            // A synthetic name for something PHP cannot redeclare.
            if class.fqn().as_str().contains("class@anonymous") {
                continue;
            }
            if !self.is_autoloadable(file, class.fqn().as_str())
                // A guarded declaration is a shim and the guard is the point -- on *either* side.
                // Asked of this file too, which the first spelling forgot: only the unguarded half
                // of a shim pair was silent, so the pair reported once instead of not at all.
                || self.declared_conditionally(file, class.fqn().as_str())
            {
                continue;
            }
            let Some(key) = self.skeleton.interner().lookup_key(class.fqn().as_str()) else {
                continue;
            };
            for declarer in self.skeleton.owners().class_like(&key) {
                if declarer.file == file {
                    continue;
                }
                let Some(other) = self.skeleton.file(declarer.file) else {
                    continue;
                };
                if other.origin() != SourceOrigin::User {
                    continue;
                }
                if self.declared_conditionally(declarer.file, class.fqn().as_str())
                    || !self.is_autoloadable(declarer.file, class.fqn().as_str())
                {
                    continue;
                }
                // This file's own declaration, not the catalog's winner for the name: every
                // class here is one *this* file declares, and the winner's range belongs to the
                // other file -- which on a shorter one is a position that does not exist.
                let Some(location) = self.declared_in(file, class.fqn()) else {
                    continue;
                };
                out.push(TwiceDeclared {
                    range: location.name_range,
                    class: class.fqn().clone(),
                    other: self.file_path(declarer.file).unwrap_or_default().to_owned(),
                });
                break;
            }
        }

        out
    }

    /// Whether this file declares `fqn` behind a guard.
    ///
    /// `if (!class_exists(Foo::class)) { class Foo {} }` is how a project ships a shim, and the
    /// guard is what makes two declarations correct. By name rather than by the declarer's symbol
    /// index, so the same question can be asked of the file being checked and of the other one --
    /// which is the whole of the rule, and asking it of one side only made a shim pair report
    /// exactly once.
    fn declared_conditionally(&self, file: phrost_model::FileId, fqn: &str) -> bool {
        self.skeleton.file(file).is_some_and(|skeleton| {
            skeleton
                .scope()
                .symbols()
                .iter()
                .any(|symbol| symbol.fqn.as_str() == fqn && symbol.is_conditional())
        })
    }

    /// Whether the autoloader would find `fqn` at this file, under the project's PSR-4 rules.
    ///
    /// Both halves of what PSR-4 asks: the directory has to permit the namespace, and the file's
    /// stem has to be the short name. A file that fails either is included by path rather than
    /// found by name, and two such files may share a name without PHP ever being asked to
    /// redeclare anything.
    ///
    /// `false` where the project states no rules at all -- a scratch project with no dumped
    /// `vendor/composer/autoload_psr4.php` has none, which §8 records as the shape that makes a
    /// check keyed on them look broken. Silence there is the same silence this file keeps over
    /// every other gap.
    fn is_autoloadable(&self, file: phrost_model::FileId, fqn: &str) -> bool {
        let Some(path) = self.file_path(file).map(std::path::Path::new) else {
            return false;
        };
        let namespace = match fqn.rfind('\\') {
            Some(at) => &fqn[..at],
            None => "",
        };
        let stem = path.file_stem().and_then(|stem| stem.to_str());
        stem == Some(phrost_model::short_name(fqn))
            && self
                .skeleton
                .permitted_namespaces(path)
                .iter()
                .any(|one| one == namespace)
    }
}
