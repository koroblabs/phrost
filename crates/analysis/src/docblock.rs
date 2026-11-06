//! Turning a raw docblock into something worth showing.
//!
//! Lexical only: it strips the comment syntax, separates prose from `@tag` lines, and splits each
//! tag into the slots its own grammar defines. It does not *interpret* a type, and must not start
//! to — docblock types are a second type language, and the layer that needs them parses them
//! properly rather than by looking at these strings. What is here is enough to render a tag as a
//! labelled line instead of dumping it as raw text, which is what it used to be.

/// A docblock, split into the parts a hover shows differently.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct Docblock {
    /// The prose, with the comment syntax removed and the author's own markup intact.
    pub(crate) prose: String,
    /// The `@tag` lines, in source order, each already joined with its continuation lines.
    pub(crate) tags: Vec<String>,
}

/// One `@tag`, split into the slots its grammar defines.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Tag {
    /// Without the `@`, as written -- a `phpstan-` or `psalm-` prefix included, because a reader
    /// deciding whether to trust a claim wants to know which tool it was written for.
    pub(crate) name: String,
    /// The type, for the tags whose grammar puts one first.
    pub(crate) ty: Option<String>,
    /// `$name`, for a tag about one parameter or property.
    pub(crate) subject: Option<String>,
    /// Everything after those, which is the author's prose and stays their markup.
    pub(crate) description: String,
}

/// Where a tag sits in PHPDoc's reading order.
///
/// One order for two questions that must not disagree: where hover shows a tag, and where the
/// quick fix writes a new one. Deprecation first, because it is a warning and everything under it
/// is about how to use something that should not be used; then what goes in and what comes back,
/// which is what a reader came for; then what it throws; then everything else, in the order it was
/// written -- ties keep source order, which is what a stable sort guarantees.
///
/// The vendor prefixes are stripped first: `@phpstan-param` is a `@param` written for a tool, not a
/// tag of its own, and sorting it away from the parameters would separate two claims about one
/// value.
pub(crate) fn reading_rank(name: &str) -> u8 {
    let name = name
        .strip_prefix("phpstan-")
        .or_else(|| name.strip_prefix("psalm-"))
        .unwrap_or(name)
        .to_ascii_lowercase();

    match name.as_str() {
        "deprecated" => 0,
        "param" | "param-out" => 1,
        "return" | "var" => 2,
        "throws" | "throw" => 3,
        // What the call proves beyond its signature, once the reader knows what it returns and what
        // it throws.
        "assert" | "assert-if-true" | "assert-if-false" => 4,
        _ => 5,
    }
}

/// The tags whose first word is a type.
const TYPED: &[&str] = &[
    "param",
    "param-out",
    "return",
    "var",
    "throws",
    "throw",
    "template",
    "extends",
    "implements",
    "mixin",
    "method",
    "property",
    "property-read",
    "property-write",
    "assert",
    "assert-if-true",
    "assert-if-false",
    "self-out",
];

/// The tags whose type is followed by the name of what it describes.
///
/// The three assertion spellings belong here too: `@phpstan-assert-if-true T $this` and
/// `@phpstan-assert T $param` both put the subject right after the type, exactly like `@param`
/// does with its order reversed. Leaving them out meant `$this`/`$param` fell into `description`
/// instead, which is where an author's prose belongs and a subject does not.
const NAMED: &[&str] = &[
    "param",
    "param-out",
    "var",
    "property",
    "property-read",
    "property-write",
    "assert",
    "assert-if-true",
    "assert-if-false",
];

impl Tag {
    /// Reads one already-joined tag line.
    ///
    /// `None` for a line that is not a tag, and for `@inheritdoc`, which has been resolved into the
    /// prose by the time anything asks.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let rest = line.trim().strip_prefix('@')?;
        let (name, rest) = match rest.find(char::is_whitespace) {
            Some(at) => (&rest[..at], rest[at..].trim_start()),
            None => (rest, ""),
        };
        if name.eq_ignore_ascii_case("inheritdoc") {
            return None;
        }

        // The vendor prefix decides nothing about the grammar: `@phpstan-return` is spelled like
        // `@return`.
        let grammar = name
            .strip_prefix("phpstan-")
            .or_else(|| name.strip_prefix("psalm-"))
            .unwrap_or(name)
            .to_ascii_lowercase();
        // A misspelt typing tag is read in the grammar of the one it means -- `@params string $x`
        // names `$x` -- so a rule counting parameters and the rule reporting the spelling agree
        // about what the line says. The name stays as written, which is what the second rule reads.
        let grammar = crate::diagnostics::misspelt_tags::meant_on_a_function(&grammar)
            .map_or(grammar, str::to_owned);

        if !TYPED.contains(&grammar.as_str()) {
            return Some(Self {
                name: name.to_owned(),
                ty: None,
                subject: None,
                description: rest.to_owned(),
            });
        }

        let (ty, rest) = split_type(rest);
        // `@param $name` with the type left out happens, and reading `$name` as the type would show
        // a variable where a type belongs.
        if names_a_variable(ty) {
            return Some(Self {
                name: name.to_owned(),
                ty: None,
                subject: Some(ty.to_owned()),
                description: rest.to_owned(),
            });
        }

        let (subject, rest) = if NAMED.contains(&grammar.as_str()) {
            let (word, tail) = split_word(rest);
            if names_a_variable(word) {
                (Some(word.to_owned()), tail)
            } else {
                (None, rest)
            }
        } else {
            (None, rest)
        };

        Some(Self {
            name: name.to_owned(),
            ty: (!ty.is_empty()).then(|| ty.to_owned()),
            subject,
            description: rest.to_owned(),
        })
    }

    /// The heading a reader sees, for the tags that have a name in English.
    pub(crate) fn label(&self) -> String {
        let grammar = self
            .name
            .strip_prefix("phpstan-")
            .or_else(|| self.name.strip_prefix("psalm-"))
            .unwrap_or(&self.name)
            .to_ascii_lowercase();

        match grammar.as_str() {
            "param" => "Parameter",
            "return" => "Returns",
            "var" => "Type",
            "throws" | "throw" => "Throws",
            "deprecated" => "Deprecated",
            "see" => "See",
            "since" => "Since",
            "todo" => "To do",
            // The previous server had a dedicated "Assertions" section for these; giving them an
            // English label here does the same job without a block kind of their own. The
            // conditional forms say so in the label because that is where the condition lives --
            // `description` is the author's prose and a generated qualifier does not belong there.
            "assert" => "Asserts",
            "assert-if-true" => "Asserts if true",
            "assert-if-false" => "Asserts if false",
            // Everything else keeps the tag as written, prefix included: a reader deciding whether
            // to trust `@phpstan-assert-if-true` wants to see that it says `phpstan`.
            _ => return format!("@{}", self.name),
        }
        .to_owned()
    }
}

/// The first whitespace-separated word, and everything after it.
/// Whether a word is the variable a tag is about, with whatever the signature does to it.
///
/// `...$rest` and `&$out` are parameters like any other: the sigils say how the *call* passes them
/// and are written in the tag because they are written in the signature. Reading them as part of
/// something else loses the name a `@param` is about, which is the whole of what it binds to.
fn names_a_variable(word: &str) -> bool {
    word.trim_start_matches("...")
        .trim_start_matches('&')
        .starts_with('$')
}

/// The type at the front of a tag, which is not a word.
///
/// `array{x: int, y: string}` has spaces in it, and so do `callable(Foo): Bar`, `list<int, string>`
/// and `Foo | Bar`. Splitting on the first space takes `array{x:` and leaves `int,` where the
/// subject should be, so `@param array{...} $options` read as a tag about nothing and the parameter
/// counted as undocumented -- which is the whole of what `doc-missing-tags` looks for.
///
/// So the split is at the first whitespace **outside** brackets, with a space that merely surrounds
/// a `|` or `&` swallowed too, since a union is one type however its author spaced it.
fn split_type(text: &str) -> (&str, &str) {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'{' | b'<' | b'(' | b'[' => depth += 1,
            b'}' | b'>' | b')' | b']' => depth -= 1,
            c if c.is_ascii_whitespace() && depth <= 0 => {
                let tail = text[at..].trim_start();
                // `Foo | Bar` and `Foo |Bar`: the type continues past the space either way, and a
                // trailing `|` on this side says the same. A colon does too, for the return half of
                // `callable(int, string): bool` -- inside brackets it is an array-shape key and
                // never reaches here, so at depth zero it can only be that.
                //
                // Except when what follows is the subject. `@param int &$count` writes the
                // by-reference sigil exactly where an intersection would go, and reading it as one
                // swallows the parameter name -- which is worse than the bug this function fixes,
                // because the tag then names nothing instead of naming the wrong thing.
                let joins = !names_a_variable(tail)
                    && (tail.starts_with(['|', '&', ':']) || text[..at].ends_with(['|', '&', ':']));
                if !joins {
                    return (&text[..at], tail);
                }
            }
            _ => {}
        }
        at += 1;
    }
    (text, "")
}

fn split_word(text: &str) -> (&str, &str) {
    match text.find(char::is_whitespace) {
        Some(at) => (&text[..at], text[at..].trim_start()),
        None => (text, ""),
    }
}

/// The tag PHPDoc uses to mean "the description above applies here too".
///
/// Matched case-insensitively and in both spellings: `{@inheritdoc}` and `{@inheritDoc}` are both
/// everywhere, and PHPDoc treats them the same.
const INHERIT_DOC: &str = "{@inheritdoc}";

impl Docblock {
    /// Whether the prose asks for an inherited description.
    pub(crate) fn inherits(&self) -> bool {
        self.prose.to_ascii_lowercase().contains(INHERIT_DOC)
    }

    /// Replaces every `{@inheritdoc}` with `inherited`, and takes the ancestor's tags when this
    /// docblock has none of its own.
    ///
    /// In place rather than appended, because that is what the tag means: `Extra note.
    /// {@inheritdoc}` reads as the note followed by the inherited text, and moving it would change
    /// what the author wrote.
    ///
    /// Tags are inherited only wholesale. An override that repeats one `@param` has not restated the
    /// others, but merging a partial list needs the parameters matched by name, which needs the tags
    /// parsed rather than kept as lines -- and that is the same work as reading a docblock type.
    pub(crate) fn resolve_inherited(&mut self, inherited: &Docblock) {
        let mut out = String::with_capacity(self.prose.len() + inherited.prose.len());
        let lowered = self.prose.to_ascii_lowercase();
        let mut rest = self.prose.as_str();
        let mut searched = lowered.as_str();

        while let Some(at) = searched.find(INHERIT_DOC) {
            out.push_str(&rest[..at]);
            out.push_str(&inherited.prose);
            rest = &rest[at + INHERIT_DOC.len()..];
            searched = &searched[at + INHERIT_DOC.len()..];
        }
        out.push_str(rest);

        self.prose = out.trim().to_owned();
        if self.tags.is_empty() {
            self.tags = inherited.tags.clone();
        }
    }

    /// Parses the text between and including `/**` and `*/`.
    pub(crate) fn parse(raw: &str) -> Self {
        let body = raw
            .trim()
            .strip_prefix("/**")
            .unwrap_or(raw)
            .strip_suffix("*/")
            .unwrap_or(raw);

        let mut prose: Vec<&str> = Vec::new();
        let mut tags: Vec<String> = Vec::new();
        // Whether the tag being continued keeps its own line breaks.
        //
        // `@code` is the one that does, and it is the whole reason this exists: joined with spaces
        // like every other continuation, a usage example becomes a paragraph of PHP on one line.
        // `strip_leading_star` already keeps the deliberate indentation of a code sample -- and
        // trimming each continuation threw it away again two lines later.
        let mut verbatim = false;
        // Whether a `@code` block *inside the description* is open.
        //
        // Drupal writes its usage examples in the middle of a description -- prose, an example, more
        // prose, another example -- and lifting each one into the tag stream broke the second half of
        // that: after `@endcode` the following sentence continued the `code` tag, because a non-tag
        // line continues the last tag and the tag was still the last one. It rendered glued to the
        // closing `];` and inside the fence.
        //
        // A description is markdown that this layer passes through, so a fence belongs in it and
        // needs no tag at all -- which also keeps the example where its author put it, between the
        // two paragraphs, rather than at the end with the tags. Only in the description: a `@code`
        // written after the tags have started has nowhere in the prose to go and keeps the tag path.
        let mut fenced = false;

        for line in body.lines().map(strip_leading_star) {
            if fenced {
                if line.trim_start().eq_ignore_ascii_case("@endcode") {
                    prose.push("```");
                    fenced = false;
                    continue;
                }
                // A docblock that forgot its `@endcode` must not swallow the `@see` lines below.
                if !line.trim_start().starts_with('@') {
                    prose.push(line);
                    continue;
                }
                prose.push("```");
                fenced = false;
            }
            if tags.is_empty()
                && let Some(tag) = line.strip_prefix('@')
                && tag.trim_end().eq_ignore_ascii_case("code")
            {
                prose.push("```php");
                fenced = true;
                continue;
            }

            if verbatim {
                // `@endcode` closes the block and is not a tag of its own: it carries nothing, and
                // rendered as one it is a bare heading under the example it ends.
                if line.trim_start().eq_ignore_ascii_case("@endcode") {
                    verbatim = false;
                    continue;
                }
                // Any other tag closes it too, and is then read as the tag it is. Only `@endcode`
                // should end a block, but a docblock that forgot it must not swallow the `@see`
                // lines below -- and this is also what happens today, so an annotation sample
                // containing an `@`-line is no worse than it was.
                if !line.trim_start().starts_with('@')
                    && let Some(previous) = tags.last_mut()
                {
                    previous.push('\n');
                    previous.push_str(line);
                    continue;
                }
                verbatim = false;
            }

            if let Some(tag) = line.strip_prefix('@') {
                tags.push(format!("@{tag}"));
                verbatim = tag.trim_end().eq_ignore_ascii_case("code");
                continue;
            }

            match tags.last_mut() {
                // Once tags have started, a non-tag line continues the last one. `@param T $x` with
                // its description wrapped onto the next line is extremely common, and showing the
                // remainder as prose would put it above the tag it belongs to.
                Some(previous) if !line.trim().is_empty() => {
                    previous.push(' ');
                    // Trimmed, unlike prose: a continuation's indentation is alignment under the
                    // tag it belongs to, not something the reader is meant to see.
                    previous.push_str(line.trim());
                }
                Some(_) => {}
                None => prose.push(line),
            }
        }

        if fenced {
            prose.push("```");
        }

        Self {
            prose: join_prose(&prose),
            tags,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.prose.is_empty() && self.tags.is_empty()
    }
}

/// Removes the indentation and the decorative `*` a docblock line carries.
fn strip_leading_star(line: &str) -> &str {
    let line = line.trim_start();
    match line.strip_prefix('*') {
        // A single space after the star is the convention and is not part of the text; more than
        // one is deliberate indentation, in a code sample for instance, and is kept.
        Some(rest) => rest.strip_prefix(' ').unwrap_or(rest).trim_end(),
        None => line.trim_end(),
    }
}

/// Joins prose lines, collapsing runs of blank lines to one.
///
/// Blank lines are paragraph breaks in markdown, so they matter; three of them do not mean anything
/// more than one, and docblocks are full of them around the `*` decoration.
fn join_prose(lines: &[&str]) -> String {
    let mut out = String::new();
    let mut blank_pending = false;

    for line in lines {
        if line.is_empty() {
            blank_pending = !out.is_empty();
            continue;
        }
        if blank_pending {
            out.push_str("\n\n");
            blank_pending = false;
        } else if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }

    out
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::Docblock;

    #[test]
    fn the_comment_decoration_is_removed() {
        let parsed =
            Docblock::parse("/**\n * Finds a user.\n *\n * Looks in the cache first.\n */");

        expect![[r#"
            Finds a user.

            Looks in the cache first."#]]
        .assert_eq(&parsed.prose);
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn tags_are_separated_from_prose() {
        let parsed = Docblock::parse(
            "/**\n * Finds a user.\n *\n * @param int $id The identifier.\n * @return ?User\n */",
        );

        expect!["Finds a user."].assert_eq(&parsed.prose);
        assert_eq!(
            parsed.tags,
            ["@param int $id The identifier.", "@return ?User"]
        );
    }

    #[test]
    fn a_wrapped_tag_description_stays_with_its_tag() {
        // Extremely common, and showing the remainder as prose would put it above the tag it
        // belongs to.
        let parsed = Docblock::parse(
            "/**\n * @param int $id\n *   The identifier of the thing\n *   to look for.\n */",
        );

        assert_eq!(
            parsed.tags,
            ["@param int $id The identifier of the thing to look for."]
        );
        assert!(parsed.prose.is_empty());
    }

    /// A usage example in the description stays in the description, fenced.
    ///
    /// Every other continuation is joined with a space and trimmed, which is right for a wrapped
    /// `@param` description and turns a usage example into one line of PHP dressed as a sentence.
    /// Drupal writes them constantly -- `Details`, `Fieldset`, every render element.
    ///
    /// It is a fence in the prose rather than a tag of its own, and that is what keeps the *second*
    /// half of a Drupal docblock intact: those write prose, an example, more prose, another example.
    /// Lifted into the tag stream, the sentence after `@endcode` continued the `code` tag -- because
    /// a non-tag line continues the last tag and the tag was still the last one -- so it rendered
    /// glued to the closing `];` and inside the fence. A description is markdown that this layer
    /// passes through, so the fence belongs in it, and the example then stays where its author put
    /// it instead of moving to the end with the tags.
    #[test]
    fn a_usage_example_in_the_description_is_fenced_where_it_was_written() {
        let parsed = Docblock::parse(
            "/**\n * Usage example:\n * @code\n * $form['a'] = [\n *   '#type' => 'details',\n * ];\n\
             * @endcode\n *\n * Set a property on one option as follows.\n *\n * @code\n\
             * $form['a'][0]['#description'] = 'x';\n * @endcode\n *\n * @see \\App\\Fieldset\n */",
        );

        expect![[r#"
            Usage example:
            ```php
            $form['a'] = [
              '#type' => 'details',
            ];
            ```

            Set a property on one option as follows.

            ```php
            $form['a'][0]['#description'] = 'x';
            ```"#]]
        .assert_eq(&parsed.prose);
        expect![r"@see \App\Fieldset"].assert_eq(&parsed.tags.join("\n"));
    }

    /// Only `@endcode` should end a block, but a docblock that forgot it is a docblock that exists --
    /// and eating the `@see` and `@param` lines under it would lose real information to keep a
    /// rendering nicety. So any tag closes the fence, and the tag is then read as the tag it is.
    #[test]
    fn a_code_block_nobody_closed_still_ends_at_the_next_tag() {
        let parsed = Docblock::parse(
            "/**\n * @code\n * $x = 1;\n * @param int $id\n *   The identifier.\n */",
        );

        expect![[r#"
            ```php
            $x = 1;
            ```"#]]
        .assert_eq(&parsed.prose);
        expect!["@param int $id The identifier."].assert_eq(&parsed.tags.join("\n"));
    }

    #[test]
    fn deliberate_indentation_is_kept_but_the_conventional_space_is_not() {
        // A code sample in a docblock relies on its indentation; the single space after the star is
        // decoration and is not part of the text.
        let parsed = Docblock::parse("/**\n * Example:\n *\n *     $x = 1;\n */");

        expect![[r#"
            Example:

                $x = 1;"#]]
        .assert_eq(&parsed.prose);
    }

    #[test]
    fn a_one_line_docblock_works() {
        let parsed = Docblock::parse("/** A user. */");
        expect!["A user."].assert_eq(&parsed.prose);
    }

    #[test]
    fn an_empty_docblock_is_empty() {
        assert!(Docblock::parse("/**\n *\n */").is_empty());
        assert!(Docblock::parse("").is_empty());
    }

    #[test]
    fn a_docblock_without_the_star_convention_still_parses() {
        // Not everyone decorates. Requiring the star would silently drop the whole comment.
        let parsed = Docblock::parse("/**\nFinds a user.\n@return ?User\n*/");

        expect!["Finds a user."].assert_eq(&parsed.prose);
        assert_eq!(parsed.tags, ["@return ?User"]);
    }

    #[test]
    fn both_spellings_of_the_inherit_tag_are_recognised() {
        assert!(Docblock::parse("/** {@inheritdoc} */").inherits());
        assert!(Docblock::parse("/** {@inheritDoc} */").inherits());
        assert!(Docblock::parse("/** Note. {@inheritdoc} */").inherits());
        assert!(!Docblock::parse("/** Note. */").inherits());
    }

    #[test]
    fn the_inherited_text_replaces_the_tag_where_it_stood() {
        let mut parsed = Docblock::parse("/**\n * {@inheritdoc}\n *\n * And caches it.\n */");
        parsed.resolve_inherited(&Docblock::parse("/** Finds a user. */"));

        expect![[r#"
            Finds a user.

            And caches it."#]]
        .assert_eq(&parsed.prose);
    }

    #[test]
    fn tags_are_inherited_only_when_the_override_has_none() {
        let inherited = Docblock::parse("/**\n * Finds.\n *\n * @return ?User\n */");

        let mut without = Docblock::parse("/** {@inheritdoc} */");
        without.resolve_inherited(&inherited);
        assert_eq!(without.tags, ["@return ?User"]);

        let mut with_own = Docblock::parse("/**\n * {@inheritdoc}\n *\n * @return ?Admin\n */");
        with_own.resolve_inherited(&inherited);
        assert_eq!(with_own.tags, ["@return ?Admin"]);
    }
}
