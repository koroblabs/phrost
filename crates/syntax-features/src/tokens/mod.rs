//! What every part of a file is, for colouring.
//!
//! Two passes, because two different things know the answer. The tree knows what a *name* is -- a
//! parameter against a local, a property against a method, a class against a function call -- and
//! nothing else can tell those apart. The lexer knows what a *word* is: a keyword, a string, a
//! number, a comment. A grammar gets the second group right and the first one wrong, which is what
//! makes a file look approximately right and read badly.
//!
//! Both are emitted, and that is a deliberate change. Emitting only the names assumed the client
//! merges this over its own grammar; told to *replace* it -- which is what a client's "full" setting
//! means -- an editor draws nothing for a range nobody claims, and a file whose every keyword and
//! string is grey reads worse than one with no semantic tokens at all. Measured on the three largest
//! files of a real project, 100% of word characters are now claimed by something.
//!
//! Not *every* token, though. Whitespace has no colour, and brackets, commas and operators are given
//! the plain foreground by nearly every theme -- so one token per `;` and `->` would multiply the
//! payload for a difference nobody sees. What is emitted is what a reader would notice missing.
//!
//! # What syntax cannot say
//!
//! Whether `Foo` is a class, an interface or an enum is decided by a declaration in another file, so
//! a name *used* is reported as a class. A name *declared* is reported for what it is, and
//! `implements` is reported as an interface because the keyword settles it. That asymmetry is the
//! honest edge of this crate: the alternative is asking the index, and then this would not be able
//! to answer before the project is indexed.

use mago_syntax::cst::{Node, Sequence};
use phrost_model::TextRange;
use phrost_syntax::ParseSession;

mod docblock;
mod emit;
mod lexical;
mod names;

#[cfg(test)]
mod tests;

/// One name, and what it is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SemanticToken {
    /// Never crossing a line: the protocol cannot encode a token that does, and every one of these
    /// is a single identifier.
    pub range: TextRange,
    pub kind: TokenKind,
    pub modifiers: TokenModifiers,
}

/// The kinds this crate can tell apart without asking the index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TokenKind {
    Namespace,
    /// Also a trait, which the protocol has no name for, and any class-like name in use — see the
    /// module documentation for why a name in use cannot say more than this.
    Class,
    Interface,
    Enum,
    EnumMember,
    Property,
    Method,
    Function,
    Parameter,
    Variable,
    /// An attribute name.
    Decorator,
    /// A type as a docblock writes it, whole: `\App\Repo|null`, `list<Item>`, `?int`.
    ///
    /// One token over all of it rather than one per name inside, because that is the distinction a
    /// reader wants -- a type against the sentence beside it -- and colouring `null` differently
    /// from `Repo` inside a `@param` is not something anyone is reading for.
    Type,
    /// A word of the language: `if`, `function`, `new`, `<?php`, `__CLASS__`, `(int)`.
    Keyword,
    /// A comment of any spelling, including a docblock's prose.
    Comment,
    /// A string literal, and each piece of an interpolated one.
    ///
    /// Named `Str` because `String` is the standard library's.
    Str,
    Number,
}

impl TokenKind {
    /// Which kind wins when two arms describe the same range.
    ///
    /// Two arms often do. `foo()` is a call whose callee is an identifier, so the arm that knows it
    /// is a call and the arm that knows nothing but "this is a name" both fire on `foo`. Rather than
    /// tracking where each token came from, the more specific kind wins — and the two kinds a
    /// blind arm can produce, [`Self::Class`] and [`Self::Variable`], are the two ranked last.
    fn rank(self) -> u8 {
        match self {
            Self::Parameter => 0,
            Self::Property => 1,
            Self::Method => 2,
            Self::Function => 3,
            Self::EnumMember => 4,
            Self::Interface => 5,
            Self::Enum => 6,
            Self::Decorator => 7,
            Self::Type => 8,
            Self::Namespace => 9,
            Self::Variable => 10,
            Self::Class => 11,
            // Below every name, because a lexical answer is the weakest one there is: where the tree
            // has said what a range is, it knows more.
            Self::Keyword => 12,
            Self::Str => 13,
            Self::Number => 14,
            Self::Comment => 15,
        }
    }
}

/// What is additionally known about a name.
///
/// Named after the protocol's own modifiers, and only the ones syntax can establish. `deprecated`
/// would need the docblock above the declaration and `defaultLibrary` would need the index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TokenModifiers {
    /// This is where the name is declared, rather than a use of it.
    pub is_declaration: bool,
    pub is_static: bool,
    pub is_readonly: bool,
    pub is_abstract: bool,
}

impl TokenModifiers {
    /// The modifiers of a declaration, read from the ones written in front of it.
    fn of_declaration(modifiers: &Sequence<'_, mago_syntax::cst::Modifier<'_>>) -> Self {
        use mago_syntax::cst::Modifier;

        let mut found = Self {
            is_declaration: true,
            ..Self::default()
        };
        for modifier in modifiers.iter() {
            match modifier {
                Modifier::Static(_) => found.is_static = true,
                Modifier::Readonly(_) => found.is_readonly = true,
                Modifier::Abstract(_) => found.is_abstract = true,
                _ => {}
            }
        }
        found
    }

    const DECLARATION: Self = Self {
        is_declaration: true,
        is_static: false,
        is_readonly: false,
        is_abstract: false,
    };
}

/// Every name in `text`, in document order, without overlaps.
pub fn tokens(text: &str) -> Vec<SemanticToken> {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    let source = parsed.source();

    let mut found = Vec::new();
    // Index 0 is the file scope, which has no parameters. Every function-like pushes one and its
    // subtree carries the index, so a variable can be told from a parameter without a second walk.
    let mut scopes: Vec<Vec<&str>> = vec![Vec::new()];
    let mut pending = vec![(Node::Program(parsed.program()), 0_usize)];

    while let Some((node, scope)) = pending.pop() {
        let scope = match Introduced::by(node) {
            Some(introduced) => {
                let mut merged = if introduced.inherits {
                    scopes[scope].clone()
                } else {
                    Vec::new()
                };
                merged.extend(introduced.parameters);
                scopes.push(merged);
                scopes.len() - 1
            }
            None => scope,
        };

        emit::from(node, &scopes[scope], source, &mut found);
        node.visit_children(|child| pending.push((child, scope)));
    }

    // Docblocks are trivia, so no node above reaches them. Their own parser does, and what it
    // finds cannot collide with anything the walk produced: a comment holds no code.
    docblock::from_docblocks(parsed.docblocks(), &mut found);

    // Everything a grammar would have coloured, added last and fitted around what is already here:
    // the docblock tags above sit *inside* a comment, and an interpolated string holds variables the
    // walk claimed, so a lexical token that overlapped one would leave the client with two tokens
    // over one range -- which the protocol has no meaning for.
    let lexical = lexical::from_tokens(text);
    found.sort_by_key(|token| (token.range.start(), token.range.end()));
    found.extend(fitted_around(lexical, &found));

    found.sort_by_key(|token| {
        (
            token.range.start(),
            token.range.end(),
            token.kind.rank(),
            // A declaration and a use of the same name at the same place is the declaration: the
            // blind arm sees a parameter inside the function it belongs to and says so without
            // knowing it is looking at the parameter list.
            u8::from(!token.modifiers.is_declaration),
        )
    });
    found.dedup_by_key(|token| (token.range.start(), token.range.end()));
    found
}

/// Every part of `lexical` that no already-claimed range covers.
///
/// A docblock is the case that needs it: the comment token runs over the whole of it, and the tags
/// and types inside were claimed by the pass above. Splitting rather than dropping is what keeps the
/// prose around a `@param` coloured as a comment.
///
/// `claimed` must be sorted by start, which is the caller's business and cheap there.
fn fitted_around(lexical: Vec<SemanticToken>, claimed: &[SemanticToken]) -> Vec<SemanticToken> {
    let mut kept = Vec::with_capacity(lexical.len());

    for token in lexical {
        // The claims that could touch this token, found by position rather than by scanning all of
        // them: a large file has tens of thousands of each.
        let from = claimed.partition_point(|other| other.range.end() <= token.range.start());
        let mut at = token.range.start();

        for other in &claimed[from..] {
            if other.range.start() >= token.range.end() {
                break;
            }
            if other.range.start() > at {
                kept.push(SemanticToken {
                    range: TextRange::new(at, other.range.start()),
                    ..token
                });
            }
            at = at.max(other.range.end());
        }

        if at < token.range.end() {
            kept.push(SemanticToken {
                range: TextRange::new(at, token.range.end()),
                ..token
            });
        }
    }

    kept
}

/// The parameters a function-like brings into scope, and whether the enclosing ones survive.
struct Introduced<'a> {
    parameters: Vec<&'a str>,
    /// An arrow function captures the enclosing scope by value, so a parameter of the function
    /// around it is still a parameter inside it. Every other function-like starts empty.
    inherits: bool,
}

impl<'a> Introduced<'a> {
    fn by(node: Node<'_, 'a>) -> Option<Self> {
        let (list, inherits) = match node {
            Node::Function(function) => (Some(&function.parameter_list), false),
            Node::Method(method) => (Some(&method.parameter_list), false),
            Node::Closure(closure) => (Some(&closure.parameter_list), false),
            Node::ArrowFunction(function) => (Some(&function.parameter_list), true),
            // `set` takes an implicit `$value`; a written list overrides it, and neither is a
            // reason to keep the enclosing method's parameters in scope.
            Node::PropertyHook(hook) => (hook.parameter_list.as_ref(), false),
            _ => return None,
        };

        Some(Self {
            parameters: list
                .into_iter()
                .flat_map(|list| list.parameters.iter())
                .map(|parameter| names::variable_name(parameter.variable.name))
                .collect(),
            inherits,
        })
    }
}
