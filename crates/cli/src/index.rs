use phrost_lsp::Workspace;

use crate::sweep::{Start, Sweep, Timings};

/// Indexes a project and reports what was found.
///
/// The point is to be able to see what the server would see, on a real checkout, without an editor
/// in the way. Numbers that look wrong here are wrong everywhere -- which only holds because it
/// indexes through [`Sweep`], the one place in this binary that knows what a project is. A command
/// that built its own would quietly see a different project: the framework contracts are installed
/// during setup, so a hand-rolled copy answers about a checkout where no service id resolves.
pub(crate) fn run(
    root: Option<std::path::PathBuf>,
    memory: bool,
    warm: bool,
) -> Result<(), String> {
    let root = match root {
        Some(root) => root,
        None => {
            std::env::current_dir().map_err(|error| format!("no working directory: {error}"))?
        }
    };

    let start = match warm {
        true => Start::Warm,
        false => Start::Cold,
    };
    let sweep = Sweep::indexed_by(root, start)?;
    report(
        sweep.workspace(),
        sweep.discovered(),
        sweep.timings(),
        start,
        sweep.reused(),
    );
    if memory {
        report_memory(sweep.workspace(), sweep.timings());
    }

    Ok(())
}

fn report(
    workspace: &Workspace,
    discovered: &[(std::path::PathBuf, phrost_model::SourceOrigin)],
    timings: Timings,
    start: Start,
    reused: phrost_index::Reused,
) {
    let layout = workspace.layout().expect("a root was just set");
    let autoload = workspace.autoload().stats();
    let roots = workspace.autoload().source_roots().len();
    let user = discovered
        .iter()
        .filter(|(_, origin)| origin.wants_full_analysis())
        .count();

    println!("root                {}", layout.root().display());
    println!("vendor              {}", layout.vendor_dir().display());
    println!("framework           {}", workspace.framework_summary());
    println!(
        "autoload            psr-4 {}, psr-0 {}, classmap {}, {roots} source roots",
        autoload.psr4_rules, autoload.psr0_rules, autoload.classmap_entries,
    );
    // The phases of a start that are not indexing, because on a warm start they are most of it:
    // indexing 30 000 unchanged files is under 0.1 s once every row is trusted on its fingerprint,
    // and what is left of the 0.7 s a person waits is the layout, the stored index and the walk.
    println!("setup               {:.2?}", timings.setup);
    if start == Start::Warm {
        println!("stored index        loaded in {:.2?}", timings.cache);
    }
    println!(
        "discovered          {} files ({user} user, {} dependency) in {:.2?}",
        discovered.len(),
        discovered.len() - user,
        timings.discovery,
    );
    println!(
        "indexed             {} files in {:.2?}, {}",
        workspace.indexed_file_count(),
        timings.indexing,
        // Said out loud because the two ways a stored index can fail to help are
        // indistinguishable in a wall-clock figure. A cold start does not read one at all -- which
        // is what this command did unconditionally, so running it twice measured two cold starts
        // and read as "the cache saves nothing". And a warm start whose stored index was rejected
        // reuses zero files and takes exactly as long as a cold one. Only the count separates
        // them, so only the count is worth printing: a warm start reusing everything on this
        // project is 4.6 s against 6.3 s cold.
        match start {
            Start::Cold =>
                "cold start, no stored index read -- pass `--warm` for the other".to_owned(),
            // Both counts, because a cache whose fingerprints never match reads every file and
            // reports everything reused, exactly like one that works.
            Start::Warm => format!(
                "warm start, {} reused from a stored index, {} of them without reading",
                reused.total(),
                reused.unread
            ),
        },
    );
    match workspace.stub_error() {
        Some(error) => println!("php stubs           unavailable: {error}"),
        None => println!("php stubs           {} files", workspace.stub_count()),
    }
    let catalogs = workspace.catalog_sizes();
    if catalogs.is_empty() {
        println!("catalogs            none");
    } else {
        println!(
            "catalogs            {}",
            catalogs
                .iter()
                .map(|(name, count)| format!("{name} {count}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("interned names      {:?}", workspace.interner_stats());
    println!(
        "distinct types      {} for the whole project",
        workspace.type_count()
    );
}

/// Where the index's heap went, largest part first.
///
/// The rule this project is judged by is that resident memory is dominated by the user-code skeleton
/// and a handful of open files, never by whole-vendor analysis -- so what matters here is the
/// *shape*, and specifically that the catalog and the type table stay small while the member rows
/// are allowed to be large.
///
/// Resident memory is printed beside it, because the gap between the two is the number nobody could
/// see. It is real and it is not the index: allocator slack, the stub set, the composer classmap,
/// the runtime and the binary are all in it. On a 30 965-file checkout the index is 88.6 MB and the
/// process holds about 205, and until both were printed together the second was something a person
/// had to remember to measure with `ps` and subtract by hand -- so nobody did, and a hundred and
/// twenty megabytes went unexamined. The gap being *stable* is the useful property, not its being
/// zero.
fn report_memory(workspace: &Workspace, timings: Timings) {
    let found = workspace.memory();
    let total = found.total();

    println!();
    if let Some(resident) = lspkit_perf::rss::current() {
        let outside = resident.saturating_sub(total as u64);
        println!(
            "resident            {}  ({} outside the index)",
            lspkit_perf::rss::format(resident),
            lspkit_perf::rss::format(outside)
        );
    }
    println!("index memory        {}", megabytes(total));
    println!(
        "given back          {} reported released to the OS after indexing",
        megabytes(timings.released)
    );
    // What went through `lspkit_heap::Heap`, the binary's allocator: zeros under another one.
    let heap = lspkit_heap::stats();
    println!(
        "heap                {} small, {} large (peak {}), {} kept for reuse",
        megabytes(heap.small),
        megabytes(heap.large),
        megabytes(heap.large_peak),
        megabytes(heap.cached)
    );
    for (label, bytes) in found.rows() {
        let share = if total == 0 {
            0.0
        } else {
            bytes as f64 * 100.0 / total as f64
        };
        println!("  {label:<16}{:>10}{share:>7.1}%", megabytes(bytes));
    }
}

fn megabytes(bytes: usize) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
