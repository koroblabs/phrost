//! Doctrine's annotation syntax, read far enough to write it out as PHP.
//!
//! The third reader of this syntax in the workspace, and the three are not one rule spelled three
//! times. Lowering reads the string literals at the top level of an annotation because one of them
//! is a catalog key, and never looks inside a group. The token layer classifies every run in the
//! text because a colour is a span. This needs the whole of it *as a value*, in order, so that it
//! can be printed again in another syntax -- and the three live in crates that cannot see each
//! other, since the first is below the parser boundary and the second may not read the index.
//!
//! # What the two syntaxes disagree about
//!
//! Four things, and nothing else:
//!
//! - a named argument is `id = "x"` rather than `id: "x"`;
//! - an array is `{ ... }` rather than `[ ... ]`, and its entries are `"a" = "b"`;
//! - a nested annotation is `@Translation("x")` rather than `new TranslatableMarkup("x")`;
//! - the whole of it is written inside a comment, one `*` per line.
//!
//! So the conversion is a scan with a stack of open delimiters, and the stack is what tells an `=`
//! that means a named argument from one that means an array entry. Nothing needs a tree.

use std::ops::Range;

use phrost_model::short_name;

/// A Doctrine-style annotation written at the start of a docblock line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Annotation<'a> {
    /// The name as written, without the `@`.
    pub(super) name: &'a str,
    /// Where the whole of it is, as a byte range into the docblock's own text.
    pub(super) span: Range<usize>,
    /// From the opening `(` to its matching `)`, inclusive, with the docblock's `*` prefixes still
    /// in it.
    arguments: &'a str,
}

/// An annotation rewritten as an attribute.
pub(super) struct Attribute {
    /// `#[` to `]`, laid out down the page the way the annotation was, with every line but the
    /// first carrying the declaration's indentation.
    pub(super) text: String,
    /// Every class the text names by its short name, fully qualified, in the order first written.
    pub(super) imports: Vec<String>,
}

impl<'a> Annotation<'a> {
    /// The first annotation in a docblock, if it holds one.
    ///
    /// A tag whose name begins a docblock line and is followed **immediately** by `(`. Both halves
    /// are exact rather than approximate: every PHPDoc tag whose value anything reads puts a space
    /// and a type after the name, and a tag written mid-sentence is prose -- `@see Foo::bar()` is
    /// both of those at once. The name must also open with a capital, which is the convention every
    /// annotation in every framework that used them is written under, and which keeps a lowercase
    /// tag nobody models from being converted into a class that does not exist.
    ///
    /// The first and not all of them: a declaration carries one plugin annotation, and converting
    /// one at a time is what lets the offer name the class in its title.
    pub(super) fn first_in(docblock: &'a str) -> Option<Self> {
        for (start, line) in line_starts(docblock) {
            let content = line.trim_start();
            let content = content.strip_prefix('*').map_or(content, str::trim_start);
            let Some(rest) = content.strip_prefix('@') else {
                continue;
            };
            let len = rest
                .find(|ch: char| !is_name_char(ch))
                .unwrap_or(rest.len());
            let (name, after) = rest.split_at(len);
            if !after.starts_with('(') || !opens_an_annotation(name) {
                continue;
            }

            let at = start + (line.len() - content.len()) + 1 + len;
            let arguments = balanced(&docblock[at..])?;
            return Some(Self {
                name,
                span: start + (line.len() - content.len())..at + arguments.len(),
                arguments,
            });
        }
        None
    }

    /// The attribute this annotation would be written as, or nothing when some part of it names a
    /// class the index cannot find.
    ///
    /// Refused rather than approximated, for the reason the module above states: a nested
    /// annotation nobody can resolve would be written out as `new Whatever(...)`, which is an
    /// import nobody can add and a class that is not there. That is a worse outcome than the menu
    /// being one entry shorter.
    pub(super) fn as_attribute(
        &self,
        indent: &str,
        class_of: &dyn Fn(&str) -> Option<String>,
    ) -> Option<Attribute> {
        let mut imports = Vec::new();
        let attribute = class_of(self.name)?;
        let short = short_name(&attribute).to_owned();
        remember(&mut imports, attribute);

        let arguments = self.undecorated(indent);
        let arguments = rewrite(&arguments, class_of, &mut imports)?;
        Some(Attribute {
            text: format!("#[{short}{arguments}]"),
            imports,
        })
    }

    /// The argument list with the comment taken off it.
    ///
    /// Every line but the first loses its leading `*` and the one space after it, and gains the
    /// declaration's own indentation -- so an annotation indented two spaces from the `*` comes out
    /// indented two from the `#[`, which is where its author put it and where the framework's own
    /// converted files have it.
    fn undecorated(&self, indent: &str) -> String {
        let mut out = String::with_capacity(self.arguments.len());
        for (at, line) in self.arguments.split('\n').enumerate() {
            if at == 0 {
                out.push_str(line.trim_end());
                continue;
            }
            let content = line.trim_start();
            let content = content
                .strip_prefix('*')
                .map_or(content, |rest| rest.strip_prefix(' ').unwrap_or(rest));
            out.push('\n');
            out.push_str(indent);
            out.push_str(content.trim_end());
        }
        out
    }
}

/// Doctrine's punctuation replaced by PHP's, in one pass over the text.
///
/// The stack of open delimiters is the whole of the state, and it exists for one question: an `=`
/// inside `{ }` separates an array entry's key from its value and becomes `=>`, and everywhere else
/// it names an argument and becomes `:`.
fn rewrite(
    body: &str,
    class_of: &dyn Fn(&str) -> Option<String>,
    imports: &mut Vec<String>,
) -> Option<String> {
    let mut out = String::with_capacity(body.len() + 16);
    let mut open: Vec<u8> = Vec::new();
    let bytes = body.as_bytes();
    let mut at = 0;

    while at < bytes.len() {
        match bytes[at] {
            // Verbatim, quotes included. Doctrine has no escape to handle -- a backslash inside an
            // annotation string is a literal one, which is how every class name in one is written.
            quote @ (b'"' | b'\'') => {
                let from = at;
                at += 1;
                while at < bytes.len() && bytes[at] != quote {
                    at += 1;
                }
                at = (at + 1).min(bytes.len());
                out.push_str(&body[from..at]);
            }
            delimiter @ (b'(' | b'{') => {
                open.push(delimiter);
                out.push(if delimiter == b'(' { '(' } else { '[' });
                at += 1;
            }
            delimiter @ (b')' | b'}') => {
                open.pop();
                out.push(if delimiter == b')' { ')' } else { ']' });
                at += 1;
            }
            b'=' if bytes.get(at + 1) != Some(&b'=') => {
                while out.ends_with(' ') {
                    out.pop();
                }
                match open.last() {
                    Some(b'{') => out.push_str(" =>"),
                    // The space in front of a named argument's `=` belongs to Doctrine's spelling
                    // and not to PHP's, where the colon sits against the name.
                    _ => out.push(':'),
                }
                at += 1;
            }
            b'@' => {
                let from = at + 1;
                let mut to = from;
                while to < bytes.len() && is_name_char(char::from(bytes[to])) {
                    to += 1;
                }
                let class = class_of(&body[from..to])?;
                out.push_str("new ");
                out.push_str(short_name(&class));
                remember(imports, class);
                at = to;
                // A nested annotation written without an argument list still constructs an object,
                // and PHP has no spelling for a constructor call with the parentheses left off.
                if !body[at..].starts_with('(') {
                    out.push_str("()");
                }
            }
            _ => {
                let ch = body[at..].chars().next().unwrap_or('\0');
                out.push(ch);
                at += ch.len_utf8();
            }
        }
    }

    Some(out)
}

/// Adds a class to the import list, keeping the order it was first written in.
fn remember(imports: &mut Vec<String>, class: String) {
    if !imports.contains(&class) {
        imports.push(class);
    }
}

/// The text from the opening `(` to its matching `)`, inclusive.
///
/// Quote-aware, so a `)` written inside a URL does not end the list early. `None` for an unclosed
/// one, which a docblock caught mid-edit is: half an annotation cannot be converted into anything.
fn balanced(from: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut quote = None;
    for (at, ch) in from.char_indices() {
        match quote {
            Some(held) => {
                if ch == held {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&from[..=at]);
                    }
                }
                _ => {}
            },
        }
    }
    None
}

/// Whether a name written after an `@` is an annotation's rather than a PHPDoc tag's.
///
/// The capital is the test. `@param` and `@see` are not annotations however they are punctuated,
/// and a namespaced one -- `@ORM\Entity` -- is judged on the segment a reader sees first.
fn opens_an_annotation(name: &str) -> bool {
    name.starts_with(|ch: char| ch.is_ascii_uppercase())
}

fn is_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '\\'
}

/// Every line of `text`, with the byte offset it starts at.
fn line_starts(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut at = 0;
    text.split('\n').map(move |line| {
        let start = at;
        at += line.len() + 1;
        (start, line)
    })
}

#[cfg(test)]
mod tests {
    use super::Annotation;

    /// What one docblock converts to, with every name resolving to itself under a namespace so the
    /// import list is visible without a project.
    fn convert(docblock: &str) -> String {
        let Some(annotation) = Annotation::first_in(docblock) else {
            return "(no annotation)".to_owned();
        };
        let class_of = |name: &str| match name {
            "Unknowable" => None,
            name => Some(format!("App\\{name}")),
        };
        match annotation.as_attribute("", &class_of) {
            Some(attribute) => format!("{}\n{}", attribute.text, attribute.imports.join("\n")),
            None => "(refused)".to_owned(),
        }
    }

    #[test]
    fn a_tag_written_the_way_phpdoc_writes_one_is_not_an_annotation() {
        // The exact test, and the reason it is exact: every tag whose value anything reads puts a
        // space and a value after the name. `@see` also names a class, so a looser rule would
        // convert a cross-reference into an attribute.
        assert_eq!(convert("/**\n * @see App\\Base\n */"), "(no annotation)");
        assert_eq!(convert("/**\n * @param int $n\n */"), "(no annotation)");
        assert_eq!(convert("/**\n * @internal\n */"), "(no annotation)");
        // Lowercase is not the convention any framework wrote annotations under, and a `@code(`
        // converted into a class would be an import nobody can add.
        assert_eq!(convert("/**\n * @code(x = 1)\n */"), "(no annotation)");
    }

    #[test]
    fn a_tag_named_mid_sentence_is_prose() {
        // A docblock's own text can hold anything, including the syntax being scanned for -- which
        // is why the tag has to begin the line rather than merely appear on one.
        assert_eq!(
            convert("/**\n * As in @Resource(id = \"x\") above.\n */"),
            "(no annotation)"
        );
    }

    #[test]
    fn the_shorthand_form_keeps_its_one_argument_positional() {
        // `@Marker("x")` has no name to repunctuate, so the only change is the delimiters.
        assert_eq!(
            convert("/**\n * @Marker(\"x\")\n */"),
            "#[Marker(\"x\")]\nApp\\Marker"
        );
    }

    #[test]
    fn a_nested_annotation_with_no_arguments_still_constructs_an_object() {
        // PHP has no spelling for a constructor call with the parentheses left off, and Doctrine
        // does: `@Marker` alone is an instance with every argument defaulted.
        assert_eq!(
            convert("/**\n * @Outer(inner = @Marker)\n */"),
            "#[Outer(inner: new Marker())]\nApp\\Outer\nApp\\Marker"
        );
    }

    #[test]
    fn a_bracket_written_inside_a_string_does_not_close_anything() {
        // A route path with a placeholder in it, which every routing annotation there is has.
        assert_eq!(
            convert("/**\n * @Route(path = \"/node/{node}\")\n */"),
            "#[Route(path: \"/node/{node}\")]\nApp\\Route"
        );
    }

    #[test]
    fn a_class_constant_and_a_bare_word_are_left_as_they_are() {
        // Both are already PHP. Doctrine's `TRUE` is PHP's, and a constant reference is written the
        // same way in both -- the conversion is punctuation, not translation.
        assert_eq!(
            convert("/**\n * @Flags(on = TRUE, mode = Modes::FAST, count = 3)\n */"),
            "#[Flags(on: TRUE, mode: Modes::FAST, count: 3)]\nApp\\Flags"
        );
    }

    #[test]
    fn one_unresolvable_name_refuses_the_whole_conversion() {
        // Half a conversion writes a class that is not there, which is worse than no offer.
        assert_eq!(
            convert("/**\n * @Outer(label = @Unknowable(\"x\"))\n */"),
            "(refused)"
        );
    }

    #[test]
    fn an_unclosed_annotation_is_not_one() {
        // What a docblock caught mid-edit looks like. There is nothing to balance the list against,
        // so there is no argument list to convert.
        assert_eq!(
            convert("/**\n * @Resource(\n *   id = \"x\"\n */"),
            "(no annotation)"
        );
    }
}
