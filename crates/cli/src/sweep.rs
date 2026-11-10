//! Walking a project's own code, one open buffer at a time.
//!
//! What `coverage` and `check` both need, and the part of it that is easy to get wrong: the server's
//! own setup, a file id, a line map for the revision being read, and a buffer that is closed again
//! afterwards. Full analysis of a whole project at once is the weight this design exists to avoid,
//! so a tool that leaves every file open is measuring a configuration the server never runs in.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use phrost_lsp::Workspace;
use phrost_model::{LineIndex, SourceOrigin};
use phrost_stubs::StubSet;
use phrost_vfs::{DocumentVersion, SourceText};
use phrost_view::IndexView;

/// Whether a sweep indexes from scratch or from what a previous run stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Start {
    Cold,
    Warm,
}

pub(crate) struct Sweep {
    workspace: Workspace,
    root: PathBuf,
    indexed: usize,
    /// How many files came out of a stored index rather than being read and lowered again.
    ///
    /// Reported rather than kept private, because it is the only way to tell a cache that worked
    /// from one that was silently thrown away -- and those look identical in a wall-clock figure.
    reused: phrost_index::Reused,
    /// Kept rather than asked for again: walking a 30 000-file checkout twice would make the
    /// discovery figure this reports a measurement of something no run of the server does.
    discovered: Vec<(PathBuf, SourceOrigin)>,
    timings: Timings,
}

/// How long each half of indexing took.
///
/// Reported by the `index` command, which exists to show the shape of a real checkout. They are
/// recorded here because this is where the work happens, and a caller timing it from outside would
/// be timing the setup as well.
#[derive(Clone, Copy)]
pub(crate) struct Timings {
    /// Reading the project's layout and asking the framework provider about it -- the autoload
    /// map, the docroot survey -- before a single file is indexed.
    pub(crate) setup: Duration,
    /// Adopting a stored index, on a warm start. Zero on a cold one.
    pub(crate) cache: Duration,
    pub(crate) discovery: Duration,
    pub(crate) indexing: Duration,
    /// Bytes the allocator reported giving back to the operating system once indexing was done.
    pub(crate) released: usize,
}

/// One file being visited: everything a check needs to name a position in it.
///
/// Two lifetimes because a view is invariant in the index it borrows, and the visit borrows the
/// view for shorter than that -- the length of one closure call.
pub(crate) struct Visit<'v, 'a> {
    pub(crate) view: &'v IndexView<'a>,
    pub(crate) lines: &'v LineIndex,
    /// Path as a person reads it, relative to the project root.
    pub(crate) display: &'v str,
    /// The file's text, for the checks that read it rather than the index.
    pub(crate) text: &'v str,
    /// Framework ids, which decide which lint rules are right for this project.
    pub(crate) frameworks: &'v [&'static str],
    /// The oldest PHP the project says it runs on, which decides which version-gated lint rules
    /// speak -- the same floor the editor lints with, so the two report a file the same way.
    pub(crate) php_floor: Option<phrost_model::PhpVersion>,
    /// Which rules the project switched off or on by name, whatever the categories are.
    pub(crate) named: (&'v [String], &'v [String]),
}

impl Sweep {
    /// Indexes a project the way the server does, stubs and framework rules included.
    ///
    /// Skipping either is how a check-style command reports the whole standard library as undeclared
    /// and every call to `strlen` as a phantom error.
    /// The same, from whichever start is being measured.
    ///
    /// The warm one is not an optimisation here, it is the *subject*: a server that has run before
    /// reuses a stored row for every file whose bytes have not changed, so what it answers is
    /// whatever that row said -- and a tool that always indexes from scratch cannot see a stored row
    /// disagreeing with a fresh one. That is a whole class of "it was wrong until I typed something"
    /// and nothing was looking for it.
    pub(crate) fn indexed_by(root: PathBuf, start: Start) -> Result<Self, String> {
        Self::indexed_watching(root, start, &mut |_, _| {})
    }

    /// The same, telling `watch` how many of how many files have been taken in.
    ///
    /// A callback rather than a flag, because what to do with the number is the command's business:
    /// `check` draws a line for a person and `index` is being timed and must not.
    pub(crate) fn indexed_watching(
        root: PathBuf,
        start: Start,
        watch: &mut dyn FnMut(usize, usize),
    ) -> Result<Self, String> {
        let stubs =
            StubSet::install().map_err(|error| format!("could not install PHP stubs: {error}"))?;
        let mut workspace = Workspace::new(stubs);
        let setup_started = Instant::now();
        let root = workspace.set_root(root).to_path_buf();
        let setup = setup_started.elapsed();
        let cache_started = Instant::now();
        if start == Start::Warm {
            workspace.load_cache();
        }
        let cache = cache_started.elapsed();

        let discovery_started = Instant::now();
        let found = workspace.discover();
        let discovered = found.php.clone();
        let discovery = discovery_started.elapsed();

        let mut reused = phrost_index::Reused::default();
        let index_started = Instant::now();
        let total = discovered.len();
        if start == Start::Warm {
            workspace.forget_undiscovered(&discovered);
        }
        // In batches, because a batch is the unit of parallelism -- see `ProjectSkeleton::
        // ingest_batch`. The progress callback then advances a batch at a time, which is what a
        // person watching a thirty-thousand-file index sees anyway.
        let mut done = 0_usize;
        for batch in discovered.chunks(phrost_index::INGEST_BATCH) {
            reused += workspace.ingest_batch(batch, start == Start::Warm);
            done += batch.len();
            watch(done, total);
        }
        // The server reads these at the end of its own bootstrap, and a tool that
        // skips them is measuring a configuration the server never runs in -- a class
        // wired only in YAML would read as referenced by nothing.
        workspace.install_config_sites(&found.configuration);
        workspace.install_template_words(&found.templates);
        workspace.mark_ready();
        let indexing = index_started.elapsed();
        // As the server does after its bootstrap, so a memory figure read after this describes the
        // server and not the allocator's cache of what bootstrap freed.
        let released = lspkit_heap::release_freed();
        let indexed = workspace.indexed_file_count();

        Ok(Self {
            workspace,
            root,
            indexed,
            reused,
            discovered,
            timings: Timings {
                setup,
                cache,
                discovery,
                indexing,
                released,
            },
        })
    }

    /// The project the server would see, for a command that reports on it rather than asks it
    /// questions.
    pub(crate) fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// The project root as the workspace resolved it, which every path this sweep holds is under.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn discovered(&self) -> &[(PathBuf, SourceOrigin)] {
        &self.discovered
    }

    pub(crate) fn timings(&self) -> Timings {
        self.timings
    }

    pub(crate) fn indexed(&self) -> usize {
        self.indexed
    }

    /// How many of those files a stored index answered for. Always zero from a cold start.
    pub(crate) fn reused(&self) -> phrost_index::Reused {
        self.reused
    }

    /// Why a configuration source was ignored, if one was.
    ///
    /// Reported by every command that sweeps, for the reason the server reports it: a rule that stays
    /// on because a setting was misspelled looks like a rule that cannot be switched off. Only
    /// `phrost.toml` can ever appear here -- a command line has no editor to send
    /// `initializationOptions` -- but it is the same list, from the same resolution, so a source that
    /// grows one is reported without this having to learn about it.
    pub(crate) fn config_errors(&self) -> &[String] {
        self.workspace.config_errors()
    }

    /// The files a sweep covers, in path order so two runs report the same way: everything the
    /// project owns, plus any dependency whose path contains `include`.
    ///
    /// The second half is for measuring the checker rather than the project. A framework's own
    /// source is code that demonstrably runs, so a strict check that lights it up is describing
    /// itself and not the code -- and that number cannot be got from user files, where a finding may
    /// well be a real bug.
    pub(crate) fn files(&self, include: Option<&str>) -> Vec<PathBuf> {
        let mut files = self
            .discovered
            .iter()
            .filter(|(path, origin)| match origin {
                SourceOrigin::User => true,
                _ => include.is_some_and(|wanted| path.to_string_lossy().contains(wanted)),
            })
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    /// Opens each file, hands it to `visit`, and closes it again.
    pub(crate) fn each(&mut self, files: &[PathBuf], mut visit: impl FnMut(Visit<'_, '_>)) {
        for path in files {
            let Ok(source) = SourceText::read(path) else {
                continue;
            };
            let text = source.shared_text();
            let lines = LineIndex::new(&text);
            let held = std::sync::Arc::clone(&text);
            self.workspace
                .set_document(path, text, DocumentVersion::new(0));

            if let Some(file) = self.workspace.file_id(path) {
                let display = display(&self.root, path);
                let view = self.workspace.view(file);
                visit(Visit {
                    view: &view,
                    lines: &lines,
                    display: &display,
                    text: &held,
                    frameworks: self.workspace.frameworks(),
                    php_floor: self.workspace.php_floor(),
                    named: self.workspace.named_lint_rules(),
                });
            }

            self.workspace.close_document(path);
        }
    }

    /// Visits every file on `jobs` threads and hands the results back in the order of `files`.
    ///
    /// What the server's project pass does, and for the same reason it can: a file is read from disk
    /// into a scratch open set under a shared borrow of the workspace, so nothing is opened or closed
    /// and the threads share the index read-only. The measurement that justified it there holds
    /// here -- 1 711 files in 1.15 s on one thread and 0.32 s on four -- and the *findings* are the
    /// same by construction, because `analysed_from` is the one path the server publishes a closed
    /// file's diagnostics through.
    ///
    /// In path order on the way out whatever order the threads finished in, because two runs of a
    /// checker have to print the same lines in the same order: a diff of two check outputs is how a
    /// change to a check is read. A file that cannot be read is skipped, as [`Self::each`] skips it.
    /// `progress` is told how many files are done each time one is.
    /// Records what every file reaches, and says the record is complete.
    ///
    /// The gather half of the two passes `unused-declaration` needs, and the reason this exists as
    /// its own method rather than as a flag: the exact rule is about *this class's* member,
    /// resolved, and it can only answer once every file has had its say. Until then the check
    /// falls back to a coarse rule -- is this name written anywhere -- and a checker that never
    /// gathers answers that coarse rule forever.
    ///
    /// Which is what `check` did, while the server gathered and the editor answered the exact one.
    /// So CI and the editor reported different numbers for the same project, and §8.2's rule about
    /// an instrument that lies applies to the checker as much as to anything it measures.
    ///
    /// Costs one extra sweep. Measured on a real checkout it is the cheaper of the two -- nothing
    /// is rendered and no check runs -- and `phrost unused` has always paid it.
    pub(crate) fn record_what_each_file_reaches(
        &self,
        files: &[PathBuf],
        jobs: usize,
        progress: &mut dyn FnMut(usize),
    ) {
        let found = self.each_in_parallel(files, jobs, |visit| visit.view.reaches(), progress);
        for (path, reaches) in files.iter().zip(found) {
            if let Some(file) = self.workspace.file_id(path) {
                self.workspace.record_reached(file, reaches);
            }
        }
        self.workspace.mark_reached_complete();
    }

    pub(crate) fn each_in_parallel<T: Send>(
        &self,
        files: &[PathBuf],
        jobs: usize,
        visit: impl Fn(Visit<'_, '_>) -> T + Sync,
        progress: &mut dyn FnMut(usize),
    ) -> Vec<T> {
        let workspace = &self.workspace;
        let root = &self.root;
        let next = AtomicUsize::new(0);
        let (send, receive) = std::sync::mpsc::channel::<(usize, T)>();
        let mut results: Vec<Option<T>> =
            std::iter::repeat_with(|| None).take(files.len()).collect();

        std::thread::scope(|scope| {
            for _ in 0..jobs.clamp(1, files.len().max(1)) {
                let send = send.clone();
                let (visit, next) = (&visit, &next);
                // With a stack, not the default. A scoped thread gets neither the size this
                // process gave `main` nor the one it gave the tokio workers, so these were the
                // 2 MiB ones -- measured aborting `phrost check` on a 4 000-deep method chain, a
                // 20 000-term `+` and a 20 000-term `.`, every one of which the server answers.
                // A checker that dies on a legal file reports a clean bill it has not earned.
                deep_thread()
                    .spawn_scoped(scope, move || {
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(path) = files.get(index) else {
                                break;
                            };
                            let Some(file) = workspace.file_id(path) else {
                                continue;
                            };
                            let Ok(source) = SourceText::read(path) else {
                                continue;
                            };
                            let text = source.shared_text();
                            let lines = LineIndex::new(&text);
                            let display = display(root, path);
                            let result = workspace.analysed_from(
                                file,
                                std::sync::Arc::clone(&text),
                                |view| {
                                    visit(Visit {
                                        view,
                                        lines: &lines,
                                        display: &display,
                                        text: &text,
                                        frameworks: workspace.frameworks(),
                                        php_floor: workspace.php_floor(),
                                        named: workspace.named_lint_rules(),
                                    })
                                },
                            );
                            // A closed channel means the collector is gone, which only happens if it
                            // panicked. Stop rather than spin.
                            if send.send((index, result)).is_err() {
                                break;
                            }
                        }
                    })
                    .expect("the operating system can start a sweep thread");
            }
            drop(send);

            let mut done = 0_usize;
            for (index, result) in receive {
                results[index] = Some(result);
                done += 1;
                progress(done);
            }
        });

        results.into_iter().flatten().collect()
    }
}

/// How many threads a sweep uses when nobody says: every core, and no more than eight.
///
/// Eight because the server's project pass measured eight buying 0.1 s over four for two more
/// megabytes, and past that the files are small enough that the work is the coordination.
pub(crate) fn default_jobs() -> usize {
    std::thread::available_parallelism().map_or(4, |cores| cores.get().min(8))
}

impl Visit<'_, '_> {
    /// `path:line:col`, one-based, so a finding can be opened where it is.
    ///
    /// The column in characters, which is what the editor following this format counts and what
    /// the server reports for the same finding -- see
    /// [`character_col`](phrost_model::LineIndex::character_col).
    pub(crate) fn position(&self, offset: phrost_model::TextSize) -> String {
        let at = self.lines.line_col(offset);
        format!(
            "{}:{}:{}",
            self.display,
            at.line + 1,
            self.lines.character_col(self.text, offset) + 1
        )
    }

    /// The checks the project asked for beyond the ones that state a fact.
    ///
    /// The same list the syntax rules read: a person naming a rule in `phrost.toml` does not care
    /// which engine implements it, and a checker that read a different list from the editor would
    /// report what the editor does not.
    pub(crate) fn asked_for(&self) -> phrost_analysis::AskedFor<'_> {
        phrost_analysis::AskedFor {
            codes: self.named.1,
            off: self.named.0,
        }
    }

    /// What the syntax-level rules find, with the integrations this project's frameworks ask for.
    ///
    /// The categories come from the command line and the named rules from the project, which is the
    /// point: a checker that ignored `phrost.toml` would report what the editor does not, and a rule
    /// switched off would look like a rule that does not work.
    pub(crate) fn lint(
        &self,
        categories: &[phrost_syntax_features::lint::LintCategory],
    ) -> Vec<phrost_syntax_features::lint::LintIssue> {
        phrost_syntax_features::lint::lint(
            self.text,
            self.display,
            self.frameworks,
            self.php_floor,
            phrost_syntax_features::lint::LintSelection {
                categories,
                off: self.named.0,
                on: self.named.1,
            },
        )
    }
}

fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// A thread builder with the stack a PHP file nobody wrote by hand needs.
///
/// Named rather than inlined so the two sites that need it cannot drift, and so the reason is in
/// one place -- see [`phrost_model::DEEP_RECURSION_STACK`].
fn deep_thread() -> std::thread::Builder {
    std::thread::Builder::new()
        .name("phrost-sweep".to_owned())
        .stack_size(phrost_model::DEEP_RECURSION_STACK)
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use super::{Start, Sweep};

    /// Two classes declaring one method name, and only one of them called.
    ///
    /// The whole difference between the two reachability rules. The coarse one asks whether the
    /// *word* `render` is written anywhere and spares both; the exact one asks about a resolved
    /// member of a named class and spares one. Until this method existed `check` never gathered, so
    /// it answered the coarse rule for the life of the process while the server answered the exact
    /// one -- and CI and the editor reported different numbers for the same project.
    const TWO_CLASSES_ONE_NAME: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Card.php
<?php
namespace App;

class Card
{
    public function render(): string { return 'card'; }
}
//- /src/Panel.php
<?php
namespace App;

class Panel
{
    public function render(): string { return 'panel'; }
}
//- /src/Page.php
<?php
namespace App;

class Page
{
    public function show(Card $card): string
    {
        return $card->render();
    }
}
"#;

    #[test]
    fn gathering_first_is_what_makes_the_checker_answer_the_exact_rule() {
        let fixture = MaterializedFixture::parse(TWO_CLASSES_ONE_NAME);
        let sweep =
            Sweep::indexed_watching(fixture.root().to_path_buf(), Start::Cold, &mut |_, _| {})
                .expect("the fixture indexes");
        let files = sweep.files(None);

        // Before gathering, the record holds nothing at all -- which is why the rule is gated on
        // completeness rather than on absence: read as "dead" it would report the whole project.
        assert!(
            !sweep.workspace().member_is_reached("App\\Card", "render"),
            "nothing is recorded before the gather pass runs"
        );

        sweep.record_what_each_file_reaches(&files, 1, &mut |_| {});

        assert!(
            sweep.workspace().member_is_reached("App\\Card", "render"),
            "the call resolves to Card::render"
        );
        assert!(
            !sweep.workspace().member_is_reached("App\\Panel", "render"),
            "nothing calls Panel::render -- a word test would have spared it"
        );
    }
}
