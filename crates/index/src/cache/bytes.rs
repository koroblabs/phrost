//! The primitives every stored shape is written in.
//!
//! Hand-written, and in this crate only. A derive on every stored type would make the *shape* of
//! `MemberRow`, `TypeRef` and a dozen others into a wire format nobody can see, spread the
//! serialization dependency across five crates, and still not answer the question that matters --
//! whether a field was forgotten. The invariant test answers that; this only has to be simple
//! enough to read.
//!
//! Little-endian, length-prefixed, no self-description. The file is only ever read by the version
//! that wrote it: a stamp check rejects anything else, so there is nothing to negotiate.

use std::io;

use phrost_model::{TextRange, TextSize};

/// Appends values to whatever is being written to.
///
/// Straight to the sink rather than into a buffer that is written at the end, and that is a memory
/// decision with a number behind it. Building the index into a `Vec` grew it by doubling to 46 MB
/// on a real checkout, so the 32 MB and 64 MB blocks it passed through were both freed -- and
/// macOS keeps recently freed large regions rather than returning them, so `vmmap` showed **93.8 MB
/// of empty-but-dirty `MALLOC_LARGE`** on a server that had written a cache, against 56.6 MB on one
/// that had only read one. That was more than a third of the process, held by nothing.
///
/// An error is kept rather than returned, so that every one of the thirty-odd call sites stays a
/// plain statement. The first failure stops the writing and [`Self::finish`] reports it, which is
/// the same guarantee `write_all` gives and none of the noise.
pub(crate) struct Writer<'a> {
    sink: &'a mut dyn io::Write,
    failed: Option<io::Error>,
}

impl std::fmt::Debug for Writer<'_> {
    /// A sink has no `Debug`, and the bytes already written have gone to it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Writer")
    }
}

impl<'a> Writer<'a> {
    pub(crate) fn new(sink: &'a mut dyn io::Write) -> Self {
        Self { sink, failed: None }
    }

    /// Whether everything written reached the sink.
    pub(crate) fn finish(mut self) -> io::Result<()> {
        match self.failed.take() {
            Some(failed) => Err(failed),
            None => self.sink.flush(),
        }
    }

    fn put(&mut self, bytes: &[u8]) {
        if self.failed.is_some() {
            return;
        }
        if let Err(failed) = self.sink.write_all(bytes) {
            self.failed = Some(failed);
        }
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.put(&[value]);
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.put(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.put(&value.to_le_bytes());
    }

    pub(crate) fn usize(&mut self, value: usize) {
        self.u32(u32::try_from(value).unwrap_or(u32::MAX));
    }

    pub(crate) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(crate) fn str(&mut self, value: &str) {
        self.usize(value.len());
        self.put(value.as_bytes());
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.put(value);
    }

    pub(crate) fn range(&mut self, value: TextRange) {
        self.u32(value.start().into());
        self.u32(value.end().into());
    }

    /// Writes a length, then each item.
    pub(crate) fn list<T>(&mut self, items: &[T], mut write: impl FnMut(&mut Self, &T)) {
        self.usize(items.len());
        for item in items {
            write(self, item);
        }
    }

    /// Writes a present flag, then the value if there is one.
    pub(crate) fn option<T>(&mut self, value: Option<&T>, write: impl FnOnce(&mut Self, &T)) {
        match value {
            Some(value) => {
                self.bool(true);
                write(self, value);
            }
            None => self.bool(false),
        }
    }
}

/// Reads values back, in the order they were written.
///
/// From a stream rather than a slice, for the reason [`Writer`] writes to one: reading the file
/// into a `Vec` to decode from cost a 46 MB allocation that macOS keeps rather than returns, and
/// `vmmap` showed it as most of the 56.6 MB of empty-but-dirty `MALLOC_LARGE` a warm server holds.
/// The format never needed random access -- every value is read exactly once, in order.
///
/// Every method returns `None` on a truncated or nonsensical stream, and one `None` aborts the whole
/// load. A half-decoded index is worse than no index: it would answer, and wrongly.
pub(crate) struct Reader<'a> {
    source: &'a mut dyn io::Read,
    /// A window over the source. Every value is read out of it and it is refilled when one would
    /// run past its end, so the source is asked for bytes a few times per megabyte rather than once
    /// per integer -- which through `&mut dyn Read` was a virtual call per value, and a 46 MB index
    /// holds about ten million of them.
    window: Vec<u8>,
    /// Where the next value starts in `window`, and where the valid bytes end.
    at: usize,
    end: usize,
    /// For the rare string longer than the window.
    scratch: Vec<u8>,
    left: u64,
}

/// How much is read from the source at once.
const WINDOW: usize = 64 * 1024;

impl std::fmt::Debug for Reader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reader")
    }
}

impl<'a> Reader<'a> {
    pub(crate) fn new(source: &'a mut dyn io::Read, len: u64) -> Self {
        Self {
            source,
            window: vec![0; WINDOW],
            at: 0,
            end: 0,
            scratch: Vec::new(),
            left: len,
        }
    }

    pub(crate) fn left(&self) -> u64 {
        self.left
    }

    pub(crate) fn is_done(&mut self) -> bool {
        if self.at < self.end {
            return false;
        }
        let mut spare = [0_u8; 1];
        matches!(self.source.read(&mut spare), Ok(0))
    }

    /// Makes sure `need` bytes are in the window, reading more from the source if they are not.
    /// `None` when the source ends first. `need` must not exceed the window.
    fn fill(&mut self, need: usize) -> Option<()> {
        if self.end - self.at >= need {
            return Some(());
        }
        // Keep what is unread at the front, then top up behind it.
        self.window.copy_within(self.at..self.end, 0);
        self.end -= self.at;
        self.at = 0;
        while self.end < need {
            match self.source.read(&mut self.window[self.end..]) {
                Ok(0) => return None,
                Ok(read) => self.end += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return None,
            }
        }
        Some(())
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        Some(self.array::<1>()?[0])
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn usize(&mut self) -> Option<usize> {
        Some(self.u32()? as usize)
    }

    pub(crate) fn bool(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }

    pub(crate) fn str(&mut self) -> Option<&str> {
        let len = self.usize()?;
        if u64::try_from(len).ok()? > self.left {
            return None;
        }
        self.left -= len as u64;
        if len <= WINDOW {
            // Straight out of the window, with no copy: every caller consumes the string before
            // asking for the next value.
            self.fill(len)?;
            let bytes = &self.window[self.at..self.at + len];
            self.at += len;
            return std::str::from_utf8(bytes).ok();
        }
        // Longer than the window: what is buffered first, the rest from the source.
        self.scratch.clear();
        self.scratch
            .extend_from_slice(&self.window[self.at..self.end]);
        self.at = 0;
        self.end = 0;
        let buffered = self.scratch.len();
        self.scratch.resize(len, 0);
        self.source.read_exact(&mut self.scratch[buffered..]).ok()?;
        std::str::from_utf8(&self.scratch).ok()
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.fill(N)?;
        let mut raw = [0_u8; N];
        raw.copy_from_slice(&self.window[self.at..self.at + N]);
        self.at += N;
        self.left = self.left.checked_sub(N as u64)?;
        Some(raw)
    }

    pub(crate) fn range(&mut self) -> Option<TextRange> {
        let start = TextSize::from(self.u32()?);
        let end = TextSize::from(self.u32()?);
        // A range whose end precedes its start would panic in `TextRange::new`, and a corrupt file
        // must be refused rather than crash the server that read it.
        (start <= end).then(|| TextRange::new(start, end))
    }

    pub(crate) fn list<T>(
        &mut self,
        mut read: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<Vec<T>> {
        let len = self.usize()?;
        // Bounded by what the file still holds: a corrupt length must not ask for a gigabyte up
        // front, and sizing it at all is what keeps a thirty-thousand-entry list from growing
        // through a chain of doublings it then keeps.
        let mut items =
            Vec::with_capacity(len.min(usize::try_from(self.left).unwrap_or(usize::MAX)));
        for _ in 0..len {
            items.push(read(self)?);
        }
        Some(items)
    }

    pub(crate) fn option<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<Option<T>> {
        if self.bool()? {
            Some(Some(read(self)?))
        } else {
            Some(None)
        }
    }
}
