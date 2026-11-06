//! Reading a PHP file Laravel keeps a table in, as text.
//!
//! A config file is a `return [...]` and `Facade::defaultAliases()` a literal collection, and nothing
//! in this crate parses PHP. What these share is the four things such a table is written with:
//! comments, `use` lines, a namespace, and a class written as `Name::class` or a quoted string.

/// The text with `//`, `#` and `/* */` comments taken out. Strings are not tracked: a config file
/// writes none of those characters inside one that matters here, and a wrong cut can only lose an
/// entry.
pub(super) fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |end| &after[end + 2..]);
        } else if rest.starts_with("//") || (rest.starts_with('#') && !rest.starts_with("#[")) {
            rest = rest.find('\n').map_or("", |end| &rest[end..]);
        } else {
            let next = rest.chars().next().unwrap_or_default();
            out.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
    out
}

/// The names a file's `use` lines and its namespace make a written name mean.
pub(super) struct Scope {
    namespace: Option<String>,
    imports: Vec<(String, String)>,
}

impl Scope {
    /// `use App\Models\User;` and `use App\Models\User as Member;`, and the `namespace` line.
    pub(super) fn of(code: &str) -> Self {
        let mut namespace = None;
        let mut imports = Vec::new();
        for line in code.lines().map(str::trim) {
            if let Some(name) = line
                .strip_prefix("namespace ")
                .and_then(|rest| rest.strip_suffix(';'))
            {
                namespace = Some(name.trim().trim_start_matches('\\').to_owned());
                continue;
            }
            let Some(line) = line
                .strip_prefix("use ")
                .and_then(|rest| rest.strip_suffix(';'))
                .map(str::trim)
            else {
                continue;
            };
            let (path, alias) = match line.split_once(" as ") {
                Some((path, alias)) => (path.trim(), alias.trim()),
                None => match line.rsplit('\\').next() {
                    Some(last) => (line, last),
                    None => continue,
                },
            };
            imports.push((alias.to_owned(), path.trim_start_matches('\\').to_owned()));
        }
        Self { namespace, imports }
    }

    /// A written name, resolved the way PHP resolves a class name in this file.
    pub(super) fn resolved(&self, written: &str) -> String {
        if let Some(full) = written.strip_prefix('\\') {
            return full.to_owned();
        }
        let (first, rest) = match written.split_once('\\') {
            Some((first, rest)) => (first, Some(rest)),
            None => (written, None),
        };
        let imported = self
            .imports
            .iter()
            .find(|(short, _)| short.eq_ignore_ascii_case(first));
        match (imported, rest) {
            (Some((_, full)), Some(rest)) => format!("{full}\\{rest}"),
            (Some((_, full)), None) => full.clone(),
            (None, _) => match &self.namespace {
                Some(namespace) => format!("{namespace}\\{written}"),
                None => written.to_owned(),
            },
        }
    }
}

/// The class a value names: `User::class`, `env('AUTH_MODEL', User::class)`, `'App\Models\User'`.
///
/// A quoted name comes back with a leading separator, because a string is always fully qualified.
pub(super) fn written_class(value: &str) -> Option<String> {
    let value = match value.strip_prefix("env(") {
        // The fallback is the second argument, after the variable's name.
        Some(arguments) => arguments.split_once(',')?.1.trim_start(),
        None => value,
    };
    if let Some(quoted) = value.strip_prefix('\'') {
        let name = &quoted[..quoted.find('\'')?];
        return Some(format!("\\{}", name.replace("\\\\", "\\")));
    }
    let end = value
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'))
        .unwrap_or(value.len());
    let (name, rest) = value.split_at(end);
    (!name.is_empty() && rest.starts_with("::class")).then(|| name.to_owned())
}

/// Every `'key' => Class::class` in a stretch of a table, with the class as written.
pub(super) fn class_entries(code: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = code;
    while let Some(open) = rest.find('\'') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('\'') else {
            break;
        };
        let key = &after[..close];
        rest = &after[close + 1..];
        let Some(value) = rest.trim_start().strip_prefix("=>") else {
            continue;
        };
        if let Some(class) = written_class(value.trim_start()) {
            out.push((key.to_owned(), class));
        }
    }
    out
}

/// The value written after `'key' =>`, up to the comma or bracket that ends it.
pub(super) fn value_of<'a>(code: &'a str, key: &str) -> Option<&'a str> {
    let quoted = format!("'{key}'");
    let at = code.find(&quoted)?;
    let value = code[at + quoted.len()..].trim_start().strip_prefix("=>")?;
    let mut depth = 0_i32;
    let mut quote = None;
    for (index, letter) in value.char_indices() {
        match (quote, letter) {
            (Some(open), _) if letter == open => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(letter),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') if depth == 0 => return Some(&value[..index]),
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => return Some(&value[..index]),
            _ => {}
        }
    }
    Some(value)
}
