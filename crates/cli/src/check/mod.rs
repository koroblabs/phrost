//! Running every check over a whole project, without an editor.
//!
//! Two jobs. It is how someone checks a project from a terminal or from CI, and it is how the
//! checker itself is kept honest: a count over real code is the only evidence that a new check is
//! precise, and "it looked right on a fixture" has never been the same claim.

use phrost_syntax_features::lint::LintCategory;
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use phrost_analysis::{Severity, diagnostics_asked_for};

mod baseline;

use crate::{
    progress::Progress,
    sweep::{Start, Sweep},
};
use lspkit_ratchet::{Baseline, Outcome};

/// Everything a run of the checker can be asked to do differently.
///
/// A struct rather than eight parameters: the list has grown once per capability and a caller
/// passing eight positional flags is a caller that will one day pass two of them the wrong way
/// round.
pub(crate) struct Options<'a> {
    /// How many files are checked at once. `None` is every core, up to eight.
    pub(crate) jobs: Option<usize>,
    pub(crate) root: Option<PathBuf>,
    /// Check only the first N user files, by path.
    pub(crate) limit: Option<usize>,
    pub(crate) quiet: bool,
    /// Also check dependency files whose path contains this.
    pub(crate) include: Option<&'a str>,
    pub(crate) warm: bool,
    /// Syntax-rule categories to run as well, comma-separated or `all`.
    pub(crate) lint: Option<&'a str>,
    pub(crate) ratchet: bool,
    pub(crate) accept: bool,
}

/// Checks every file a project owns and reports what it found.
pub(crate) fn run(options: Options<'_>) -> Result<(), String> {
    let Options {
        jobs,
        root,
        limit,
        quiet,
        include,
        warm,
        lint,
        ratchet,
        accept,
    } = options;
    let jobs = jobs.unwrap_or_else(crate::sweep::default_jobs).max(1);
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
    // Said while it happens rather than after: a real checkout is several seconds of indexing before
    // the first line of output, and from outside there is nothing to tell that from a hang.
    let mut progress = Progress::on_stderr();
    progress.say("phrost: discovering files");
    let sweep = Sweep::indexed_watching(root.clone(), start, &mut |done, total| {
        progress.say(&format!("phrost: indexing {done} of {total} files"));
    })?;
    progress.clear();
    println!(
        "indexed             {} files, {} start",
        sweep.indexed(),
        match start {
            Start::Cold => "cold",
            Start::Warm => "warm",
        }
    );

    // Before anything it produces, so a run whose settings were ignored says so first rather than
    // leaving somebody to wonder why a rule they switched off is still reported.
    for reason in sweep.config_errors() {
        println!("configuration       {reason} (ignored)");
    }

    let mut files = sweep.files(include);
    let available = files.len();
    if let Some(limit) = limit {
        files.truncate(limit);
    }

    // What the project asked for when the command line says nothing, and not an empty list: the
    // server resolves the same setting from the same `phrost.toml` and falls back to the same two
    // categories, so a checker starting from nothing was silent about findings the editor drew in
    // the file next to it. The named rules were already read from the project -- see `Sweep::lint`
    // -- and the categories were the half still decided somewhere else.
    let categories = match lint {
        None => sweep.workspace().lint_categories().to_vec(),
        Some("all") => LintCategory::ALL.to_vec(),
        Some(names) => names
            .split(',')
            .map(|name| {
                LintCategory::named(name.trim())
                    .ok_or_else(|| format!("no lint category called {name:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?,
    };

    // Gather, then judge -- the same two passes the server makes, and for the same reason: the
    // exact reachability rule is about a resolved member of a named class, and it can only answer
    // once every file has had its say. Without this the checker answered the coarse rule forever
    // while the editor answered the exact one, so CI and the editor disagreed about the same
    // project. Over the files being checked, which is the scope the server's pass has too.
    progress.say("phrost: reading what each file reaches");
    sweep.record_what_each_file_reaches(&files, jobs, &mut |done| {
        progress.say(&format!("phrost: reading {done} of {} files", files.len()));
    });
    progress.clear();

    let mut by_code: BTreeMap<&'static str, usize> = BTreeMap::new();
    // Owned keys, because a rule's name comes from the linter rather than from a list written here.
    let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_category: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut total = 0_usize;
    let started = Instant::now();
    let count = files.len();

    // Every file on `jobs` threads, each handing back what it found; counted and printed here, in
    // path order, once the sweep is done. Collected rather than printed as they are found for two
    // reasons: a finding goes to stdout and the progress line to stderr, and a terminal showing
    // both has one cursor to share; and the threads finish in whatever order they finish in, while
    // two runs of this have to print the same lines in the same order.
    let checked = sweep.each_in_parallel(
        &files,
        jobs,
        |visit| {
            let mut found = Checked::default();
            // Before the index-backed checks, and separately counted: these are one file's own
            // business and the point of running them here is to see how many there are and how much
            // they overlap what the index already says.
            // Not over a template, whose text is markup: what is PHP in it is read through the
            // view, which is where the checks below look.
            if !categories.is_empty() && !visit.view.is_template() {
                for issue in visit.lint(&categories) {
                    found
                        .rules
                        .push((issue.code.clone(), issue.category.as_str()));
                    if !quiet {
                        found.reported.push(format!(
                            "{}: {} {}: {}",
                            visit.position(issue.range.start()),
                            match issue.severity {
                                phrost_syntax_features::lint::LintSeverity::Error => "error",
                                phrost_syntax_features::lint::LintSeverity::Warning => "warning",
                                phrost_syntax_features::lint::LintSeverity::Hint => "hint",
                            },
                            issue.code,
                            issue.message
                        ));
                    }
                }
            }

            let mut issues = diagnostics_asked_for(visit.view, visit.asked_for());
            issues.extend(phrost_analysis::template_diagnostics(
                visit.view,
                visit.text,
                visit.asked_for(),
            ));
            for issue in issues {
                found.codes.push(issue.code);
                if !quiet {
                    found.reported.push(format!(
                        "{}: {} {}: {}",
                        visit.position(issue.range.start()),
                        match issue.severity {
                            Severity::Error => "error",
                            Severity::Warning => "warning",
                            Severity::Note => "note",
                        },
                        issue.code,
                        issue.message
                    ));
                }
            }
            found
        },
        &mut |done| progress.say(&format!("phrost: checking {done} of {count} files")),
    );
    let elapsed = started.elapsed();
    progress.clear();

    for found in checked {
        for (rule, category) in found.rules {
            total += 1;
            *by_rule.entry(rule).or_default() += 1;
            *by_category.entry(category).or_default() += 1;
        }
        for code in found.codes {
            total += 1;
            *by_code.entry(code).or_default() += 1;
        }
        for line in found.reported {
            println!("{line}");
        }
    }

    println!();
    println!(
        "checked             {} of {available} files in {elapsed:.2?} on {jobs} thread{}",
        files.len(),
        if jobs == 1 { "" } else { "s" }
    );
    for (code, count) in &by_code {
        println!("  {code:<24}{count:>8}");
    }
    if !by_rule.is_empty() {
        println!();
        println!("lint rules that fired  {:>7}", by_rule.len());
        let mut ranked = by_rule.iter().collect::<Vec<_>>();
        ranked.sort_by_key(|(rule, count)| (std::cmp::Reverse(**count), (*rule).clone()));
        for (rule, count) in ranked.iter().take(25) {
            println!("  {rule:<44}{count:>8}");
        }
        println!();
        for (category, count) in &by_category {
            println!("  {category:<44}{count:>8}");
        }
    }

    if ratchet || accept {
        return compare_to_baseline(&root, files.len(), &by_code, accept);
    }

    if total == 0 {
        println!("no problems found");
        return Ok(());
    }
    Err(format!("{total} problems found"))
}

/// What one file's check found, handed back from whichever thread checked it.
#[derive(Default)]
struct Checked {
    /// Findings as printed, in the order they were found.
    reported: Vec<String>,
    /// One entry per index-backed finding, by code.
    codes: Vec<&'static str>,
    /// One entry per syntax rule that fired, as `(rule, category)`.
    rules: Vec<(String, &'static str)>,
}

/// Reports this run against what the project agreed to, or writes a new agreement.
///
/// The finding list is still printed above, so a broken ratchet is read together with what broke it
/// rather than sending somebody back for a second run.
fn compare_to_baseline(
    root: &std::path::Path,
    files: usize,
    counts: &BTreeMap<&'static str, usize>,
    accept: bool,
) -> Result<(), String> {
    let found = baseline::counts(counts);
    let path = baseline::path(root);
    if accept {
        Baseline::new(Some(files), found).store(&path, baseline::COMMAND)?;
        println!();
        println!("baseline            written to {}", path.display());
        return Ok(());
    }

    // A malformed baseline reads as none, and says so: refusing to check a project because its
    // baseline has a typo would be refusing the more useful of the two answers.
    let agreed = match Baseline::load(&path) {
        Ok(Some(agreed)) => agreed,
        Ok(None) => {
            return Err(format!(
                "no baseline to compare against -- run again with `--accept` to write {}",
                path.display()
            ));
        }
        Err(unread) => {
            return Err(format!(
                "{unread} -- run again with `--accept` to write a new one"
            ));
        }
    };

    println!();
    // Said before the verdict, because a baseline taken over a different sweep makes every number
    // below meaningless and that is not obvious from the numbers themselves.
    if let Some(taken) = agreed.files
        && taken != files
    {
        println!("baseline            taken over {taken} files, this run covered {files}");
    }

    let comparison = agreed.compare(&found);
    match comparison.outcome() {
        Outcome::Held => {
            println!("ratchet             held, every count unchanged");
            Ok(())
        }
        Outcome::Fell => {
            println!(
                "ratchet             held, and {} went down:",
                comparison.moved.len()
            );
            print!("{}", comparison.report());
            println!();
            println!("  run with `--accept` to hold the new numbers");
            Ok(())
        }
        Outcome::Rose => {
            print!("{}", comparison.report());
            let rose = comparison
                .moved
                .iter()
                .filter(|moved| moved.rose() && moved.was.is_some())
                .count();
            let new = comparison
                .moved
                .iter()
                .filter(|moved| moved.was.is_none())
                .count();
            Err(format!(
                "ratchet broken: {rose} code(s) rose, {new} not in the baseline"
            ))
        }
    }
}
