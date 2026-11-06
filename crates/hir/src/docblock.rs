use phrost_model::TextRange;

/// A docblock attached to a declaration, kept as raw text.
///
/// Deliberately unparsed at this layer. Docblock types are a second, much richer type language
/// than PHP's native hints — generics, array shapes, conditional returns — and parsing them here
/// would put that whole surface in the path of every declaration lookup. The type layer parses a
/// docblock when it actually needs the type, and only then.
///
/// What is kept is enough to do that later without re-reading the file: the text and where it was.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocblockData {
    /// Raw comment text including the `/**` and `*/` delimiters, so offsets inside it stay
    /// relative to `range` and any position reported from a docblock type maps back to the file.
    pub text: Box<str>,
    pub range: TextRange,
}

impl DocblockData {
    /// Whether the comment contains any `@tag` at all.
    ///
    /// A cheap pre-filter: most docblocks in real code are prose, and skipping them saves the
    /// type layer from parsing text that cannot contribute a type.
    pub fn has_tags(&self) -> bool {
        self.text.contains('@')
    }

    /// The names this docblock declares as type aliases: `@phpstan-type Row array{...}`, Psalm's
    /// `@psalm-type Row = array{...}`, and the local name of an `@phpstan-import-type` /
    /// `@psalm-import-type` -- its `as` name when it has one.
    pub fn alias_names(&self) -> impl Iterator<Item = &str> {
        self.text.match_indices('@').filter_map(|(at, _)| {
            let rest = &self.text[at + 1..];
            let rest = rest
                .strip_prefix("phpstan-")
                .or_else(|| rest.strip_prefix("psalm-"))?;
            let (imported, rest) = match rest.strip_prefix("import-type") {
                Some(rest) => (true, rest),
                None => (false, rest.strip_prefix("type")?),
            };
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            let line = rest.trim_start().lines().next().unwrap_or_default();
            let mut words = line.split_whitespace();
            let first = words.next()?;
            let local = match imported {
                // `Row from Source as Line` is used as `Line`.
                true => {
                    let rest = words.collect::<Vec<_>>();
                    match rest.iter().position(|word| *word == "as") {
                        Some(at) => rest.get(at + 1).copied().unwrap_or(first),
                        None => first,
                    }
                }
                false => first,
            };
            let end = local
                .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .unwrap_or(local.len());
            (end > 0).then(|| &local[..end])
        })
    }

    /// The names this docblock declares as template parameters, in any of the spellings tools
    /// accept: `@template T`, `@template-covariant T`, and both under a `phpstan-` or `psalm-`
    /// prefix.
    pub fn template_names(&self) -> impl Iterator<Item = &str> {
        self.text.match_indices('@').filter_map(|(at, _)| {
            let rest = &self.text[at + 1..];
            let rest = rest
                .strip_prefix("phpstan-")
                .or_else(|| rest.strip_prefix("psalm-"))
                .unwrap_or(rest);
            let rest = rest.strip_prefix("template")?;
            let rest = rest
                .strip_prefix("-covariant")
                .or_else(|| rest.strip_prefix("-contravariant"))
                .unwrap_or(rest);
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            let name = rest.trim_start();
            let end = name
                .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .unwrap_or(name.len());
            (end > 0).then(|| &name[..end])
        })
    }
}
