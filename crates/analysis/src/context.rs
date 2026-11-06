//! Working out what a completion request is asking for.
//!
//! This reads the characters before the cursor, which is unlike everything else in this project and
//! is not a shortcut. Completion is asked for at the moment the code does *not* parse: `$this->` is
//! not an expression, `new ` is not a statement, and a half-typed name is not a name. The tree has
//! nothing to say about the cursor, so the answer has to come from the text.
//!
//! It stays strictly lexical. It decides which *kind* of thing is being asked for and what the
//! receiver was written as. It does not decide what a receiver's type is — that needs inference, and
//! a request whose receiver it cannot name honestly offers nothing rather than guessing.

#[cfg(test)]
use phrost_model::SymbolKind;
use phrost_model::{TextRange, TextSize};

/// What the cursor is asking to complete.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum CompletionContext {
    /// `$this->fi|`, `$repo->fi|`
    Member {
        receiver: Receiver,
        prefix: String,
        /// Whether the access being completed is the whole of a statement whose `;` is not written
        /// yet. See [`ends_a_statement`].
        whole_statement: bool,
    },
    /// `self::fi|`, `Repo::fi|`, `Repo::$fi|`
    StaticMember {
        receiver: Receiver,
        prefix: String,
        /// Whether the `$` of a static property has already been typed. It is part of what an
        /// offered item writes, so it has to be part of what the item replaces.
        dollar: bool,
        /// Whether the access being completed is the whole of a statement whose `;` is not written
        /// yet. See [`ends_a_statement`].
        whole_statement: bool,
    },
    /// `$re|` — a local variable or a superglobal.
    Variable { prefix: String },
    /// `function () use (|)` — a closure's capture list, where only a variable of the enclosing
    /// scope can stand.
    ///
    /// PHP's third `use`, and the one with no name in it at all: the file-level one takes a path,
    /// the one in a class body takes a trait, and this one takes variables. Apart from
    /// [`Self::Variable`] because the `$` has not been typed yet -- it is the item that writes it --
    /// and because `$this` is not capturable.
    Capture {
        prefix: String,
        /// The names the list already holds. Capturing one twice is a fatal error, so offering it
        /// is offering a mistake -- the same rule named-argument completion follows.
        already: Vec<String>,
    },
    /// A bare name: after `new`, in a type position, anywhere a class can be written.
    Name { prefix: String, place: NamePlace },
    /// Inside a `use` statement, where only a fully-qualified path makes sense.
    Import { prefix: String },
    /// `$row[`, `$row['fi|` — inside a subscript, where the keys of a shape are what is legal.
    Key {
        receiver: Receiver,
        prefix: String,
        /// Whether a quote has already been typed, which decides whether accepting an item has to
        /// write one. `$row[` needs `'id'`; `$row['` needs `id`.
        quoted: bool,
    },
    /// `[$this, 'vali|']`, `[Repo::class, 'fi|']` -- the method a callable written as a pair names.
    ///
    /// The receiver is the pair's first element, read the way a receiver before an operator is, and
    /// only in the shapes goto already follows there: a bare variable, and a class named with
    /// `::class`. Offering a method that goto then cannot find again is two answers to one question.
    Callable {
        receiver: Receiver,
        prefix: String,
        /// Whether the class was named outright, so the pair reaches what `::` reaches rather than
        /// what `->` does: `[Repo::class, 'instanceMethod']` is not callable outside the class.
        statically: bool,
    },
}

/// What stands immediately before a bare name.
///
/// Lexical, and only a hint: it is the character the reader is already standing on, and what is
/// really legal at a position is the tree's answer. It exists to decide what is worth *asking* --
/// each of the two named places costs something a plain name position should not pay, a walk over
/// every call in the file or a list of keywords, and paying either on every letter typed is what
/// this avoids.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NamePlace {
    /// After a `(` or a `,`, where an argument goes -- so the callee's parameters belong in the list.
    ///
    /// `[1, ` ends in a comma too, and there the tree finds no argument and nothing is offered.
    Argument,
    /// After a `;`, a `{`, a `}`, or the opening tag, where a statement may start -- so the keywords
    /// that start one belong in the list.
    Statement,
    /// After `new`, where the only name that can stand is a class's.
    New,
    /// In an `implements` list, where only an interface can stand.
    Implements,
    /// After `extends`, where a class extends a class and an interface extends interfaces.
    ///
    /// Which of the two is decided by the keyword the declaration opened with, since PHP files them
    /// under one catalog kind apart.
    Extends { interface: bool },
    /// Inside an `#[...]`, where an attribute is a class.
    Attribute,
    /// Where a type is written, and which sort of place it is.
    ///
    /// The kinds are the same everywhere -- a class, an interface or an enum -- and the *keywords*
    /// are not, which is why the place travels: `void` describes a return and nothing else, and an
    /// enum's backing type is `int` or `string` and nothing else at all.
    Type(TypePlace),
    /// In a `use` written inside a class body, where only a trait can stand.
    ///
    /// Not [`crate::context::CompletionContext::Import`], though it is the same keyword: a
    /// file-level `use` names any of PHP's three tables and is anchored at the root, and this one
    /// names a trait and resolves relatively. Writing the qualified path an import wants produces
    /// `Ed\\Ed\\Timestamps` inside `namespace Ed`.
    UseTrait,
    /// A type position, an operand. A name and nothing else.
    Anywhere,
}

/// Which sort of type position the cursor is in.
///
/// Only the keywords differ between them; the names offered are the same three kinds everywhere.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TypePlace {
    /// A parameter's type, including a promoted constructor property's.
    Parameter,
    /// A return type, which is the only place `void`, `never` and `static` can be written.
    Return,
    /// A property's type, which is the one place `callable` is refused.
    Property,
    /// `enum Status: |`, which takes `int` or `string` and nothing else.
    EnumBacking,
    /// `catch (|)`, which takes a class and no keyword at all.
    Catch,
    /// A docblock's `@var`, `@param`, `@return` and their neighbours.
    Docblock,
}

impl NamePlace {
    fn of(trimmed: &str) -> Self {
        // The first statement of a file, which has no punctuation before it.
        if trimmed.ends_with("<?php") || trimmed.ends_with("<?") {
            return Self::Statement;
        }
        if ends_with_keyword(trimmed, "new") {
            return Self::New;
        }
        // The declaration being written, read off the line the cursor is on. A class header is one
        // line in every style PHP is written in, and the keyword that opened it is what decides
        // whether `extends` takes a class or interfaces -- `interface A extends B` names an
        // interface, `class A extends B` a class.
        if let Some(place) = Self::in_a_class_header(trimmed) {
            return place;
        }
        if in_an_attribute(trimmed) {
            return Self::Attribute;
        }
        if let Some(place) = type_place(trimmed) {
            return Self::Type(place);
        }
        match trimmed.chars().last() {
            Some('(' | ',') => Self::Argument,
            Some(';' | '{' | '}') | None => Self::Statement,
            _ => Self::Anywhere,
        }
    }

    /// The place a class header puts the cursor in, when it is in one.
    ///
    /// Textual, like the rest of this scanner, and the list separators are what make it more than a
    /// keyword test: `implements A, B|` is still an implements list, and `extends A implements B|`
    /// has left the `extends` behind. So the *last* of the two keywords on the line decides, and a
    /// `{` means the header is over.
    fn in_a_class_header(trimmed: &str) -> Option<Self> {
        // Lowercased because PHP's keywords are: `Class Foo Implements Bar` runs.
        let line = trimmed.rsplit('\n').next()?.to_ascii_lowercase();
        let head = line.trim_start();
        let interface = head.starts_with("interface ");
        if !interface
            && !head.starts_with("class ")
            && !head.starts_with("abstract class ")
            && !head.starts_with("final class ")
            && !head.starts_with("readonly class ")
            && !head.starts_with("enum ")
        {
            return None;
        }
        // A brace ends the header, and everything after it is a body.
        let header = line.split('{').next()?;
        // Without a trailing space: the cursor may sit immediately after the keyword, which is
        // where the first letter of the first name goes.
        let implements = header.rfind(" implements");
        let extends = header.rfind(" extends");
        match (implements, extends) {
            (Some(at), Some(before)) if at > before => Some(Self::Implements),
            (Some(_), None) => Some(Self::Implements),
            (_, Some(_)) => Some(Self::Extends { interface }),
            (None, None) => None,
        }
    }

    /// Which top-level symbol kinds can stand here.
    ///
    /// Only `new` narrows it, and it narrows to one kind: `new strlen()`, `new PHP_EOL` and
    /// `new Countable()` are each a fatal error, so on a real project the catalog spends the whole
    /// list on rows nobody can accept -- with an empty prefix, measured, two hundred of them before
    /// the first class.
    ///
    /// Everywhere else the three tables are genuinely ambiguous: a bare name is a class, a function
    /// or a constant depending on what follows the cursor, and choosing between them from the
    /// characters before it is the guess this scanner refuses to make.
    ///
    /// Given to the index rather than kept here, so a position that can only take a class does not
    /// spend a completion's cap on rows it would only discard afterward -- see
    /// [`crate::completion::names::names`].
    pub(crate) fn kinds(self) -> phrost_view::NameKinds {
        match self {
            // An attribute is an ordinary class, and PHP constructs one to read it -- so both take
            // a class and nothing else. What the attribute's own kind adds is the ranking: the
            // classes that carry `#[Attribute]` are offered first.
            Self::New => phrost_view::NameKinds::ClassOnly,
            Self::Attribute => phrost_view::NameKinds::Attribute,
            // An enum implements interfaces too, and a trait cannot be implemented.
            Self::Implements => phrost_view::NameKinds::InterfaceOnly,
            Self::Extends { interface: true } => phrost_view::NameKinds::InterfaceOnly,
            // A class extends a class, and an enum extends nothing at all -- but the header is
            // being typed, so the keyword is what there is to go on.
            Self::Extends { interface: false } => phrost_view::NameKinds::ClassOnly,
            Self::UseTrait => phrost_view::NameKinds::TraitOnly,
            // An enum is backed by `int` or `string` and by nothing a project declares, so the
            // catalog has no answer here at all.
            Self::Type(TypePlace::EnumBacking) => phrost_view::NameKinds::Nothing,
            // A trait is not a type -- nothing is an instance of one -- and a function or a
            // constant is not a name that can stand here at all.
            Self::Type(_) => phrost_view::NameKinds::TypeName,
            Self::Argument | Self::Statement | Self::Anywhere => phrost_view::NameKinds::Any,
        }
    }

    /// Whether a top-level name of this kind can stand here.
    #[cfg(test)]
    fn accepts(self, kind: SymbolKind) -> bool {
        self.kinds().accepts(kind)
    }
}

/// The sort of type position the cursor is in, when it is in one.
///
/// Textual, like the rest of this scanner, and deliberately narrow: every shape it claims is one
/// where PHP allows a type and nothing else, so a place it cannot be sure of keeps the list it had.
/// Bounded to the current statement for the reason every scan here is -- a request that is not one
/// of these must not pay for the search.
fn type_place(before: &str) -> Option<TypePlace> {
    let chunk = before.rsplit([';', '{', '}']).next()?;
    // A nullable marker and a union or intersection operator each leave the cursor exactly where it
    // was: still writing a type. `?` is stripped last, since `?int|` is written.
    let head = chunk
        .trim_end()
        .trim_end_matches(['?', '|', '&'])
        .trim_end();

    // `function f(): |`, `fn (): |`, and `enum Status: |` -- the three colons that introduce a type.
    if let Some(before_colon) = head.strip_suffix(':') {
        let before_colon = before_colon.trim_end();
        if before_colon.ends_with(')') && holds_keyword(chunk, "function") {
            return Some(TypePlace::Return);
        }
        if before_colon.ends_with(')') && holds_keyword(chunk, "fn") {
            return Some(TypePlace::Return);
        }
        // `enum Status: string` -- the backing type, and the only other colon a declaration writes.
        // A ternary's and a named argument's are both inside a statement that declares nothing.
        if declaration_line(chunk)
            .to_ascii_lowercase()
            .starts_with("enum ")
        {
            return Some(TypePlace::EnumBacking);
        }
        return None;
    }

    // Inside a parameter list that is still open, before the `$` that ends the type.
    if let Some(at) = open_bracket(head) {
        let opener = head[..at].trim_end();
        if ends_with_keyword(opener, "catch") {
            return Some(TypePlace::Catch);
        }
        if opens_a_parameter_list(opener) {
            // The parameter being written. Past its `$` or its `=` the type is finished, and what
            // comes next is a name or a default rather than anything this could complete.
            let current = head[at..].rsplit(',').next().unwrap_or_default();
            if !current.contains('$') && !current.contains('=') {
                return Some(TypePlace::Parameter);
            }
        }
        return None;
    }

    // A member declaration whose modifiers are written and whose type is not: `private readonly |`.
    // Refused the moment `function` or `const` appears, where what follows is a name rather than a
    // type -- and `const string KIND` is legal, so the two really are ambiguous there.
    let mut words = declaration_line(head).split_whitespace().peekable();
    words.peek().is_some().then_some(())?;
    words
        .all(is_a_member_modifier)
        .then_some(TypePlace::Property)
}

/// The line a declaration is being written on, with the opening tag taken off.
///
/// The tag is not a statement separator, so a declaration written on the same line as it -- which is
/// what a one-line probe and a short file both do -- would otherwise not look like the start of a
/// line at all. The same strip `in_use_statement` makes, for the same reason.
fn declaration_line(chunk: &str) -> &str {
    let line = chunk.rsplit('\n').next().unwrap_or(chunk).trim_start();
    line.strip_prefix("<?php").unwrap_or(line).trim_start()
}

/// The innermost bracket the text leaves open, if it leaves one open.
fn open_bracket(head: &str) -> Option<usize> {
    let at = head.rfind('(')?;
    (!head[at..].contains(')')).then_some(at)
}

/// Whether what stands before a `(` makes it a parameter list rather than a call.
///
/// Three spellings: `function (`, `fn (`, and `function name(` -- which is why the name is stripped
/// before the keyword is looked for. A method's modifiers sit further left and change nothing.
fn opens_a_parameter_list(head: &str) -> bool {
    if ends_with_keyword(head, "function") || ends_with_keyword(head, "fn") {
        return true;
    }
    let stripped = head.trim_end_matches(|ch: char| ch.is_alphanumeric() || ch == '_');
    stripped.len() < head.len() && ends_with_keyword(stripped.trim_end(), "function")
}

/// Whether a word is one of the modifiers a member declaration opens with.
///
/// `private(set)` and `public(get)` are PHP 8.4's asymmetric visibility, written as one word.
fn is_a_member_modifier(word: &str) -> bool {
    let word = word.split('(').next().unwrap_or(word);
    [
        "public",
        "protected",
        "private",
        "readonly",
        "static",
        "final",
        "abstract",
        "var",
    ]
    .iter()
    .any(|modifier| word.eq_ignore_ascii_case(modifier))
}

/// Whether the text holds this keyword as a word of its own.
fn holds_keyword(text: &str, keyword: &str) -> bool {
    text.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .any(|word| word.eq_ignore_ascii_case(keyword))
}

/// Whether the cursor sits inside an `#[...]` that has not been closed.
///
/// The last line only, for the reason the class header is read there: an attribute list is written
/// on its own line above what it annotates, and a `#[` on an earlier line has been closed by the
/// time the cursor is here.
fn in_an_attribute(trimmed: &str) -> bool {
    let Some(line) = trimmed.rsplit('\n').next() else {
        return false;
    };
    let Some(at) = line.rfind("#[") else {
        return false;
    };
    let open = &line[at..];
    // `#[Route(path: |)]` is inside the attribute's *arguments*, which is an argument position like
    // any other -- the attribute's own name was finished two characters after the bracket.
    !open.contains(']') && !open.contains('(')
}

/// Whether the text ends in a keyword written as a word of its own.
///
/// The boundary is what makes it a keyword rather than the tail of a name: `$renew` ends in `new`
/// and is a variable. Case-insensitive, as PHP's keywords are -- `NEW Foo()` runs.
fn ends_with_keyword(trimmed: &str, keyword: &str) -> bool {
    let Some(head) = trimmed
        .len()
        .checked_sub(keyword.len())
        .and_then(|at| trimmed.is_char_boundary(at).then(|| trimmed.split_at(at)))
        .filter(|(_, tail)| tail.eq_ignore_ascii_case(keyword))
        .map(|(head, _)| head)
    else {
        return false;
    };

    !head
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '$' | '\\'))
}

/// A receiver written before a `->` or `::`, read backwards from the cursor.
///
/// Backwards and lexically, not from the tree: at the moment someone asks for completions,
/// `$this->repo->` is not an expression, so there is nothing parsed to look at. The characters
/// before the cursor are all there is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Receiver {
    pub(crate) root: ReceiverRoot,
    /// Applied to the root in written order.
    pub(crate) steps: Vec<ReceiverStep>,
}

impl Receiver {
    fn unknown() -> Self {
        Self {
            root: ReceiverRoot::Unknown,
            steps: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum ReceiverRoot {
    /// `$this` or `static`. Kept apart from `self` because only these two say what the value *is*,
    /// which is what an `instanceof` guard proves.
    This,
    /// `self`.
    SelfClass,
    Parent,
    /// A variable, without its `$`.
    Local(String),
    /// A written class name, exactly as it was written.
    ///
    /// With its leading separator when there was one, because that is not decoration: `\Drupal` is
    /// absolute and `Drupal` inside `namespace Drupal\app_core;` is `Drupal\app_core\Drupal`. This
    /// used to be stored stripped, and every completion after `\Drupal::` -- 256 sites in one
    /// project's own code -- offered nothing while the same class reached through an import offered
    /// everything. See `NameAnchor::of`, which is now the one place the three spellings are read.
    Class(String),
    /// A free function's result: `reset($rows)->`, `array_pop($queue)?->`, `make_thing()->`.
    ///
    /// The name as written, with whatever leading separator it had -- the same reason
    /// [`Self::Class`] keeps one. The arguments travel because what some of these functions return
    /// is decided by what they were given: `reset($rows)` is a `Row` only because `$rows` holds
    /// them.
    ///
    /// As **written** and not as literals, unlike a [`ReceiverStep::Member`]'s: what settles one of
    /// these is nearly always a plain `$local`, which is not a literal at all, and the reader that
    /// wants a literal reads the same text.
    Function {
        name: String,
        arguments: Vec<Box<str>>,
    },
    /// Something this scanner cannot read: a parenthesized expression that names no class, an
    /// invoked callable, a chain through an element of a call's result. A deliberate miss rather
    /// than a guess.
    Unknown,
}

/// A written index, as far as characters can identify the slot -- see `ChainKey`, which this
/// becomes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum ScannedKey {
    Literal(String),
    /// The local's name, without its `$`.
    Variable(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum ReceiverStep {
    /// `->name` or `::name`.
    Member {
        name: String,
        kind: StepKind,
        /// The arguments a call was written with, as far as text can say -- one entry per position,
        /// and `None` for a slot this cannot read.
        ///
        /// Read because for some methods an argument *is* part of the type: `getStorage('node')` and
        /// `getStorage('user')` hand back different classes. Empty for anything that is not a call,
        /// and for a call whose list nothing here can make sense of.
        arguments: Vec<Option<Box<str>>>,
    },
    /// `(...)` on a value rather than on a name: `$handler()` calls the object's `__invoke`.
    Invoked,
    /// `[...]`, carrying the key when it was written as a literal.
    ///
    /// The key matters twice over. A *literal* is what a shape is keyed by -- `array{app:
    /// Application}` indexed by `app` is not what it holds "in general". A *variable* names no
    /// shape key and still identifies the slot, which is what lets a guard written about
    /// `$rows[$id]` narrow the read below it. `None` for a key that is neither, where the element
    /// type is all anyone can say.
    Element(Option<ScannedKey>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StepKind {
    /// `->name` -- an instance property.
    Property,
    /// `::$name` -- a static property, which is the one member spelled with its sigil.
    StaticProperty,
    /// `::name` -- a class constant or an enum case. The text cannot tell them apart and neither
    /// declaration is written with a sigil, so both are tried.
    Constant,
    /// `->name(...)` or `::name(...)`.
    MethodReturn,
}

/// Reads the receiver written immediately before a `->` or `::`.
///
/// Walks backwards a segment at a time: an identifier, then whatever is applied to it -- a call's
/// argument list, any number of subscripts -- then optionally an access operator. The first segment
/// with no operator before it is the root.
pub(crate) fn read_receiver(before: &str) -> Receiver {
    read_receiver_from(before).0
}

/// The same, and the text left of the identifier the root was written as.
///
/// The head is the other half of "does a `;` belong here", and this walk is the only thing that
/// knows it: everything else in this file reads backwards from the cursor a token at a time, and
/// where the *expression* started is what the walk works out on the way.
///
/// `None` for a receiver nothing here could read -- there is then no root, so there is no text
/// before one either, and saying "the statement starts here" about an unreadable expression is the
/// guess this scanner refuses to make.
fn read_receiver_from(before: &str) -> (Receiver, Option<&str>) {
    read_receiver_within(before, 0)
}

/// How many brackets deep [`regrouped_receiver`] may follow a receiver into.
///
/// `((($row)))` is written by nobody and a half-typed line can hold any number of them, so this is
/// a bound on a walk over text rather than a judgement about PHP -- the walk is on the keystroke
/// path and its input is whatever is in the buffer.
const NESTED_BRACKETS: u8 = 8;

fn read_receiver_within(before: &str, depth: u8) -> (Receiver, Option<&str>) {
    let mut head = before;
    let mut steps = Vec::new();

    loop {
        head = head.trim_end();

        // What is applied to the identifier about to be read, collected right to left: any number
        // of subscripts, then at most one argument list.
        //
        // At most one, and it has to be the innermost, because that is the only shape whose parts
        // this can name. `$repo->all()[0]` is a call and then an element of what it returned;
        // `$handlers['a']()` invokes whatever the element holds, and what that is is written
        // nowhere near the cursor. So reading the argument list ends the collection, and a `]` or a
        // second `(` left of it leaves an identifier that does not parse as a name -- which is the
        // deliberate miss.
        let mut elements = Vec::new();
        let mut called = false;
        let mut arguments = Vec::new();
        // The text inside a trailing `(...)`, kept because until the identifier left of it is read
        // there is no telling an argument list from a parenthesised expression -- and one shape of
        // the second names its own type. See below.
        let mut grouped = None;
        loop {
            head = head.trim_end();
            if let Some(rest) = head.strip_suffix(']') {
                let Some((before_bracket, key)) = split_subscript(rest) else {
                    return (Receiver::unknown(), None);
                };
                elements.push(literal_key(key));
                head = before_bracket;
                continue;
            }
            if head.ends_with(')') {
                // Unbalanced, or a `)` inside a string this scanner cannot see into.
                let Some((rest, written)) = skip_arguments(head) else {
                    return (Receiver::unknown(), None);
                };
                called = true;
                arguments = literal_arguments(written);
                grouped = Some(written);
                head = rest;
            }
            break;
        }

        let (name, rest) = split_trailing_identifier(head);
        if name.is_empty() {
            // Nothing named left of the parentheses, so they were not an argument list: this is a
            // parenthesised expression. `(new Foo())->` names its own class outright, and 42 sites
            // of the reference project's own code are written that way.
            if let Some(class) = grouped.and_then(instantiated_class) {
                steps.extend(elements.into_iter().map(ReceiverStep::Element));
                steps.reverse();
                return (
                    Receiver {
                        // The written class name, exactly as a static receiver gives: what separates
                        // `Foo::` from `(new Foo())->` is the operator, and the steps and the
                        // completion context both already carry it.
                        root: ReceiverRoot::Class(class),
                        steps,
                    },
                    Some(head),
                );
            }
            // Otherwise what is inside the parentheses is itself a receiver, and this walk is what
            // reads one. `(clone $row)->`, `($rows |> head(...))->`, `(self::of($id))->` -- the
            // value of each is the value of its last operand, and unlike the rest of this scanner's
            // input that text is *complete*, so reading it costs nothing but the call.
            if let Some(inside) = grouped.and_then(|written| regrouped_receiver(written, depth)) {
                steps.extend(elements.into_iter().map(ReceiverStep::Element));
                steps.reverse();
                let mut written = inside.steps;
                written.extend(steps);
                return (
                    Receiver {
                        root: inside.root,
                        steps: written,
                    },
                    Some(head),
                );
            }
            return (Receiver::unknown(), None);
        }

        let rest = rest.trim_end();
        // PHP 8.4's `new Foo()->method()`, which is the same expression as `(new Foo())->method()`
        // with the parentheses the language stopped requiring. Read here rather than in the branch
        // above because the shapes part company at exactly this point: with the parentheses nothing
        // is named left of them, and without them the class *is* the trailing identifier -- so that
        // branch never sees this one and this one falls through to "a name with `new ` left of it",
        // which is no operator and so a deliberate miss.
        //
        // 111 sites in the reference project's own modules and 220 in Symfony, against 42 for the
        // parenthesised spelling -- a project on 8.4 stops writing the older one.
        if called && !name.starts_with('$') && ends_with_keyword(rest, "new") {
            steps.extend(elements.into_iter().map(ReceiverStep::Element));
            steps.reverse();
            return (
                Receiver {
                    root: ReceiverRoot::Class(name.to_owned()),
                    steps,
                },
                Some(head),
            );
        }
        // Which operator it was decides what kind of member the name can be, and `::` is the one
        // that is not a property: `Suit::Hearts` is an enum case and `Foo::BAR` a constant, both
        // written exactly like a property and neither stored as one.
        let operator = rest
            .strip_suffix("?->")
            .or_else(|| rest.strip_suffix("->"))
            .map(|head| (head, false))
            .or_else(|| rest.strip_suffix("::").map(|head| (head, true)));

        // Everything is pushed in reverse written order and turned round once at the end, so the
        // subscripts -- already collected right to left -- go in before the name they were applied
        // to.
        steps.extend(elements.into_iter().map(ReceiverStep::Element));

        match operator {
            Some((remaining, double_colon)) => {
                steps.push(ReceiverStep::Member {
                    // Without the sigil, which is how every member is stored: `$` is part of how a
                    // static property is written, not part of what it is called.
                    name: name.trim_start_matches('$').to_owned(),
                    kind: match (called, double_colon, name.starts_with('$')) {
                        (true, _, _) => StepKind::MethodReturn,
                        (false, false, _) => StepKind::Property,
                        (false, true, true) => StepKind::StaticProperty,
                        (false, true, false) => StepKind::Constant,
                    },
                    arguments: std::mem::take(&mut arguments),
                });
                head = remaining;
            }
            None => {
                // No operator before it, so this segment is the root.
                if called {
                    // `$handler()->` is a value being invoked, which PHP answers with the object's
                    // `__invoke`. A bare `helper()->` is a *function's* result, looked up by name
                    // rather than by class -- and a different root, because nothing about it is a
                    // class the scanner can name.
                    if !name.starts_with('$') {
                        steps.reverse();
                        return (
                            Receiver {
                                root: ReceiverRoot::Function {
                                    name: name.to_owned(),
                                    arguments: written_arguments(grouped.unwrap_or_default()),
                                },
                                steps,
                            },
                            Some(rest),
                        );
                    }
                    steps.push(ReceiverStep::Invoked);
                }
                steps.reverse();
                return (
                    Receiver {
                        root: root_of(name),
                        steps,
                    },
                    Some(rest),
                );
            }
        }
    }
}

fn root_of(name: &str) -> ReceiverRoot {
    match name {
        "$this" | "static" => ReceiverRoot::This,
        "self" => ReceiverRoot::SelfClass,
        "parent" => ReceiverRoot::Parent,
        _ => match name.strip_prefix('$') {
            Some(local) => ReceiverRoot::Local(local.to_owned()),
            // Kept whole, separator included: see [`ReceiverRoot::Class`].
            None => ReceiverRoot::Class(name.to_owned()),
        },
    }
}

/// Splits a trailing `[...]` into the text before it and what was written inside.
///
/// `rest` is the text with the closing bracket already taken off. `None` when the brackets do not
/// balance, which is a half-written line rather than something to guess at.
fn split_subscript(rest: &str) -> Option<(&str, &str)> {
    let mut depth = 1_u32;
    for (index, ch) in rest.char_indices().rev() {
        match ch {
            ']' => depth += 1,
            '[' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&rest[..index], &rest[index + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// The key a subscript names, when it names one a shape could match.
///
/// A quoted string or a plain integer. Anything else -- a variable, a constant, an expression --
/// reaches a slot this scanner cannot identify, and saying so is what makes the element type the
/// answer instead of a wrong field.
fn literal_key(inside: &str) -> Option<ScannedKey> {
    let key = inside.trim();
    for quote in ['\'', '"'] {
        if let Some(unquoted) = key.strip_prefix(quote).and_then(|k| k.strip_suffix(quote))
            && !unquoted.contains(quote)
        {
            return Some(ScannedKey::Literal(unquoted.to_owned()));
        }
    }
    if let Some(name) = key.strip_prefix('$')
        && !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !name.as_bytes()[0].is_ascii_digit()
    {
        return Some(ScannedKey::Variable(name.to_owned()));
    }
    key.chars()
        .all(|ch| ch.is_ascii_digit())
        .then(|| ScannedKey::Literal(key.to_owned()))
        .filter(|key| !matches!(key, ScannedKey::Literal(text) if text.is_empty()))
}

/// The class a parenthesised `new` expression instantiates, without a leading separator.
///
/// `(new Foo())->`, `(new Foo)->` and `(new \App\Foo($a, $b))->` all name `Foo`; everything else
/// inside parentheses is `None`, which keeps the scanner's rule -- a deliberate miss rather than a
/// guess. Two shapes are refused on purpose and both would be guesses: `new $class()` names a class
/// this cannot read, and `new class { ... }` has no name at all.
fn instantiated_class(inside: &str) -> Option<String> {
    let rest = inside.trim().strip_prefix("new")?;
    // `new` has to be the whole word: `newFoo()` is a function call.
    if !rest.starts_with(|ch: char| ch.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    let end = rest
        .char_indices()
        .find(|(_, ch)| !(ch.is_alphanumeric() || matches!(ch, '_' | '\\')))
        .map_or(rest.len(), |(index, _)| index);
    let (name, after) = rest.split_at(end);
    // An argument list may follow and nothing else may: `new Foo()[0]` does not parse, and
    // `new Foo() + 1` is an expression whose type is not the class.
    if !after.trim_start().is_empty() && !after.trim_start().starts_with('(') {
        return None;
    }
    let bare = name.trim_start_matches('\\');
    (!bare.is_empty() && bare != "class").then(|| name.to_owned())
}

/// The receiver a parenthesised expression is, when its value is one this walk can name.
///
/// The value of `(clone $row)` is `$row`'s, of `($x = of($id))` is the call's, and of
/// `($rows |> head(...))` is the pipe's call -- in each the trailing operand *is* the value, which
/// is the assumption the whole backwards walk is built on. What is refused is where that assumption
/// stops holding:
///
/// * a **choice** -- `?:`, `??`, `? :` -- whose value is either operand and whose honest answer is
///   a union this vocabulary has one slot for. Detected as a `?` at depth zero that is not a `?->`,
///   which catches all three spellings at once.
/// * a **closure**, whose value is the `Closure` and not the expression after the `=>`.
/// * `yield`, `print` and `throw`, each of which hands back something other than what follows it.
/// * `new`, which [`instantiated_class`] above has already had its say about -- and refused for a
///   reason: `new $class()` constructs whatever the *variable names*, so reading it as this walk
///   reads everything else makes it the invocation of a local, which is a different expression
///   with a different answer.
///
/// Everything else is left to the walk. A binary operator between two objects is not PHP, so
/// `($a . $b)->` and `($n + 1)->` do not occur -- and where one is written anyway the walk reads
/// the trailing operand and the index has no member to offer for it, which is the miss it would
/// have been either way.
fn regrouped_receiver(written: &str, depth: u8) -> Option<Receiver> {
    let trimmed = written.trim();
    if depth >= NESTED_BRACKETS || trimmed.is_empty() || holds_a_choice(trimmed) {
        return None;
    }
    let opening = trimmed
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .next()
        .unwrap_or_default();
    if matches!(
        opening,
        "fn" | "function" | "static" | "new" | "yield" | "print" | "throw"
    ) {
        return None;
    }
    let receiver = read_receiver_within(trimmed, depth + 1).0;
    (receiver.root != ReceiverRoot::Unknown).then_some(receiver)
}

/// Whether a `?` stands at depth zero -- a ternary, an `?:` or a `??`, all of which are a choice.
///
/// A `?->` is not one: it is a member access that may not happen, and what it is applied to is
/// still the single value this walk reads. Nested parentheses and brackets are skipped, so the `?`
/// of `foo($a ?: $b)` belongs to the call and not to what the call's result is.
fn holds_a_choice(written: &str) -> bool {
    let bytes = written.as_bytes();
    let mut depth = 0_i32;
    for (index, ch) in written.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '?' if depth == 0 && bytes.get(index + 1..index + 3) != Some(b"->") => return true,
            _ => {}
        }
    }
    false
}

/// The text before a trailing `(...)`, or `None` when the parentheses do not balance.
fn skip_arguments(head: &str) -> Option<(&str, &str)> {
    let mut depth = 0_u32;
    for (index, ch) in head.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&head[..index], &head[index + 1..head.len() - 1]));
                }
            }
            _ => {}
        }
    }
    None
}

/// The arguments a call was written with, as far as text can say.
///
/// Read because for some methods an argument *is* part of the type: `getStorage('node')` and
/// `getStorage('user')` hand back different classes, and `$container->get(Repo::class)` hands back
/// whatever the argument named. A chain is the one place nothing else can say -- the statement being
/// typed does not parse, so there is no expression to read the arguments off.
///
/// One entry per position, and `None` for a slot this cannot read. A named argument makes every slot
/// unreadable rather than only its own: naming one moves what the positions mean, and guessing which
/// is worse than saying nothing.
/// Each argument as it was written, trimmed, in position order.
///
/// Whole text rather than the literals beside it: the arguments that settle a free function's
/// return are written as `$rows`, which is no literal, and a reader wanting a literal has the same
/// characters to read.
fn written_arguments(written: &str) -> Vec<Box<str>> {
    split_arguments(written)
        .into_iter()
        .map(|part| Box::from(part.trim()))
        .collect()
}

fn literal_arguments(written: &str) -> Vec<Option<Box<str>>> {
    let mut found = Vec::new();
    for part in split_arguments(written) {
        let part = part.trim();
        if names_a_parameter(part) {
            return Vec::new();
        }
        found.push(literal_argument(part));
    }
    found
}

/// Whether an argument was written under a parameter's name: `id: Foo::class`.
///
/// A single colon, and not the `::` of a class constant, which is what nearly every argument this
/// reads is written with.
fn names_a_parameter(part: &str) -> bool {
    let mut chars = part.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        match ch {
            ':' if chars.peek().is_some_and(|(_, next)| *next == ':') => {
                chars.next();
            }
            ':' => return !part[..at].trim().is_empty(),
            '\'' | '"' | '(' | '[' => return false,
            _ => {}
        }
    }
    false
}

/// One argument, when it is a string this layer can read.
///
/// A quoted string or the class a `::class` names -- both are strings at runtime, and the two uses
/// cannot collide for the reason the body's own reader gives: a catalog has no key spelled like a
/// class.
fn literal_argument(part: &str) -> Option<Box<str>> {
    if let Some(name) = part.strip_suffix("::class") {
        let name = name.trim();
        return (!name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '\\')))
        .then(|| name.trim_start_matches('\\').into());
    }
    for quote in ['\'', '"'] {
        if let Some(rest) = part.strip_prefix(quote)
            && let Some(inner) = rest.strip_suffix(quote)
            // An escape means the text is not the value, and this reader does not decode them.
            && !inner.contains('\\')
            && !inner.contains(quote)
        {
            return Some(inner.into());
        }
    }
    None
}

/// An argument list split at its top-level commas.
///
/// Nesting and quotes are stepped over, because a comma inside either belongs to it: `f(g(1, 2))` is
/// one argument and `f('a, b')` is one string.
fn split_arguments(written: &str) -> Vec<&str> {
    if written.trim().is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut depth = 0_u32;
    let mut quote = None;
    let mut start = 0;
    for (at, ch) in written.char_indices() {
        match (quote, ch) {
            (Some(open), ch) if ch == open => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(ch),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                parts.push(&written[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&written[start..]);
    parts
}

/// Splits a trailing identifier — including a leading `$` or `\` — from what precedes it.
fn split_trailing_identifier(head: &str) -> (&str, &str) {
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, ch)| ch.is_alphanumeric() || matches!(ch, '_' | '\\' | '$'))
        .last()
        .map_or(head.len(), |(index, _)| index);

    (&head[start..], &head[..start])
}

/// Whether the access being completed is the whole of a statement whose `;` has not been written.
///
/// Two questions, and both have to answer yes. What stands left of the receiver's root has to be a
/// place a statement may begin -- a `;`, a brace, the opening tag -- which rules out `$x = $repo->`,
/// `return $repo->`, `if ($repo->` and `f($repo->` in one test, since each of those puts something
/// else there. And what stands right of the cursor has to be the end of the line and then the start
/// of something new, which rules out a `;` already written, a chain continued on the next line, and
/// a closing `)` the access is an argument to.
///
/// Deliberately narrow. A `;` written where one does not belong makes the buffer invalid and is
/// deleted by hand every single time, so anything this cannot read -- a trailing comment, a `?>`, a
/// second statement on the same line -- is answered no. `$repo->m() or die()` is why the word after
/// the cursor is checked as well: PHP's word operators continue an expression across a line break.
///
/// `root_head` is `None` for a receiver [`read_receiver_from`] could not read, and an unreadable
/// expression is never claimed to be a whole statement.
fn ends_a_statement(text: &str, offset: TextSize, root_head: Option<&str>) -> bool {
    if root_head.map(NamePlace::of) != Some(NamePlace::Statement) {
        return false;
    }
    let Some(after) = text.get(usize::from(offset)..) else {
        return false;
    };

    // Nothing else on the line. A `;`, a `)`, a `,` or a comment all land here, and each of them
    // means the access is part of something already written rather than the whole of it.
    let rest_of_line = after.split('\n').next().unwrap_or(after);
    if !rest_of_line.trim().is_empty() {
        return false;
    }

    // What comes next has to start something new rather than continue this. A name, a variable or a
    // closing brace do; an operator on the next line -- `->`, `?->`, `::`, `+`, `.` -- does not, and
    // is the shape a chain broken over several lines has.
    let next = after.trim_start();
    match next.chars().next() {
        None | Some('}') | Some('$') => true,
        Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '\\' => !continues_an_expression(next),
        Some(_) => false,
    }
}

/// Whether the text starts with one of PHP's word-spelled operators.
///
/// They are the one way an expression carries on into something that otherwise reads as the start of
/// a statement, and a `;` written in front of one is a syntax error.
fn continues_an_expression(next: &str) -> bool {
    let word = next
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .next()
        .unwrap_or(next);
    ["and", "or", "xor", "instanceof"]
        .iter()
        .any(|operator| word.eq_ignore_ascii_case(operator))
}

impl CompletionContext {
    /// Reads the text before `offset` to decide what is being asked for.
    pub(crate) fn at(text: &str, offset: TextSize) -> Option<Self> {
        let before = text.get(..usize::from(offset))?;
        let (prefix, before) = split_identifier(before);

        // A closure's capture list before the import check, because a `use (` broken onto its own
        // line starts one exactly as an import does -- and this is the only `use` with a `(`.
        if let Some(already) = captured_so_far(before) {
            return Some(Self::Capture {
                prefix: prefix.to_owned(),
                already,
            });
        }

        // A `use` line is decided by how it starts, not by what is under the cursor: the same
        // `App\Re` means a path to import there and a type reference anywhere else.
        if in_use_statement(before) {
            return Some(Self::Import {
                prefix: prefix.to_owned(),
            });
        }

        // `Repo::$fi` -- a static property, the one member written with a sigil. Ahead of the bare
        // `$` below, which would otherwise read the sigil as the start of a local and offer the
        // file's variables where only this class's statics can go.
        if let Some(head) = before.strip_suffix('$')
            && let Some(head) = head.trim_end().strip_suffix("::")
        {
            let (receiver, root_head) = read_receiver_from(head);
            return Some(Self::StaticMember {
                receiver,
                prefix: prefix.to_owned(),
                dollar: true,
                whole_statement: ends_a_statement(text, offset, root_head),
            });
        }

        // A `$` before the typed prefix means a variable, and it is checked before the access
        // operators: `$re` has no operator to find, and treating it as a name request offers classes
        // where only a local can go.
        if before.ends_with('$') {
            return Some(Self::Variable {
                prefix: prefix.to_owned(),
            });
        }

        // An attribute before the subscript, because `#[` ends in a bracket and reading it as one
        // offers the keys of a shape where an attribute's class goes.
        if in_an_attribute(before) {
            return Some(Self::Name {
                prefix: prefix.to_owned(),
                place: NamePlace::Attribute,
            });
        }

        // A subscript, before the operators: `$row['` ends in a quote and not in an operator, and
        // reading it as a name request offers every class in the project inside a string.
        let unquoted = before
            .strip_suffix('\'')
            .or_else(|| before.strip_suffix('"'));
        if let Some(head) = unquoted.unwrap_or(before).trim_end().strip_suffix('[') {
            return Some(Self::Key {
                receiver: read_receiver(head),
                prefix: prefix.to_owned(),
                quoted: unquoted.is_some(),
            });
        }

        if let Some((receiver, statically)) = callable_pair(before) {
            return Some(Self::Callable {
                receiver,
                prefix: prefix.to_owned(),
                statically,
            });
        }

        let trimmed = before.trim_end();
        if let Some(head) = trimmed.strip_suffix("::") {
            let (receiver, root_head) = read_receiver_from(head);
            return Some(Self::StaticMember {
                receiver,
                prefix: prefix.to_owned(),
                dollar: false,
                whole_statement: ends_a_statement(text, offset, root_head),
            });
        }
        // Nullsafe first: stripping `->` from `?->` leaves a stray `?` and the receiver is then
        // unreadable. Getting this order wrong is a real bug with a real history.
        let arrow = trimmed
            .strip_suffix("?->")
            .or_else(|| trimmed.strip_suffix("->"));
        if let Some(head) = arrow {
            let (receiver, root_head) = read_receiver_from(head);
            return Some(Self::Member {
                receiver,
                prefix: prefix.to_owned(),
                whole_statement: ends_a_statement(text, offset, root_head),
            });
        }

        Some(Self::Name {
            prefix: prefix.to_owned(),
            place: NamePlace::of(trimmed),
        })
    }

    /// The text an offered item replaces.
    ///
    /// Given explicitly rather than left to the client. A client that is not told computes the range
    /// from its own idea of a word, and PHP's names do not match it: for `\App\Nested\Rep` the
    /// client sees the word as `Rep`, replaces only that with the qualified name it was offered, and
    /// leaves `\App\Nested\App\Nested\Repo` in the file.
    pub(crate) fn replaced(&self, offset: TextSize) -> TextRange {
        let typed = TextSize::of(self.prefix());
        // A sigil is written by the person and offered back as part of the label, so it is part of
        // what gets replaced. Two things carry one: a local, and a static property.
        let sigil = TextSize::from(u32::from(matches!(
            self,
            Self::Variable { .. } | Self::StaticMember { dollar: true, .. }
        )));
        TextRange::new(offset - typed - sigil, offset)
    }

    /// How much of the name has already been typed.
    pub(crate) fn prefix(&self) -> &str {
        match self {
            Self::Member { prefix, .. }
            | Self::StaticMember { prefix, .. }
            | Self::Variable { prefix }
            | Self::Name { prefix, .. }
            | Self::Capture { prefix, .. }
            | Self::Import { prefix }
            | Self::Key { prefix, .. }
            | Self::Callable { prefix, .. } => prefix,
        }
    }
}

/// The first element of a two-element array whose quoted second element the cursor is in.
///
/// `[$this, '` and `array($repo, '`, read backwards: a quote, a comma, a receiver, and the bracket
/// that opened the array. The bracket is what makes it the *first* element -- `[$a, $this, '` reads
/// `$this` and then finds a comma where the bracket should be.
///
/// Textual for the reason every receiver here is: the string is being typed, and the tree has no
/// literal for one whose quote is not closed yet.
fn callable_pair(before: &str) -> Option<(Receiver, bool)> {
    let head = before
        .strip_suffix('\'')
        .or_else(|| before.strip_suffix('"'))?;
    let head = head.trim_end().strip_suffix(',')?;
    let (mut receiver, opened) = read_receiver_from(head);
    let opened = opened?.trim_end();
    let array = opened.ends_with('[')
        || opened
            .strip_suffix('(')
            .is_some_and(|head| ends_with_keyword(head.trim_end(), "array"));
    if !array {
        return None;
    }

    let statically = match receiver.steps.as_slice() {
        [] => match receiver.root {
            ReceiverRoot::This | ReceiverRoot::Local(_) => false,
            _ => return None,
        },
        [
            ReceiverStep::Member {
                name,
                kind: StepKind::Constant,
                ..
            },
        ] if name.eq_ignore_ascii_case("class") => true,
        _ => return None,
    };
    receiver.steps.clear();
    Some((receiver, statically))
}

/// Splits the identifier immediately before the cursor from what precedes it.
///
/// A namespace separator is part of the prefix: someone typing `App\Re` has typed a qualified name,
/// and treating `Re` as the whole prefix would offer everything called `Re…` in the project.
fn split_identifier(before: &str) -> (&str, &str) {
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_' || *ch == '\\')
        .last()
        .map_or(before.len(), |(index, _)| index);

    (&before[start..], &before[..start])
}

/// The names a closure's `use (...)` list already holds, when the cursor is inside one.
///
/// The `(` is what tells this `use` from the other two: neither an import nor a trait use has one,
/// and nothing else in PHP writes the keyword before a parenthesis. Open, because a `)` between it
/// and the cursor means the list is finished and what follows is the closure's body.
fn captured_so_far(before: &str) -> Option<Vec<String>> {
    // The statement the cursor is in, which is as far back as any of this scanner looks -- and what
    // keeps the search for the bracket from walking the whole file on a request that is not one of
    // these, which is nearly all of them.
    let before = before.rsplit([';', '{', '}']).next()?;
    let at = before.rfind('(')?;
    if before[at..].contains(')') {
        return None;
    }
    if !ends_with_keyword(before[..at].trim_end(), "use") {
        return None;
    }
    // Every `$name` between the bracket and the cursor. The one being typed has no `$` yet, so
    // nothing here can swallow it.
    let mut already = Vec::new();
    let mut rest = &before[at..];
    while let Some(sigil) = rest.find('$') {
        let tail = &rest[sigil + 1..];
        let end = tail
            .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .unwrap_or(tail.len());
        if end > 0 {
            already.push(tail[..end].to_owned());
        }
        rest = &tail[end..];
    }
    Some(already)
}

/// Whether the cursor's line is a `use` statement.
///
/// The line, not the file: a `use` inside a class body imports a trait, and its own line still
/// starts with the keyword. Which of the two it is takes the tree to say, so the split is made
/// where the view is -- see the dispatch in `crate::completion`.
fn in_use_statement(before: &str) -> bool {
    before
        .rsplit(['\n', ';', '{', '}'])
        .next()
        .is_some_and(|line| {
            // The opening tag is not a statement separator, so a `use` written on the same line as
            // it would otherwise not look like the start of a line at all.
            let line = line
                .trim_start()
                .strip_prefix("<?php")
                .unwrap_or(line)
                .trim_start();
            line.strip_prefix("use ")
                .or_else(|| line.strip_prefix("use\t"))
                .is_some()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(source: &str) -> CompletionContext {
        // The cursor is wherever the marker is; the marker is not part of the text.
        let offset = source.find('|').expect("the test marks a cursor with |");
        let text = source.replace('|', "");
        CompletionContext::at(&text, TextSize::from(offset as u32)).expect("in range")
    }

    /// The receiver of a member request, rendered as `root->step->step`.
    fn receiver(source: &str) -> String {
        let receiver = match at(source) {
            CompletionContext::Member { receiver, .. }
            | CompletionContext::StaticMember { receiver, .. } => receiver,
            other => panic!("not a member request: {other:?}"),
        };

        let mut out = match &receiver.root {
            ReceiverRoot::This => "<this>".to_owned(),
            ReceiverRoot::SelfClass => "<self>".to_owned(),
            ReceiverRoot::Parent => "<parent>".to_owned(),
            ReceiverRoot::Local(name) => format!("${name}"),
            ReceiverRoot::Class(name) => name.clone(),
            ReceiverRoot::Function { name, arguments } => {
                format!("{name}({})", arguments.join(", "))
            }
            ReceiverRoot::Unknown => "<unreadable>".to_owned(),
        };
        for step in &receiver.steps {
            out.push_str(&match step {
                ReceiverStep::Member {
                    name,
                    kind: StepKind::Property,
                    ..
                } => format!("->${name}"),
                ReceiverStep::Member {
                    name,
                    kind: StepKind::StaticProperty,
                    ..
                } => format!("::${name}"),
                ReceiverStep::Member {
                    name,
                    kind: StepKind::Constant,
                    ..
                } => format!("::{name}"),
                // The arguments are rendered because they are part of what the step means: two calls
                // written with different ones can reach two different classes.
                ReceiverStep::Member {
                    name,
                    kind: StepKind::MethodReturn,
                    arguments,
                } => format!(
                    "->{name}({})",
                    arguments
                        .iter()
                        .map(|argument| match argument {
                            Some(written) => format!("'{written}'"),
                            None => "?".to_owned(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                ReceiverStep::Invoked => "()".to_owned(),
                ReceiverStep::Element(Some(ScannedKey::Literal(key))) => format!("[{key}]"),
                ReceiverStep::Element(Some(ScannedKey::Variable(name))) => format!("[${name}]"),
                ReceiverStep::Element(None) => "[?]".to_owned(),
            });
        }
        out
    }

    #[test]
    fn the_prefix_is_what_has_been_typed_after_the_operator() {
        assert_eq!(at("<?php $this->fi|").prefix(), "fi");
        assert_eq!(at("<?php Repo::|").prefix(), "");
        assert_eq!(at("<?php App\\Re|").prefix(), "App\\Re");
    }

    #[test]
    fn this_self_static_and_parent_are_read_as_the_class_they_mean() {
        assert_eq!(receiver("<?php $this->fi|"), "<this>");
        assert_eq!(receiver("<?php self::fi|"), "<self>");
        assert_eq!(receiver("<?php static::fi|"), "<this>");
        assert_eq!(receiver("<?php parent::fi|"), "<parent>");
    }

    #[test]
    fn a_nullsafe_arrow_is_stripped_before_the_plain_one() {
        // `?->` ends in `->`, so stripping the plain arrow first leaves a stray `?` and the receiver
        // becomes unreadable. This exact order bug cost a previous version every nullsafe feature.
        assert_eq!(receiver("<?php $this?->fi|"), "<this>");
        assert_eq!(receiver("<?php $this->repo?->fi|"), "<this>->$repo");
    }

    #[test]
    fn a_property_chain_is_read_step_by_step() {
        // The reason this is a chain and not a single token: reading only `repo` and looking it up as
        // a class name happens to work whenever a class shares the property's name, and is wrong the
        // rest of the time. Class names are matched case-insensitively, so it works often enough to
        // look correct.
        assert_eq!(receiver("<?php $this->repo->fi|"), "<this>->$repo");
        assert_eq!(receiver("<?php $this->a->b->c->fi|"), "<this>->$a->$b->$c");
    }

    #[test]
    fn a_calls_result_is_a_step_of_its_own_kind() {
        assert_eq!(receiver("<?php $this->find()->fi|"), "<this>->find()");
        assert_eq!(
            receiver("<?php $this->repo->find(1)->na|"),
            "<this>->$repo->find(?)"
        );
    }

    /// The strings and class names, because for some methods an argument *is* part of the type.
    /// Everything else is a slot this cannot read, which is a `?` and not a guess.
    #[test]
    fn the_arguments_a_call_was_written_with_are_read() {
        assert_eq!(
            receiver("<?php $this->get(Repo::class)->na|"),
            "<this>->get('Repo')"
        );
        assert_eq!(
            receiver("<?php $this->getStorage('node')->na|"),
            "<this>->getStorage('node')"
        );
        assert_eq!(
            receiver("<?php $this->get(\\App\\Repo::class)->na|"),
            "<this>->get('App\\Repo')"
        );
        assert_eq!(
            receiver("<?php $this->find($id, 'node')->na|"),
            "<this>->find(?, 'node')"
        );
    }

    /// Nesting and quotes are stepped over, because a comma inside either belongs to it.
    #[test]
    fn a_comma_inside_a_call_or_a_string_does_not_split_the_list() {
        assert_eq!(
            receiver("<?php $this->find(inner($x), 2)->na|"),
            "<this>->find(?, ?)"
        );
        assert_eq!(
            receiver("<?php $this->find('a, b')->na|"),
            "<this>->find('a, b')"
        );
    }

    /// A named argument moves what the positions mean, so none of them is read: guessing which slot
    /// a contract meant is worse than saying nothing.
    #[test]
    fn a_named_argument_makes_the_whole_list_unreadable() {
        assert_eq!(
            receiver("<?php $this->get(id: Repo::class)->na|"),
            "<this>->get()"
        );
    }

    #[test]
    fn a_variable_and_a_class_are_told_apart_by_the_dollar() {
        assert_eq!(receiver("<?php $repo->fi|"), "$repo");
        assert_eq!(receiver("<?php Repo::fi|"), "Repo");
        assert_eq!(receiver("<?php $repo::fi|"), "$repo");
    }

    /// A leading separator is part of the name and is kept.
    ///
    /// This line used to assert the opposite -- that `\App\Repo::` reads as `App\Repo` -- which
    /// pinned the bug rather than a rule: the separator is what makes the name absolute, and without
    /// it `\Drupal` inside `namespace Drupal\app_core;` resolves as `Drupal\app_core\Drupal` and is
    /// nothing at all. Completion after `\Drupal::` offered nothing on a real project, at 256 sites
    /// of its own code, while `Drupal::` reached through an import offered every service.
    #[test]
    fn a_leading_separator_belongs_to_the_name() {
        assert_eq!(receiver("<?php \\App\\Repo::fi|"), "\\App\\Repo");
        assert_eq!(receiver("<?php \\Drupal::se|"), "\\Drupal");
        assert_eq!(receiver("<?php App\\Repo::fi|"), "App\\Repo");
    }

    /// A parenthesised `new` names its own class.
    #[test]
    fn a_new_expression_is_a_receiver_and_other_parentheses_are_not() {
        assert_eq!(receiver("<?php (new Repo())->fi|"), "Repo");
        assert_eq!(receiver("<?php (new Repo)->fi|"), "Repo");
        assert_eq!(
            receiver("<?php (new \\App\\Repo($a, $b))->fi|"),
            "\\App\\Repo"
        );
        assert_eq!(receiver("<?php (new Repo())->one()->fi|"), "Repo->one()");
        // The guesses this scanner refuses: a class named by a variable, an anonymous class, and a
        // parenthesised expression that is not an instantiation at all.
        assert_eq!(receiver("<?php (new $class())->fi|"), "<unreadable>");
        assert_eq!(receiver("<?php (new class { })->fi|"), "<unreadable>");
    }

    #[test]
    fn a_static_call_can_start_a_chain() {
        assert_eq!(receiver("<?php Factory::make()->fi|"), "Factory->make()");
    }

    #[test]
    fn a_subscript_asks_for_a_key() {
        assert!(matches!(
            at("<?php $row[|"),
            CompletionContext::Key { quoted: false, .. }
        ));
        assert!(matches!(
            at("<?php $row['|"),
            CompletionContext::Key { quoted: true, .. }
        ));
        assert_eq!(at("<?php $row['ap|").prefix(), "ap");
    }

    #[test]
    fn a_subscript_is_a_step_carrying_the_key_when_one_was_written() {
        assert_eq!(receiver("<?php $items[0]->fi|"), "$items[0]");
        assert_eq!(receiver("<?php $row['first']->fi|"), "$row[first]");
        assert_eq!(receiver(r#"<?php $row["first"]->fi|"#), "$row[first]");
        // A variable key names no *field* and still identifies the slot: `$row[$key]` read twice
        // is the same element, which is what lets a guard written about it narrow the read below.
        // The two readings are kept apart downstream -- `ChainKey::literal` is what a shape's keys
        // are looked up with, and a variable answers `None` there.
        assert_eq!(receiver("<?php $row[$key]->fi|"), "$row[$key]");
        // Anything else is a slot this scanner cannot identify, and says so.
        assert_eq!(receiver("<?php $row[self::KEY]->fi|"), "$row[?]");
        assert_eq!(receiver("<?php $row[$a->b]->fi|"), "$row[?]");
    }

    #[test]
    fn subscripts_stack_and_apply_to_the_step_they_are_written_on() {
        assert_eq!(receiver("<?php $grid[0][1]->fi|"), "$grid[0][1]");
        assert_eq!(receiver("<?php $this->rows['a']->fi|"), "<this>->$rows[a]");
        assert_eq!(receiver("<?php $repo->all()[0]->fi|"), "$repo->all()[0]");
        assert_eq!(
            receiver("<?php $this->map['a'][0]->name|"),
            "<this>->$map[a][0]"
        );
    }

    /// A free function's result is a root of its own: the name is in a different table from every
    /// other root, and what it gives back takes a lookup rather than a class the characters name.
    #[test]
    fn a_function_call_is_a_root_with_its_arguments() {
        assert_eq!(receiver("<?php helper()->fi|"), "helper()");
        assert_eq!(receiver("<?php reset($rows)->fi|"), "reset($rows)");
        assert_eq!(
            receiver("<?php \\Acme\\make(1, $x)->fi|"),
            "\\Acme\\make(1, $x)"
        );
        // The steps after it are read as they are for any other root, subscripts included: an
        // element of a call's result used to be unreadable because the call was.
        assert_eq!(
            receiver("<?php current($rows)->row->fi|"),
            "current($rows)->$row"
        );
        assert_eq!(receiver("<?php helper()[0]->fi|"), "helper()[0]");
    }

    #[test]
    fn what_this_scanner_cannot_read_it_says_so_about() {
        // Each of these has a type in principle and none of it is in the characters before the
        // cursor. Offering the enclosing class's members instead is the guess worth avoiding.

        // A callable held in an element: the `(...)` belongs to the element, not to the name.
        assert_eq!(receiver("<?php $handlers['a']()->fi|"), "<unreadable>");
        assert_eq!(receiver("<?php 'text'->fi|"), "<unreadable>");
    }

    /// A parenthesised expression is read by reading what is inside it.
    ///
    /// `($this->repo)->` was one of the refusals above for as long as every parenthesis that was
    /// not a `new` was one. It is not a guess: unlike the rest of this walk's input the text inside
    /// the brackets is a *complete* expression, and reading it is the same walk again.
    #[test]
    fn a_parenthesised_expression_is_read_by_reading_inside_it() {
        assert_eq!(receiver("<?php ($this->repo)->fi|"), "<this>->$repo");
        assert_eq!(receiver("<?php ($repo)->fi|"), "$repo");
        assert_eq!(receiver("<?php (clone $repo)->fi|"), "$repo");
        assert_eq!(receiver("<?php (self::of($id))->fi|"), "<self>->of(?)");
        // PHP 8.5's pipe belongs here too -- its value is the call it makes, which is what reading
        // right to left reaches with nothing about the pipe to know -- and cannot be written in
        // this test: the helper takes the first `|` for the cursor, and `|>` holds one. It is
        // pinned in the completion fixtures instead.
        //
        // And what is applied to the brackets afterwards belongs to the same chain.
        assert_eq!(
            receiver("<?php ($this->repo)->one()->fi|"),
            "<this>->$repo->one()"
        );
        assert_eq!(receiver("<?php ($this->rows)[0]->fi|"), "<this>->$rows[0]");
    }

    /// What stays refused, because the value is not the trailing operand.
    #[test]
    fn a_parenthesised_expression_whose_value_is_not_its_last_operand_is_refused() {
        // A choice, in all three spellings: the honest answer is a union and this vocabulary has
        // one slot. A `?->` inside is not one of them.
        assert_eq!(receiver("<?php ($repo ?? $other)->fi|"), "<unreadable>");
        assert_eq!(receiver("<?php ($repo ?: $other)->fi|"), "<unreadable>");
        assert_eq!(
            receiver("<?php ($cond ? $repo : $other)->fi|"),
            "<unreadable>"
        );
        assert_eq!(receiver("<?php ($this?->repo)->fi|"), "<this>->$repo");
        // A closure is a `Closure`, not the expression it returns.
        assert_eq!(receiver("<?php (fn () => $repo)->fi|"), "<unreadable>");
        // `new $class()` names the class the *variable* holds, which is why the reader above
        // refuses it -- falling through to this walk would read it as invoking a local.
        assert_eq!(receiver("<?php (new $class())->fi|"), "<unreadable>");
        assert_eq!(receiver("<?php (new class { })->fi|"), "<unreadable>");
    }

    #[test]
    fn a_bare_prefix_asks_for_a_name() {
        assert_eq!(
            at("<?php $x = Re|"),
            CompletionContext::Name {
                prefix: "Re".to_owned(),
                place: NamePlace::Anywhere,
            }
        );
    }

    #[test]
    fn what_stands_before_a_name_says_what_else_may_go_there() {
        let place = |source| match at(source) {
            CompletionContext::Name { place, .. } => place,
            other => panic!("not a name request: {other:?}"),
        };

        for source in ["<?php find(Re|", "<?php find(1, Re|", "<?php find(1,Re|"] {
            assert_eq!(place(source), NamePlace::Argument, "{source}");
        }
        for source in [
            "<?php re|",
            "<?php $a = 1; re|",
            "<?php class A { pub|",
            "<?php if ($a) { } els|",
        ] {
            assert_eq!(place(source), NamePlace::Statement, "{source}");
        }
        for source in ["<?php find(1 + Re|", "<?php $x instanceof Re|"] {
            assert_eq!(place(source), NamePlace::Anywhere, "{source}");
        }
    }

    /// Where PHP allows a type and nothing else, which is where its own words for one belong.
    ///
    /// The negative half is the load-bearing one: three of these shapes end in the same character
    /// as a type position and are not one, and offering `void` in a ternary is the failure this
    /// would have.
    #[test]
    fn a_type_position_is_told_from_the_expression_that_looks_like_one() {
        let place = |source| match at(source) {
            CompletionContext::Name { place, .. } => place,
            other => panic!("not a name request: {other:?}"),
        };

        for (source, expected) in [
            ("<?php function f(): Re|", TypePlace::Return),
            ("<?php function f(): ?Re|", TypePlace::Return),
            ("<?php $g = fn (): Re|", TypePlace::Return),
            ("<?php function f(Re|", TypePlace::Parameter),
            ("<?php function f(int $a, Re|", TypePlace::Parameter),
            ("<?php function f(int|Re|", TypePlace::Parameter),
            (
                "<?php function __construct(private readonly Re|",
                TypePlace::Parameter,
            ),
            ("<?php class A { private Re|", TypePlace::Property),
            ("<?php class A { public readonly Re|", TypePlace::Property),
            ("<?php enum Status: Re|", TypePlace::EnumBacking),
            ("<?php try { } catch (Re|", TypePlace::Catch),
        ] {
            assert_eq!(place(source), NamePlace::Type(expected), "{source}");
        }

        // A colon that introduces no type, a bracket that opens a call rather than a parameter
        // list, and a parameter whose type is already written.
        for source in [
            "<?php $x = $a ? 1 : Re|",
            "<?php f(where: Re|",
            "<?php f(Re|",
            "<?php function f(int $a = Re|",
            "<?php $x = $a | Re|",
        ] {
            assert_ne!(
                std::mem::discriminant(&place(source)),
                std::mem::discriminant(&NamePlace::Type(TypePlace::Return)),
                "{source}"
            );
        }
    }

    /// Only a class can be built, so only a class is offered -- and only where `new` is a word of
    /// its own, which is what keeps `$renew` from reading as one.
    #[test]
    fn new_is_read_as_the_one_place_a_class_is_the_only_answer() {
        let place = |source| match at(source) {
            CompletionContext::Name { place, .. } => place,
            other => panic!("not a name request: {other:?}"),
        };

        for source in ["<?php new Re|", "<?php $x = new Re|", "<?php NEW Re|"] {
            assert_eq!(place(source), NamePlace::New, "{source}");
        }
        // A name that merely ends in the word, and the keyword with nothing typed after it yet.
        assert_ne!(place("<?php $x = renew Re|"), NamePlace::New);
        assert_ne!(place("<?php new|"), NamePlace::New);

        assert!(NamePlace::New.accepts(SymbolKind::Class));
        for kind in [
            SymbolKind::Interface,
            SymbolKind::Trait,
            SymbolKind::Enum,
            SymbolKind::Function,
            SymbolKind::Constant,
        ] {
            assert!(!NamePlace::New.accepts(kind), "{kind:?}");
            assert!(NamePlace::Anywhere.accepts(kind), "{kind:?}");
        }
    }

    #[test]
    fn a_use_line_asks_for_a_path_to_import() {
        assert_eq!(
            at("<?php use App\\Re|"),
            CompletionContext::Import {
                prefix: "App\\Re".to_owned()
            }
        );
    }

    #[test]
    fn a_use_inside_a_class_is_still_an_import_line() {
        // It imports a trait rather than a class, but what is being asked for is the same: a path.
        assert_eq!(
            at("<?php class C { use App\\Ti| }"),
            CompletionContext::Import {
                prefix: "App\\Ti".to_owned()
            }
        );
    }

    #[test]
    fn what_is_replaced_is_everything_already_typed() {
        // The bug this pins: a client works the range out from its own idea of a word, which for
        // `\App\Nested\Rep` is `Rep` -- so replacing only that with the qualified name it was
        // offered leaves `\App\Nested\App\Nested\Repo` in the file.
        let replaced = |source: &str| {
            let offset = source.find('|').expect("the test marks a cursor with |");
            let text = source.replace('|', "");
            let at = TextSize::from(offset as u32);
            let range = CompletionContext::at(&text, at)
                .expect("in range")
                .replaced(at);
            text[usize::from(range.start())..usize::from(range.end())].to_owned()
        };

        assert_eq!(replaced("<?php new Rep|"), "Rep");
        assert_eq!(
            replaced("<?php new \\App\\Nested\\Rep|"),
            "\\App\\Nested\\Rep"
        );
        assert_eq!(
            replaced("<?php /** @var \\App\\Nested\\Rep| */"),
            "\\App\\Nested\\Rep"
        );
        assert_eq!(replaced("<?php use App\\Nes|"), "App\\Nes");
        // A member has no separators of its own, and a variable's sigil is part of what is offered.
        assert_eq!(replaced("<?php $this->fi|"), "fi");
        assert_eq!(replaced("<?php $ap|"), "$ap");
    }
}
