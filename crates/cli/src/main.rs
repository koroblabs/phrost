//! The phrost binary.
//!
//! Two kinds of command, and the distinction matters. `serve` is the product. Everything else is a
//! tool for looking at what the product knows — and those may be as slow and as thorough as they
//! like, because a person asked for them explicitly, which is exactly the licence a request handler
//! does not have.

mod check;
mod coverage;
mod index;
mod perf;
mod progress;
mod query;
mod reprint;
mod sweep;
mod unused;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// See the `mimalloc` feature in `Cargo.toml`, and `lspkit-heap` for who asks it to collect.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Large blocks from the operating system with a bounded cache, small ones to the system
/// allocator: see `lspkit_heap::Heap`.
#[cfg(not(feature = "mimalloc"))]
#[global_allocator]
static GLOBAL: lspkit_heap::Heap = lspkit_heap::Heap;

#[derive(Parser)]
#[command(
    name = "phrost",
    version,
    about = "A fast, resource-cheap PHP language server"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the Language Server Protocol over stdin and stdout.
    Serve,
    /// Index a project and report what was found.
    Index {
        /// Project root. Defaults to the working directory.
        root: Option<PathBuf>,
        /// Break the index's heap down by part, to say where resident memory went.
        #[arg(long)]
        memory: bool,
        /// Index from what a previous run stored, the way a restarted server does.
        ///
        /// Off by default, because a cold start is the figure this command exists to report. Only a
        /// server writes a stored index, so a warm run on a project no server has opened reuses
        /// nothing and is a cold start wearing a different label -- which the report says.
        #[arg(long)]
        warm: bool,
    },
    /// Run every check over a project, the way CI would.
    Check {
        /// Project root. Defaults to the working directory.
        root: Option<PathBuf>,
        /// Check only the first N user files, by path.
        #[arg(long)]
        files: Option<usize>,
        /// Print only the totals.
        #[arg(long)]
        quiet: bool,
        /// Index from what a previous run stored, the way a restarted server does.
        ///
        /// For finding a stored row that disagrees with a fresh one -- the shape of "it was wrong
        /// until I typed something", which a cold sweep cannot see.
        #[arg(long)]
        warm: bool,
        /// Run these syntax-level lint categories instead of the ones the project asked for.
        ///
        /// Comma-separated, or `all`. Categories: correctness, safety, security, deprecation,
        /// best-practices, clarity, consistency, maintainability, redundancy. Left out, the
        /// categories are the ones `phrost.toml` names -- or, where it names none, the same two an
        /// editor runs by default, so that CI and the editor report the same file the same way.
        ///
        /// A list rather than a switch because the category is not a good enough filter on its own:
        /// on one real project `all` produces 6855 findings against 508 from the index, and
        /// `correctness` alone is 1303 of them -- 987 being one rule about `declare(strict_types=1)`
        /// that the project deliberately does not follow. Naming that one rule in `phrost.toml`
        /// takes the category to 316, which is the difference between a list and a wall.
        #[arg(long, value_name = "CATEGORIES")]
        lint: Option<String>,
        /// Compare the counts against `phrost-baseline.toml` beside the project, and fail if any
        /// rose.
        ///
        /// The ratchet `docs/02-testing.md` asks for, and the reason it is not optional there:
        /// keeping `?T where T expected` on is only liveable if the number of findings can fall and
        /// cannot rise. Per code and not by total, because a total hides a trade -- four findings
        /// closed while four others open reads as no change and is not the same project.
        ///
        /// Counts the index's findings and not the syntax rules: those are chosen by `phrost.toml`,
        /// so a project turning a category on would break a ratchet by agreeing with itself.
        #[arg(long)]
        ratchet: bool,

        /// Write what this run found as the agreed counts.
        ///
        /// The same shape as accepting a snapshot: the file changes only when somebody says so, so a
        /// count that falls is reported and left alone until it is agreed to.
        #[arg(long)]
        accept: bool,

        /// Also check dependency files whose path contains this.
        ///
        /// For measuring the checker rather than the project: a framework's own source is code that
        /// demonstrably runs, so a strict check that lights it up is describing itself.
        #[arg(long)]
        include: Option<String>,

        /// How many files to check at once. Defaults to every core, up to eight.
        ///
        /// `--jobs 1` is the instrument: the wall time of a serial sweep over a whole project is the
        /// large, quiet sample a change to a check is decided by, and eight threads sharing one
        /// index would make it a measurement of contention.
        #[arg(long, short = 'j')]
        jobs: Option<usize>,
    },
    /// Report how much of a project's own code resolves, and what the gaps are.
    Coverage {
        /// Project root. Defaults to the working directory.
        root: Option<PathBuf>,
        /// Sweep only the first N user files, by path.
        #[arg(long)]
        files: Option<usize>,
        /// Skip files whose path contains this. For asking how much of a gap is the corpus rather
        /// than the server -- generated code with `object` parameters resolves for nobody.
        #[arg(long)]
        without: Option<String>,
        /// Write every answer the sweep saw -- each access's class, each assignment's type -- to
        /// this file, sorted, for `diff` against the same sweep by another build.
        #[arg(long)]
        dump: Option<PathBuf>,
    },
    /// Measure what the server costs on a real project, and check it against the budgets.
    Perf {
        /// Project root. Defaults to the working directory.
        root: Option<PathBuf>,
        /// How many files to open and sweep.
        #[arg(long, default_value_t = 20)]
        files: usize,
        /// Most member accesses to measure per file, so one huge file cannot dominate.
        #[arg(long, default_value_t = 200)]
        sites: usize,
        /// After the sweep, run diagnostics on the slowest file in a loop for this many seconds.
        ///
        /// For a profiler to attach to: `sample <pid>` during the hold sees that file's checks and
        /// nothing else -- no indexing, no per-site requests.
        #[arg(long, default_value_t = 0)]
        hold: u64,
        /// What `--hold` loops: `diagnostics`, or `actions` -- every code action on the whole file --
        /// on the file whose diagnostics were slowest; or `completion`, at the slowest completion
        /// site the sweep found.
        #[arg(long, default_value = "diagnostics")]
        hold_on: String,
    },
    /// Reprint a tree of files beside itself, for a coding standard's own program to judge.
    ///
    /// Writes `<out>/before` and `<out>/after`. The errors that appear only in the second are the
    /// formatter's; see `tools/drupal-style.sh`, which runs `phpcs` over both and reports the
    /// difference by sniff.
    Reprint {
        /// Directory of user code to walk.
        root: PathBuf,
        /// Where to write the two trees.
        out: PathBuf,
        /// House style: `drupal`, `psr-12`, `pint`, `tempest`, `hack`, or the default.
        ///
        /// Left out, the style is the one the nearest `phrost.toml` above this directory names --
        /// which is the one the server prints with, so the two doors into the formatter agree.
        #[arg(long)]
        style: Option<String>,
    },
    /// Report the class-likes a project declares and nothing reaches.
    Unused {
        /// Project root. Defaults to the working directory.
        root: Option<PathBuf>,
        /// Sweep only the first N user files, by path.
        #[arg(long)]
        files: Option<usize>,
    },
    /// Ask what is at a position, the way an editor would.
    Query {
        /// Project root.
        root: PathBuf,
        /// File to ask about, relative to the root or absolute.
        file: PathBuf,
        /// One-based line, as an editor shows it.
        line: u32,
        /// One-based column, in characters.
        column: u32,
    },
}

/// The stack every thread in this process gets. See the constant it comes from for the
/// measurements, and for why it is one number in one place: two of the three thread-spawning sites
/// in this workspace had the default until it was.
const STACK_SIZE: usize = phrost_model::DEEP_RECURSION_STACK;

/// Runs everything on a thread whose stack this process chose.
///
/// The main thread's stack is fixed by the OS before `main` is entered and cannot be grown, so the
/// work moves to one that can be given a size. `run` builds the runtime, which hands the same size
/// to every worker it spawns.
fn main() -> std::process::ExitCode {
    let phrost = std::thread::Builder::new()
        .name("phrost".to_owned())
        .stack_size(STACK_SIZE)
        .spawn(run)
        .expect("the operating system can start a thread");

    match phrost.join() {
        Ok(code) => code,
        // Re-raised rather than turned into an exit code. A panic that reaches here is one the
        // server path did not contain -- a tool, or startup -- and those are supposed to be fatal
        // and loud. `join` has already let the default hook print it.
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn run() -> std::process::ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(STACK_SIZE)
        .build()
        .expect("the tokio runtime starts");

    runtime.block_on(dispatch())
}

async fn dispatch() -> std::process::ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Serve => return phrost_lsp::serve().await,
        Command::Index { root, memory, warm } => index::run(root, memory, warm),
        Command::Check {
            root,
            files,
            quiet,
            include,
            warm,
            lint,
            ratchet,
            accept,
            jobs,
        } => check::run(check::Options {
            jobs,
            root,
            limit: files,
            quiet,
            include: include.as_deref(),
            warm,
            lint: lint.as_deref(),
            ratchet,
            accept,
        }),
        Command::Coverage {
            root,
            files,
            without,
            dump,
        } => coverage::run(root, files, without.as_deref(), dump.as_deref()),
        Command::Perf {
            root,
            files,
            sites,
            hold,
            hold_on,
        } => perf::run(root, files, sites, hold, &hold_on),
        Command::Unused { root, files } => unused::run(root, files),
        Command::Query {
            root,
            file,
            line,
            column,
        } => query::run(&root, &file, line, column),
        Command::Reprint { root, out, style } => reprint::run(root, out, style),
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("phrost: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
