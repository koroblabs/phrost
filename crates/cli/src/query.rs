//! What the server would answer at one position, without an editor in the way.
//!
//! The tool for reproducing a report. It indexes through [`Sweep`] for the reason that matters here:
//! a hand-rolled setup used to skip the framework contracts, so `\Drupal::service('id')` resolved in
//! the editor and not here -- and the command that exists to check the server disagreed with it.

use std::path::Path;

use phrost_analysis::{
    HoverBlock, completions, diagnostics, goto_definition, goto_external, hover, signature_help,
};
use phrost_model::{LineCol, LineIndex};

use crate::sweep::{Start, Sweep};

/// Answers goto, hover, and diagnostics at a position, the way an editor would.
///
/// Line and column are one-based and counted in characters, because that is what an editor's status
/// bar shows — the caller should not have to convert.
pub(crate) fn run(root: &Path, file: &Path, line: u32, column: u32) -> Result<(), String> {
    let mut sweep = Sweep::indexed_by(root.to_path_buf(), Start::Cold)?;
    // Against the root the sweep actually indexed, not the one the argument spelled: a relative
    // root is resolved on the way in, so `phrost query . src/X.php` looked for `./src/X.php` in an
    // index holding absolute paths and reported the file as not in the project.
    let target = if file.is_absolute() {
        file.to_path_buf()
    } else {
        sweep.root().join(file)
    };
    if !sweep.discovered().iter().any(|(path, _)| path == &target) {
        return Err(format!(
            "{} is not in the indexed project",
            target.display()
        ));
    }

    let mut answered = false;
    let mut failure = None;
    sweep.each(std::slice::from_ref(&target), |visit| {
        let column =
            character_to_byte_column(visit.text, visit.lines, line.saturating_sub(1), column);
        let Some(offset) = visit.lines.offset(LineCol {
            line: line.saturating_sub(1),
            col: column,
        }) else {
            failure = Some(format!("{line}:{column} is outside {}", target.display()));
            return;
        };
        answered = true;
        report(visit.view, offset);
    });

    if let Some(failure) = failure {
        return Err(failure);
    }
    if !answered {
        return Err(format!("cannot read {}", target.display()));
    }
    Ok(())
}

fn report(view: &phrost_view::IndexView<'_>, offset: phrost_model::TextSize) {
    println!("goto");
    // Both answers, because the server sends both and a tool that prints one of them says "nothing
    // to go to" about a jump that works. A route name and a permission are declared in YAML, which
    // holds no symbol -- so they come back as a path and a position rather than as a target, on a
    // path of their own, and printing only the symbols made this command disagree with the editor
    // it exists to reproduce.
    let targets = goto_definition(view, offset);
    let external = goto_external(view, offset);
    if targets.is_empty() && external.is_none() {
        println!("  (nothing to go to)");
    }
    for found in targets {
        let path = view.file_path(found.file).unwrap_or("<unknown>");
        println!("  {} {} @ {path}", found.kind.as_str(), found.name);
    }
    if let Some(found) = external {
        println!(
            "  {} @ {}:{}:{}",
            found.key,
            found.path.display(),
            found.line + 1,
            found.character + 1
        );
    }

    println!("hover");
    match hover(view, offset) {
        None => println!("  (nothing to describe)"),
        Some(info) => {
            for block in info.blocks {
                match block {
                    HoverBlock::Code(code) => println!("  code: {code}"),
                    HoverBlock::Documentation(text) => println!("  doc:  {text}"),
                    HoverBlock::Note(text) => println!("  note: {text}"),
                    HoverBlock::Tag {
                        label,
                        ty,
                        subject,
                        description,
                    } => println!(
                        "  tag:  {label}{}{} {description}",
                        subject.map(|s| format!(" {s}")).unwrap_or_default(),
                        ty.map(|t| format!(" {t}")).unwrap_or_default(),
                    ),
                    HoverBlock::Separator => println!("  ---"),
                }
            }
        }
    }

    println!("signature");
    match signature_help(view, offset) {
        Some(help) => {
            println!("  {}", help.label);
            if let Some(active) = help.active {
                println!("  active {active}");
            }
        }
        None => println!("  (no signature help)"),
    }

    // What would be offered here. Capped, because a bare name matches a thousand class names and
    // this exists to check a feature rather than to read a list -- the count is printed so a cut is
    // never mistaken for the whole answer.
    println!("completion");
    let offered = completions(view, offset);
    if offered.items.is_empty() {
        println!("  (nothing offered)");
    }
    for item in offered.items.iter().take(12) {
        match &item.detail {
            Some(detail) => println!("  {} — {detail}", item.label),
            None => println!("  {}", item.label),
        }
    }
    if offered.items.len() > 12 {
        println!("  ... and {} more", offered.items.len() - 12);
    }

    // The whole file's, not the position's: a report of the shape "there should be an error here"
    // is answered by what the file is underlined for, and the reason it is missing is often
    // somewhere else entirely -- a parse break above it silences everything below.
    println!("diagnostics");
    let found = diagnostics(view);
    if found.is_empty() {
        println!("  (nothing wrong with this file)");
    }
    for problem in found {
        println!("  [{}] {}", problem.code, problem.message);
    }
}

/// Turns a one-based character column into a byte column.
///
/// Characters rather than bytes because that is what an editor reports, and rather than UTF-16
/// because a person typing a command should not have to think about surrogate pairs.
fn character_to_byte_column(text: &str, index: &LineIndex, line: u32, column: u32) -> u32 {
    let Some(range) = index.line_range(line) else {
        return 0;
    };
    let line_text = &text[usize::from(range.start())..usize::from(range.end())];

    line_text
        .char_indices()
        .nth(column.saturating_sub(1) as usize)
        .map_or_else(
            || u32::try_from(line_text.len()).unwrap_or(0),
            |(offset, _)| u32::try_from(offset).unwrap_or(0),
        )
}
