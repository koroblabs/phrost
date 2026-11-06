//! The allowed reasons to touch the filesystem, and the check that enforces them.
//!
//! §1.2 of `CLAUDE.md` says the index either has an answer or returns a deterministic *not found* --
//! no "try the index, else re-parse, else scan". The shape that rule actually takes in a language
//! server is a **read**: something asks the index, gets nothing, and goes to disk to make up for it.
//! `phrost-old` had seven of those and they were the whole reason it worked sometimes.
//!
//! So the reads are enumerated. A file that touches the filesystem is listed here with why, and a
//! new one fails this test until somebody writes down what it is for. That is the same shape as
//! `dependency_rules`, for the same reason: a rule with no check does not hold.
//!
//! # Why an allow-list rather than the cfg the docs used to describe
//!
//! The plan called for a `deny-fallbacks` cfg that panics on any second path. A cfg only fires from
//! code that calls it, so it can only catch a fallback whose author remembered to mark it -- which
//! is the author who would not have written one. An enumeration of reads catches the one who did
//! not remember, which is the only case that matters.
//!
//! The list is phrost's and lives here. The scan is `lspkit_arch::reads`, which sees a read that
//! starts with punctuation after a name -- `path.exists()` -- where this check's own scan saw it
//! only after a `)`.

use lspkit_arch::{
    reads::{Allowed, Reads},
    workspace::Workspace,
};

/// What a filesystem read looks like in this codebase.
///
/// Deliberately shallow. It matches the calls that reach the disk, not every mention of a path --
/// building a `PathBuf` is not a read, and a check that flagged it would be turned off within a
/// week.
const READS: &[&str] = &[
    "fs::read",
    "fs::write",
    "fs::metadata",
    "fs::read_dir",
    "fs::read_to_string",
    "fs::File",
    "SourceText::read",
    "WalkBuilder::new",
    // Asking whether a path is there is a read, and the cheapest possible shape for a fallback:
    // `if path.exists()` is how one gets written by somebody who would not have written
    // `read_to_string`. Every production use of these is a deliberate probe for configuration that
    // nothing records, and each is named below.
    ".is_file()",
    ".is_dir()",
    ".exists()",
    ".try_exists()",
];

/// Every read the design intends. Adding an entry is a deliberate statement, not a formality.
const ALLOWED: &[Allowed] = &[
    Allowed {
        file: "crates/vfs/src/source.rs",
        why: "reading a file's text is what the VFS is",
    },
    Allowed {
        file: "crates/vfs/src/vfs.rs",
        why: "stamping a file on disk, to notice a change made outside the editor",
    },
    Allowed {
        file: "crates/index/src/skeleton.rs",
        why: "one slice of one file for text the skeleton deliberately does not retain -- a \
              docblock. Where the fact always came from, not a stand-in for a missing answer",
    },
    Allowed {
        file: "crates/index/src/bulk.rs",
        why: "reading each file of a batch, on the worker that will lower it -- the bootstrap read \
              itself, moved off one thread. Not a stand-in for a missing answer: nothing has been \
              asked yet when this runs",
    },
    Allowed {
        file: "crates/index/src/cache/mod.rs",
        why: "writing the index down and reading it back, which is what the cache is",
    },
    Allowed {
        file: "crates/project/src/composer_json.rs",
        why: "composer's own configuration, which says where the code is",
    },
    Allowed {
        file: "crates/project/src/generated/mod.rs",
        why: "composer's generated autoload maps, same reason",
    },
    Allowed {
        file: "crates/project/src/discovery.rs",
        why: "walking the project to find the files to index",
    },
    Allowed {
        file: "crates/stubs/src/lib.rs",
        why: "materialising the bundled standard library into a cache directory",
    },
    Allowed {
        file: "crates/stubs/build.rs",
        why: "bundling the standard library at build time",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/layout.rs",
        why: "probing for where composer put the docroot, which nothing records",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/extensions.rs",
        why: "reading the descriptors a framework registers namespaces from, which are not in \
              composer's map",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/services.rs",
        why: "reading the container configuration, which is the only statement of what a service \
              id names",
    },
    Allowed {
        file: "crates/framework/src/providers/mockery/mod.rs",
        why: "one probe for the installed package, which is what says the interfaces it names exist \
              for anything to be typed as -- read once, when the root is fixed",
    },
    Allowed {
        file: "crates/framework/src/providers/pest/mod.rs",
        why: "one probe for the installed package, which is what says `uses()` configures a test \
              case at all -- read once, when the root is fixed",
    },
    Allowed {
        file: "crates/framework/src/providers/phpunit/mod.rs",
        why: "one probe for the installed package, which is what says a `TestCase` exists for \
              anything to inherit -- read once, when the root is fixed",
    },
    Allowed {
        file: "crates/framework/src/providers/twig/mod.rs",
        why: "detection: whether the Twig package is installed, asked once when the root is fixed \
              and not on the path a request takes",
    },
    Allowed {
        file: "crates/framework/src/providers/laravel/mod.rs",
        why: "two probes: the console script and the application factory that say a checkout is a \
              Laravel *application* rather than the framework itself, and the two generated \
              directories it compiles PHP into -- read once, when the root is fixed, so that \
              neither is indexed whatever the project's ignore files say. The writes the scan \
              also sees are the module's own tests building a fixture tree",
    },
    Allowed {
        file: "crates/framework/src/providers/symfony/mod.rs",
        why: "the same two questions for Symfony: whether the checkout is an application, and \
              whether it has compiled a container into `var/cache` -- read once, when the root is \
              fixed, and not on the path a request takes. The writes are its tests' fixture \
              tree, as above",
    },
    Allowed {
        file: "crates/project/src/config.rs",
        why: "reading the project's own settings, which nothing else states -- and which is read \
              once, when the root is fixed, rather than on the path a request takes",
    },
    Allowed {
        file: "crates/framework/src/providers/laravel/aliases.rs",
        why: "the provider reads `config/app.php` and the installed framework's `Facade.php` once at \
              startup to list the global names its loader aliases; the writes are its tests' fixtures",
    },
    Allowed {
        file: "crates/lsp/src/workspace/templates.rs",
        why: "the files that render a template, read to be lowered beside it when it is analysed -- \
              the call that renders one is the only statement of what its variables are, and a \
              closed file keeps no body",
    },
    Allowed {
        file: "crates/framework/src/providers/laravel/views.rs",
        why: "the provider lists the views directory once at startup to file each template under the \
              name a call renders it by; the writes are its tests' fixtures",
    },
    Allowed {
        file: "crates/framework/src/providers/laravel/translations.rs",
        why: "the provider reads the source locale's language files once at startup to list the \
              translation keys holding arrays; the writes are its tests' fixtures",
    },
    Allowed {
        file: "crates/framework/src/providers/laravel/auth.rs",
        why: "the provider reads `config/auth.php` once at startup to name the user model, as every \
              provider reads its project's configuration; the writes are its tests' fixtures",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/config.rs",
        why: "listing the directories that hold configuration objects, whose file names are the only \
              statement that a configuration object exists, and finding the exported copy by the \
              marker file Drupal requires in it",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/fields.rs",
        why: "reading the exported field configuration, which is the only statement that a field \
              exists -- a field is configuration and no PHP declaration mentions one",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/sites.rs",
        why: "the listing of `sites/` that says which of its files are settings and which \
              directory is the public files store -- read once, when the root is fixed, so that \
              neither is indexed or checked whatever the project's ignore files say",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/survey.rs",
        why: "the one walk of the docroot: it finds the descriptors a framework registers \
              namespaces from and the files it loads by hand, neither of which composer's map or \
              any autoload rule names -- read once, when the root is fixed, and not on the path a \
              request takes",
    },
    Allowed {
        file: "crates/framework/src/providers/drupal/places.rs",
        why: "reading the routing and permission declarations, which are YAML and so are the only \
              statement anywhere of which route names and permission ids exist",
    },
    Allowed {
        file: "crates/lsp/src/workspace/mod.rs",
        why: "opening a buffer, reading a file the editor does not hold, the byte filter in front \
              of a reference sweep, the project's configuration read once at bootstrap for the \
              classes it names, and the running executable's own stamp -- which is what keeps a \
              rebuilt server from reusing rows a different build derived",
    },
    Allowed {
        file: "crates/lsp/src/workspace/composer.rs",
        why: "when composer last wrote its lock and its install record, which is the only statement \
              anywhere of whether a run is still replacing packages -- read when one of the two is \
              reported changed, and while a run is waited out, never on the path a request takes",
    },
    Allowed {
        file: "crates/lsp/src/answers/positions.rs",
        why: "the text of a configuration file an answer points into, to turn its byte range into \
              a line and a column. A configuration file is not indexed and has no `FileId` -- it \
              holds no declarations, only names of them -- so there is no stored line map to ask, \
              and the file the editor has open is preferred over the disk where it has one",
    },
    Allowed {
        file: "crates/lsp/src/documents.rs",
        why: "the text of a file nobody has open, for checking it: the project-wide pass and a tab \
              closing both report what the disk holds, and the disk is the only place that text is",
    },
];

/// Paths whose reads are not policed, and why. Test sources -- a `tests/` directory or a `tests.rs`
/// file -- are skipped by the scan itself.
///
/// A test may read whatever it needs to set up a fixture, and a command-line tool exists to look at
/// the project from outside. Neither is on the path a request takes.
const UNPOLICED: &[(&str, &str)] = &[
    (
        "crates/cli/",
        "the command line looks at a project from outside, and no request passes through it",
    ),
    (
        "crates/tools/",
        "the checks, which read the workspace in order to check it",
    ),
    (
        "crates/lib/testkit/",
        "test support: materialising a fixture is writing files to read back",
    ),
];

#[test]
fn every_filesystem_read_is_one_the_design_intends() {
    let workspace = Workspace::above(env!("CARGO_MANIFEST_DIR"), 3);
    let reads = Reads {
        patterns: READS,
        unpoliced: UNPOLICED,
        allowed: ALLOWED,
        // The walk has to have seen something, or an empty result means the check is not running
        // rather than that the codebase is clean.
        min_policed: 21,
    };
    let problems = reads.violations(&workspace.rust_files("crates"));
    assert!(
        problems.is_empty(),
        "these reads are not ones the design intends. A read that is where a fact comes from goes \
         in `ALLOWED` with what it is for; one that is there because the index did not have an \
         answer is the fallback §1.2 forbids: make the index answer, or return nothing.\n\n{}",
        problems.join("\n")
    );
}
