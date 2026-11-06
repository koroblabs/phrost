//! A line that says what a long command is doing.
//!
//! `phrost check` on a real checkout is five seconds of silence, then two more of it, and only then
//! the first finding -- from outside there is nothing to tell a slow index from a hung one. This is
//! the line that says which.
//!
//! On stderr, and only to a terminal. The findings are stdout's, so a run whose output is piped or
//! redirected is byte-identical to what it was before this existed -- which matters more here than
//! it looks: every measurement in `docs/` and half the notes in CLAUDE.md were taken by grepping
//! that output, and a control character in it would have made them all disagree with each other.

use std::{
    io::{IsTerminal, Write},
    time::{Duration, Instant},
};

/// How often the line is redrawn.
///
/// Fast enough to read as live and slow enough that the writing is not part of what is being
/// measured: a per-file tick over 30 000 files is 30 000 writes to a terminal otherwise, and a
/// terminal is not fast.
const REDRAW: Duration = Duration::from_millis(80);

pub(crate) struct Progress {
    /// `None` when nothing is watching.
    ///
    /// Boxed rather than a `Stderr`, so that a test can read what would have been drawn: the whole
    /// of this is control characters, and a line that erases the wrong amount of what came before
    /// is not something the type system has an opinion about.
    stream: Option<Box<dyn Write>>,
    /// Whether there is a line on screen to erase before anything else is printed.
    showing: bool,
    drawn_at: Instant,
}

impl Progress {
    pub(crate) fn on_stderr() -> Self {
        Self::writing_to(
            std::io::stderr()
                .is_terminal()
                .then(|| Box::new(std::io::stderr()) as Box<dyn Write>),
        )
    }

    fn writing_to(stream: Option<Box<dyn Write>>) -> Self {
        Self {
            stream,
            showing: false,
            drawn_at: Instant::now(),
        }
    }

    /// Replaces the line with `text`, at most every [`REDRAW`].
    ///
    /// Dropped rather than queued when it comes too soon: what a progress line is for is the
    /// *last* thing said, and a caller that ticks per file has already said the next one by the
    /// time a queue would have got to this.
    pub(crate) fn say(&mut self, text: &str) {
        if self.showing && self.drawn_at.elapsed() < REDRAW {
            return;
        }
        let Some(stream) = self.stream.as_mut() else {
            return;
        };
        // `\r` to the start, then the text, then erase whatever the last line left past it.
        let _ = write!(stream, "\r{text}\x1b[K");
        let _ = stream.flush();
        self.showing = true;
        self.drawn_at = Instant::now();
    }

    /// Takes the line off the screen, so whatever is printed next starts on a clean one.
    ///
    /// Called before every line of real output, not only at the end: findings go to stdout while
    /// this goes to stderr, and a terminal showing both has one cursor.
    pub(crate) fn clear(&mut self) {
        if !self.showing {
            return;
        }
        if let Some(stream) = self.stream.as_mut() {
            let _ = write!(stream, "\r\x1b[K");
            let _ = stream.flush();
        }
        self.showing = false;
    }
}

impl Drop for Progress {
    /// So that an early return cannot leave half a line under the shell prompt.
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, io::Write, rc::Rc};

    use expect_test::expect;

    use super::Progress;

    /// A stream a test can read back.
    #[derive(Clone)]
    struct Written(Rc<RefCell<Vec<u8>>>);

    impl Write for Written {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Every escape shown, because the whole of this feature is the escapes.
    #[test]
    fn a_line_is_replaced_and_then_taken_away() {
        let written = Written(Rc::default());
        let mut progress = Progress::writing_to(Some(Box::new(written.clone())));

        progress.say("indexing 1 of 2");
        // Far enough past the redraw window that the second one is not dropped as too soon.
        progress.drawn_at = std::time::Instant::now() - super::REDRAW * 2;
        progress.say("indexing 2 of 2");
        progress.clear();
        // Nothing to erase, so nothing is written: a second clear must not push the cursor around
        // under output that has already scrolled past.
        progress.clear();

        let bytes = written.0.borrow().clone();
        expect![[r#""\rindexing 1 of 2\u{1b}[K\rindexing 2 of 2\u{1b}[K\r\u{1b}[K""#]]
            .assert_eq(&format!("{:?}", String::from_utf8_lossy(&bytes)));
    }

    /// A redirected run must be byte-identical to what it was before this existed, which is what
    /// every measurement taken by grepping that output relies on. A test harness is not a terminal,
    /// so this is the gate itself being asked.
    #[test]
    fn nothing_is_drawn_where_nothing_is_watching() {
        assert!(Progress::on_stderr().stream.is_none());

        // And the drawing calls are still safe to make, which is what lets the caller tick without
        // asking first.
        let mut progress = Progress::writing_to(None);
        progress.say("indexing 1 of 2");
        progress.clear();
        assert!(!progress.showing);
    }
}
