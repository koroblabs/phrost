//! Every answer the sweep saw, one per line, for comparing two builds.
//!
//! The finding lists are the usual A/B, and they see only the places a check speaks: a change that
//! turns a receiver's class into a wrong one where nothing is reported moves no finding at all.
//! This writes what the index *answered* -- the class each member access resolved on and whether
//! it may be null or false, the type of each assignment -- so `diff` over two dumps shows every
//! answer that moved. Sorted, so two runs over one tree produce the same file.

use std::io::Write as _;
use std::path::Path;

/// The lines of one sweep, gathered and written sorted.
#[derive(Default)]
pub(super) struct Dump {
    lines: Vec<String>,
}

impl Dump {
    pub(super) fn access(&mut self, at: &str, name: &str, answer: &str) {
        self.lines.push(format!("{at}\taccess\t{name}\t{answer}"));
    }

    pub(super) fn assignment(&mut self, at: &str, ty: Option<&str>) {
        self.lines
            .push(format!("{at}\tassign\t{}", ty.unwrap_or("(no type)")));
    }

    pub(super) fn write(mut self, path: &Path) -> Result<(), String> {
        self.lines.sort();
        let mut file = std::fs::File::create(path)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        for line in &self.lines {
            writeln!(file, "{line}").map_err(|error| format!("could not write: {error}"))?;
        }
        println!(
            "dumped              {} answers to {}",
            self.lines.len(),
            path.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_sweeps_in_any_order_write_the_same_file() {
        let dir = std::env::temp_dir().join(format!("phrost-dump-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (one, two) = (dir.join("one.txt"), dir.join("two.txt"));

        let mut first = Dump::default();
        first.access("b.php:2:1", "run", "Resolved");
        first.assignment("a.php:1:1", Some("int"));
        first.write(&one).unwrap();

        let mut second = Dump::default();
        second.assignment("a.php:1:1", Some("int"));
        second.access("b.php:2:1", "run", "Resolved");
        second.write(&two).unwrap();

        let written = std::fs::read_to_string(&one).unwrap();
        assert_eq!(written, std::fs::read_to_string(&two).unwrap());
        assert_eq!(
            written,
            "a.php:1:1\tassign\tint\nb.php:2:1\taccess\trun\tResolved\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
