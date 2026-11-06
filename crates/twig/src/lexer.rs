//! A template's tags, as the tokens Twig's own lexer would make of them.
//!
//! Only what is inside a tag is a token, and a comment is one token whole. The markup between tags is
//! not -- nothing read from a template is about it, and a token per run of HTML is most of a
//! template's bytes spent on nothing. A `verbatim` block is markup by Twig's own rule and is skipped whole.
//!
//! The rules are Twig's, so that a template this reads differently from Twig is a bug here and not a
//! dialect: a tag closes at its own closer only when every bracket opened inside it has closed, a
//! double-quoted string may hold `#{...}` whose contents are an expression of their own, and a string
//! may run over several lines. A template that does not compile still lexes -- an unclosed tag runs to
//! the end of the text and an unclosed string to the end of the tag's text -- because the template
//! being typed is the one a reader is asked about most.

use std::ops::Range;

/// One token inside a tag.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Token {
    pub kind: TokenKind,
    /// The token's bytes in the template.
    pub range: Range<usize>,
}

/// What a token is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    /// `{# ... #}`, whole. A comment is markup to Twig, and a token here only because a type is
    /// sometimes declared in one: `{# @var post \App\Entity\Post #}`.
    Comment,
    /// `{{`, opening a tag that prints an expression. With its whitespace control, when it has one.
    PrintOpen,
    /// `{%`, opening a tag that runs a statement.
    StatementOpen,
    /// `}}` or `%}`, with whatever whitespace control is written before it.
    Close,
    /// A word: a variable, a tag's or a function's name, an operator spelled as one (`not`, `in`).
    Name,
    Number,
    String(Quoted),
    /// One of `( ) [ ] { } ? : . , |`.
    Punctuation,
    /// Every other operator: `~`, `==`, `..`, `??`, `=`.
    Operator,
}

/// What a string literal holds besides its text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Quoted {
    /// Whether a double-quoted string holds `#{...}`, so that what it says is computed rather than
    /// written.
    pub interpolated: bool,
    /// Whether the closing quote is there.
    pub terminated: bool,
}

/// Every token inside every tag of `text`, in order.
pub fn tokens(text: &str) -> Vec<Token> {
    let mut lexer = Lexer {
        bytes: text.as_bytes(),
        at: 0,
        out: Vec::new(),
    };
    lexer.run();
    lexer.out
}

struct Lexer<'a> {
    bytes: &'a [u8],
    at: usize,
    out: Vec<Token>,
}

/// Which closer ends the tag being lexed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Closer {
    Print,
    Statement,
}

impl Closer {
    fn second(self) -> u8 {
        match self {
            Self::Print => b'}',
            Self::Statement => b'%',
        }
    }
}

impl Lexer<'_> {
    fn run(&mut self) {
        while let Some(open) = self.next_opener() {
            let kind = self.bytes[open + 1];
            let mut after = open + 2;
            // Whitespace control belongs to the delimiter: `{{-` trims the markup before it.
            if matches!(self.bytes.get(after), Some(b'-' | b'~')) {
                after += 1;
            }
            match kind {
                b'#' => {
                    self.at = self.comment_end(after);
                    self.push(TokenKind::Comment, open..self.at);
                }
                b'{' => {
                    self.push(TokenKind::PrintOpen, open..after);
                    self.at = after;
                    self.expression(Closer::Print);
                }
                _ => {
                    if let Some(end) = self.verbatim_end(after) {
                        self.at = end;
                        continue;
                    }
                    self.push(TokenKind::StatementOpen, open..after);
                    self.at = after;
                    self.expression(Closer::Statement);
                }
            }
        }
    }

    /// Where the next tag or comment opens, from the current position.
    fn next_opener(&self) -> Option<usize> {
        let mut at = self.at;
        while at + 1 < self.bytes.len() {
            if self.bytes[at] == b'{' && matches!(self.bytes[at + 1], b'{' | b'%' | b'#') {
                return Some(at);
            }
            at += 1;
        }
        None
    }

    /// Past the `#}` closing a comment, or the end of the text when nothing closes it.
    fn comment_end(&self, from: usize) -> usize {
        find(self.bytes, from, b"#}").map_or(self.bytes.len(), |at| at + 2)
    }

    /// Past the end of a `verbatim` block, when the tag opening at `after` starts one.
    ///
    /// `raw` is the name Twig 1 gave it and Twig 3 still lexes. What is between the two tags is
    /// markup, delimiters included, so it is skipped rather than read.
    fn verbatim_end(&self, after: usize) -> Option<usize> {
        let word_at = skip_space(self.bytes, after);
        let word = word_end(self.bytes, word_at);
        let end_word = match &self.bytes[word_at..word] {
            b"verbatim" => &b"endverbatim"[..],
            b"raw" => &b"endraw"[..],
            _ => return None,
        };
        let mut close = skip_space(self.bytes, word);
        if matches!(self.bytes.get(close), Some(b'-' | b'~')) {
            close += 1;
        }
        if !self.bytes[close..].starts_with(b"%}") {
            return None;
        }
        let mut at = close + 2;
        while let Some(open) = find(self.bytes, at, b"{%") {
            let mut inner = open + 2;
            if matches!(self.bytes.get(inner), Some(b'-' | b'~')) {
                inner += 1;
            }
            let word_at = skip_space(self.bytes, inner);
            let word = word_end(self.bytes, word_at);
            if &self.bytes[word_at..word] == end_word {
                return Some(find(self.bytes, word, b"%}").map_or(self.bytes.len(), |at| at + 2));
            }
            at = open + 2;
        }
        Some(self.bytes.len())
    }

    /// The tokens of one tag, up to and including its closer.
    fn expression(&mut self, closer: Closer) {
        let mut brackets: Vec<u8> = Vec::new();
        loop {
            self.at = skip_space(self.bytes, self.at);
            let Some(&byte) = self.bytes.get(self.at) else {
                return;
            };
            // Twig looks for the closer only once every bracket has closed: `{{ {a: 1}}}` is a hash
            // and then the end of the tag.
            if brackets.is_empty()
                && let Some(end) = self.closer_at(closer)
            {
                self.push(TokenKind::Close, self.at..end);
                self.at = end;
                return;
            }
            let start = self.at;
            match byte {
                b'\'' | b'"' => {
                    let quoted = self.string(byte);
                    self.push(TokenKind::String(quoted), start..self.at);
                }
                b'0'..=b'9' => {
                    self.number();
                    self.push(TokenKind::Number, start..self.at);
                }
                _ if is_name_start(byte) => {
                    self.at = word_end(self.bytes, self.at);
                    self.push(TokenKind::Name, start..self.at);
                }
                b'(' | b'[' | b'{' => {
                    brackets.push(byte);
                    self.at += 1;
                    self.push(TokenKind::Punctuation, start..self.at);
                }
                b')' | b']' | b'}' => {
                    brackets.pop();
                    self.at += 1;
                    self.push(TokenKind::Punctuation, start..self.at);
                }
                _ => {
                    let kind = self.operator_or_punctuation();
                    self.push(kind, start..self.at);
                }
            }
        }
    }

    /// Where the tag's closer ends, when one starts at the current position.
    fn closer_at(&self, closer: Closer) -> Option<usize> {
        let mut at = self.at;
        if matches!(self.bytes.get(at), Some(b'-' | b'~')) {
            at += 1;
        }
        (self.bytes.get(at) == Some(&closer.second()) && self.bytes.get(at + 1) == Some(&b'}'))
            .then_some(at + 2)
    }

    /// A string literal opening at the current position, leaving the position past it.
    fn string(&mut self, quote: u8) -> Quoted {
        let mut interpolated = false;
        let mut at = self.at + 1;
        while let Some(&byte) = self.bytes.get(at) {
            match byte {
                b'\\' => at += 2,
                _ if byte == quote => {
                    self.at = at + 1;
                    return Quoted {
                        interpolated,
                        terminated: true,
                    };
                }
                b'#' if quote == b'"' && self.bytes.get(at + 1) == Some(&b'{') => {
                    interpolated = true;
                    at = self.interpolation_end(at + 2);
                }
                _ => at += 1,
            }
        }
        self.at = self.bytes.len();
        Quoted {
            interpolated,
            terminated: false,
        }
    }

    /// Past the `}` closing an interpolation whose expression starts at `from`.
    ///
    /// The expression is Twig and may hold braces and strings of its own: `"#{ {a: 'b'}.a }"`.
    fn interpolation_end(&self, from: usize) -> usize {
        let mut depth = 0_usize;
        let mut at = from;
        while let Some(&byte) = self.bytes.get(at) {
            match byte {
                b'{' => depth += 1,
                b'}' if depth == 0 => return at + 1,
                b'}' => depth -= 1,
                b'\'' | b'"' => {
                    at += 1;
                    while let Some(&inner) = self.bytes.get(at) {
                        match inner {
                            b'\\' => at += 1,
                            _ if inner == byte => break,
                            _ => {}
                        }
                        at += 1;
                    }
                }
                _ => {}
            }
            at += 1;
        }
        self.bytes.len()
    }

    /// A number opening at the current position: `12`, `1_000`, `3.14`, `1e10`.
    fn number(&mut self) {
        let digits = |bytes: &[u8], mut at: usize| {
            while bytes
                .get(at)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
            {
                at += 1;
            }
            at
        };
        self.at = digits(self.bytes, self.at);
        // A fraction only when a digit follows the dot: `1..5` is a range.
        if self.bytes.get(self.at) == Some(&b'.')
            && self.bytes.get(self.at + 1).is_some_and(u8::is_ascii_digit)
        {
            self.at = digits(self.bytes, self.at + 1);
        }
        if matches!(self.bytes.get(self.at), Some(b'e' | b'E')) {
            let mut at = self.at + 1;
            if matches!(self.bytes.get(at), Some(b'+' | b'-')) {
                at += 1;
            }
            if self.bytes.get(at).is_some_and(u8::is_ascii_digit) {
                self.at = digits(self.bytes, at);
            }
        }
    }

    /// The operator or punctuation at the current position, leaving the position past it.
    ///
    /// The two-byte operators are matched first, as Twig matches the longest: `..` is a range and
    /// not two attribute dots, `??` and `?:` are operators and not a condition's `?`.
    fn operator_or_punctuation(&mut self) -> TokenKind {
        const PAIRS: &[&[u8; 2]] = &[
            b"..", b"??", b"?:", b"?.", b"==", b"!=", b"<=", b">=", b"**", b"//", b"=>",
        ];
        let rest = &self.bytes[self.at..];
        if PAIRS.iter().any(|pair| rest.starts_with(*pair)) {
            self.at += 2;
            return TokenKind::Operator;
        }
        // Never inside a character: every byte of a multi-byte one starts a name.
        let byte = rest[0];
        self.at += 1;
        match byte {
            b'?' | b':' | b'.' | b',' | b'|' => TokenKind::Punctuation,
            _ => TokenKind::Operator,
        }
    }

    fn push(&mut self, kind: TokenKind, range: Range<usize>) {
        self.out.push(Token { kind, range });
    }
}

/// Whether a byte can start a name. Twig's rule, under which any byte of a multi-byte character can.
fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

/// Past the name starting at `from`.
fn word_end(bytes: &[u8], mut from: usize) -> usize {
    while bytes
        .get(from)
        .is_some_and(|byte| is_name_start(*byte) || byte.is_ascii_digit())
    {
        from += 1;
    }
    from
}

fn skip_space(bytes: &[u8], mut from: usize) -> usize {
    while bytes.get(from).is_some_and(u8::is_ascii_whitespace) {
        from += 1;
    }
    from
}

fn find(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| from + at)
}
