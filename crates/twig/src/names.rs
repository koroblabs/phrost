//! The templates a template names: what it extends, includes, embeds and imports.
//!
//! A name is read only where it is *written*: a string literal the loader is handed as it stands.
//! `{% include 'card.html.twig' %}` names a template; `{% include 'cards/' ~ kind %}` computes one,
//! and the `'cards/'` in it is not a name at all -- reading it as one would send a reader to a file
//! that does not exist and report a template that was never asked for. So an expression gives up its
//! literals only when it is nothing but them: one string, a list of candidates, or the arms of a
//! condition, each of which the loader may be handed whole.

use std::ops::Range;

use crate::lexer::{Token, TokenKind, tokens};

/// One template name written in a template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateName {
    /// The name as the loader receives it, with the literal's escapes undone.
    pub name: String,
    /// The literal's bytes in the template, quotes included.
    pub literal: Range<usize>,
    /// The name's own bytes, between the quotes: what a reader underlines and what a completion
    /// replaces. A literal still being typed has no closing quote, and then everything after the
    /// opening one is the name.
    pub written: Range<usize>,
    pub kind: NameKind,
    /// Whether the template says a missing one is allowed.
    ///
    /// `ignore missing` says so outright. A list says so of each candidate in it -- the loader takes
    /// the first that exists, so only all of them missing is a mistake.
    pub may_be_missing: bool,
}

/// Which construct a name is written in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NameKind {
    Extends,
    Include,
    Embed,
    Import,
    From,
    Use,
    /// `include('...')`, the function.
    IncludeFunction,
    /// `source('...')`, which reads a template's text without rendering it.
    SourceFunction,
    /// `block('name', '...')`, which renders one block of another template.
    BlockFunction,
}

/// The namespace a name is written in, when it is written in one: `@node` holds `@node/node.html.twig`.
///
/// Twig's filesystem loader's own spelling, which every loader built on it shares. Without the `@`,
/// and up to the first separator, which is how the loader splits it.
pub fn namespace_of(name: &str) -> Option<&str> {
    let rest = name.strip_prefix('@')?;
    let (namespace, _) = rest.split_once('/')?;
    (!namespace.is_empty()).then_some(namespace)
}

/// Every template name written in `text`, in order.
pub fn template_names(text: &str) -> Vec<TemplateName> {
    let tokens = tokens(text)
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    for tag in tags(&tokens) {
        let reader = Reader { text, tag };
        reader.statement(&mut found);
        reader.calls(&mut found);
    }
    found
}

/// One tag's tokens, its opener first and its closer -- when it has one -- last.
pub(crate) fn tags(tokens: &[Token]) -> impl Iterator<Item = &[Token]> {
    let mut rest = tokens;
    std::iter::from_fn(move || {
        let (first, after) = rest.split_first()?;
        debug_assert!(matches!(
            first.kind,
            TokenKind::PrintOpen | TokenKind::StatementOpen
        ));
        // To the closer, or to the next opener when a tag is never closed: a template being typed is
        // still worth reading past the tag it is typed in.
        let end = after
            .iter()
            .position(|token| {
                matches!(
                    token.kind,
                    TokenKind::Close | TokenKind::PrintOpen | TokenKind::StatementOpen
                )
            })
            .map_or(rest.len(), |at| match after[at].kind {
                TokenKind::Close => at + 2,
                _ => at + 1,
            });
        let (tag, next) = rest.split_at(end);
        rest = next;
        Some(tag)
    })
}

struct Reader<'a> {
    text: &'a str,
    tag: &'a [Token],
}

impl Reader<'_> {
    fn word(&self, token: &Token) -> &str {
        &self.text[token.range.clone()]
    }

    fn is_word(&self, token: Option<&Token>, word: &str) -> bool {
        token.is_some_and(|token| token.kind == TokenKind::Name && self.word(token) == word)
    }

    fn is_punctuation(&self, token: Option<&Token>, mark: &str) -> bool {
        token.is_some_and(|token| token.kind == TokenKind::Punctuation && self.word(token) == mark)
    }

    /// The names a statement tag's own expression is made of: `{% include '...' %}`.
    fn statement(&self, found: &mut Vec<TemplateName>) {
        let Some((open, body)) = self.tag.split_first() else {
            return;
        };
        if open.kind != TokenKind::StatementOpen {
            return;
        }
        let Some((tag, rest)) = body.split_first() else {
            return;
        };
        if tag.kind != TokenKind::Name {
            return;
        }
        let kind = match self.word(tag) {
            "extends" => NameKind::Extends,
            "include" => NameKind::Include,
            "embed" => NameKind::Embed,
            "import" => NameKind::Import,
            "from" => NameKind::From,
            "use" => NameKind::Use,
            _ => return,
        };
        let rest = match rest.last() {
            Some(last) if last.kind == TokenKind::Close => &rest[..rest.len() - 1],
            _ => rest,
        };
        // The expression runs to the first word at its own level that begins what follows it:
        // `with` and `only` an include's variables, `ignore missing`, `as` and `import` an import's.
        let mut depth = 0_usize;
        let mut end = rest.len();
        for (at, token) in rest.iter().enumerate() {
            match token.kind {
                TokenKind::Punctuation => match self.word(token) {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth = depth.saturating_sub(1),
                    _ => {}
                },
                TokenKind::Name
                    if depth == 0
                        && matches!(
                            self.word(token),
                            "with" | "only" | "ignore" | "as" | "import"
                        ) =>
                {
                    end = at;
                    break;
                }
                _ => {}
            }
        }
        let ignored = rest[end..].windows(2).any(|pair| {
            self.is_word(pair.first(), "ignore") && self.is_word(pair.get(1), "missing")
        });
        for (literal, candidate) in self.literals(&rest[..end]) {
            self.push(found, literal, kind, ignored || candidate);
        }
    }

    /// The names handed to a function that takes one: `include('...')`, `source('...')`,
    /// `block('title', '...')`.
    ///
    /// Anywhere in any tag, since a call is an expression: `{{ include(...) }}` and
    /// `{% set card = include(...) %}` are the same call.
    fn calls(&self, found: &mut Vec<TemplateName>) {
        for (at, token) in self.tag.iter().enumerate() {
            if token.kind != TokenKind::Name || !self.is_punctuation(self.tag.get(at + 1), "(") {
                continue;
            }
            // A statement's own word is the statement, `{% include(...) %}` included.
            if at == 1 && self.tag[0].kind == TokenKind::StatementOpen {
                continue;
            }
            // A method of something, not the function: `loader.source(...)`.
            if let Some(before) = at.checked_sub(1).and_then(|before| self.tag.get(before))
                && matches!(self.word(before), "." | "?.")
            {
                continue;
            }
            let (kind, template_at, named, missing_at) = match self.word(token) {
                "include" => (NameKind::IncludeFunction, 0, "template", 3),
                "source" => (NameKind::SourceFunction, 0, "name", 1),
                "block" => (NameKind::BlockFunction, 1, "template", usize::MAX),
                _ => continue,
            };
            let arguments = self.arguments(at + 2);
            let Some(template) = argument(&arguments, template_at, named, self) else {
                continue;
            };
            let ignored = argument(&arguments, missing_at, "ignore_missing", self)
                .is_some_and(|value| value.len() == 1 && self.is_word(value.first(), "true"));
            for (literal, candidate) in self.literals(template) {
                self.push(found, literal, kind, ignored || candidate);
            }
        }
    }

    /// The arguments of a call whose first token after `(` is at `from`, split at its own commas.
    fn arguments(&self, from: usize) -> Vec<&[Token]> {
        let mut out = Vec::new();
        let mut depth = 0_usize;
        let mut start = from;
        for (at, token) in self.tag.iter().enumerate().skip(from) {
            if token.kind != TokenKind::Punctuation {
                continue;
            }
            match self.word(token) {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" if depth == 0 => {
                    out.push(&self.tag[start..at]);
                    return out;
                }
                ")" | "]" | "}" => depth -= 1,
                "," if depth == 0 => {
                    out.push(&self.tag[start..at]);
                    start = at + 1;
                }
                _ => {}
            }
        }
        // A call still being typed: what there is of it.
        out.push(&self.tag[start.min(self.tag.len())..]);
        out
    }

    /// The literals an expression is made of when it is nothing but literals, each with whether it
    /// is one candidate among several.
    fn literals<'t>(&self, expression: &'t [Token]) -> Vec<(&'t Token, bool)> {
        match expression {
            [only] => match only.kind {
                TokenKind::String(quoted) if !quoted.interpolated => vec![(only, false)],
                _ => Vec::new(),
            },
            // Brackets around the whole of it say nothing.
            [open, inner @ .., close]
                if self.word(open) == "("
                    && self.word(close) == ")"
                    && self.encloses(expression) =>
            {
                self.literals(inner)
            }
            // A list of candidates, the first of which that exists is loaded.
            [open, inner @ .., close]
                if self.word(open) == "["
                    && self.word(close) == "]"
                    && self.encloses(expression) =>
            {
                let items = inner
                    .split(|token| self.is_punctuation(Some(token), ","))
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>();
                let several = items.len() > 1;
                let mut found = Vec::new();
                for item in items {
                    let [literal] = item else {
                        return Vec::new();
                    };
                    if !matches!(literal.kind, TokenKind::String(quoted) if !quoted.interpolated) {
                        return Vec::new();
                    }
                    found.push((literal, several));
                }
                found
            }
            _ => self.arms(expression),
        }
    }

    /// The arms of a condition at the expression's own level: `ajax ? 'a.twig' : 'b.twig'`.
    ///
    /// The condition binds loosest of every Twig operator, so whatever is before the `?` is the
    /// condition however it is written, and each arm is an expression in its own right.
    fn arms<'t>(&self, expression: &'t [Token]) -> Vec<(&'t Token, bool)> {
        let mut depth = 0_usize;
        let mut question = None;
        for (at, token) in expression.iter().enumerate() {
            if token.kind != TokenKind::Punctuation {
                continue;
            }
            match self.word(token) {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                "?" if depth == 0 && question.is_none() => question = Some(at),
                ":" if depth == 0 => {
                    if let Some(question) = question {
                        let mut found = self.literals(&expression[question + 1..at]);
                        found.extend(self.literals(&expression[at + 1..]));
                        return found;
                    }
                }
                _ => {}
            }
        }
        Vec::new()
    }

    /// Whether the brackets opening and closing `expression` are one pair, and not `(a) ~ (b)`.
    fn encloses(&self, expression: &[Token]) -> bool {
        let mut depth = 0_usize;
        for (at, token) in expression.iter().enumerate() {
            if token.kind != TokenKind::Punctuation {
                continue;
            }
            match self.word(token) {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 && at + 1 != expression.len() {
                        return false;
                    }
                }
                _ => {}
            }
        }
        true
    }

    fn push(&self, found: &mut Vec<TemplateName>, literal: &Token, kind: NameKind, missing: bool) {
        let terminated = matches!(literal.kind, TokenKind::String(quoted) if quoted.terminated);
        let start = literal.range.start + 1;
        let end = match terminated {
            true => literal.range.end - 1,
            false => literal.range.end,
        };
        found.push(TemplateName {
            name: unescape(&self.text[start..end]),
            literal: literal.range.clone(),
            written: start..end,
            kind,
            may_be_missing: missing,
        });
    }
}

/// One argument of a call, by position or by name: `include('a', ignore_missing = true)`.
fn argument<'t>(
    arguments: &[&'t [Token]],
    position: usize,
    name: &str,
    reader: &Reader<'_>,
) -> Option<&'t [Token]> {
    let named = arguments.iter().find_map(|argument| match *argument {
        [label, mark, value @ ..]
            if label.kind == TokenKind::Name
                && reader.word(label) == name
                && matches!(reader.word(mark), "=" | ":") =>
        {
            Some(value)
        }
        _ => None,
    });
    named.or_else(|| {
        // Positional arguments come before named ones, so the position counts only those.
        arguments
            .iter()
            .take_while(|argument| {
                !matches!(argument, [label, mark, ..]
                    if label.kind == TokenKind::Name && matches!(reader.word(mark), "=" | ":"))
            })
            .nth(position)
            .copied()
            .filter(|argument| !argument.is_empty())
    })
}

/// A literal's text as the loader receives it: Twig undoes a backslash before any character.
fn unescape(written: &str) -> String {
    if !written.contains('\\') {
        return written.to_owned();
    }
    let mut out = String::with_capacity(written.len());
    let mut chars = written.chars();
    while let Some(char) = chars.next() {
        match char {
            '\\' => out.extend(chars.next()),
            _ => out.push(char),
        }
    }
    out
}
