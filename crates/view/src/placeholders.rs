//! A format string's placeholders against the map of values written beside it.
//!
//! `f('Hello @name', ['@name' => $x])` is one expression saying the same thing twice, and nothing
//! makes the two halves agree. When they disagree the value is dropped and the placeholder reaches
//! the page as the literal text `@name` -- silently, on every run, with no error anywhere.
//!
//! # Which calls this is about, and why it does not need to know their names
//!
//! The map identifies the call. A placeholder key is a string beginning with `@`, `%` or `:`
//! followed by a word character, and an array literal holding one of those, written immediately
//! after a string literal, is not a shape any other kind of call is written in. So the shape is the
//! contract, and this projection recognises it without a list of callee names -- which is what keeps
//! it a fact about PHP rather than about one framework. The same spelling reaches four unrelated
//! things in one real project (a translation call, a translatable markup object, a logger channel,
//! and a formatted-markup constructor) and this reads all four without naming any of them.
//!
//! One such key and not every key, because a map may legitimately hold both: a PSR-3 logger's
//! context array carries `exception` and `channel` beside its placeholders, and the formatter drops
//! those rather than looking for them in the message. So a key that is not shaped like a placeholder
//! is passed over rather than taken as evidence against the call.
//!
//! # The one position it reads
//!
//! Arguments 0 and 1, and no other pair. A call that puts the message somewhere else -- a plural
//! form taking `(count, singular, plural, args)`, a logger taking `(level, message, context)`, a
//! query builder taking `(table, alias, condition, args)` -- has a *different* contract about which
//! placeholders are supplied from where: a plural form injects `@count` itself, and a builder's
//! condition is SQL, where `'%Y-%m-%d %H:%i'` is a date format and not two placeholders and a third.
//! Scanning every adjacent pair would read all of those as this one, so the pair is fixed and the
//! rest are silence. See the tests for each shape that is declined.
//!
//! # Substring, not token equality
//!
//! Replacement is `strtr($string, $args)` -- see `FormattableMarkup::placeholderFormat` -- so a key
//! is used if and only if its text occurs in the message, and a placeholder is supplied if and only
//! if some key matches the message from that point on. Both directions are decided that way rather
//! than by comparing two sets of tokens, which is what lets a key holding a character the scanner
//! stops at -- `@node.title` -- still count as supplying the `@node` the scanner found.
//!
//! # The map declares the alphabet
//!
//! Only a sigil some key of *this call* is written with counts as marking a slot in *this* message.
//! Without that rule the check reads every other language that spells something with `@`, `%` or
//! `:`, and there are more of them than there are placeholder conventions: an XPath predicate is
//! full of `@href` and `@class`, a `LIKE` pattern of `%`, a date format of `%H:%i`.
//!
//! It is measured, not defensive. Run over a framework's own 12 000-file source, reading every sigil
//! produced **181** reports, of which the overwhelming majority were XPath attribute names in
//! `$this->xpath('//a[@href=:path]', [':path' => ...])` -- a call whose map is keyed entirely on `:`,
//! saying plainly that `@` is not how this string marks a slot. Restricting to the map's own sigils
//! took that to **14**, and excluding `::` took it to **one**, which was a real bug in that
//! framework. The cost is real and bounded: a message writing `%name` while its map supplies only
//! `@name` loses the *unsupplied* half of the report and keeps the *unused* half, so the call is
//! still pointed at.

use phrost_hir::{
    FileHir,
    body::{ArrayElementKind, ExprData, LiteralKind},
};
use phrost_model::{Name, TextRange, TextSize};

use crate::view::IndexView;

/// One placeholder whose two halves do not line up.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlaceholderMismatch {
    /// The placeholder itself -- where it is written in the message, or where its key is written in
    /// the map. Never the call, which is mostly correct and would be the wrong thing to underline.
    pub range: TextRange,
    /// As written, sigil included, because the sigil is part of the key.
    pub name: Box<str>,
    pub fault: PlaceholderFault,
}

/// Which half of the pair is missing the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaceholderFault {
    /// The message names it and no key supplies a value. It reaches the output as its own text.
    Unsupplied,
    /// A key supplies a value the message never writes. `strtr` has nothing to replace, so the value
    /// is computed and thrown away.
    Unused,
}

impl IndexView<'_> {
    /// Every placeholder in the current file that a call names without supplying, or supplies
    /// without naming.
    ///
    /// Silent wherever either half is not fully written out, because a half-read call is a guess:
    ///
    /// - a message that is not a single string literal -- a variable, a concatenation, an
    ///   interpolated string, a heredoc -- says nothing about which placeholders it holds;
    /// - a message literal containing a backslash, whose escapes mean the text `strtr` sees is not
    ///   the text between the quotes and the offsets of a report would be wrong;
    /// - a second argument that is not an array literal, or that is spread or named;
    /// - a map with no key shaped like a placeholder, which is the evidence that it is one. An empty
    ///   `[]` is included here: it supplies nothing and says nothing about what it is;
    /// - a sigil in the message that no key of the same call is written with -- see the module doc
    ///   on the alphabet, which is what keeps XPath and SQL out.
    ///
    /// And silent about the *unsupplied* direction alone where the map is not fully readable -- a
    /// computed key, a spread, or a bare positional element could each be the value that is
    /// supposedly missing. The *unused* direction survives that: nothing another element does can
    /// make a literal key that the message never writes into one that it does.
    pub fn placeholder_mismatches(&self) -> Vec<PlaceholderMismatch> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let text = open.text();
        let mut out = Vec::new();

        for call in phrost_body::call_sites(hir) {
            // The first two and no others. A third argument is `$options` -- a langcode, a context
            // -- so a call carrying one is still this shape and is still read; it is simply not
            // another map.
            let Some([message, map]) = call.arguments.get(..2) else {
                continue;
            };
            Self::mismatches_in(hir, text, message, map, &mut out);
        }

        out.sort_by_key(|found| found.range.start());
        out
    }

    /// What one candidate pair of arguments disagrees about, appended in written order.
    fn mismatches_in(
        hir: &FileHir,
        text: &str,
        message: &phrost_body::ArgumentSite,
        map: &phrost_body::ArgumentSite,
        out: &mut Vec<PlaceholderMismatch>,
    ) {
        if message.name.is_some() || message.spread || map.name.is_some() || map.spread {
            return;
        }
        // Both shapes before either is read: matching an expression's kind is an index and a
        // comparison, and scanning the literal's text is not. Nearly every call in a file fails one
        // of these two.
        let ExprData::Literal {
            kind: LiteralKind::String,
            range,
        } = hir.expr(message.value)
        else {
            return;
        };
        let ExprData::ArrayLiteral { elements, .. } = hir.expr(map.value) else {
            return;
        };
        let Some((written, at)) = unescaped(text, *range) else {
            return;
        };

        let mut keys: Vec<(&Name, TextRange)> = Vec::new();
        let mut every_key_is_readable = true;
        for &element in hir.expr_list(*elements) {
            match hir.expr(element) {
                ExprData::ArrayElement {
                    kind: ArrayElementKind::LiteralKey { name, range },
                    ..
                } => keys.push((name, *range)),
                // A computed key, a spread, or a bare element keyed by position: each could hold the
                // placeholder that otherwise reads as unsupplied.
                _ => every_key_is_readable = false,
            }
        }

        // The sigils this call writes its slots with, and the evidence that it is one of these calls
        // at all. Without a single placeholder-shaped key the shape is just a string and an array,
        // which is half the calls in any project.
        let alphabet = keys
            .iter()
            .filter(|(name, _)| is_placeholder(name.as_str()))
            .fold(Alphabet::default(), |alphabet, (name, _)| {
                alphabet.with(name.as_str().as_bytes()[0])
            });
        if alphabet.is_empty() {
            return;
        }

        if every_key_is_readable {
            for (offset, placeholder) in placeholders_in(written) {
                // Only a sigil this call actually writes its own slots with. A message full of `@`
                // beside a map keyed entirely on `:` is not a message with unsupplied placeholders:
                // it is a string in a language where `@` means something else, and the map is what
                // says so. Measured on a framework's own source this is the whole difference between
                // 181 reports and 14 -- see the module doc.
                if !alphabet.holds(placeholder.as_bytes()[0]) {
                    continue;
                }
                let rest = &written[offset..];
                if keys.iter().any(|(name, _)| rest.starts_with(name.as_str())) {
                    continue;
                }
                let start = at + TextSize::from(offset as u32);
                out.push(PlaceholderMismatch {
                    range: TextRange::at(start, TextSize::from(placeholder.len() as u32)),
                    name: placeholder.into(),
                    fault: PlaceholderFault::Unsupplied,
                });
            }
        }

        for (name, range) in keys {
            // A key that is not shaped like a placeholder is not one this can judge: a logger's
            // context array carries `exception` and `link` beside its placeholders, and Drupal's own
            // formatter drops them rather than treating them as text to replace.
            if !is_placeholder(name.as_str()) || written.contains(name.as_str()) {
                continue;
            }
            out.push(PlaceholderMismatch {
                range,
                name: name.as_str().into(),
                fault: PlaceholderFault::Unused,
            });
        }
    }
}

/// Which of the three sigils one call's map writes its keys with.
///
/// A set of at most three things, so a set of bits rather than a `Vec`: this is built for every call
/// in the file whose first two arguments are a literal and an array, which on a real project is a
/// few hundred per file and every one of them on the path a keystroke takes.
#[derive(Clone, Copy, Default, Debug)]
struct Alphabet(u8);

impl Alphabet {
    fn bit(sigil: u8) -> u8 {
        match sigil {
            b'@' => 1,
            b'%' => 2,
            b':' => 4,
            _ => 0,
        }
    }

    fn with(self, sigil: u8) -> Self {
        Self(self.0 | Self::bit(sigil))
    }

    fn holds(self, sigil: u8) -> bool {
        self.0 & Self::bit(sigil) != 0
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Whether a key is spelled the way a placeholder is: one of the three sigils, then a word
/// character.
///
/// All three, and the sigil is part of the key: `@name` escapes, `%name` escapes and wraps the value
/// in emphasis, `:name` is escaped for use as a URL. They are three different keys and a message
/// asking for one is not answered by another.
fn is_placeholder(key: &str) -> bool {
    let mut bytes = key.bytes();
    matches!(bytes.next(), Some(b'@' | b'%' | b':')) && bytes.next().is_some_and(is_word_byte)
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Every placeholder written in a message, as an offset into it and the text found there.
///
/// A sigil, a word character, and the run of word characters after it -- and the sigil must start a
/// word: it may follow neither a word character nor another sigil. Both halves of that condition
/// were measured rather than guessed, against a framework's own source:
///
/// - after a word character, a sigil is part of the word. `support@example.com` would otherwise read
///   as a placeholder `@example`, and `12:30` as `:30`.
/// - after another sigil, `::` is somebody else's operator. An XPath axis (`following::a`,
///   `ancestor::section`) and a PostgreSQL cast (`refobjid::regclass`) between them accounted for
///   thirteen of the fourteen reports left after every other rule here, and every one of them was
///   wrong.
///
/// A `-` or a `.` *between* two word characters stays in the token, because real keys are written
/// that way -- `:views-ui`, `%node-add`, `@node.title`. Between, and not at the end: a sentence
/// ending `... at @date.` names `@date`, and `@a-@b` is two placeholders rather than one with a
/// hyphen on it. Which key actually supplies the token is still decided by the prefix test in
/// [`IndexView::placeholder_mismatches`], so the run only decides how much of the text a report
/// quotes -- getting it long enough is what makes the report say `:views-ui` rather than `:views`.
fn placeholders_in(message: &str) -> Vec<(usize, &str)> {
    let bytes = message.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;

    while at < bytes.len() {
        if !matches!(bytes[at], b'@' | b'%' | b':')
            || at > 0
                && (is_word_byte(bytes[at - 1]) || matches!(bytes[at - 1], b'@' | b'%' | b':'))
            || !bytes.get(at + 1).copied().is_some_and(is_word_byte)
        {
            at += 1;
            continue;
        }
        let mut end = at + 1;
        while end < bytes.len() {
            let joined = matches!(bytes[end], b'-' | b'.')
                && bytes.get(end + 1).copied().is_some_and(is_word_byte);
            if !is_word_byte(bytes[end]) && !joined {
                break;
            }
            end += 1;
        }
        found.push((at, &message[at..end]));
        at = end;
    }

    found
}

/// The text of a quoted literal that needs no unescaping, and where that text starts.
///
/// `None` for anything that is not a single- or double-quoted string, and for one holding a
/// backslash. The escape is not fussiness: what `strtr` matches against is the *decoded* string, and
/// every character after an escape sits at a different offset there than in the file -- so a report
/// built from these offsets would underline the wrong span. Declining is the only honest answer this
/// layer can give without decoding, which is the lexer's job rather than this one's.
fn unescaped(text: &str, range: TextRange) -> Option<(&str, TextSize)> {
    let written = text.get(usize::from(range.start())..usize::from(range.end()))?;
    let quote = written.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let inner = written.strip_prefix(quote)?.strip_suffix(quote)?;
    (!inner.contains('\\')).then(|| (inner, range.start() + TextSize::from(1)))
}
