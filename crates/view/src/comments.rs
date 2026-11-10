//! Whether a position is inside a comment, and whether that comment is asking for a type.
//!
//! Completion fired everywhere in a comment, which means it fired on every word of every sentence
//! anybody wrote. A list of class names over prose is not a suggestion, it is an interruption.
//!
//! But a comment is where PHP's types actually live in code of any age, so the answer is not simply
//! "no". `@var`, `@param` and `@return` are exactly the places a class name belongs.

use phrost_model::TextSize;

use crate::view::IndexView;

/// What kind of comment a position falls in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommentPlace {
    /// Prose. Nothing is offered here.
    Prose,
    /// After a tag whose value is a type.
    TypeTag,
    /// After `@use`, whose value is a trait -- the one docblock name that is not a type, since
    /// nothing is an instance of a trait.
    TraitTag,
    /// After `@see`, which may name anything that has a declaration: a class, a function.
    ReferenceTag,
}

/// The docblock tags whose value is a type, without their `@`.
///
/// A `@phpstan-`/`@psalm-` prefix is stripped before the comparison, so the vendor-specific
/// spellings of each of these are covered without listing them twice.
const TYPE_TAGS: &[&str] = &[
    "var",
    "param",
    "param-out",
    "return",
    "throws",
    "template",
    "template-covariant",
    "template-contravariant",
    "extends",
    "implements",
    "method",
    "property",
    "property-read",
    "property-write",
    "mixin",
    "self-out",
    "assert",
    "assert-if-true",
    "assert-if-false",
    // WordPress's spelling of what a function reads from the global scope: `@global MO $l10n`.
    "global",
];

impl IndexView<'_> {
    /// The buffer with every comment that closes before `offset` blanked to spaces, for a reader
    /// that walks the characters before a cursor.
    ///
    /// Completion reads a chain's receiver that way -- the statement being typed does not parse --
    /// and counts parentheses as it goes. A `)` in a comment inside a closure's body, `// expected
    /// :)`, unbalanced the count, so `})->` after a closure holding one offered nothing while goto
    /// on the call answered. Lengths and line breaks are kept, so every offset still means what it
    /// did; the comment the cursor is in is left as written, since that is where a docblock tag is
    /// being typed. Borrowed when no comment closes before the cursor.
    pub fn code_text_before(&self, offset: TextSize) -> Option<std::borrow::Cow<'_, str>> {
        let open = self.current()?;
        let text = open.text();
        let closed = open
            .hir()
            .comment_ranges()
            .iter()
            .take_while(|range| range.end() <= offset)
            .collect::<Vec<_>>();
        if closed.is_empty() {
            return Some(std::borrow::Cow::Borrowed(text));
        }
        let mut bytes = text.as_bytes().to_vec();
        for range in closed {
            for byte in &mut bytes[usize::from(range.start())..usize::from(range.end())] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
        }
        // Every byte replaced was part of a comment, and a space is one byte: what was valid UTF-8
        // is still valid, a multi-byte character becoming as many spaces as it had bytes.
        String::from_utf8(bytes).ok().map(std::borrow::Cow::Owned)
    }

    /// Where in a comment `offset` falls, or `None` when it is in code.
    ///
    /// Two sources, because the file's IR holds only the comments that are attached to something. A
    /// docblock is in there with its range, so that answer is exact. A `//` or `#` comment is
    /// attached to nothing and reaches no layer above the parser, so it is read from the one line
    /// the cursor is on — quote-aware, because `'http://x'` is not a comment and suppressing
    /// completion after one would be worse than the bug this fixes.
    ///
    /// What is not detected: the middle of a multi-line `/* */` that documents nothing. It would
    /// need a scan back through the file, and in PHP that comment is either a docblock, which is
    /// covered, or commented-out code, where an unwanted list is the least of it.
    pub fn comment_at(&self, offset: TextSize) -> Option<CommentPlace> {
        let open = self.current()?;
        let text = open.text();

        for (_, docblock) in open.hir().docblocks() {
            if docblock.range.contains(offset) {
                // From the start of the docblock rather than of the line: a type may run over
                // several, and `@param array{` above a line reading `*   fs: \Drupal\Core\File` put
                // that line inside the type with no tag on it. The last tag above the cursor is the
                // one it belongs to, and the type ends where a space falls outside its brackets --
                // so a newline inside `array{ ... }` keeps it open, and one after `$name` closes it.
                return Some(place_in(
                    &text[usize::from(docblock.range.start())..usize::from(offset)],
                ));
            }
        }

        let line = &text[line_start(text, offset)..usize::from(offset)];
        comment_body(line).map(place_in)
    }
}

/// Whether the last tag written before the cursor on this line is one whose value is a type.
///
/// The *last*, because `@param Repo $x` puts the type first: once a name has been written after the
/// tag, the cursor is no longer in the type's place. Checking the whole line for a tag would keep
/// offering classes for the parameter name as well.
fn place_in(before: &str) -> CommentPlace {
    let Some((tag, rest)) = tag_before(before) else {
        return CommentPlace::Prose;
    };

    let tag = tag
        .strip_prefix("phpstan-")
        .or_else(|| tag.strip_prefix("psalm-"))
        .unwrap_or(tag);
    // `@method static Builder where()` says `static` before the return type -- a word about the
    // method, not part of the type, and Laravel's facades write it on every line.
    let rest = match tag {
        "method" => rest
            .trim_start()
            .strip_prefix("static")
            .filter(|after| after.starts_with(char::is_whitespace))
            .unwrap_or(rest),
        _ => rest,
    };

    // `@template TModel of \Illuminate\...\Model` writes the name first: the name is a declaration
    // and the bound after `of` -- psalm's `as` -- is the type. Read as the first word, the space
    // after the name ended the type and the bound offered nothing.
    let rest = match tag {
        "template" | "template-covariant" | "template-contravariant" => {
            match template_bound(rest) {
                Some(bound) => bound,
                None => return CommentPlace::Prose,
            }
        }
        _ => rest,
    };

    // Inside a `@method`'s parentheses each parameter starts with a type of its own:
    // `setDefaultDriver(\UnitEnum|string $name)`, on a third of Laravel's facade lines.
    if tag == "method"
        && let Some(parameter) = parameter_being_written(rest)
    {
        let parameter = parameter.trim_start();
        return match !parameter.starts_with('$') && still_in_the_first_type(parameter) {
            true => CommentPlace::TypeTag,
            false => CommentPlace::Prose,
        };
    }

    // One type only. `@param Repo $x` is past the type at the space before `$x`.
    if !still_in_the_first_type(rest) {
        return CommentPlace::Prose;
    }

    if TYPE_TAGS.contains(&tag) {
        CommentPlace::TypeTag
    } else if tag == "use" {
        // The trait is the name before the brackets; what is inside them is its arguments, which
        // are types like any other -- `@use HasFactory<UserFactory>` offered only traits there.
        match inside_brackets(rest) {
            true => CommentPlace::TypeTag,
            false => CommentPlace::TraitTag,
        }
    } else if tag == "see" {
        CommentPlace::ReferenceTag
    } else {
        CommentPlace::Prose
    }
}

/// Whether the cursor at the end of `rest` is inside a bracket the text has opened and not closed.
fn inside_brackets(rest: &str) -> bool {
    let mut depth = 0_u32;
    for character in rest.chars() {
        match character {
            '<' | '{' | '(' | '[' => depth += 1,
            '>' | '}' | ')' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

/// Whether the cursor at the end of `rest` is still inside the first type written after a tag.
///
/// A space ends the type only outside brackets: `array<string, Transport` is one type with a space
/// in it, and counting words read the cursor as past the type -- every generic with a key, which is
/// every map, offered nothing for its value.
///
/// Nor does a space the type goes on past: the one after the colon of a callable's return,
/// `callable(TValue, TKey): TMapValue`, and the ones some projects write round a `|` or an `&`.
/// Every callback parameter Laravel's collections document is written the first way, and its
/// return type offered nothing.
fn still_in_the_first_type(rest: &str) -> bool {
    let rest = rest.trim_start();
    let mut depth = 0_u32;
    for (at, character) in rest.char_indices() {
        match character {
            '<' | '{' | '(' | '[' => depth += 1,
            '>' | '}' | ')' | ']' => depth = depth.saturating_sub(1),
            _ if character.is_whitespace() && depth == 0 => {
                let before = rest[..at].trim_end();
                let after = rest[at..].trim_start();
                // `&$name` is a parameter taken by reference, which is past the type.
                let goes_on = before.ends_with("):")
                    || before.ends_with(['|', '&'])
                    || after.starts_with('|')
                    || (after.starts_with('&') && !after.starts_with("&$"));
                if !goes_on {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// What a template tag writes after its name and `of` (or `as`): the bound, which is a type.
/// `None` while the cursor is still in the name or before the keyword.
fn template_bound(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    let after_name = rest.find(char::is_whitespace)?;
    let rest = rest[after_name..].trim_start();
    let keyword = ["of", "as"].into_iter().find(|keyword| {
        rest.strip_prefix(keyword)
            .is_some_and(|after| after.starts_with(char::is_whitespace))
    })?;
    Some(&rest[keyword.len()..])
}

/// The parameter being written inside a still-open parameter list, from just after its `(` or
/// its `,`. `None` when no list is open.
///
/// Depth-aware for the reason [`still_in_the_first_type`] is: `array<int, string> $map` holds a
/// comma that does not end a parameter.
fn parameter_being_written(rest: &str) -> Option<&str> {
    let mut opened: Option<usize> = None;
    let mut start = 0;
    let mut depth = 0_u32;
    for (at, character) in rest.char_indices() {
        match character {
            '(' if opened.is_none() => {
                opened = Some(at);
                start = at + 1;
            }
            ')' if opened.is_some() && depth == 0 => {
                opened = None;
            }
            '<' | '{' | '[' | '(' => depth += 1,
            '>' | '}' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if opened.is_some() && depth == 0 => start = at + 1,
            _ => {}
        }
    }
    opened.map(|_| &rest[start..])
}

/// The last `@tag` on the line, and the text written after it.
fn tag_before(before: &str) -> Option<(&str, &str)> {
    let at = before.rfind('@')?;
    let after = &before[at + 1..];
    let end = after
        .find(|character: char| !character.is_alphanumeric() && character != '-')
        .unwrap_or(after.len());
    Some((&after[..end], &after[end..]))
}

/// The part of a line that is inside a comment, if the cursor has passed a comment marker.
fn comment_body(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) => {
                if byte == b'\\' {
                    index += 2;
                    continue;
                }
                if byte == open {
                    quote = None;
                }
            }
            None => match byte {
                b'\'' | b'"' => quote = Some(byte),
                // `#[` opens an attribute, which is code. This distinction has been paid for once
                // already, in the harness that masks comments before sampling positions.
                b'#' if bytes.get(index + 1) != Some(&b'[') => return Some(&line[index + 1..]),
                b'/' if bytes.get(index + 1) == Some(&b'/') => return Some(&line[index + 2..]),
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    // A block comment that closes on this line leaves the cursor back in code.
                    return match line[index + 2..].find("*/") {
                        Some(close) => {
                            index += 2 + close + 2;
                            continue;
                        }
                        None => Some(&line[index + 2..]),
                    };
                }
                _ => {}
            },
        }
        index += 1;
    }

    None
}

fn line_start(text: &str, offset: TextSize) -> usize {
    let before = &text[..usize::from(offset)];
    before.rfind('\n').map_or(0, |at| at + 1)
}
