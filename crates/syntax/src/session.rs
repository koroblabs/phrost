use mago_allocator::LocalArena;
use mago_database::file::FileId as MagoFileId;
use mago_syntax::cst::{Program, TriviaKind};
use mago_syntax::error::ParseError;
use mago_syntax::token::TokenKind;
use phrost_model::TextSize;

use crate::{
    docblock::{Docblock, DocblockIndex},
    error::SyntaxError,
};

/// Owns one bump arena and the parse trees allocated in it.
///
/// Not `Sync`, deliberately: the arena is the fast thread-local kind, and sharing it would put a
/// lock on the hottest allocation path in the program. One session per thread, per unit of work.
#[derive(Default)]
pub struct ParseSession {
    arena: LocalArena,
}

impl std::fmt::Debug for ParseSession {
    /// The arena has no `Debug` and printing its contents would be meaningless anyway.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ParseSession")
    }
}

impl ParseSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses PHP source text.
    ///
    /// Takes `&str` rather than bytes so that decoding — and the lossy-UTF-8 decision that goes
    /// with it — stays in the VFS, where the file was read. The parser copies the text into the
    /// arena, so the caller's buffer does not need to outlive this call; the returned tree does
    /// borrow the session.
    ///
    /// There is no `Result`: PHP parsing is error-tolerant by design, because an editor spends
    /// most of its time looking at code that is briefly invalid. A tree always comes back, and
    /// [`ParsedFile::errors`] says what was wrong with it.
    ///
    /// # The tail a person is still typing
    ///
    /// mago recovers on a closing token and *drops* what it cannot finish, so a statement at the
    /// end of a file survives nowhere: it has no `}` after it to resynchronise on. That is the one
    /// statement anybody is looking at -- `h1(` with the cursor inside is what a buffer holds while
    /// signature help is the whole point -- and goto, hover and signature help were all answering
    /// nothing there.
    ///
    /// So a parse whose only complaint is at end of file is retried with the token mago says is
    /// missing. Driven by its own diagnosis rather than by guessing: `UnexpectedEndOfFile` carries
    /// the token kinds it wanted, and only a closing delimiter or a terminator is ever supplied --
    /// see [`supplied_for`]. The errors *reported* are always the first parse's, because the file as
    /// written is what has them.
    ///
    /// One consequence to know about: [`ParsedFile::source`] is then a few bytes longer than the
    /// text handed in. Those bytes are past every position a cursor can occupy, and the spans the
    /// tree carries are honest about the tree -- so a statement recovered this way ends just past the
    /// last byte of the file. [`ParsedFile::written_len`] is where the caller's text ends, and
    /// lowering holds every range it hands out to it: a reader above slices the caller's text.
    pub fn parse<'a>(&'a self, text: &str) -> ParsedFile<'a> {
        let first = self.parse_content(text);
        if first.program.errors.is_empty() {
            return first;
        }

        let reported = first.parsed_errors();
        let mut held = first;
        let mut supplied = String::new();
        for _ in 0..REPAIRS {
            let Some(token) = missing_at_the_end(held.program) else {
                break;
            };
            supplied.push_str(token);
            let attempt = self.parse_content(&format!("{text}{supplied}"));
            // Not worse, rather than strictly better. Supplying a `)` to `h4(` leaves exactly one
            // complaint where there was one -- the `;` it now wants instead -- and that step is the
            // whole of the progress: each pass writes a token mago itself asked for, and the bound
            // above is what stops the walk. Refusing an equal count refused every repair that takes
            // more than one token, which is most of them.
            if attempt.program.errors.len() > held.program.errors.len() {
                break;
            }
            let done = attempt.program.errors.is_empty();
            held = attempt;
            if done {
                break;
            }
        }

        held.reported = reported;
        held.written = text.len();
        held
    }

    /// One parse of exactly the text given.
    fn parse_content<'a>(&'a self, content: &str) -> ParsedFile<'a> {
        let program = mago_syntax::parser::parse_file_content_with_settings(
            &self.arena,
            // phrost keeps its own file identity; mago's would only travel inside spans that are
            // already known to belong to this file.
            MagoFileId::zero(),
            content.as_bytes(),
            mago_syntax::settings::ParserSettings {
                // Whitespace no comment precedes is a node of trivia per gap between two tokens,
                // and nothing here reads one: a docblock is found through the whitespace *after* a
                // comment, which is kept, and the formatter and the linter parse for themselves.
                elide_detached_whitespace: true,
                ..Default::default()
            },
        );

        // The parser copied the text into the arena, so borrow it from there. Slicing the caller's
        // buffer instead would tie every name in the tree to a lifetime the caller controls.
        // `alloc_slice_copy` copied `content.as_bytes()` verbatim, so this cannot fail. Asserting
        // the invariant beats a silent empty fallback, which would drop every docblock.
        let source = std::str::from_utf8(program.source_text)
            .expect("the arena holds a verbatim copy of a &str");
        let docblocks = DocblockIndex::build(
            source,
            program
                .trivia
                .iter()
                .filter(|trivia| trivia.kind == TriviaKind::DocBlockComment)
                .map(|trivia| (trivia.span, trivia.value)),
        );

        let mut parsed = ParsedFile {
            program,
            source,
            written: source.len(),
            docblocks,
            reported: Vec::new(),
        };
        parsed.reported = parsed.parsed_errors();
        // And what the parser accepted that the language does not -- see `refused`. Appended
        // rather than folded in, because one is what mago could not read and the other is what
        // PHP would not.
        parsed.reported.extend(crate::refused::refused_by_php(
            parsed.program,
            parsed.source,
        ));
        parsed
    }
}

/// How many tokens a repair may supply before giving up.
///
/// `$x->` inside a function wants a `;` and then a `}`; a half-written call wants a `)` and then a
/// `;`. Four is past anything one dropped statement asks for, and the bound is what keeps a file
/// mago cannot finish at all from being parsed over and over.
const REPAIRS: usize = 4;

/// The token mago says is missing at the end of the file, when it is one this may supply.
fn missing_at_the_end(program: &Program<'_>) -> Option<&'static str> {
    program.errors.iter().find_map(|error| match error {
        ParseError::UnexpectedEndOfFile(expected, ..) => {
            expected.kinds().iter().copied().find_map(supplied_for)
        }
        _ => None,
    })
}

/// The text of a token a repair may write, or `None` for one it may not.
///
/// Closing delimiters and the statement terminator, which between them are everything a *dropped*
/// statement lacks: the tail of a file has no `)`, `]`, `}` or `;` because somebody is still typing
/// it. Anything else mago asks for is content -- a name, an operand -- and inventing that would put
/// a declaration in the index the file does not contain.
///
/// `?>` is deliberately not here even though mago offers it beside `;`. It leaves PHP mode, so
/// everything after it would lex as inline HTML, and `;` is always the alternative.
fn supplied_for(kind: TokenKind) -> Option<&'static str> {
    Some(match kind {
        TokenKind::Semicolon => ";",
        TokenKind::RightParenthesis => ")",
        TokenKind::RightBracket => "]",
        TokenKind::RightBrace => "}",
        _ => return None,
    })
}

/// One parsed file: the tree, its text, and its docblocks.
///
/// Everything here borrows the [`ParseSession`]. Lowering consumes it in one pass and produces an
/// owned `FileHir`; nothing else should hold on to it.
#[derive(Debug)]
pub struct ParsedFile<'a> {
    program: &'a Program<'a>,
    source: &'a str,
    /// How much of [`Self::source`] the caller wrote, which is all of it unless a repair supplied
    /// the tokens a cut buffer lacks after it.
    written: usize,
    docblocks: DocblockIndex<'a>,
    /// The errors of the file *as written*, kept across a repair.
    reported: Vec<SyntaxError>,
}

impl<'a> ParsedFile<'a> {
    /// The parse tree. Only `phrost-lower` and the syntax-only feature crate may call this.
    pub fn program(&self) -> &'a Program<'a> {
        self.program
    }

    /// The source text, as the arena copy the tree's spans index into.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// The length of the text the caller handed in, which a repair may have written past.
    pub fn written_len(&self) -> usize {
        self.written
    }

    pub fn has_errors(&self) -> bool {
        !self.reported.is_empty()
    }

    /// The right-hand sides of `instanceof` written without the parentheses PHP needs there.
    ///
    /// One of the shapes [`Self::errors`] reports, as ranges, for the formatter: mago's drops those
    /// parentheses, and putting them back from this list keeps one rule in one place.
    pub fn bare_instanceof_operands(&self) -> Vec<phrost_model::TextRange> {
        crate::refused::bare_instanceof_operands(self.program, self.source)
    }

    /// Syntax errors, as phrost values with no tie to the parser.
    ///
    /// The file as written, always. A repair changes what the *tree* holds and never what is wrong
    /// with the source -- a missing `;` is a missing `;` whether or not the statement above it was
    /// recovered.
    pub fn errors(&self) -> Vec<SyntaxError> {
        self.reported.clone()
    }

    /// This parse's own complaints, before a repair has anything to say about them.
    fn parsed_errors(&self) -> Vec<SyntaxError> {
        self.program
            .errors
            .iter()
            .map(SyntaxError::from_parse_error)
            .collect()
    }

    /// The docblock attached to a declaration starting at `decl_start`.
    ///
    /// Pass the first byte of the whole declaration, attributes included — see
    /// [`DocblockIndex::preceding`].
    pub fn docblock_before(&self, decl_start: TextSize) -> Option<Docblock<'a>> {
        self.docblocks.preceding(self.source, decl_start)
    }

    /// Every docblock in the file, in position order.
    pub fn docblocks(&self) -> &[Docblock<'a>] {
        self.docblocks.all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mago_syntax::cst::Statement;

    #[test]
    fn parses_a_class_and_reports_no_errors() {
        let session = ParseSession::new();
        let parsed = session.parse("<?php\nclass Repo { public function find(): void {} }\n");

        assert!(!parsed.has_errors());
        assert!(parsed.errors().is_empty());
        assert!(
            parsed
                .program()
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Class(_)))
        );
    }

    #[test]
    fn broken_source_still_yields_a_tree() {
        // Error tolerance is the whole reason there is no Result here: a buffer mid-edit is the
        // normal case, not an exception.
        let session = ParseSession::new();
        let parsed = session.parse("<?php\nclass Repo { public function \n");

        assert!(parsed.has_errors());
        assert!(!parsed.errors().is_empty());
        assert!(!parsed.errors()[0].message.is_empty());
    }

    /// The tail somebody is typing survives, and the file is still reported as broken.
    ///
    /// Four shapes, and the point of having all four is that each needs a different number of
    /// tokens: `h()` wants a `;`, `h(` wants a `)` and then a `;`, and a statement inside an
    /// unterminated function wants a `;` and then a `}` before mago will recover on the brace at
    /// all. The last is the one the repair cannot finish -- there is no member name to invent -- and
    /// it still recovers the *declaration*, which is what makes the parameter above it exist.
    #[test]
    fn a_tail_mago_dropped_is_recovered_and_still_reported_as_broken() {
        let session = ParseSession::new();

        for (source, wanted) in [
            ("<?php\nfunction h(int $a) {}\nh()", 1),
            ("<?php\nfunction h(int $a) {}\nh(", 1),
            ("<?php\nfunction h(int $a) {}\nh(1", 1),
        ] {
            let parsed = session.parse(source);
            let calls = parsed
                .program()
                .statements
                .iter()
                .filter(|statement| matches!(statement, Statement::Expression(_)))
                .count();
            assert_eq!(calls, wanted, "{source:?}");
            // The repair changes the tree and never the verdict on the source.
            assert!(parsed.has_errors(), "{source:?}");
        }
    }

    /// A tail nothing can finish is left alone rather than repaired into something else.
    ///
    /// `$x =` wants an operand, and an operand is content. Supplying one would put a value in the
    /// index that the file does not contain, so the bound is what a *closing* token is -- see
    /// `supplied_for`.
    #[test]
    fn a_tail_missing_content_rather_than_a_terminator_is_not_invented() {
        let session = ParseSession::new();
        let parsed = session.parse("<?php\n$x =");

        assert!(parsed.has_errors());
        assert_eq!(
            parsed
                .program()
                .statements
                .iter()
                .filter(|statement| matches!(statement, Statement::Expression(_)))
                .count(),
            0
        );
    }

    #[test]
    fn a_docblock_attaches_to_the_declaration_below_it() {
        let session = ParseSession::new();
        let source = "<?php\n/** Repository. */\nclass Repo {}\n";
        let parsed = session.parse(source);

        let class_start = TextSize::from(
            u32::try_from(source.find("class Repo").expect("fixture has the class")).expect("fits"),
        );
        let docblock = parsed
            .docblock_before(class_start)
            .expect("the comment directly above should attach");
        assert_eq!(docblock.text, "/** Repository. */");
    }

    #[test]
    fn a_docblock_separated_by_code_does_not_attach() {
        let session = ParseSession::new();
        let source = "<?php\n/** Not about Repo. */\n$x = 1;\nclass Repo {}\n";
        let parsed = session.parse(source);

        let class_start = TextSize::from(
            u32::try_from(source.find("class Repo").expect("fixture has the class")).expect("fits"),
        );
        assert_eq!(parsed.docblock_before(class_start), None);
    }

    #[test]
    fn only_docblock_comments_count() {
        // `/* ... */` and `// ...` are comments but not documentation, and treating them as
        // documentation would put stray remarks into hover for the next declaration.
        let session = ParseSession::new();
        let source = "<?php\n/* plain block */\n// line\nclass Repo {}\n";
        let parsed = session.parse(source);

        let class_start = TextSize::from(
            u32::try_from(source.find("class Repo").expect("fixture has the class")).expect("fits"),
        );
        assert_eq!(parsed.docblock_before(class_start), None);
    }

    #[test]
    fn the_nearest_docblock_wins() {
        let session = ParseSession::new();
        let source = "<?php\n/** For A. */\nclass A {}\n/** For B. */\nclass B {}\n";
        let parsed = session.parse(source);

        let b_start = TextSize::from(
            u32::try_from(source.find("class B").expect("fixture has B")).expect("fits"),
        );
        assert_eq!(
            parsed.docblock_before(b_start).map(|block| block.text),
            Some("/** For B. */")
        );
    }
}
