//! What a template reads and what it binds: every variable an expression starts from with what is
//! read off it, and every name a tag binds with where it can be seen.
//!
//! The two halves a reader of a template's variables needs and nothing more. `{{ panel.label }}` is
//! a read of `panel` and then of `label` off whatever `panel` holds; `{% for panel in panels %}`
//! binds `panel` until its `endfor` to an element of whatever `panels` holds. What each holds is
//! for someone who knows PHP's types -- here it is only said which expression each name is bound
//! to, as far as an answer about its type could depend on it.
//!
//! # Not a parser of the whole language
//!
//! An expression is read for the chains in it and summarised as a [`Value`], and that summary stops
//! at what a type could be read from: a chain and the filters after it, a literal, a list or a hash
//! of those. An operator between two of them makes something no rule here types, and is
//! [`Value::Unknown`] -- except the three whose result is one kind whatever the operands are.

use std::ops::Range;

use crate::lexer::{Token, TokenKind, tokens};

/// Everything one template reads and binds.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Reading {
    /// Every chain, in the order the template writes them.
    pub chains: Vec<Chain>,
    /// Every name a tag or a declaration binds, in the order they are written.
    pub bindings: Vec<Binding>,
    /// The bodies of macros, where nothing outside is visible: a macro sees its arguments and the
    /// globals, and not the template it is written in.
    pub isolated: Vec<Range<usize>>,
    /// Every place this template hands its variables to another: an `include`, an `embed`, the
    /// `include()` function, and an `extends` -- whose parent renders with the child's variables.
    pub includes: Vec<Include>,
    /// Every filter, function and test the template calls by name, in order.
    pub called: Vec<Called>,
}

/// A filter, function or test called by name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Called {
    /// The name's bytes: both words of `same as`, and nothing -- where it is still to be typed --
    /// right after a `|`.
    pub name: Range<usize>,
    pub kind: CalledKind,
}

/// Which of the three a name is called as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CalledKind {
    Filter,
    Function,
    Test,
}

/// One place a template renders another with its variables.
///
/// Which template it is, is the name reader's question: the literal naming it is inside `range`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Include {
    /// The tag, or the call, whole.
    pub range: Range<usize>,
    /// What `with` -- or the call's second argument -- hands over besides.
    pub with: Option<Value>,
    /// Whether the template rendered sees nothing but that: `only`.
    pub only: bool,
}

/// A variable, and what is read off it in turn: `panel.customer.node`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chain {
    /// The variable's name.
    pub root: Range<usize>,
    pub steps: Vec<Step>,
}

/// One thing read off what a chain holds so far.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    /// The name read: an attribute's, or a key's between its quotes. Empty, at the position after
    /// the dot, for an attribute still to be typed.
    pub name: Range<usize>,
    pub kind: StepKind,
}

/// How a step reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepKind {
    /// `a.b`: Twig's attribute, which is a key of an array, or a property or a method of an object,
    /// whichever the value has.
    Attribute,
    /// `a.b(...)`: a method, called.
    Method,
    /// `a['b']` or `a[0]`: a key, written as a literal.
    Key,
    /// `a[expression]`: some key or other.
    Index,
}

/// A name bound by the template, where, and to what.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Binding {
    pub name: String,
    /// Where the name is written in the tag binding it.
    pub range: Range<usize>,
    /// Where the binding can be read: from the end of the tag binding it to the end of whatever
    /// holds that tag.
    pub visible: Range<usize>,
    pub kind: BindingKind,
}

/// What a binding holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BindingKind {
    /// `{% for item in items %}`: an element of what the value holds.
    Element(Value),
    /// `{% for key, item in items %}`: a key of what the value holds.
    Key(Value),
    /// `loop`, inside a `for`.
    Loop,
    /// `{% set x = ... %}`, or a key of `{% with {...} %}`.
    Assigned(Value),
    /// `{% set x %}...{% endset %}`: what the body renders.
    Captured,
    /// A macro's argument, or an arrow function's.
    Parameter,
    /// `{% import '...' as forms %}`: the macros of a template.
    Macros,
    /// `{% from '...' import field %}`: one macro.
    Macro,
    /// A type written for it: `{% types {post: '\App\Entity\Post'} %}` or
    /// `{# @var post \App\Entity\Post #}`. The type as written.
    Declared(String),
}

/// An expression, summarised as far as its type could be read from it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Value {
    /// One of [`Reading::chains`], and the filters applied to it after, by name.
    Chain {
        chain: usize,
        filters: Vec<String>,
    },
    String,
    Number,
    Bool,
    Null,
    /// `[...]`, with its elements.
    List(Vec<Value>),
    /// `{...}`, with its keys.
    Hash(Vec<(String, Value)>),
    /// A function's call, by the function's name: `path('home')`.
    Call(String),
    /// Something other than a chain, and the filters applied to it after: `'Hello'|t`.
    Filtered {
        base: Box<Value>,
        filters: Vec<String>,
    },
    Unknown,
}

impl Reading {
    /// The chain at `offset`, and which part of it: `None` for its root, or the step.
    ///
    /// The narrowest part holding it, because a computed key holds a chain of its own: in
    /// `rows[i].name` the cursor on `i` is on `i`, not on the step `[i]` it is inside.
    pub fn chain_at(&self, offset: usize) -> Option<(&Chain, Option<usize>)> {
        self.chains
            .iter()
            .flat_map(|chain| {
                std::iter::once((chain, None, &chain.root)).chain(
                    chain
                        .steps
                        .iter()
                        .enumerate()
                        .map(move |(at, step)| (chain, Some(at), &step.name)),
                )
            })
            .filter(|(_, _, range)| covers(range, offset))
            .min_by_key(|(_, _, range)| range.len())
            .map(|(chain, step, _)| (chain, step))
    }

    /// The binding a read of `name` at `offset` sees: the latest one visible there.
    pub fn binding_of(&self, name: &str, offset: usize) -> Option<&Binding> {
        self.bindings
            .iter()
            .filter(|binding| binding.name == name && binding.visible.contains(&offset))
            .max_by_key(|binding| binding.range.start)
    }

    /// Every name visible at `offset`, each once, the latest binding of it.
    pub fn visible_at(&self, offset: usize) -> Vec<&Binding> {
        let mut found: Vec<&Binding> = Vec::new();
        for binding in &self.bindings {
            if !binding.visible.contains(&offset) {
                continue;
            }
            match found.iter_mut().find(|held| held.name == binding.name) {
                Some(held) if held.range.start < binding.range.start => *held = binding,
                Some(_) => {}
                None => found.push(binding),
            }
        }
        found
    }

    /// The filter, function or test whose name is at `offset`.
    pub fn called_at(&self, offset: usize) -> Option<&Called> {
        self.called
            .iter()
            .find(|called| covers(&called.name, offset))
    }

    /// Whether `offset` is inside a macro, where only its own arguments are visible.
    pub fn is_isolated(&self, offset: usize) -> bool {
        self.isolated.iter().any(|body| body.contains(&offset))
    }
}

/// Whether a range holds an offset, its end included: the cursor right after a name is on it.
fn covers(range: &Range<usize>, offset: usize) -> bool {
    range.start <= offset && offset <= range.end
}

/// What `text` reads and binds.
pub fn read(text: &str) -> Reading {
    let tokens = tokens(text);
    let mut reader = Reader {
        text,
        reading: Reading::default(),
        frames: Vec::new(),
        file: Vec::new(),
    };
    let mut rest = &tokens[..];
    while let Some((first, after)) = rest.split_first() {
        if first.kind == TokenKind::Comment {
            reader.comment(first);
            rest = after;
            continue;
        }
        let end = after
            .iter()
            .position(|token| {
                matches!(
                    token.kind,
                    TokenKind::Close
                        | TokenKind::PrintOpen
                        | TokenKind::StatementOpen
                        | TokenKind::Comment
                )
            })
            .map_or(rest.len(), |at| match after[at].kind {
                TokenKind::Close => at + 2,
                _ => at + 1,
            });
        let (tag, next) = rest.split_at(end);
        reader.tag(tag);
        rest = next;
    }
    reader.finish()
}

/// A construct still open while the template is read: what a binding inside it is visible until
/// the end of.
struct Frame {
    tag: &'static str,
    /// The bindings visible to the end of this construct.
    bindings: Vec<usize>,
    /// Where the body began, for a macro's.
    body: usize,
    /// Whether a `set` inside this construct is visible only to its end.
    scopes: bool,
}

struct Reader<'a> {
    text: &'a str,
    reading: Reading,
    frames: Vec<Frame>,
    /// The bindings visible to the end of the template.
    file: Vec<usize>,
}

/// The constructs a template opens with a tag and closes with `end` and its name.
const CONTAINERS: &[(&str, bool)] = &[
    ("for", true),
    ("macro", true),
    ("with", true),
    ("block", true),
    ("embed", true),
    ("if", false),
    ("apply", false),
    ("autoescape", false),
    ("sandbox", false),
    ("spaceless", false),
    ("trans", false),
    ("cache", false),
    ("guard", false),
];

impl<'a> Reader<'a> {
    fn word(&self, token: &Token) -> &'a str {
        &self.text[token.range.clone()]
    }

    fn finish(mut self) -> Reading {
        let end = self.text.len();
        while let Some(frame) = self.frames.pop() {
            self.close(frame, end);
        }
        for at in std::mem::take(&mut self.file) {
            self.reading.bindings[at].visible.end = end;
        }
        self.reading
    }

    /// Ends what a construct bound, at `end`.
    fn close(&mut self, frame: Frame, end: usize) {
        for at in frame.bindings {
            self.reading.bindings[at].visible.end = end;
        }
        if frame.tag == "macro" {
            self.reading.isolated.push(frame.body..end);
        }
    }

    /// Adds a binding visible from `from` to the end of the innermost construct that scopes it.
    fn bind(&mut self, name: &Token, from: usize, kind: BindingKind) {
        let at = self.reading.bindings.len();
        self.reading.bindings.push(Binding {
            name: self.word(name).to_owned(),
            range: name.range.clone(),
            visible: from..from,
            kind,
        });
        match self.frames.iter_mut().rev().find(|frame| frame.scopes) {
            Some(frame) => frame.bindings.push(at),
            None => self.file.push(at),
        }
    }

    /// A comment declaring a variable's type, PhpStorm's way: `{# @var post \App\Post #}`, either
    /// order.
    fn comment(&mut self, token: &Token) {
        let text = self.word(token);
        let body = text
            .trim_start_matches("{#")
            .trim_end_matches("#}")
            .trim_matches(|c: char| c == '-' || c == '~' || c.is_whitespace());
        let Some(rest) = body.strip_prefix("@var") else {
            return;
        };
        let mut words = rest.split_whitespace();
        let (Some(first), Some(second)) = (words.next(), words.next()) else {
            return;
        };
        let is_name = |word: &str| {
            word.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        };
        let (name, written) = match (is_name(first), is_name(second)) {
            (true, false) => (first, second),
            (false, true) => (second, first),
            // Both look like names: a class with no namespace written second, the way the
            // convention puts the variable first.
            (true, true) => (first, second),
            (false, false) => return,
        };
        let offset = token.range.start + text.find(name).unwrap_or(0);
        let name_token = Token {
            kind: TokenKind::Name,
            range: offset..offset + name.len(),
        };
        self.bind(
            &name_token,
            token.range.end,
            BindingKind::Declared(written.to_owned()),
        );
    }

    fn tag(&mut self, tag: &[Token]) {
        let Some((open, body)) = tag.split_first() else {
            return;
        };
        let (body, end) = match body.last() {
            Some(last) if last.kind == TokenKind::Close => {
                (&body[..body.len() - 1], last.range.end)
            }
            _ => (
                body,
                body.last().map_or(open.range.end, |last| last.range.end),
            ),
        };
        match open.kind {
            TokenKind::PrintOpen => {
                let mut cursor = Cursor::new(body);
                while !cursor.done() {
                    self.expression(&mut cursor);
                    // Whatever an expression could not read is skipped a token at a time, so that
                    // what follows a mistake is still read.
                    if !cursor.done() {
                        cursor.next();
                    }
                }
            }
            _ => self.statement(open, body, end),
        }
    }

    fn statement(&mut self, open: &Token, body: &[Token], end: usize) {
        let Some((first, rest)) = body.split_first() else {
            return;
        };
        let word = match first.kind {
            TokenKind::Name => self.word(first),
            _ => "",
        };
        let mut cursor = Cursor::new(rest);
        match word {
            "for" => {
                // `for key, item in items` or `for item in items`, then an optional `if`.
                let mut names = Vec::new();
                while let Some(token) = cursor.peek() {
                    if token.kind == TokenKind::Name && self.word(token) == "in" {
                        cursor.next();
                        break;
                    }
                    if token.kind == TokenKind::Name {
                        names.push(token.clone());
                    }
                    cursor.next();
                }
                let value = self.until_word(&mut cursor, &["if"]);
                if !cursor.done() {
                    cursor.next();
                    self.rest(&mut cursor);
                }
                self.open_frame("for", end);
                match names.as_slice() {
                    [item] => self.bind(item, end, BindingKind::Element(value)),
                    [key, item, ..] => {
                        self.bind(key, end, BindingKind::Key(value.clone()));
                        self.bind(item, end, BindingKind::Element(value));
                    }
                    [] => {}
                }
                let loop_token = Token {
                    kind: TokenKind::Name,
                    range: first.range.clone(),
                };
                let at = self.reading.bindings.len();
                self.bind(&loop_token, end, BindingKind::Loop);
                self.reading.bindings[at].name = "loop".to_owned();
            }
            "set" => {
                let mut names = Vec::new();
                while let Some(token) = cursor.peek() {
                    match token.kind {
                        TokenKind::Name => names.push(token.clone()),
                        TokenKind::Operator if self.word(token) == "=" => break,
                        _ => {}
                    }
                    cursor.next();
                }
                if cursor.done() {
                    // `{% set x %}...{% endset %}`: bound when the body ends, and not inside it.
                    for name in names {
                        let at = self.reading.bindings.len();
                        self.bind(&name, end, BindingKind::Captured);
                        self.reading.bindings[at].visible = usize::MAX..usize::MAX;
                    }
                    self.frames.push(Frame {
                        tag: "set",
                        bindings: Vec::new(),
                        body: end,
                        scopes: false,
                    });
                    return;
                }
                cursor.next();
                let mut values = Vec::new();
                while !cursor.done() {
                    values.push(self.expression(&mut cursor));
                    match cursor.peek() {
                        Some(token) if self.word(token) == "," => {
                            cursor.next();
                        }
                        Some(_) => {
                            cursor.next();
                        }
                        None => {}
                    }
                }
                for (at, name) in names.iter().enumerate() {
                    let value = values.get(at).cloned().unwrap_or(Value::Unknown);
                    self.bind(name, end, BindingKind::Assigned(value));
                }
            }
            "endset" => {
                if let Some(at) = self.frames.iter().rposition(|frame| frame.tag == "set") {
                    self.frames.remove(at);
                    // The captured names start being visible here, to the end of what holds them --
                    // which closing that will set.
                    for binding in &mut self.reading.bindings {
                        if binding.kind == BindingKind::Captured
                            && binding.visible.start == usize::MAX
                        {
                            binding.visible = end..end;
                        }
                    }
                }
            }
            "with" => {
                let value = self.until_word(&mut cursor, &["only"]);
                self.open_frame("with", end);
                if let Value::Hash(entries) = value {
                    self.bind_hash(rest, &entries, end);
                }
            }
            "macro" => {
                // `macro name(a, b = 1)`: the arguments, bound inside the body only.
                self.open_frame("macro", end);
                if let Some(frame) = self.frames.last_mut() {
                    frame.body = end;
                }
                let mut depth = 0_usize;
                let mut expecting = false;
                while let Some(token) = cursor.next() {
                    match (token.kind, self.word(token)) {
                        (TokenKind::Punctuation, "(") => {
                            depth += 1;
                            expecting = depth == 1;
                        }
                        (TokenKind::Punctuation, ")") => depth = depth.saturating_sub(1),
                        (TokenKind::Punctuation, ",") if depth == 1 => expecting = true,
                        (TokenKind::Name, _) if depth == 1 && expecting => {
                            let token = token.clone();
                            self.bind(&token, end, BindingKind::Parameter);
                            expecting = false;
                        }
                        (TokenKind::Operator, "=") if depth == 1 => {
                            self.expression(&mut cursor);
                        }
                        _ => {}
                    }
                }
                let varargs = Token {
                    kind: TokenKind::Name,
                    range: first.range.clone(),
                };
                let at = self.reading.bindings.len();
                self.bind(&varargs, end, BindingKind::Parameter);
                self.reading.bindings[at].name = "varargs".to_owned();
            }
            "block" => {
                // The block's name, and in the short form what it prints.
                cursor.next();
                if !cursor.done() {
                    self.rest(&mut cursor);
                    return;
                }
                self.open_frame("block", end);
            }
            "import" => {
                self.until_word(&mut cursor, &["as"]);
                cursor.next();
                if let Some(name) = cursor.next().cloned() {
                    self.bind(&name, end, BindingKind::Macros);
                }
            }
            "from" => {
                self.until_word(&mut cursor, &["import"]);
                cursor.next();
                // `field`, or `field as input`: the last name of each is the one bound.
                let mut last: Option<Token> = None;
                while let Some(token) = cursor.next() {
                    match (token.kind, self.word(token)) {
                        (TokenKind::Punctuation, ",") => {
                            if let Some(name) = last.take() {
                                self.bind(&name, end, BindingKind::Macro);
                            }
                        }
                        (TokenKind::Name, "as") => {}
                        (TokenKind::Name, _) => last = Some(token.clone()),
                        _ => {}
                    }
                }
                if let Some(name) = last {
                    self.bind(&name, end, BindingKind::Macro);
                }
            }
            "types" => self.declared_types(rest, end),
            "include" | "embed" | "extends" | "use" => {
                // The template's name is the name reader's; what is read here is what the tag
                // hands over.
                self.until_word(&mut cursor, &["with", "only", "ignore"]);
                let mut with = None;
                let mut only = false;
                while let Some(token) = cursor.peek() {
                    match (token.kind, self.word(token)) {
                        (TokenKind::Name, "with") => {
                            cursor.next();
                            with = Some(self.expression(&mut cursor));
                        }
                        (TokenKind::Name, "only") => {
                            only = true;
                            cursor.next();
                        }
                        _ => {
                            cursor.next();
                        }
                    }
                }
                // `use` brings in blocks, not a template rendered with these variables.
                if word != "use" {
                    self.reading.includes.push(Include {
                        range: open.range.start..end,
                        with,
                        only,
                    });
                }
                if word == "embed" {
                    self.open_frame("embed", end);
                }
            }
            "apply" | "autoescape" | "sandbox" | "spaceless" | "guard" | "deprecated" | "flush"
            | "verbatim" | "else" => {
                // Filters, strategies and plain markers: no variable is read.
                if let Some(&(tag, _)) = CONTAINERS.iter().find(|(tag, _)| *tag == word) {
                    self.open_frame(tag, end);
                }
                if word == "else" {
                    self.else_branch(open.range.start);
                }
            }
            _ if word.starts_with("end") => {
                let closed = &word[3..];
                if let Some(at) = self.frames.iter().rposition(|frame| frame.tag == closed) {
                    while self.frames.len() > at {
                        if let Some(frame) = self.frames.pop() {
                            self.close(frame, open.range.start);
                        }
                    }
                }
            }
            _ => {
                if let Some(&(tag, _)) = CONTAINERS.iter().find(|(tag, _)| *tag == word) {
                    self.open_frame(tag, end);
                }
                self.rest(&mut cursor);
            }
        }
    }

    /// An `else` inside a `for` ends what the loop bound: the branch runs when there was nothing to
    /// loop over.
    fn else_branch(&mut self, at: usize) {
        if let Some(frame) = self.frames.last_mut()
            && frame.tag == "for"
        {
            for binding in std::mem::take(&mut frame.bindings) {
                self.reading.bindings[binding].visible.end = at;
            }
        }
    }

    fn open_frame(&mut self, tag: &'static str, body: usize) {
        let scopes = CONTAINERS
            .iter()
            .find(|(name, _)| *name == tag)
            .is_some_and(|(_, scopes)| *scopes);
        self.frames.push(Frame {
            tag,
            bindings: Vec::new(),
            body,
            scopes,
        });
    }

    /// The keys of `{% with {a: ...} %}`, each bound to its value.
    fn bind_hash(&mut self, tokens: &[Token], entries: &[(String, Value)], end: usize) {
        for (key, value) in entries {
            let Some(token) = tokens.iter().find(|token| {
                let word = self.word(token);
                word == key || word.trim_matches(['\'', '"']) == key
            }) else {
                continue;
            };
            let name = Token {
                kind: TokenKind::Name,
                range: match token.kind {
                    TokenKind::String(_) => token.range.start + 1..token.range.end - 1,
                    _ => token.range.clone(),
                },
            };
            self.bind(&name, end, BindingKind::Assigned(value.clone()));
        }
    }

    /// `{% types {post: '\App\Post', count?: 'int'} %}`: each key, declared with its string.
    fn declared_types(&mut self, tokens: &[Token], end: usize) {
        let mut at = 0;
        while at + 2 < tokens.len() {
            let key = &tokens[at];
            // `count?: 'int'` marks the key optional, and the lexer reads `?:` as one operator.
            let separator = match self.word(&tokens[at + 1]) {
                ":" | "?:" => Some(at + 1),
                "?" if tokens
                    .get(at + 2)
                    .is_some_and(|token| self.word(token) == ":") =>
                {
                    Some(at + 2)
                }
                _ => None,
            };
            let is_key = matches!(key.kind, TokenKind::Name | TokenKind::String(_));
            if is_key
                && let Some(next) = separator
                && let Some(value) = tokens.get(next + 1)
                && let TokenKind::String(_) = value.kind
            {
                let written = &self.text[value.range.start + 1..value.range.end - 1];
                let name = Token {
                    kind: TokenKind::Name,
                    range: match key.kind {
                        TokenKind::String(_) => key.range.start + 1..key.range.end - 1,
                        _ => key.range.clone(),
                    },
                };
                self.bind(
                    &name,
                    end,
                    BindingKind::Declared(written.replace("\\\\", "\\")),
                );
                at = next + 2;
                continue;
            }
            at += 1;
        }
    }

    /// Every expression to the end of a tag, read for its chains.
    fn rest(&mut self, cursor: &mut Cursor<'_>) {
        while !cursor.done() {
            self.expression(cursor);
            if !cursor.done() {
                cursor.next();
            }
        }
    }

    /// One expression, up to a word at its own level that begins what follows it.
    fn until_word(&mut self, cursor: &mut Cursor<'_>, words: &[&str]) -> Value {
        let value = self.expression(cursor);
        while let Some(token) = cursor.peek() {
            if token.kind == TokenKind::Name && words.contains(&self.word(token)) {
                break;
            }
            // Something the expression could not read; read past it rather than lose the rest.
            cursor.next();
            self.expression(cursor);
        }
        value
    }

    /// An expression: terms joined by binary operators.
    fn expression(&mut self, cursor: &mut Cursor<'_>) -> Value {
        let first = self.term(cursor);
        let mut operators: Vec<&str> = Vec::new();
        let mut terms = 1;
        while let Some(token) = cursor.peek() {
            let word = self.word(token);
            let binary = match token.kind {
                TokenKind::Operator => !matches!(word, "=" | "=>"),
                TokenKind::Punctuation => word == "?",
                TokenKind::Name => {
                    matches!(
                        word,
                        "and" | "or" | "xor" | "in" | "not" | "matches" | "starts" | "ends"
                    ) || (word == "b"
                        && cursor.peek_at(1).is_some_and(|next| self.word(next) == "-"))
                }
                _ => false,
            };
            if !binary {
                break;
            }
            cursor.next();
            match word {
                // `x is defined`, `x is not same as(y)`: a test, which is not a variable.
                "is" => {}
                "starts" | "ends" => {
                    cursor.next_word(self.text, "with");
                }
                "not" => {
                    cursor.next_word(self.text, "in");
                }
                "b" => {
                    // `b-and`, `b-or`, `b-xor`.
                    cursor.next();
                    cursor.next();
                }
                "?" => {
                    // A condition: whatever its arms are, the result is one or the other.
                    self.expression(cursor);
                    if cursor.peek().is_some_and(|token| self.word(token) == ":") {
                        cursor.next();
                    }
                }
                _ => {}
            }
            operators.push(word);
            self.term(cursor);
            terms += 1;
        }
        // `is` is lexed as a name and is handled where a term ends.
        if terms == 1 {
            return first;
        }
        if operators.iter().all(|operator| *operator == "~") {
            return Value::String;
        }
        if operators.iter().any(|operator| {
            matches!(
                *operator,
                "==" | "!="
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | "and"
                    | "or"
                    | "xor"
                    | "in"
                    | "not"
                    | "matches"
                    | "starts"
                    | "ends"
            )
        }) {
            return Value::Bool;
        }
        if operators
            .iter()
            .all(|operator| matches!(*operator, "+" | "-" | "*" | "/" | "//" | "%" | "**"))
        {
            return Value::Number;
        }
        Value::Unknown
    }

    /// One operand, with whatever is read off it and the filters and test after it.
    fn term(&mut self, cursor: &mut Cursor<'_>) -> Value {
        let Some(token) = cursor.peek().cloned() else {
            return Value::Unknown;
        };
        let word = self.word(&token);
        let mut value = match token.kind {
            TokenKind::String(_) => {
                cursor.next();
                self.postfix(cursor, None);
                Value::String
            }
            TokenKind::Number => {
                cursor.next();
                Value::Number
            }
            TokenKind::Operator if matches!(word, "-" | "+") => {
                cursor.next();
                self.term(cursor);
                Value::Number
            }
            TokenKind::Name if word == "not" => {
                cursor.next();
                self.term(cursor);
                Value::Bool
            }
            TokenKind::Name => self.name(cursor, &token),
            TokenKind::Punctuation => match word {
                "(" => {
                    cursor.next();
                    let inner = self.arrow_or_group(cursor);
                    self.postfix(cursor, None);
                    inner
                }
                "[" => {
                    cursor.next();
                    let items = self.items(cursor, "]");
                    self.postfix(cursor, None);
                    Value::List(items)
                }
                "{" => {
                    cursor.next();
                    let entries = self.hash(cursor);
                    self.postfix(cursor, None);
                    Value::Hash(entries)
                }
                _ => return Value::Unknown,
            },
            _ => return Value::Unknown,
        };
        // Filters bind to the operand before them: `a.b|upper ~ c` is `(a.b|upper) ~ c`.
        while let Some(bar) = cursor
            .peek()
            .filter(|token| self.word(token) == "|")
            .cloned()
        {
            cursor.next();
            let Some(filter) = cursor
                .peek()
                .filter(|token| token.kind == TokenKind::Name)
                .cloned()
            else {
                // A filter still to be typed: nothing after the bar yet.
                self.reading.called.push(Called {
                    name: bar.range.end..bar.range.end,
                    kind: CalledKind::Filter,
                });
                break;
            };
            cursor.next();
            self.reading.called.push(Called {
                name: filter.range.clone(),
                kind: CalledKind::Filter,
            });
            if cursor.peek().is_some_and(|token| self.word(token) == "(") {
                cursor.next();
                self.items(cursor, ")");
            }
            let name = self.word(&filter).to_owned();
            value = match value {
                Value::Chain { chain, mut filters } => {
                    filters.push(name);
                    Value::Chain { chain, filters }
                }
                Value::Filtered { base, mut filters } => {
                    filters.push(name);
                    Value::Filtered { base, filters }
                }
                base => Value::Filtered {
                    base: Box::new(base),
                    filters: vec![name],
                },
            };
        }
        // A test: `x is defined`, `x is not null`, `x is same as(y)`, `x is divisible by(3)`.
        if cursor
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Name && self.word(token) == "is")
        {
            cursor.next();
            cursor.next_word(self.text, "not");
            // One word, or the two of `same as` and `divisible by` -- and no more, or the `and`
            // after `x is defined` would be read as part of the test.
            if let Some(test) = cursor.next().cloned() {
                match self.word(&test) {
                    "same" => cursor.next_word(self.text, "as"),
                    "divisible" => cursor.next_word(self.text, "by"),
                    _ => {}
                }
                self.reading.called.push(Called {
                    name: test.range.start..cursor.previous_end().unwrap_or(test.range.end),
                    kind: CalledKind::Test,
                });
            }
            if cursor.peek().is_some_and(|token| self.word(token) == "(") {
                cursor.next();
                self.items(cursor, ")");
            }
            value = Value::Bool;
        }
        value
    }

    /// A name at the start of an operand: a literal, a function's call, or a variable and what is
    /// read off it.
    fn name(&mut self, cursor: &mut Cursor<'_>, token: &Token) -> Value {
        cursor.next();
        let word = self.word(token);
        match word.to_ascii_lowercase().as_str() {
            "true" | "false" => return Value::Bool,
            "null" | "none" => return Value::Null,
            _ => {}
        }
        // An arrow function's one argument: `v => v.active`.
        if cursor.peek().is_some_and(|next| self.word(next) == "=>") {
            cursor.next();
            let from = cursor
                .peek()
                .map_or(token.range.end, |next| next.range.start);
            let body_start = self.reading.bindings.len();
            self.bind(token, from, BindingKind::Parameter);
            self.expression(cursor);
            let to = cursor.previous_end().unwrap_or(from);
            self.reading.bindings[body_start].visible = from..to;
            self.unscope(body_start);
            return Value::Unknown;
        }
        if cursor.peek().is_some_and(|next| self.word(next) == "(") {
            // A function, which is not a variable: `range(1, 5)`, `path('home')`.
            self.reading.called.push(Called {
                name: token.range.clone(),
                kind: CalledKind::Function,
            });
            cursor.next();
            let arguments = self.items(cursor, ")");
            // `include('...', {...})` renders a template with the caller's variables and those.
            if word == "include" {
                self.reading.includes.push(Include {
                    range: token.range.start..cursor.previous_end().unwrap_or(token.range.end),
                    with: arguments.get(1).cloned(),
                    only: false,
                });
            }
            let before = cursor.position();
            self.postfix(cursor, None);
            // What the call returns, unless something is read off it -- which no chain records.
            return match cursor.position() == before {
                true => Value::Call(word.to_owned()),
                false => Value::Unknown,
            };
        }
        let at = self.reading.chains.len();
        self.reading.chains.push(Chain {
            root: token.range.clone(),
            steps: Vec::new(),
        });
        self.postfix(cursor, Some(at));
        Value::Chain {
            chain: at,
            filters: Vec::new(),
        }
    }

    /// Takes a binding out of the scope it was filed in: an arrow function's argument is visible in
    /// its body and nowhere else, which its own range already says.
    fn unscope(&mut self, at: usize) {
        for frame in &mut self.frames {
            frame.bindings.retain(|binding| *binding != at);
        }
        self.file.retain(|binding| *binding != at);
    }

    /// What is read off an operand: attributes, methods and keys, added to `chain` when it is one.
    fn postfix(&mut self, cursor: &mut Cursor<'_>, chain: Option<usize>) {
        loop {
            let Some(token) = cursor.peek().cloned() else {
                return;
            };
            match self.word(&token) {
                "." | "?." => {
                    cursor.next();
                    let step = match cursor.peek().cloned() {
                        Some(name) if matches!(name.kind, TokenKind::Name | TokenKind::Number) => {
                            cursor.next();
                            let called = cursor.peek().is_some_and(|next| self.word(next) == "(");
                            if called {
                                cursor.next();
                                self.items(cursor, ")");
                            }
                            Step {
                                name: name.range.clone(),
                                kind: match (called, name.kind) {
                                    (true, _) => StepKind::Method,
                                    (false, TokenKind::Number) => StepKind::Key,
                                    _ => StepKind::Attribute,
                                },
                            }
                        }
                        // A dot with nothing after it yet: the attribute being typed.
                        _ => Step {
                            name: token.range.end..token.range.end,
                            kind: StepKind::Attribute,
                        },
                    };
                    if let Some(at) = chain {
                        self.reading.chains[at].steps.push(step);
                    }
                }
                "[" => {
                    cursor.next();
                    let inside = cursor.peek().cloned();
                    let closes = cursor.peek_at(1).is_some_and(|next| self.word(next) == "]");
                    let step = match inside {
                        Some(key) if closes && matches!(key.kind, TokenKind::String(_)) => {
                            cursor.next();
                            cursor.next();
                            Step {
                                name: key.range.start + 1..key.range.end.saturating_sub(1),
                                kind: StepKind::Key,
                            }
                        }
                        Some(key) if closes && key.kind == TokenKind::Number => {
                            cursor.next();
                            cursor.next();
                            Step {
                                name: key.range.clone(),
                                kind: StepKind::Key,
                            }
                        }
                        _ => {
                            let start = token.range.start;
                            self.items(cursor, "]");
                            Step {
                                name: start..cursor.previous_end().unwrap_or(start),
                                kind: StepKind::Index,
                            }
                        }
                    };
                    if let Some(at) = chain {
                        self.reading.chains[at].steps.push(step);
                    }
                }
                _ => return,
            }
        }
    }

    /// After `(`: an arrow function's arguments, or a bracketed expression.
    fn arrow_or_group(&mut self, cursor: &mut Cursor<'_>) -> Value {
        // `(a, b) => ...`: names and commas, then `)` and `=>`.
        let mut ahead = 0;
        let mut names = Vec::new();
        while let Some(token) = cursor.peek_at(ahead) {
            match (token.kind, self.word(token)) {
                (TokenKind::Name, _) => names.push(token.clone()),
                (TokenKind::Punctuation, ",") => {}
                (TokenKind::Punctuation, ")") => break,
                _ => {
                    names.clear();
                    break;
                }
            }
            ahead += 1;
        }
        let arrow = !names.is_empty()
            && cursor
                .peek_at(ahead + 1)
                .is_some_and(|token| self.word(token) == "=>");
        if arrow {
            for _ in 0..ahead + 2 {
                cursor.next();
            }
            let from = cursor.peek().map_or(0, |token| token.range.start);
            let first = self.reading.bindings.len();
            for name in &names {
                self.bind(name, from, BindingKind::Parameter);
            }
            self.expression(cursor);
            let to = cursor.previous_end().unwrap_or(from);
            for at in first..self.reading.bindings.len() {
                self.reading.bindings[at].visible = from..to;
                self.unscope(at);
            }
            return Value::Unknown;
        }
        let inner = self.expression(cursor);
        while let Some(token) = cursor.next() {
            if self.word(token) == ")" {
                break;
            }
        }
        inner
    }

    /// Comma-separated expressions up to `close`, which is taken too. A call's named arguments are
    /// read for their values and not as variables.
    fn items(&mut self, cursor: &mut Cursor<'_>, close: &str) -> Vec<Value> {
        let mut items = Vec::new();
        loop {
            match cursor.peek() {
                None => return items,
                Some(token) if self.word(token) == close => {
                    cursor.next();
                    return items;
                }
                Some(token) if self.word(token) == "," => {
                    cursor.next();
                    continue;
                }
                _ => {}
            }
            // `name = value` or `name: value`, as a call's named argument.
            if cursor
                .peek()
                .is_some_and(|token| token.kind == TokenKind::Name)
                && cursor
                    .peek_at(1)
                    .is_some_and(|next| matches!(self.word(next), "=" | ":"))
            {
                cursor.next();
                cursor.next();
            }
            let before = cursor.position();
            items.push(self.expression(cursor));
            if cursor.position() == before {
                // Nothing an expression could read: skip it rather than stop.
                cursor.next();
            }
        }
    }

    /// The entries of a hash, after its `{`, which are taken through its `}`.
    fn hash(&mut self, cursor: &mut Cursor<'_>) -> Vec<(String, Value)> {
        let mut entries = Vec::new();
        loop {
            let Some(token) = cursor.peek().cloned() else {
                return entries;
            };
            match (token.kind, self.word(&token)) {
                (TokenKind::Punctuation, "}") => {
                    cursor.next();
                    return entries;
                }
                (TokenKind::Punctuation, ",") => {
                    cursor.next();
                    continue;
                }
                _ => {}
            }
            let key = match token.kind {
                TokenKind::Name | TokenKind::Number => Some(self.word(&token).to_owned()),
                TokenKind::String(_) => Some(
                    self.text[token.range.start + 1..token.range.end.saturating_sub(1)].to_owned(),
                ),
                _ => None,
            };
            let separated = cursor.peek_at(1).is_some_and(|next| self.word(next) == ":");
            match (key, separated) {
                (Some(key), true) => {
                    cursor.next();
                    cursor.next();
                    let value = self.expression(cursor);
                    entries.push((key, value));
                }
                // `{ post }` is `{ post: post }`.
                (Some(key), false) if token.kind == TokenKind::Name => {
                    let value = self.expression(cursor);
                    entries.push((key, value));
                }
                _ => {
                    let before = cursor.position();
                    self.expression(cursor);
                    if cursor.peek().is_some_and(|next| self.word(next) == ":") {
                        cursor.next();
                        self.expression(cursor);
                    }
                    if cursor.position() == before {
                        cursor.next();
                    }
                }
            }
        }
    }
}

/// A position in one tag's tokens.
struct Cursor<'t> {
    tokens: &'t [Token],
    at: usize,
}

impl<'t> Cursor<'t> {
    fn new(tokens: &'t [Token]) -> Self {
        Self { tokens, at: 0 }
    }

    fn done(&self) -> bool {
        self.at >= self.tokens.len()
    }

    fn position(&self) -> usize {
        self.at
    }

    fn peek(&self) -> Option<&'t Token> {
        self.tokens.get(self.at)
    }

    fn peek_at(&self, ahead: usize) -> Option<&'t Token> {
        self.tokens.get(self.at + ahead)
    }

    fn next(&mut self) -> Option<&'t Token> {
        let token = self.tokens.get(self.at);
        if token.is_some() {
            self.at += 1;
        }
        token
    }

    /// Takes the next token when it is the word `word`.
    fn next_word(&mut self, text: &str, word: &str) {
        if self.peek().is_some_and(|token| {
            token.kind == TokenKind::Name && &text[token.range.clone()] == word
        }) {
            self.at += 1;
        }
    }

    /// Where the last token taken ends.
    fn previous_end(&self) -> Option<usize> {
        self.at
            .checked_sub(1)
            .and_then(|at| self.tokens.get(at))
            .map(|token| token.range.end)
    }
}
