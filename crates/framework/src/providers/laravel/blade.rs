//! A Blade template read as PHP, byte for byte in place.
//!
//! Blade is HTML with PHP expressions in it -- `{{ $user->name }}`, `@if ($user->isAdmin())`,
//! `@foreach ($posts as $post)` -- and every question a reader asks of a template is about those
//! expressions. So the template is handed to the PHP pipeline as a PHP file of **the same length,
//! with every newline where it was**: the expressions keep their bytes, and everything around them
//! -- the markup, and Blade's own delimiters -- is overwritten with whitespace, or with the few
//! characters that make the expressions a program: `;`, `{`, `}`, a keyword. An offset in the
//! template is then the same offset in the PHP, and nothing above needs to translate a position.
//!
//! What each construct becomes, the delimiters overwritten and the expression untouched:
//!
//! ```text
//! {{ $user->name }}            ->    $user->name ;
//! @if ($user)                  ->  if ($user)      and a `{` in the next blank byte
//! @foreach ($posts as $post)   ->  foreach ($posts as $post)    and a `{`
//! @endif  @endforeach          ->       }           }
//! @include('row', ['p' => $p]) ->          ['row', ['p' => $p]]  and a `;`
//! {{-- @var User $user --}}    ->  /** @var User $user */
//! ```
//!
//! Where a character has to follow an expression -- the `{` after a condition, the `;` after a
//! directive's arguments -- it is owed, and written into the next byte that was going to be blank,
//! which every construct supplies: each one begins with a delimiter. The opening tag is placed last,
//! in the first run of blanks before the first expression.
//!
//! Markup holding characters outside ASCII is kept verbatim inside a `/* */` comment rather than
//! blanked, because a client counts columns in UTF-16: two spaces for one `é` would move every
//! column after it on the line, and a template written in Ukrainian is all such characters.
//!
//! Only the directives Blade ships are read. A name it does not know is left as markup -- `@media`
//! in a stylesheet, Alpine's `@click` -- because what follows one is not PHP.

use std::{collections::VecDeque, ops::Range};

/// What a component tag is written as: `<x-empty-state>` is `x(\empty_state)`, in the three bytes
/// `<x-` is. The name is one fully qualified name -- `.` written `\`, `-` written `_` -- because PHP
/// reads a keyword inside one as part of the name, and `empty`, `list` and `default` are names a
/// component has.
pub(super) const COMPONENT: &str = "x";

/// What an include is written as: `@include('posts.row', ['post' => $post])` is
/// `_inc('posts.row', ['post' => $post])`, in four of the eight bytes `@include` is. A call of its
/// own rather than `view()`, because an include hands the template every variable of the one
/// including it besides -- Blade passes `get_defined_vars()` -- which a `view()` written in a
/// controller does not, and neither does `@component`.
pub(super) const INCLUDE: &str = "_inc";

/// What a Livewire tag is written as: `<livewire:likes>` is `livewire(\likes)`, in the ten bytes
/// `<livewire:` is.
pub(super) const LIVEWIRE: &str = "livewire";

/// The template as PHP, and how many constructs could not be written in place.
///
/// A construct that does not fit -- a condition whose `{` has nowhere to go before the next
/// expression -- leaves the PHP malformed at that point, and the parser's own recovery takes it
/// from there. Counted so that a corpus can say how often that is.
pub(crate) fn to_php(template: &str) -> (String, usize) {
    let masker = Masker::masked(template);
    let failures = masker.failures.len();
    // Every write is ASCII over ASCII, or a whole character blanked or kept, so the bytes are
    // still UTF-8; the conversion cannot fail, and a template it somehow did is markup.
    match String::from_utf8(masker.out) {
        Ok(php) => (php, failures),
        Err(_) => (blank_text(template), failures + 1),
    }
}

/// What a directive becomes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Directive {
    /// Its condition is PHP and narrows: `@if`, `@elseif`, `@while`.
    Condition(&'static str),
    /// A loop whose header binds names: `@foreach`, `@forelse`, `@for`.
    Loop(&'static str),
    /// A block whose arguments are not a condition PHP could narrow by: `@isset`, `@auth`, `@can`.
    /// Its arguments become a list, so any number of them is one expression.
    Block,
    /// The `else` of a block: `@else`, and `@elsecan` with its arguments.
    Else,
    /// The end of anything that opened a brace.
    End,
    /// `@forelse`'s branch for an empty list -- a block of its own after the loop's -- or, with
    /// arguments, a block of its own.
    Empty,
    Switch,
    Case,
    Default,
    Break,
    Continue,
    /// Raw PHP: `@php ... @endphp`. With arguments it is an expression like any other.
    Php,
    /// Markup Blade prints as written.
    Verbatim,
    /// A template rendered inside this one with variables of its own: written as the call it is,
    /// so that the index records it where it records a controller's -- [`INCLUDE`] for one that
    /// also hands over every variable of this one, `view` for one that does not.
    Include(&'static str),
    /// `@use('App\Models\Post')`: an import, which Blade compiles to the `use` statement it is.
    Use,
    /// Anything else Blade ships: its arguments are PHP, and it opens nothing.
    Statement,
}

impl Directive {
    /// By its name in any case: Blade finds a directive's compiler with `method_exists`, which PHP
    /// answers case-insensitively, so `@endGuest` ends `@guest`.
    fn named(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "if" => Self::Condition("if"),
            "elseif" => Self::Condition("}elseif"),
            "while" => Self::Condition("while"),
            "foreach" | "forelse" => Self::Loop("foreach"),
            "for" => Self::Loop("for"),
            "unless" | "isset" | "auth" | "guest" | "can" | "cannot" | "canany" | "env"
            | "production" | "error" | "hassection" | "sectionmissing" | "once" | "session"
            | "context" => Self::Block,
            "else" | "elsecan" | "elsecannot" | "elsecanany" | "elseauth" | "elseguest" => {
                Self::Else
            }
            "endif" | "endunless" | "endisset" | "endempty" | "endauth" | "endguest" | "endcan"
            | "endcannot" | "endcanany" | "endenv" | "endproduction" | "enderror" | "endonce"
            | "endsession" | "endcontext" | "endforeach" | "endforelse" | "endfor" | "endwhile"
            | "endswitch" => Self::End,
            "empty" => Self::Empty,
            "switch" => Self::Switch,
            "case" => Self::Case,
            "default" => Self::Default,
            "break" => Self::Break,
            "continue" => Self::Continue,
            "php" => Self::Php,
            "use" => Self::Use,
            "verbatim" => Self::Verbatim,
            "include" | "includeif" => Self::Include(INCLUDE),
            // `@component('alert', [...])` renders a template with the variables it is handed and a
            // slot -- and nothing else of this one's.
            "component" => Self::Include("view"),
            "includewhen" | "includeunless" | "includefirst" | "each" | "extends" | "section"
            | "endsection" | "show" | "stop" | "overwrite" | "append" | "yield" | "parent"
            | "push" | "endpush" | "pushif" | "endpushif" | "pushonce" | "endpushonce"
            | "prepend" | "endprepend" | "prependonce" | "endprependonce" | "stack"
            | "endcomponent" | "slot" | "endslot" | "componentfirst" | "endcomponentfirst"
            | "props" | "aware" | "csrf" | "method" | "json" | "js" | "lang" | "choice"
            | "inject" | "class" | "style" | "checked" | "selected" | "disabled" | "readonly"
            | "required" | "vite" | "vitereactrefresh" | "fragment" | "endfragment" | "dd"
            | "dump" | "livewire" | "livewirestyles" | "livewirescripts" | "persist"
            | "endpersist" | "teleport" | "endteleport" => Self::Statement,
            _ => return None,
        })
    }
}

/// What reading one construct did.
enum Read {
    /// Written; everything before `at` is accounted for.
    Written,
    /// Not a construct after all: the bytes stay markup, and the scan moves past them.
    Markup(usize),
}

struct Masker<'a> {
    source: &'a [u8],
    out: Vec<u8>,
    at: usize,
    /// Where the markup not yet blanked begins.
    markup: usize,
    /// Characters an expression needs after it, waiting for the next blank byte.
    owed: VecDeque<u8>,
    /// The comments written, which hold markup or a docblock and are no place for the opening tag.
    comments: Vec<Range<usize>>,
    /// Whether anything PHP has to read as code has been written yet.
    coded: bool,
    /// Whether the opening tag has been written.
    opened: bool,
    /// The blocks open, innermost last: whether each was written with braces.
    blocks: Vec<bool>,
    /// Where a construct could not be written in place.
    failures: Vec<usize>,
    /// Every directive read, in order: where its name is, the name in lower case, and whether it
    /// has arguments -- what [`pairing`] pairs.
    directives: Vec<(Range<usize>, String, bool)>,
    /// A raw block whose end is nowhere: `@php` or `@verbatim` read to the end of the file.
    unended: Vec<(Range<usize>, String)>,
}

impl<'a> Masker<'a> {
    /// The template read through, and the opening tag placed.
    fn masked(template: &'a str) -> Self {
        let mut masker = Masker {
            source: template.as_bytes(),
            out: template.as_bytes().to_vec(),
            at: 0,
            markup: 0,
            owed: VecDeque::new(),
            comments: Vec::new(),
            coded: false,
            opened: false,
            blocks: Vec::new(),
            failures: Vec::new(),
            directives: Vec::new(),
            unended: Vec::new(),
        };
        masker.run();
        masker.open();
        masker
    }
}

impl Masker<'_> {
    fn run(&mut self) {
        while self.at < self.source.len() {
            let rest = &self.source[self.at..];
            let read = if rest.starts_with(b"{{--") {
                self.comment()
            } else if rest.starts_with(b"@{{") || rest.starts_with(b"@{!!") {
                self.escaped_echo()
            } else if rest.starts_with(b"{{") {
                self.echo(b"{{", b"}}")
            } else if rest.starts_with(b"{!!") {
                self.echo(b"{!!", b"!!}")
            } else if rest.starts_with(b"<?php") || rest.starts_with(b"<?=") {
                self.raw_php()
            } else if rest.starts_with(b"<x-") {
                self.component(b"<x-", COMPONENT)
            } else if rest.starts_with(b"<livewire:") {
                self.component(b"<livewire:", LIVEWIRE)
            } else if rest[0] == b'@' {
                self.directive()
            } else {
                Read::Markup(1)
            };
            match read {
                Read::Written => self.markup = self.at,
                Read::Markup(skip) => self.at += skip,
            }
        }
        self.blank(self.markup..self.source.len());
        if self.owed.iter().any(|byte| !byte.is_ascii_whitespace()) {
            self.failures.push(self.at);
        }
    }

    /// `{{-- ... --}}`, as the doc comment a `@var` in it is written for.
    fn comment(&mut self) -> Read {
        let start = self.at;
        let Some(end) = find(self.source, start + 4, b"--}}").map(|end| end + 4) else {
            return Read::Markup(4);
        };
        self.blank(self.markup..start);
        let inner = &self.source[start + 4..end - 4];
        if self.owed.is_empty() && find(inner, 0, b"*/").is_none() {
            // A template that begins with its `@var` has nowhere before it for the opening tag,
            // and a docblock read as markup annotates nothing -- so the tag goes in the comment's
            // own delimiter. `*/` is two bytes shorter than `--}}`, which pays for the tag; the text
            // between moves one byte right, which a single line can afford and a newline cannot.
            if !self.coded && !self.opened && !inner.contains(&b'\n') {
                let moved = inner.to_vec();
                self.out[start..start + 5].copy_from_slice(b"<?/**");
                self.out[start + 5..start + 5 + moved.len()].copy_from_slice(&moved);
                self.out[end - 3..end].copy_from_slice(b"*/ ");
                self.opened = true;
                self.coded = true;
            } else if !self.coded && !self.opened {
                // Over several lines there is no byte to spare, and the tag is worth more than the
                // docblock: a plain comment carries it, and the directive after keeps its block.
                self.out[start..start + 4].copy_from_slice(b"<?/*");
                self.out[end - 4..end].copy_from_slice(b" */ ");
                self.opened = true;
                self.coded = true;
            } else {
                self.out[start..start + 4].copy_from_slice(b"/** ");
                self.out[end - 4..end].copy_from_slice(b" */ ");
                self.comments.push(start..end);
            }
        } else {
            self.blank(start..end);
        }
        self.at = end;
        Read::Written
    }

    /// `@{{ ... }}`, which Blade prints as written.
    fn escaped_echo(&mut self) -> Read {
        let start = self.at;
        let close: &[u8] = match self.source[start + 1..].starts_with(b"{!!") {
            true => b"!!}",
            false => b"}}",
        };
        match find(self.source, start + 3, close) {
            Some(end) => Read::Markup(end + close.len() - start),
            None => Read::Markup(3),
        }
    }

    /// `{{ expression }}` and `{!! expression !!}`: the expression, and a `;` where it closes.
    fn echo(&mut self, open: &[u8], close: &[u8]) -> Read {
        let start = self.at;
        let Some(end) = find(self.source, start + open.len(), close) else {
            return Read::Markup(open.len());
        };
        self.blank(self.markup..start + open.len());
        self.code(start + open.len()..end);
        self.coded = true;
        self.out[end] = b';';
        self.out[end + 1..end + close.len()].fill(b' ');
        self.at = end + close.len();
        Read::Written
    }

    /// `<?php ... ?>` written in a template: already PHP, and only its tags go.
    fn raw_php(&mut self) -> Read {
        let start = self.at;
        let open = match self.source[start..].starts_with(b"<?php") {
            true => 5,
            false => 3,
        };
        self.blank(self.markup..start + open);
        let end = find(self.source, start + open, b"?>").unwrap_or(self.source.len());
        self.code(start + open..end);
        if end + 2 <= self.source.len() {
            self.out[end..end + 2].copy_from_slice(b"; ");
        }
        self.at = (end + 2).min(self.source.len());
        Read::Written
    }

    /// A component tag, written as the call it is: `<x-alert :post="$post" type="info" />` becomes
    /// `x(\alert,post:$post,type:"info");` in place.
    ///
    /// The component's name is one name -- see [`COMPONENT`] -- which the index reads back as text,
    /// and every attribute that can be a named argument becomes one: a bound attribute
    /// (`:post`) with its PHP expression, a plain one with its string, an attribute that is one echo
    /// (`class="{{ $c }}"`) with the echo's expression. A kebab-cased name is the camel-cased
    /// variable the component sees, which is never longer, so it fits. What cannot be an argument --
    /// a valueless flag, `::class` (Blade's escape for Alpine), `wire:model`, `@click`, text with an
    /// echo inside -- is blank.
    ///
    /// Slots and a package's components (`<x-slot:title>`, `<x-mail::button>`) stay markup.
    ///
    /// `opener` is what the tag begins with and `call` what it is written as, which has to fit in
    /// the opener with its parenthesis and separator -- or with its parenthesis and an owed byte.
    fn component(&mut self, opener: &[u8], call: &str) -> Read {
        debug_assert!(call.len() + 2 <= opener.len());
        let start = self.at;
        let Some(end) = self.tag_end(start) else {
            return Read::Markup(opener.len());
        };
        let name_start = start + opener.len();
        let name_end = name_start
            + self.source[name_start..end]
                .iter()
                .take_while(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
                })
                .count();
        let name = &self.source[name_start..name_end];
        let ends_name = matches!(self.source.get(name_end), Some(b'/' | b'>'))
            || self
                .source
                .get(name_end)
                .is_some_and(u8::is_ascii_whitespace);
        if !ends_name || !component_name(name) {
            return Read::Markup(end - start);
        }
        self.blank(self.markup..start);
        // A character still owed -- the `;` of an `@props(...)` the tag follows with only a newline
        // between -- takes the `<`, and the name goes without its leading separator: a qualified
        // name is one token as surely as a fully qualified one.
        let owed = match self
            .owed
            .iter()
            .filter(|byte| !byte.is_ascii_whitespace())
            .count()
        {
            1 => self
                .owed
                .iter()
                .find(|byte| !byte.is_ascii_whitespace())
                .copied(),
            _ => None,
        };
        if owed.is_some() {
            self.owed.clear();
        }
        self.settle();
        for index in start..end {
            if !matches!(self.source[index], b'\n' | b'\r') {
                self.out[index] = b' ';
            }
        }
        // Right against the name, so that the call reads as one: `   x(\alert` and not
        // `x(   \alert`, which a template reader takes the name of from where it begins.
        let call_at = name_start - call.len() - 2 + usize::from(owed.is_some());
        if let Some(owed) = owed {
            self.out[call_at - 1] = owed;
        }
        self.out[call_at..call_at + call.len()].copy_from_slice(call.as_bytes());
        self.out[call_at + call.len()] = b'(';
        if owed.is_none() {
            self.out[call_at + call.len() + 1] = b'\\';
        }
        for index in name_start..name_end {
            self.out[index] = match self.source[index] {
                b'.' => b'\\',
                b'-' => b'_',
                byte => byte,
            };
        }
        self.coded = true;

        let closes_itself = end >= 2 && self.source[end - 2] == b'/';
        let tail = if closes_itself { end - 2 } else { end - 1 };
        let mut index = name_end;
        while index < tail {
            if self.source[index].is_ascii_whitespace() {
                index += 1;
                continue;
            }
            let attribute = index;
            let attribute_end = attribute
                + self.source[attribute..tail]
                    .iter()
                    .take_while(|byte| {
                        !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'>' | b'/')
                    })
                    .count()
                    .max(1);
            index = attribute_end;
            if self.source.get(attribute_end) != Some(&b'=')
                || !matches!(self.source.get(attribute_end + 1), Some(b'"' | b'\''))
            {
                continue;
            }
            let quote = attribute_end + 1;
            let Some(close) = self.closing_quote(quote).filter(|close| *close < tail) else {
                break;
            };
            index = close + 1;
            let written = &self.source[attribute..attribute_end];
            if written.starts_with(b"::") {
                continue;
            }
            let (bound, key) = match written.strip_prefix(b":") {
                Some(key) => (true, key),
                None => (false, written),
            };
            let Some(key) = camel_identifier(key) else {
                continue;
            };
            let inner = &self.source[quote + 1..close];
            // What the argument's value is: the bound expression, the one echo the value is, or
            // the plain string.
            let value = if bound {
                Some((quote + 1, close))
            } else if let Some(echo) = sole_echo(inner) {
                Some((quote + 1 + echo.start, quote + 1 + echo.end))
            } else if inner.iter().any(|byte| matches!(byte, b'{' | b'$' | b'\\')) {
                None
            } else {
                Some((quote, close + 1))
            };
            let Some((from, to)) = value else {
                continue;
            };
            // `,key:` over the blank and the name before the value, which always has the room:
            // the name it replaces is at least as long.
            let at = attribute - 1;
            self.out[at] = b',';
            self.out[at + 1..at + 1 + key.len()].copy_from_slice(&key);
            self.out[at + 1 + key.len()] = b':';
            self.out[from..to].copy_from_slice(&self.source[from..to]);
        }
        match closes_itself {
            true => self.out[end - 2..end].copy_from_slice(b");"),
            false => {
                self.out[end - 1] = b')';
                self.owe(b";");
            }
        }
        self.markup = end;
        self.at = end;
        Read::Written
    }

    /// The `>` that ends the tag beginning at `start`, past any quoted attribute.
    fn tag_end(&self, start: usize) -> Option<usize> {
        let mut index = start + 1;
        while index < self.source.len() {
            match self.source[index] {
                b'"' | b'\'' => index = self.closing_quote(index)? + 1,
                b'>' => return Some(index + 1),
                _ => index += 1,
            }
        }
        None
    }

    /// The quote that closes the one at `open`.
    fn closing_quote(&self, open: usize) -> Option<usize> {
        let quote = self.source[open];
        self.source[open + 1..]
            .iter()
            .position(|byte| *byte == quote)
            .map(|at| open + 1 + at)
    }

    fn directive(&mut self) -> Read {
        let start = self.at;
        // `\B@`, as Blade matches it: an address is not a directive, and `@@if` prints `@if`.
        if start > 0 && is_word(self.source[start - 1]) {
            return Read::Markup(1);
        }
        if self.source.get(start + 1) == Some(&b'@') {
            let word = self.source[start + 2..]
                .iter()
                .take_while(|byte| is_word(**byte))
                .count();
            return Read::Markup(2 + word);
        }
        let name_end = start
            + 1
            + self.source[start + 1..]
                .iter()
                .take_while(|byte| is_word(**byte))
                .count();
        let Some((name, kind)) = std::str::from_utf8(&self.source[start + 1..name_end])
            .ok()
            .and_then(|name| Some((name, Directive::named(name)?)))
        else {
            return Read::Markup(name_end - start);
        };
        let arguments = self.arguments(name_end);
        self.directives.push((
            start..name_end,
            name.to_ascii_lowercase(),
            arguments.is_some(),
        ));
        self.blank(self.markup..start);
        // The name and the blanks before its parenthesis, which is the room a keyword has.
        let word = start..arguments.as_ref().map_or(name_end, |(open, _)| *open);
        self.at = arguments.as_ref().map_or(name_end, |(_, close)| close + 1);
        // Whether the open block on top of the stack was written with braces; `false` for one
        // that had to be written as a plain statement, whose `else` and end are then blanks.
        let braced = self.blocks.last().copied().unwrap_or(false);
        match (kind, arguments) {
            (Directive::Condition(keyword), Some((open, close))) if keyword.starts_with('}') => {
                match braced && self.fits(&word, keyword.len()) {
                    true => {
                        self.keyword(word, keyword);
                        self.code(open..close + 1);
                        self.owe(b"{");
                    }
                    false => self.statement(word, open, close),
                }
            }
            (Directive::Condition(keyword), Some((open, close))) => {
                match self.fits(&word, keyword.len()) {
                    true => {
                        self.keyword(word, keyword);
                        self.code(open..close + 1);
                        self.owe(b"{");
                        self.blocks.push(true);
                    }
                    // No room for both the keyword and the opening tag: the condition is kept as
                    // an expression, and only its narrowing is lost.
                    false => {
                        self.statement(word, open, close);
                        self.blocks.push(false);
                    }
                }
            }
            (Directive::Loop(keyword), Some((open, close))) => {
                match self.fits(&word, keyword.len()) {
                    true => {
                        self.keyword(word, keyword);
                        self.code(open..close + 1);
                        self.owe(b"{");
                        self.blocks.push(true);
                    }
                    // A loop header is no expression, so with no room it is markup.
                    false => {
                        self.blank(start..close + 1);
                        self.blocks.push(false);
                    }
                }
            }
            (Directive::Block | Directive::Empty, Some((open, close))) => {
                match self.fits(&word, 3) {
                    true => {
                        self.keyword(word, "if(");
                        self.list(open, close);
                        self.owe(b"){");
                        self.blocks.push(true);
                    }
                    false => {
                        self.statement(word, open, close);
                        self.blocks.push(false);
                    }
                }
            }
            (Directive::Block, None) => match self.fits(&word, 5) {
                true => {
                    self.keyword(word, "if(1)");
                    self.owe(b"{");
                    self.blocks.push(true);
                }
                false => {
                    self.blank(word);
                    self.blocks.push(false);
                }
            },
            (Directive::Else, Some((open, close))) => match braced && self.fits(&word, 8) {
                true => {
                    self.keyword(word, "}elseif(");
                    self.list(open, close);
                    self.owe(b"){");
                }
                false => self.statement(word, open, close),
            },
            (Directive::Else, None) => match braced && self.fits(&word, 5) {
                true => {
                    self.keyword(word, "}else");
                    self.owe(b"{");
                }
                false => self.blank(word),
            },
            (Directive::End, _) => match self.blocks.pop() {
                Some(true) => self.closing(word),
                _ => self.blank(word),
            },
            (Directive::Empty, None) => match braced {
                true => self.keyword(word, "}{"),
                false => self.blank(word),
            },
            (Directive::Switch, Some((open, close))) => match self.fits(&word, 6) {
                true => {
                    self.keyword(word, "switch");
                    self.code(open..close + 1);
                    self.owe(b"{");
                    self.blocks.push(true);
                }
                false => {
                    self.statement(word, open, close);
                    self.blocks.push(false);
                }
            },
            (Directive::Case, Some((open, close))) => match braced {
                true => {
                    self.keyword(word, "case");
                    self.code(open..close + 1);
                    self.owe(b":");
                }
                false => self.statement(word, open, close),
            },
            (Directive::Default, None) if braced => self.keyword(word, "default:"),
            (Directive::Break, None) if braced => self.keyword(word, "break;"),
            (Directive::Continue, None) if braced => self.keyword(word, "continue;"),
            (Directive::Php, None) => {
                let close = find(self.source, name_end, b"@endphp").unwrap_or_else(|| {
                    self.unended
                        .push((start..name_end, "`@php` is never closed".to_owned()));
                    self.source.len()
                });
                let end = (close + b"@endphp".len()).min(self.source.len());
                self.blank(word);
                self.code(name_end..close);
                self.owe(b";");
                self.blank(close..end);
                self.at = end;
            }
            (Directive::Verbatim, None) => {
                let end = match find(self.source, name_end, b"@endverbatim") {
                    Some(close) => close + b"@endverbatim".len(),
                    None => {
                        self.unended
                            .push((start..name_end, "`@verbatim` is never closed".to_owned()));
                        self.source.len()
                    }
                };
                self.blank(start..end);
                self.at = end;
            }
            (Directive::Include(call), Some((open, close))) => match self.fits(&word, call.len()) {
                true => {
                    self.keyword(word, call);
                    self.code(open..close + 1);
                    self.owe(b";");
                }
                false => self.statement(word, open, close),
            },
            (Directive::Use, Some((open, close))) => self.import(word, open, close),
            (_, Some((open, close))) => self.statement(word, open, close),
            (_, None) => self.blank(word),
        }
        Read::Written
    }

    /// `@use('App\Models\Post')` as the import Blade compiles it to, `use App\Models\Post;`, so
    /// that the template's names resolve as its compiled PHP's do -- and `@use('App\Models\Post',
    /// 'P')` as `use App\Models\Post as P;` where the bytes between the two strings have room for
    /// ` as `. Anything else is the list any statement is.
    fn import(&mut self, word: Range<usize>, open: usize, close: usize) {
        let quoted = |from: usize| -> Option<(usize, usize)> {
            let at = from
                + self.source[from..close]
                    .iter()
                    .take_while(|byte| byte.is_ascii_whitespace())
                    .count();
            let quote = *self.source.get(at)?;
            if !matches!(quote, b'\'' | b'"') {
                return None;
            }
            let end = at
                + 1
                + self.source[at + 1..close]
                    .iter()
                    .position(|&byte| byte == quote)?;
            self.source[at + 1..end]
                .iter()
                .all(|&byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\\'))
                .then_some((at, end))
        };
        let Some(name) = quoted(open + 1) else {
            return self.statement(word, open, close);
        };
        let alias = match self.source[name.1 + 1..close]
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
        {
            None => None,
            Some(comma) if self.source[name.1 + 1 + comma] == b',' => {
                match quoted(name.1 + 2 + comma) {
                    Some(alias) if alias.0 - name.1 >= 3 => Some(alias),
                    _ => return self.statement(word, open, close),
                }
            }
            Some(_) => return self.statement(word, open, close),
        };
        let last = alias.map_or(name.1, |alias| alias.1);
        // The keyword has the name of the directive and everything up to the name's quote: the
        // opening tag may need two of them, at the very start of a template.
        let room = word.start..name.0 + 1;
        if self.source[last + 1..close]
            .iter()
            .any(|byte| !byte.is_ascii_whitespace())
            || !self.fits(&room, 4)
        {
            return self.statement(word, open, close);
        }
        self.keyword(room, "use ");
        for at in name.1..=close {
            if !matches!(self.source[at], b'\n' | b'\r') {
                self.out[at] = b' ';
            }
        }
        if let Some(alias) = alias {
            self.out[name.1 + 1..name.1 + 3].copy_from_slice(b"as");
            self.out[alias.0 + 1..alias.1].copy_from_slice(&self.source[alias.0 + 1..alias.1]);
        }
        self.out[last] = b';';
    }

    /// A directive's arguments as a statement of their own: a list, so that any number of them is
    /// one expression.
    fn statement(&mut self, word: Range<usize>, open: usize, close: usize) {
        self.blank(word);
        self.list(open, close);
        self.owe(b";");
    }

    /// Whether `length` bytes of keyword fit in `word` along with what is owed before it -- and,
    /// while no opening tag has a place yet, the two bytes of one.
    fn fits(&self, word: &Range<usize>, length: usize) -> bool {
        let owed = self
            .owed
            .iter()
            .filter(|byte| !byte.is_ascii_whitespace())
            .count();
        let tag = match self.opened || self.coded || self.room_for_tag(word.start) {
            true => 0,
            false => 2,
        };
        word.len() >= length + owed + tag
    }

    /// Whether the blanks before `at` hold a run long enough for the short opening tag.
    fn room_for_tag(&self, at: usize) -> bool {
        let mut run = 0;
        let mut index = 0;
        while index < at {
            if let Some(comment) = self
                .comments
                .iter()
                .find(|comment| comment.contains(&index))
            {
                run = 0;
                index = comment.end;
                continue;
            }
            match self.out[index] {
                b' ' | b'\t' => {
                    run += 1;
                    if run >= 2 {
                        return true;
                    }
                }
                _ => run = 0,
            }
            index += 1;
        }
        false
    }

    /// The parenthesised arguments after a directive's name, with blanks allowed between, as the
    /// positions of the two parentheses.
    fn arguments(&self, from: usize) -> Option<(usize, usize)> {
        let open = from
            + self.source[from..]
                .iter()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count();
        if self.source.get(open) != Some(&b'(') {
            return None;
        }
        let mut depth = 0_usize;
        let mut quote: Option<u8> = None;
        let mut escaped = false;
        for (index, &byte) in self.source.iter().enumerate().skip(open) {
            if let Some(open_quote) = quote {
                match byte {
                    _ if escaped => escaped = false,
                    b'\\' => escaped = true,
                    _ if byte == open_quote => quote = None,
                    _ => {}
                }
                continue;
            }
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((open, index));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The directive's room overwritten with `text`, right-aligned so that whatever is owed goes in
    /// front of it.
    fn keyword(&mut self, word: Range<usize>, text: &str) {
        if text.len() > word.len() {
            self.failures.push(self.at);
            self.blank(word);
            return;
        }
        let keyword_at = word.end - text.len();
        self.blank(word.start..keyword_at);
        self.settle();
        self.out[keyword_at..word.end].copy_from_slice(text.as_bytes());
        self.coded = true;
    }

    /// An end directive: blank, with its `}` last so that anything owed lands before it.
    fn closing(&mut self, word: Range<usize>) {
        let last = word.end - 1;
        self.blank(word.start..last);
        self.settle();
        self.out[last] = b'}';
        self.coded = true;
    }

    /// Arguments written as a list: the parentheses become brackets, everything between is PHP.
    fn list(&mut self, open: usize, close: usize) {
        self.settle();
        self.out[open] = b'[';
        self.code(open + 1..close);
        self.out[close] = b']';
    }

    /// Bytes kept as PHP. Whatever is still owed is written first, into the blanks the expression
    /// begins with, or counted as lost.
    fn code(&mut self, range: Range<usize>) {
        self.coded = true;
        for at in range {
            if self.owed.is_empty() {
                break;
            }
            match self.source[at] {
                b'\n' | b'\r' => {}
                b' ' | b'\t' => self.out[at] = self.owed.pop_front().unwrap_or(b' '),
                _ => break,
            }
        }
        self.settle();
    }

    /// Forgets owed whitespace, and counts anything else still owed as a construct not written.
    fn settle(&mut self) {
        if self.owed.iter().any(|byte| !byte.is_ascii_whitespace()) {
            self.failures.push(self.at);
        }
        self.owed.clear();
    }

    fn owe(&mut self, text: &[u8]) {
        self.owed.extend(text);
    }

    /// Markup, overwritten: owed characters first, then blanks -- and where the markup holds a
    /// character outside ASCII, a comment around it, so its columns stay where a client counts them.
    fn blank(&mut self, range: Range<usize>) {
        let kept = match self.source[range.clone()].is_ascii() {
            true => None,
            false => self.comment_room(range.clone()),
        };
        let mut index = range.start;
        while index < range.end {
            if let Some(kept) = &kept
                && kept.contains(&index)
            {
                index = kept.end;
                continue;
            }
            let byte = self.source[index];
            match byte {
                b'\n' | b'\r' => {}
                _ if byte.is_ascii() => match self.owed.pop_front() {
                    Some(owed) => {
                        self.out[index] = owed;
                        self.coded |= !owed.is_ascii_whitespace();
                    }
                    None => self.out[index] = b' ',
                },
                // A character outside ASCII with no comment around it is blanked whole, every
                // byte of it, and its columns are lost.
                _ => self.out[index] = b' ',
            }
            index += 1;
        }
        if let Some(kept) = kept {
            self.comments.push(kept.clone());
            self.out[kept.start..kept.start + 2].copy_from_slice(b"/*");
            self.out[kept.end - 2..kept.end].copy_from_slice(b"*/");
            // The comment holds the markup verbatim; only a `*/` of its own would end it early.
            for index in kept.start + 2..kept.end - 3 {
                if self.out[index] == b'*' && self.out[index + 1] == b'/' {
                    self.out[index..index + 2].fill(b' ');
                }
            }
        }
    }

    /// The span of `range` a comment can keep: from two plain ASCII bytes before its first
    /// character outside ASCII to two after its last.
    fn comment_room(&self, range: Range<usize>) -> Option<Range<usize>> {
        let plain = |index: usize| {
            self.source[index].is_ascii() && !matches!(self.source[index], b'\n' | b'\r')
        };
        let first = range
            .clone()
            .find(|&index| !self.source[index].is_ascii())?;
        let last = range
            .clone()
            .rev()
            .find(|&index| !self.source[index].is_ascii())?;
        let start = (range.start..first.saturating_sub(1))
            .rev()
            .find(|&index| plain(index) && plain(index + 1))?;
        let end = (last + 1..range.end.saturating_sub(1))
            .find(|&index| plain(index) && plain(index + 1))?;
        Some(start..end + 2)
    }

    /// The opening tag, in the first run of blanks long enough to hold it before the first
    /// expression: `<?php ` where there is room, and the short `<?` -- which the parser reads --
    /// where there is not. A comment written over markup is no place for it, and a template whose
    /// first expression starts at its first byte has nowhere at all, and stays markup.
    fn open(&mut self) {
        if self.opened || !self.coded {
            return;
        }
        let mut run = 0;
        let mut short: Option<usize> = None;
        let mut index = 0;
        while index < self.out.len() {
            if let Some(comment) = self
                .comments
                .iter()
                .find(|comment| comment.contains(&index))
            {
                run = 0;
                index = comment.end;
                continue;
            }
            match self.out[index] {
                b' ' | b'\t' => {
                    run += 1;
                    if run == 6 {
                        self.out[index - 5..=index].copy_from_slice(b"<?php ");
                        return;
                    }
                    if run == 2 && short.is_none() {
                        short = Some(index - 1);
                    }
                }
                b'\n' | b'\r' => run = 0,
                _ => break,
            }
            index += 1;
        }
        match short {
            Some(at) => self.out[at..at + 2].copy_from_slice(b"<?"),
            None => {
                self.failures.push(self.at);
                self.out = blank_text(&String::from_utf8_lossy(self.source)).into_bytes();
            }
        }
    }
}

/// Text of the same length with only its newlines kept: markup, as far as PHP can tell.
fn blank_text(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\n' | '\r' => character.to_string(),
            _ => " ".repeat(character.len_utf8()),
        })
        .collect()
}

/// Whether a component's name, as written after `<x-`, is one this reads: words joined by `-` and
/// `.`, which parse as constants -- not a slot, and not a package's `name::component`.
fn component_name(name: &[u8]) -> bool {
    if name.is_empty() || name == b"slot" || name.contains(&b':') {
        return false;
    }
    name.split(|byte| matches!(byte, b'-' | b'.')).all(|word| {
        word.first()
            .is_some_and(|first| first.is_ascii_alphabetic() || *first == b'_')
            && word
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    })
}

/// A kebab-cased attribute name as the camel-cased variable a component reads it as -- `show-refresh`
/// as `showRefresh` -- when it is one.
fn camel_identifier(written: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(written.len());
    let mut upper = false;
    for &byte in written {
        match byte {
            b'-' if !out.is_empty() => upper = true,
            _ if byte.is_ascii_alphanumeric() || byte == b'_' => {
                out.push(if upper {
                    byte.to_ascii_uppercase()
                } else {
                    byte
                });
                upper = false;
            }
            _ => return None,
        }
    }
    let first = *out.first()?;
    (first.is_ascii_alphabetic() || first == b'_').then_some(out)
}

/// Where the expression is, when a value is exactly one echo: `{{ $classes }}`.
fn sole_echo(value: &[u8]) -> Option<Range<usize>> {
    let start = value.iter().position(|byte| !byte.is_ascii_whitespace())?;
    let end = value.len()
        - value
            .iter()
            .rev()
            .position(|byte| !byte.is_ascii_whitespace())?;
    let body = value
        .get(start..end)?
        .strip_prefix(b"{{")?
        .strip_suffix(b"}}")?;
    if body.windows(2).any(|pair| pair == b"{{" || pair == b"}}") {
        return None;
    }
    Some(start + 2..end - 2)
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| from + position)
}

mod pairing;

pub(crate) use pairing::problems;

#[cfg(test)]
mod tests;
