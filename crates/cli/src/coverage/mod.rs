//! How much of a real project actually resolves.
//!
//! The number that has to exist before the first diagnostic does. A checker built on a layer that
//! silently fails to type a quarter of the member accesses in a project reports a quarter of that
//! project as broken, and the resulting noise is what makes people turn a checker off -- so the
//! honest order is to measure the gap, then close it, then speak.
//!
//! Deliberately separate from `perf`: that one asks what a request costs, this one asks whether the
//! answer was there at all. Both sweep the same sites, and neither number means much alone.

mod dump;
mod tally;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use phrost_lsp::Workspace;
use phrost_model::{LineIndex, SourceOrigin};
use phrost_stubs::StubSet;
use phrost_vfs::{DocumentVersion, SourceText};
use phrost_view::MemberAccessOutcome;

use self::tally::Tally;

/// How many rows of each cause to print.
const TOP: usize = 15;

/// Indexes a project, then asks what every member access in its own code resolves to.
pub(crate) fn run(
    root: Option<PathBuf>,
    limit: Option<usize>,
    without: Option<&str>,
    dump_to: Option<&Path>,
) -> Result<(), String> {
    let mut dump = dump_to.map(|_| dump::Dump::default());
    let root = match root {
        Some(root) => root,
        None => {
            std::env::current_dir().map_err(|error| format!("no working directory: {error}"))?
        }
    };

    let stubs =
        StubSet::install().map_err(|error| format!("could not install PHP stubs: {error}"))?;
    let mut workspace = Workspace::new(stubs);
    let root = workspace.set_root(root).to_path_buf();

    let found = workspace.discover();
    let discovered = found.php.clone();
    let indexing_started = Instant::now();
    for batch in discovered.chunks(phrost_index::INGEST_BATCH) {
        workspace.ingest_batch(batch, false);
    }
    // The server reads these at the end of its own bootstrap, and a tool that
    // skips them is measuring a configuration the server never runs in -- a class
    // wired only in YAML would read as referenced by nothing.
    workspace.install_config_sites(&found.configuration);
    workspace.install_template_words(&found.templates);
    workspace.mark_ready();
    println!(
        "indexed             {} files in {:.2?}",
        workspace.indexed_file_count(),
        indexing_started.elapsed()
    );

    let mut user_files = discovered
        .iter()
        .filter(|(_, origin)| matches!(origin, SourceOrigin::User))
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    user_files.sort();
    if let Some(without) = without {
        user_files.retain(|path| !path.to_string_lossy().contains(without));
    }
    let available = user_files.len();
    if let Some(limit) = limit {
        user_files.truncate(limit);
    }

    let mut totals = Totals::default();
    let mut unknown_classes = Tally::default();
    let mut missing_members = Tally::default();
    let mut unknown_receivers = Tally::default();
    let mut not_objects = Tally::default();
    let mut incomplete = Tally::default();
    let mut dynamic = Tally::default();
    let mut any_objects = Tally::default();
    let mut catalog_open = Tally::default();
    let mut catalog_near = Tally::default();
    let mut untyped_assignments = Tally::default();

    let sweep_started = Instant::now();
    for path in &user_files {
        // Opened one at a time and closed again. Full analysis of a whole project at once is the
        // weight this design exists to avoid, and a tool that ignored that would be measuring a
        // configuration the server never runs in.
        let Ok(source) = SourceText::read(path) else {
            continue;
        };
        let text = source.shared_text();
        let lines = LineIndex::new(&text);
        workspace.set_document(path, Arc::clone(&text), DocumentVersion::new(0));

        let Some(file) = workspace.file_id(path) else {
            continue;
        };

        for access in workspace.view(file).member_accesses().iter() {
            let at = lines.line_col(access.range.start());
            let where_ = format!(
                "{}:{}:{}",
                display(&root, path),
                at.line + 1,
                lines.character_col(&text, access.range.start()) + 1
            );

            totals.accesses += 1;
            if let Some(dump) = dump.as_mut() {
                dump.access(
                    &where_,
                    access.name.as_str(),
                    &format!(
                        "{:?} null={:?} false={:?}",
                        access.outcome, access.null_reach, access.false_reach
                    ),
                );
            }
            match &access.outcome {
                // A property an object shape names is answered as surely as a declared one.
                MemberAccessOutcome::Resolved { .. } | MemberAccessOutcome::ShapeProperty => {
                    totals.resolved += 1
                }
                MemberAccessOutcome::UnknownReceiver => {
                    totals.unknown_receiver += 1;
                    // The receiver as written, not just the member's name. "118 sites of
                    // `getValue`" is a number; "111 sites of `$x->get('field')`" is a task, and the
                    // two readings sent this list to the wrong place once already.
                    unknown_receivers.record(
                        &format!(
                            "{}->{}",
                            shape_of(&text, access.receiver_range),
                            access.name
                        ),
                        &where_,
                    );
                }
                // Counted apart from the gaps above it: `@param object $x` is the author saying
                // every class at once, so no inference will narrow it and it belongs nowhere near
                // a list of things to go and fix.
                MemberAccessOutcome::AnyObject => {
                    totals.any_object += 1;
                    any_objects.record(
                        &format!(
                            "{}->{}",
                            shape_of(&text, access.receiver_range),
                            access.name
                        ),
                        &where_,
                    );
                }
                MemberAccessOutcome::UnknownClass { class } => {
                    totals.unknown_class += 1;
                    unknown_classes.record(class.as_str(), &where_);
                }
                MemberAccessOutcome::IncompleteHierarchy { class } => {
                    totals.incomplete_hierarchy += 1;
                    incomplete.record(class.as_str(), &where_);
                }
                MemberAccessOutcome::Dynamic { class } => {
                    totals.dynamic += 1;
                    dynamic.record(class.as_str(), &where_);
                }
                MemberAccessOutcome::MissingMember { class } => {
                    totals.missing_member += 1;
                    missing_members.record(&format!("{}::{}", class, access.name), &where_);
                }
                // Counted with the accesses the index *did* settle: it named a type and the type
                // says the access cannot work, which is an answer and not a gap.
                MemberAccessOutcome::NotAnObject { kind } => {
                    totals.not_an_object += 1;
                    not_objects.record(
                        &format!(
                            "{}->{} on {}",
                            shape_of(&text, access.receiver_range),
                            access.name,
                            kind.as_str()
                        ),
                        &where_,
                    );
                }
            }
        }

        // Every string a call or an array entry reads as a catalog key, counted by what the catalog
        // could say about it. The three outcomes are three different things and only one of them is
        // a gap: a key the catalog holds is understood, a key a *closed* catalog does not hold is a
        // fact about the code, and a key an open catalog does not hold is the one this cannot judge
        // -- unless it is a near miss, which is the population the report draws from.
        for used in workspace.view(file).catalog_uses() {
            totals.catalog_keys += 1;
            let at = lines.line_col(used.range.start());
            let where_ = format!(
                "{}:{}:{}",
                display(&root, path),
                at.line + 1,
                lines.character_col(&text, used.range.start()) + 1
            );
            match (used.known, used.closed, used.nearest.is_some()) {
                (true, ..) => totals.catalog_known += 1,
                (false, true, _) => totals.catalog_absent += 1,
                (false, false, true) => {
                    totals.catalog_near += 1;
                    catalog_near.record(&format!("{} {}", used.catalog, used.key), &where_);
                }
                (false, false, false) => {
                    totals.catalog_open += 1;
                    catalog_open.record(&format!("{} {}", used.catalog, used.key), &where_);
                }
            }
        }

        // Assignments are where a local gets its type, so an assignment with none is a variable with
        // none -- and the right-hand side says which expression to go and look at.
        for assigned in workspace.view(file).assigned_types() {
            totals.assignments += 1;
            if let Some(dump) = dump.as_mut() {
                let at = lines.line_col(assigned.range.start());
                dump.assignment(
                    &format!(
                        "{}:{}:{}",
                        display(&root, path),
                        at.line + 1,
                        lines.character_col(&text, assigned.range.start()) + 1
                    ),
                    assigned.ty.as_deref(),
                );
            }
            if assigned.ty.is_some() {
                totals.assignments_typed += 1;
                continue;
            }

            let at = lines.line_col(assigned.range.start());
            let where_ = format!(
                "{}:{}:{}",
                display(&root, path),
                at.line + 1,
                lines.character_col(&text, assigned.range.start()) + 1
            );
            untyped_assignments.record(&shape_of(&text, assigned.value_range), &where_);
        }

        workspace.close_document(path);
    }
    let sweep_time = sweep_started.elapsed();

    println!(
        "swept               {} of {available} user files in {sweep_time:.2?}",
        user_files.len()
    );
    println!();
    totals.print();

    print_causes("unknown classes", &unknown_classes);
    print_causes("incomplete hierarchies", &incomplete);
    print_causes("dynamic classes", &dynamic);
    print_causes("missing members", &missing_members);
    print_causes("untyped receivers", &unknown_receivers);
    print_causes("receivers declared object", &any_objects);
    print_causes("receivers no object can be", &not_objects);
    print_causes("assignments with no type", &untyped_assignments);
    print_causes("keys an open catalog cannot judge", &catalog_open);
    print_causes("keys that look like a misspelling", &catalog_near);

    if let (Some(dump), Some(path)) = (dump, dump_to) {
        dump.write(path)?;
    }
    Ok(())
}

/// How many accesses reached each outcome.
#[derive(Debug, Default)]
struct Totals {
    accesses: usize,
    resolved: usize,
    unknown_receiver: usize,
    any_object: usize,
    catalog_keys: usize,
    catalog_known: usize,
    catalog_absent: usize,
    catalog_open: usize,
    catalog_near: usize,
    assignments: usize,
    assignments_typed: usize,
    unknown_class: usize,
    incomplete_hierarchy: usize,
    dynamic: usize,
    missing_member: usize,
    not_an_object: usize,
}

impl Totals {
    fn print(&self) {
        println!("member accesses     {}", self.accesses);
        self.row("resolved", self.resolved);
        self.row("untyped receiver", self.unknown_receiver);
        self.row("declared object", self.any_object);
        self.row("unknown class", self.unknown_class);
        self.row("incomplete class", self.incomplete_hierarchy);
        self.row("dynamic class", self.dynamic);
        self.row("missing member", self.missing_member);
        self.row("not an object", self.not_an_object);

        println!();
        println!("catalog keys         {}", self.catalog_keys);
        self.catalog_row("registered", self.catalog_known, self.catalog_keys);
        self.catalog_row("nothing registers", self.catalog_absent, self.catalog_keys);
        self.catalog_row("looks misspelt", self.catalog_near, self.catalog_keys);
        self.catalog_row("open, cannot judge", self.catalog_open, self.catalog_keys);

        println!();
        println!("assignments         {}", self.assignments);
        let share = if self.assignments == 0 {
            0.0
        } else {
            #[expect(clippy::cast_precision_loss, reason = "a percentage for display")]
            {
                self.assignments_typed as f64 * 100.0 / self.assignments as f64
            }
        };
        println!(
            "  {:<18}{:>8}  {share:>5.1}%",
            "typed", self.assignments_typed
        );
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "a percentage for display, over a count that fits a f64 exactly"
    )]
    /// The same row, out of the catalog-key total rather than the member-access one.
    fn catalog_row(&self, label: &str, count: usize, of: usize) {
        let share = match of {
            0 => 0.0,
            of => count as f64 * 100.0 / of as f64,
        };
        println!("  {label:<18}{count:>8}  {share:>5.1}%");
    }

    fn row(&self, label: &str, count: usize) {
        let share = if self.accesses == 0 {
            0.0
        } else {
            count as f64 * 100.0 / self.accesses as f64
        };
        println!("  {label:<18}{count:>8}  {share:>5.1}%");
    }
}

fn print_causes(title: &str, tally: &Tally) {
    if tally.is_empty() {
        return;
    }

    let (rows, remaining) = tally.top(TOP);
    println!();
    println!("top {title}");
    for (count, key, example) in rows {
        println!("  {count:>6}  {key}");
        println!("          first at {example}");
    }
    if remaining > 0 {
        println!("  ... and {remaining} more distinct");
    }
}

fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The expression a value came from, as a one-line label to group misses by.
///
/// The source text rather than a name for the node kind. A kind says `Unmodelled`, which is true and
/// useless; the text says `(int) $node->id()`, which says what to go and model.
fn shape_of(text: &str, range: phrost_model::TextRange) -> String {
    let written = text
        .get(usize::from(range.start())..usize::from(range.end()))
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Long enough to recognise the shape, short enough that a hundred of them read as a list.
    if written.chars().count() > 48 {
        format!("{}…", written.chars().take(48).collect::<String>())
    } else {
        written
    }
}
