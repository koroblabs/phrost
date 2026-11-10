//! Measuring what the server costs, on a real project.
//!
//! This is the enforcement half of the one rule the project is judged by. A budget nobody measures
//! is a wish, and "it compiles" has never been the same claim as "it is fast".
//!
//! Deliberately built on the server's own [`Workspace`], not on a re-implementation of its setup.
//! A measurement harness that assembles the world its own way measures its own way of assembling
//! the world -- and the previous attempt learned that lesson twice, once when a `check` command
//! skipped the standard library and reported a day's worth of phantom errors.

mod budget;
mod check_costs;
mod sample;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use phrost_analysis::{
    CallSubject, call_hierarchy_head, callable_declaration, completions, describe_completion,
    diagnostics_asked_for, goto_definition, hover, incoming_calls, inlay_hints, outgoing_calls,
    reference_target, references_in, signature_help, subject_of,
};
use phrost_lsp::Workspace;
use phrost_model::{FileId, LineIndex, TextSize};
use phrost_stubs::StubSet;
use phrost_syntax_features::{folding, tokens};
use phrost_vfs::{DocumentVersion, SourceText};

use lspkit_perf::{Judged, Samples, Verdict, ms};

use self::budget::BUDGETS;

/// One opened sample file, and what positions inside it are worth asking about.
struct Opened {
    path: PathBuf,
    file: FileId,
    lines: LineIndex,
    /// The same allocation the workspace holds for this open buffer, for turning an offset into the
    /// column a person reads. A second `Arc` to it and not a second copy of it, so nothing measured
    /// here is measuring this tool.
    text: Arc<str>,
    /// Length in bytes, for asking a whole-file question about it.
    length: TextSize,
}

/// Indexes a project, then times the requests an editor makes against it.
///
/// Returns an error when a budget is exceeded, so this is usable as a gate and not only as a
/// report.
pub(crate) fn run(
    root: Option<PathBuf>,
    file_count: usize,
    sites_per_file: usize,
    hold_seconds: u64,
    hold_on: &str,
) -> Result<(), String> {
    let root = match root {
        Some(root) => root,
        None => {
            std::env::current_dir().map_err(|error| format!("no working directory: {error}"))?
        }
    };

    let stubs_started = Instant::now();
    let stubs =
        StubSet::install().map_err(|error| format!("could not install PHP stubs: {error}"))?;
    let stubs_time = stubs_started.elapsed();
    let stub_count = stubs.len();

    let mut workspace = Workspace::new(stubs);
    let configuration_started = Instant::now();
    let root = workspace.set_root(root).to_path_buf();
    let configuration_time = configuration_started.elapsed();

    let discovery_started = Instant::now();
    let discovered = workspace.discover().php;
    let discovery_time = discovery_started.elapsed();
    let user_files = discovered
        .iter()
        .filter(|(_, origin)| origin.wants_full_analysis())
        .count();

    let indexing_started = Instant::now();
    for batch in discovered.chunks(phrost_index::INGEST_BATCH) {
        workspace.ingest_batch(batch, false);
    }
    workspace.mark_ready();
    let indexing_time = indexing_started.elapsed();
    let indexed = workspace.indexed_file_count();

    // As the server does at the end of its bootstrap, so the figure describes the server and not
    // the allocator's cache of what bootstrap freed. Read before the release, the footprint on the
    // reference checkout stood at 181 MB without lspkit-heap's `override` and 177-179 with it, the
    // whole difference hidden; read after it, 170 against 151-156.
    lspkit_heap::release_freed();
    let rss_after_bootstrap = lspkit_perf::rss::current();

    println!("root                {}", root.display());
    println!("framework           {}", workspace.framework_summary());
    println!("php stubs           {stub_count} files installed in {stubs_time:.2?}");
    println!("configuration       read in {configuration_time:.2?}");
    println!(
        "discovery           {} files ({user_files} user, {} dependency) in {discovery_time:.2?}",
        discovered.len(),
        discovered.len() - user_files
    );
    println!(
        "indexing            {indexed} files in {indexing_time:.2?} ({} per file)",
        per_file(indexing_time, indexed)
    );
    if let Some(bytes) = rss_after_bootstrap {
        println!("rss after bootstrap {}", lspkit_perf::rss::format(bytes));
    }

    // --- opening the sample ---

    let sample = sample::largest_user_files(&discovered, file_count);
    println!();
    println!(
        "sample              {} largest user files of {}",
        sample.files.len(),
        sample.considered
    );

    let mut opened = Vec::new();
    let mut open_samples = Samples::new("open document");
    for path in &sample.files {
        let Ok(source) = SourceText::read(path) else {
            continue;
        };
        let text = source.shared_text();
        let lines = LineIndex::new(&text);
        let length = TextSize::try_from(text.len()).unwrap_or_default();

        let started = Instant::now();
        workspace.set_document(path, Arc::clone(&text), DocumentVersion::new(0));
        let elapsed = started.elapsed();

        let Some(file) = workspace.file_id(path) else {
            continue;
        };
        open_samples.record(elapsed, true, &display(&root, path));
        opened.push(Opened {
            path: path.clone(),
            file,
            lines,
            text,
            length,
        });
    }

    let rss_with_open = lspkit_perf::rss::current();
    println!(
        "opened              {} files, p95 {}, worst {}",
        opened.len(),
        open_samples
            .percentile(95)
            .map_or_else(|| "-".to_owned(), ms),
        open_samples.worst().map_or_else(
            || "-".to_owned(),
            |worst| format!("{} at {}", ms(worst.duration), worst.location)
        )
    );
    if let (Some(after), Some(before)) = (rss_with_open, rss_after_bootstrap) {
        println!(
            "rss with {} open    {} (+{})",
            opened.len(),
            lspkit_perf::rss::format(after),
            lspkit_perf::rss::format(after.saturating_sub(before))
        );
    }

    // --- the sweep ---

    let mut goto_samples = Samples::new("goto definition");
    let mut hover_samples = Samples::new("hover");
    let mut highlight_samples = Samples::new("document highlight");
    let mut completion_samples = Samples::new("completion");
    let mut diagnostic_samples = Samples::new("diagnostics");
    let mut inlay_samples = Samples::new("inlay hints");
    let mut signature_samples = Samples::new("signature help");
    let mut commit_samples = Samples::new("completion + commit");
    let mut name_samples = Samples::new("completion of a name");
    let mut half_name_samples = Samples::new("completion of half a name");
    let mut folding_samples = Samples::new("folding ranges");
    let mut token_samples = Samples::new("semantic tokens");
    let mut outline_samples = Samples::new("document symbols");
    let mut action_samples = Samples::new("code actions");
    let mut cursor_action_samples = Samples::new("code actions at a cursor");
    let mut head_samples = Samples::new("call hierarchy head");
    let mut outgoing_samples = Samples::new("outgoing calls");
    let mut capped = 0_usize;
    let mut slowest_diagnostics: Option<(std::time::Duration, &Opened)> = None;
    // Where completion was slowest, for `--hold-on completion` to loop on.
    let mut slowest_completion: Option<(
        std::time::Duration,
        &Opened,
        phrost_model::TextSize,
        String,
    )> = None;
    // What the project-wide search is measured against, gathered while the cheap rows run and swept
    // for afterwards. Held rather than swept in place because each one reads every file the project
    // owns, and doing that inside the per-site loop would measure the sweep thousands of times.
    let mut callers_wanted: Vec<(CallSubject, String)> = Vec::new();
    // Names to type into a symbol picker, taken from what the sample itself declares. A query
    // nobody would write measures nothing; these are names somebody is plausibly looking for.
    let mut searches_wanted: Vec<String> = Vec::new();

    for entry in &opened {
        // Once per file, not per site: diagnostics are a whole-buffer answer, and that is the shape
        // the budget is written against.
        let view = workspace.view(entry.file);
        let started = Instant::now();
        // With the checks the project switched on, which is what the server runs for it: the
        // default set alone measured 18 ms on a file the editor was spending 34 ms on.
        let found = diagnostics_asked_for(&view, workspace.asked_for());
        let spent = started.elapsed();
        diagnostic_samples.record(spent, true, &display(&root, &entry.path));
        if slowest_diagnostics.is_none_or(|(most, _)| spent > most) {
            slowest_diagnostics = Some((spent, entry));
        }
        drop(found);

        // Once per file, and the only row here that includes a parse: folding reads the buffer
        // instead of the index, so what it costs is what mago costs on this file plus a walk. The
        // text is fetched outside the timer, like the views are for every other row.
        if let Some((text, _)) = workspace.document_text(entry.file) {
            let started = Instant::now();
            let folds = folding::ranges(&text);
            folding_samples.record(
                started.elapsed(),
                !folds.is_empty(),
                &display(&root, &entry.path),
            );

            // The same shape and the same parse, measured separately because it produces two orders
            // of magnitude more and sorting that many is its own cost.
            let started = Instant::now();
            let names = tokens::tokens(&text);
            token_samples.record(
                started.elapsed(),
                !names.is_empty(),
                &display(&root, &entry.path),
            );
        }

        // Once per file, because that is the whole of the request: an editor asks for the outline
        // on every open and again after every edit that settles, and draws the breadcrumb from it.
        // Read from the skeleton rather than the buffer, so this is the cost of a projection and
        // not of a parse.
        let started = Instant::now();
        let outline = phrost_analysis::document_symbols(&workspace.view(entry.file));
        outline_samples.record(
            started.elapsed(),
            !outline.is_empty(),
            &display(&root, &entry.path),
        );

        // A class and one of its members from each sample file, for the project-wide search below.
        // Both kinds, because they are found through two different halves of the index and a row
        // that only ever queried one of them would leave the other unmeasured.
        for declared in &outline {
            if searches_wanted.len() >= SEARCH_NAMES {
                break;
            }
            searches_wanted.push(declared.label.clone());
            if let Some(member) = declared.children.first() {
                searches_wanted.push(member.label.clone());
            }
        }

        // Also once per file, and over the whole of it: a client asks per viewport, so this is the
        // worst case rather than the usual one.
        let view = workspace.view(entry.file);
        let whole = phrost_model::TextRange::new(TextSize::from(0), entry.length);

        // The whole file as the selection, which is every offer the file has at once. A client sends
        // a cursor or a line, so this is several hundred times the real ask -- kept as the ceiling,
        // beside the cursor row below which is what an editor actually costs.
        let started = Instant::now();
        // No `only`, which is the worst case on purpose: the sweep should measure every offer
        // running, not what a client that filtered them down would pay.
        let request = phrost_analysis::ActionRequest {
            view: &view,
            at: whole,
            only: &[],
            asked: workspace.asked_for(),
        };
        let offered = phrost_analysis::code_actions(&request);
        action_samples.record(
            started.elapsed(),
            !offered.is_empty(),
            &display(&root, &entry.path),
        );

        let started = Instant::now();
        let hints = inlay_hints(&view, whole);
        inlay_samples.record(
            started.elapsed(),
            !hints.is_empty(),
            &display(&root, &entry.path),
        );

        // Name completion is a different path from member completion and costs differently: it
        // ranks hundreds of candidates from the whole project and works out how each would have to
        // be written. Sampled from the names the file writes, at the end of each -- where a cursor
        // is when the list is being filtered by what has been typed.
        let mut names = workspace
            .view(entry.file)
            .name_references()
            .into_iter()
            .map(|reference| reference.range)
            .collect::<Vec<_>>();
        if names.len() > sites_per_file {
            capped += names.len() - sites_per_file;
            names.truncate(sites_per_file);
        }
        for name in names {
            let location = position(&root, entry, name.start());
            let view = workspace.view(entry.file);
            let started = Instant::now();
            let list = completions(&view, name.end());
            name_samples.record(started.elapsed(), !list.items.is_empty(), &location);

            // And with half of it typed, which is the other end of the same interaction and the
            // dear one -- see the budget. The last segment's half, since that is the word being
            // written; the text after the cursor is not read.
            let written = &entry.text[usize::from(name.start())..usize::from(name.end())];
            let last = written.rfind('\\').map_or(0, |separator| separator + 1);
            let mut cut = last + ((written.len() - last) / 2).max(1);
            while !written.is_char_boundary(cut) {
                cut -= 1;
            }
            let half = name.start() + TextSize::try_from(cut).unwrap_or_default();
            let started = Instant::now();
            let list = completions(&view, half);
            half_name_samples.record(started.elapsed(), !list.items.is_empty(), &location);
        }

        // Once per method the file declares, over the whole of its body: what a reader expanding a
        // node in the tree waits for. Enumerated from the outline rather than from the accesses,
        // because the question is about declarations and there is one of these per method rather
        // than one per call.
        let mut declared = workspace
            .view(entry.file)
            .outline()
            .into_iter()
            .flat_map(|node| node.children)
            .filter(|member| matches!(member.kind, phrost_model::SymbolKind::Method))
            .map(|member| member.name_range)
            .collect::<Vec<_>>();
        declared.truncate(sites_per_file);
        // At most one per file: the cost of the search is dominated by how many files hold the
        // name, and taking several from one file would measure one file's naming habits.
        let mut searched_here: Option<(CallSubject, String)> = None;
        for at in declared {
            let location = position(&root, entry, at.start());
            let view = workspace.view(entry.file);
            let Some(head) = call_hierarchy_head(&view, at.start()) else {
                continue;
            };
            // Through the key, as the protocol does: a follow-up gets a string, not the target.
            let subject = CallSubject::of_key(&subject_of(&head).key());

            let view = workspace.view(entry.file);
            let started = Instant::now();
            let called = outgoing_calls(&view, &subject);
            outgoing_samples.record(started.elapsed(), !called.is_empty(), &location);

            // A constructor in preference to a method, because the two are searched for
            // differently: a constructor's candidate files are the ones naming *any* class that
            // inherits it, which is the wider list of the two and the only one a flat row of
            // methods would never measure.
            let constructs = subject.key().ends_with("::__construct");
            if !called.is_empty() && (searched_here.is_none() || constructs) {
                searched_here = Some((subject, location));
            }
        }
        if let Some(found) = searched_here
            && callers_wanted.len() < CALLER_SEARCHES
        {
            callers_wanted.push(found);
        }

        let mut sites = workspace
            .view(entry.file)
            .member_accesses()
            .iter()
            .map(|access| access.range)
            .collect::<Vec<_>>();
        if sites.len() > sites_per_file {
            capped += sites.len() - sites_per_file;
            sites.truncate(sites_per_file);
        }

        for site in sites {
            let location = position(&root, entry, site.start());

            // A fresh view per request, because that is what the server builds. Sharing one across
            // the three would leave two of them reading a memo the editor never warms, and report
            // them as several times faster than the thing being budgeted.
            //
            // No warm-up either. The first request against a freshly opened file is a cost a person
            // pays, so hiding it would measure a state the editor never sees.
            let view = workspace.view(entry.file);
            let started = Instant::now();
            let targets = goto_definition(&view, site.start());
            goto_samples.record(started.elapsed(), !targets.is_empty(), &location);

            let view = workspace.view(entry.file);
            let started = Instant::now();
            let info = hover(&view, site.start());
            hover_samples.record(started.elapsed(), info.is_some(), &location);

            // What an editor asks on every cursor move: the target, then its uses in this file --
            // the two calls the protocol handler makes. Absent from the sweep until a change put a
            // whole-file walk in front of every target and nothing here could see it.
            let view = workspace.view(entry.file);
            let started = Instant::now();
            let highlighted = reference_target(&view, site.start())
                .map(|target| references_in(&view, &target))
                .unwrap_or_default();
            highlight_samples.record(started.elapsed(), !highlighted.is_empty(), &location);

            // What an editor sends: an empty selection at the cursor. The whole-file row above is
            // the ceiling and cannot see an applicability gate at all -- a range that covers the
            // file matches every subject in it -- so without this row the one thing keeping the
            // menu affordable is unmeasured.
            let view = workspace.view(entry.file);
            let cursor = phrost_model::TextRange::empty(site.start());
            let started = Instant::now();
            let request = phrost_analysis::ActionRequest {
                view: &view,
                at: cursor,
                only: &[],
                asked: workspace.asked_for(),
            };
            let offered = phrost_analysis::code_actions(&request);
            cursor_action_samples.record(started.elapsed(), !offered.is_empty(), &location);

            // Which callable the cursor is on, or the one it is inside. Asked at a call site rather
            // than at a declaration, because that is where a reader invokes it: on the call they
            // are looking at, wanting to know who else makes it.
            let view = workspace.view(entry.file);
            let started = Instant::now();
            let head = call_hierarchy_head(&view, site.start());
            head_samples.record(started.elapsed(), head.is_some(), &location);

            // At the end of the name, which is where a cursor sits when completion fires: the
            // member has been typed and the list is being filtered by it.
            let view = workspace.view(entry.file);
            let started = Instant::now();
            let list = completions(&view, site.end());
            let spent = started.elapsed();
            completion_samples.record(spent, !list.items.is_empty(), &location);
            if slowest_completion
                .as_ref()
                .is_none_or(|(most, ..)| spent > *most)
            {
                slowest_completion = Some((spent, entry, site.end(), location.clone()));
            }

            // What a person actually waits through: the list, and then the declaration of the row
            // they land on, which arrives as a second request. Timed together and from the same
            // start, because the two are one interaction and a budget per half would let the pair
            // exceed what anyone would sit through while both rows read as fine.
            //
            // Answered slightly less often than the list, and the gap is not a failure: a local
            // variable has no declaration to show, so an offer of one describes as nothing.
            let described = list
                .items
                .first()
                .and_then(|item| item.describe.as_ref())
                .and_then(|member| describe_completion(&view, member))
                .is_some();
            commit_samples.record(started.elapsed(), described, &location);

            // Just past the member name, where the `(` of a call goes: signature help fires on the
            // character that opens the argument list, so this is where a person is when it runs.
            let view = workspace.view(entry.file);
            let started = Instant::now();
            let help = signature_help(&view, site.end());
            signature_samples.record(started.elapsed(), help.is_some(), &location);
        }
    }

    if capped > 0 {
        println!("sites capped        {capped} accesses skipped at {sites_per_file} per file");
    }

    // The symbol picker, typed one character at a time. Run outside the per-file loop because it is
    // a project-wide question that has nothing to do with which file is open -- and swept over
    // growing prefixes of each name because that is the shape a client sends: the one- and
    // two-character queries are where the work is, and are the ones a p95 has to include.
    let mut search_samples = Samples::new("workspace symbols");
    for name in &searches_wanted {
        let mut prefix = String::new();
        for character in name.chars().take(SEARCH_PREFIX_LENGTH) {
            prefix.push(character);
            let started = Instant::now();
            let found = phrost_analysis::workspace_symbols(&workspace.project_view(), &prefix);
            search_samples.record(
                started.elapsed(),
                !found.is_empty(),
                &format!("query {prefix}"),
            );
        }
        // And the whole name, which is where the person stops typing and expects one row.
        if name.chars().count() > SEARCH_PREFIX_LENGTH {
            let started = Instant::now();
            let found = phrost_analysis::workspace_symbols(&workspace.project_view(), name);
            search_samples.record(
                started.elapsed(),
                !found.is_empty(),
                &format!("query {name}"),
            );
        }
    }

    // The project-wide row, run last and a handful of times: every one of these greps every file
    // the project owns and then analyses the ones that matched. Deliberately the whole sweep the
    // server runs, lock discipline aside -- a number measured over one file would describe nothing.
    let mut incoming_samples = Samples::new("incoming calls");
    for (subject, location) in &callers_wanted {
        let started = Instant::now();
        let mut found = 0_usize;
        let mut visited = 0_usize;
        let query = subject.sweep_query(&workspace.project_view());
        for path in workspace.reference_candidates(&query) {
            let Some(file) = workspace.file_id(&path) else {
                continue;
            };
            visited += 1;
            found += workspace
                .analysed(file, |view| incoming_calls(view, subject).len())
                .unwrap_or(0);
        }
        incoming_samples.record(started.elapsed(), found > 0, location);
        println!(
            "callers of {:<24} {found} in {visited} candidate files",
            subject.key()
        );
    }
    // Where a follow-up lands, which is the half of the feature the rows above cannot see: an item
    // is resolved back from its key before either direction is answered.
    let unresolved = callers_wanted
        .iter()
        .filter(|(subject, _)| callable_declaration(&workspace.project_view(), subject).is_none())
        .count();
    if unresolved > 0 {
        println!(
            "items not resolvable {unresolved} of {}",
            callers_wanted.len()
        );
    }

    if let Some((_, entry)) = slowest_diagnostics {
        check_costs::print(&workspace, entry.file, &display(&root, &entry.path));
        if hold_on != "completion" {
            check_costs::hold(
                &workspace,
                entry.file,
                &display(&root, &entry.path),
                hold_seconds,
                hold_on,
            );
        }
    }
    if hold_on == "completion"
        && let Some((_, entry, at, location)) = &slowest_completion
    {
        check_costs::hold_completion(&workspace, entry.file, *at, location, hold_seconds);
    }

    report(&[
        &goto_samples,
        &hover_samples,
        &highlight_samples,
        &completion_samples,
        &commit_samples,
        &diagnostic_samples,
        &inlay_samples,
        &signature_samples,
        &name_samples,
        &half_name_samples,
        &folding_samples,
        &token_samples,
        &outline_samples,
        &action_samples,
        &cursor_action_samples,
        &head_samples,
        &outgoing_samples,
        &search_samples,
        &incoming_samples,
    ])
}

/// How many declared names the symbol-picker row is swept over.
///
/// A couple of dozen: enough that a p95 is not one name's naming habits, few enough that the row
/// does not dominate a sweep whose point is the per-keystroke ones.
const SEARCH_NAMES: usize = 24;

/// How far into each name the prefix sweep goes.
///
/// The first characters are the expensive ones -- a single letter matches thousands of names on a
/// real project -- and they are also the ones a client really sends, one per keystroke, before the
/// query is narrow enough to match anything.
const SEARCH_PREFIX_LENGTH: usize = 6;

/// How many project-wide caller searches the sweep runs.
///
/// A handful, because each reads every file the project owns: enough for a p95 to mean something
/// and few enough that measuring the feature does not dominate measuring everything else.
const CALLER_SEARCHES: usize = 5;

/// Prints the budget table and decides the verdict.
fn report(measured: &[&Samples]) -> Result<(), String> {
    println!();
    println!(
        "{:<20}{:>7}{:>10}{:>10}{:>10}{:>10}{:>10}  verdict",
        "request", "n", "p50", "p95", "max", "answered", "budget"
    );

    let mut verdict = Verdict::new();
    for budget in BUDGETS {
        let samples = measured
            .iter()
            .find(|samples| samples.what() == budget.what);
        let p95 = samples.and_then(|samples| samples.percentile(95));
        let judged = verdict.judge(budget, p95.map(|p95| p95.as_secs_f64() * 1000.0));

        let Some(samples) = samples.filter(|_| judged != Judged::NotMeasured) else {
            println!(
                "{:<20}{:>7}{:>10}{:>10}{:>10}{:>10}{:>10}  {}",
                budget.what,
                "-",
                "-",
                "-",
                "-",
                "-",
                budget.limit_text(),
                judged.word()
            );
            continue;
        };

        println!(
            "{:<20}{:>7}{:>10}{:>10}{:>10}{:>9.1}%{:>10}  {}",
            budget.what,
            samples.len(),
            samples.percentile(50).map_or_else(|| "-".to_owned(), ms),
            p95.map_or_else(|| "-".to_owned(), ms),
            samples.max().map_or_else(|| "-".to_owned(), ms),
            samples.answered_percent(),
            budget.limit_text(),
            judged.word()
        );
    }

    println!();
    for samples in measured {
        if let Some(worst) = samples.worst() {
            println!(
                "worst {:<21}{} at {}",
                samples.what(),
                ms(worst.duration),
                worst.location
            );
        }
    }

    Ok(verdict.into_result()?)
}

fn per_file(total: std::time::Duration, files: usize) -> String {
    if files == 0 {
        return "-".to_owned();
    }
    format!("{:.0?}", total / u32::try_from(files).unwrap_or(u32::MAX))
}

/// A path as a person reads it: relative to the project root when it is under it.
fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// `path:line:col`, one-based and counting characters, so the slow case can be opened where it is.
fn position(root: &Path, entry: &Opened, offset: TextSize) -> String {
    let at = entry.lines.line_col(offset);
    format!(
        "{}:{}:{}",
        display(root, &entry.path),
        at.line + 1,
        entry.lines.character_col(&entry.text, offset) + 1
    )
}
