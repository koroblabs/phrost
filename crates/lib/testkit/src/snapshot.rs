use std::fmt::Write as _;

use phrost_model::{LineIndex, TextRange, TextSize};

/// Builder for the rendered text that a snapshot test compares against.
///
/// The shape is fixed on purpose — a label, then `- ` rows, blank line between sections:
///
/// ```text
/// member goto
/// - method App\Repo::find @ src/Repo.php 4:21-4:25
///
/// receiver type
/// - ?App\User
/// ```
///
/// Every layer renders into this, so a diff reads the same whether it came from the def-map or
/// from completion. Rows are written by the caller in a deliberate order; nothing here sorts,
/// because *which* order a layer reports in is itself part of what the snapshot should capture.
#[derive(Debug, Default)]
pub struct Snapshot {
    out: String,
    section_open: bool,
}

impl Snapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a labelled section, separated from the previous one by a blank line.
    pub fn section(&mut self, label: impl AsRef<str>) -> &mut Self {
        if self.section_open {
            self.out.push('\n');
        }
        self.out.push_str(label.as_ref());
        self.out.push('\n');
        self.section_open = true;
        self
    }

    /// Adds one result row.
    pub fn row(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.out.push_str("- ");
        self.out.push_str(text.as_ref());
        self.out.push('\n');
        self
    }

    /// Adds an explicit empty-result row.
    ///
    /// Written out rather than left blank so that "the query returned nothing" and "the query was
    /// never run" cannot look the same in a diff.
    pub fn none(&mut self) -> &mut Self {
        self.row("(none)")
    }

    /// Adds a row for each item, or `(none)` if there are none.
    pub fn rows<I>(&mut self, items: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut any = false;
        for item in items {
            any = true;
            self.row(item);
        }
        if !any {
            self.none();
        }
        self
    }

    /// Notes that a result set was truncated, so a shrinking list is visible in the diff.
    pub fn truncated(&mut self, remaining: usize) -> &mut Self {
        if remaining > 0 {
            let _ = writeln!(self.out, "- ... +{remaining} more");
        }
        self
    }

    /// Finishes the snapshot, normalizing to exactly one trailing newline.
    pub fn finish(self) -> String {
        format!("{}\n", self.out.trim_end())
    }
}

/// Renders a range as `line:col-line:col`, one-based, for snapshot rows.
///
/// One-based because these lines are read by a human against an editor gutter. `LineCol` itself
/// stays zero-based, matching LSP, so only this rendering shifts.
pub fn render_range(index: &LineIndex, range: TextRange) -> String {
    let (start, end) = index.range_line_cols(range);
    format!(
        "{}:{}-{}:{}",
        start.line + 1,
        start.col + 1,
        end.line + 1,
        end.col + 1
    )
}

/// Renders a single offset as `line:col`, one-based.
pub fn render_position(index: &LineIndex, offset: TextSize) -> String {
    let position = index.line_col(offset);
    format!("{}:{}", position.line + 1, position.col + 1)
}
