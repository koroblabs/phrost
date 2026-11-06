//! Which checks the slowest diagnostics file spent its time in.
//!
//! The diagnostics row says what a file costs and not why. This reruns every check over the worst
//! file of the sweep, each timed alone, and prints the few that dominate -- so a p95 that moved has
//! a place to start looking that is not a guess.

use std::time::Duration;

use phrost_lsp::Workspace;
use phrost_model::FileId;

/// How many runs each check's median is taken over.
const RUNS: usize = 7;

/// How many checks the line names.
const SHOWN: usize = 6;

/// Prints the checks that cost the most on `file`, by median over [`RUNS`] runs.
pub(super) fn print(workspace: &Workspace, file: FileId, shown_as: &str) {
    let mut runs: Vec<Vec<phrost_analysis::CheckCost>> = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        runs.push(phrost_analysis::check_costs(
            &workspace.view(file),
            workspace.asked_for(),
        ));
    }
    let Some(first) = runs.first() else {
        return;
    };
    let mut medians: Vec<(&'static str, Duration)> = (0..first.len())
        .map(|index| {
            let mut spent: Vec<Duration> = runs.iter().map(|run| run[index].spent).collect();
            spent.sort();
            (first[index].check, spent[spent.len() / 2])
        })
        .collect();
    medians.sort_by_key(|(_, spent)| std::cmp::Reverse(*spent));
    let total: Duration = medians.iter().map(|(_, spent)| *spent).sum();

    println!();
    println!(
        "slowest checks      {} total by median of {RUNS} at {shown_as}",
        lspkit_perf::ms(total)
    );
    for (check, spent) in medians.into_iter().take(SHOWN) {
        println!("  {check:<32}{:>10}", lspkit_perf::ms(spent));
    }
}

/// Runs the file's diagnostics in a loop for `seconds`, for a profiler to watch.
pub(super) fn hold(workspace: &Workspace, file: FileId, shown_as: &str, seconds: u64, on: &str) {
    if seconds == 0 {
        return;
    }
    eprintln!(
        "holding             {on} of {shown_as} for {seconds}s, pid {}",
        std::process::id()
    );
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    let mut runs = 0_usize;
    while std::time::Instant::now() < until {
        let view = workspace.view(file);
        match on {
            "actions" => {
                let length = workspace
                    .document_text(file)
                    .map_or(0, |(text, _)| text.len());
                let whole = phrost_model::TextRange::new(
                    phrost_model::TextSize::from(0),
                    phrost_model::TextSize::try_from(length).unwrap_or_default(),
                );
                let request = phrost_analysis::ActionRequest {
                    view: &view,
                    at: whole,
                    only: &[],
                    asked: workspace.asked_for(),
                };
                std::hint::black_box(phrost_analysis::code_actions(&request));
            }
            _ => {
                std::hint::black_box(phrost_analysis::diagnostics_asked_for(
                    &view,
                    workspace.asked_for(),
                ));
            }
        }
        runs += 1;
    }
    eprintln!("held                {runs} runs");
}

/// Runs completion at one site in a loop for `seconds`, for a profiler to watch: the slowest the
/// sweep found, so `sample <pid>` sees that one request and nothing else. Each run on a fresh view,
/// as the server answers each keystroke.
pub(super) fn hold_completion(
    workspace: &Workspace,
    file: FileId,
    at: phrost_model::TextSize,
    shown_as: &str,
    seconds: u64,
) {
    if seconds == 0 {
        return;
    }
    eprintln!(
        "holding             completion at {shown_as} for {seconds}s, pid {}",
        std::process::id()
    );
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    let mut runs = 0_usize;
    while std::time::Instant::now() < until {
        let view = workspace.view(file);
        std::hint::black_box(phrost_analysis::completions(&view, at));
        runs += 1;
    }
    eprintln!("held                {runs} runs");
}
