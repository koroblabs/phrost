//! The translation keys that name an array, read out of the language files.
//!
//! `trans('auth.failed')` declares `array|string`, and which one comes back is the language files'
//! answer: the line a key names, the whole array where it names a group (`trans('auth')`) or a
//! nested list (`trans('validation.between')`) -- and the key itself, a string, where it names
//! nothing. So only the keys holding arrays are filed, and the rule beside the catalog says every
//! other literal holds a string.
//!
//! The framework's own files first, which an application that never published them still reads,
//! then the application's: one locale, the one `config/app.php` falls back to.
//!
//! Read as text with a small lexer of its own, because a translation is prose: it holds `//` in
//! a URL and `#` in a sentence, which the config reader's comment stripper would cut.

use std::path::{Path, PathBuf};

use phrost_model::BuiltinTy;

use crate::{CatalogEntry, CatalogName};

/// The catalog the keys holding arrays are filed under. A name this provider chose.
pub(super) const CATALOG: &str = "laravel.translation-arrays";

/// Every key of the source locale's files that names an array: each group, and each nested list.
pub(super) fn array_keys(root: &Path) -> Vec<CatalogEntry> {
    let locale = source_locale(root);
    let directories = [
        root.join("vendor/laravel/framework/src/Illuminate/Translation/lang"),
        root.join("lang"),
        root.join("resources/lang"),
    ];
    let mut entries: Vec<CatalogEntry> = Vec::new();
    for directory in directories.iter().map(|directory| directory.join(&locale)) {
        let Ok(listing) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut files: Vec<PathBuf> = listing
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
            .collect();
        files.sort();
        for path in files {
            let Some(group) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let mut keys = vec![group.to_owned()];
            keys.extend(
                nested_arrays(&text)
                    .into_iter()
                    .map(|nested| format!("{group}.{nested}")),
            );
            for key in keys {
                if entries.iter().any(|known| known.key == key) {
                    continue;
                }
                entries.push(CatalogEntry {
                    catalog: CATALOG,
                    key,
                    names: Some(CatalogName::Shape(BuiltinTy::Array)),
                    declared_in: Some(path.clone()),
                });
            }
        }
    }
    entries
}

/// The locale the application's own strings are written in: `config/app.php`'s `fallback_locale`,
/// as a literal or as the default of an `env()` -- English where it says neither.
fn source_locale(root: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(root.join("config").join("app.php")) else {
        return "en".to_owned();
    };
    let code = super::config_text::without_comments(&text);
    super::config_text::value_of(&code, "fallback_locale")
        .and_then(|value| {
            let value = value.trim();
            let value = match value.strip_prefix("env(") {
                Some(arguments) => arguments.split_once(',')?.1.trim_start(),
                None => value,
            };
            let quoted = value.strip_prefix('\'')?;
            Some(quoted[..quoted.find('\'')?].to_owned())
        })
        .filter(|locale| {
            !locale.is_empty()
                && locale
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .unwrap_or_else(|| "en".to_owned())
}

/// The dotted paths, below the file's own array, of every key whose value is an array.
fn nested_arrays(text: &str) -> Vec<String> {
    let tokens = Token::all(text);
    let mut out = Vec::new();
    // The key each open array was written under; `None` for the file's own and for a list.
    let mut open: Vec<Option<String>> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        match &tokens[index] {
            Token::Open => open.push(None),
            Token::Close => {
                open.pop();
            }
            Token::Quoted(key) if matches!(tokens.get(index + 1), Some(Token::Arrow)) => {
                if matches!(tokens.get(index + 2), Some(Token::Open)) && !open.is_empty() {
                    let mut path: Vec<&str> = open.iter().flatten().map(String::as_str).collect();
                    path.push(key);
                    out.push(path.join("."));
                    open.push(Some(key.clone()));
                    index += 3;
                    continue;
                }
                index += 2;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    out
}

/// What the reader needs of PHP: strings, `=>`, and the brackets an array is written with.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Token {
    Quoted(String),
    Arrow,
    Open,
    Close,
    Other,
}

impl Token {
    /// The file's tokens, comments and the opening tag skipped.
    fn all(text: &str) -> Vec<Self> {
        let bytes = text.as_bytes();
        let mut tokens = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let rest = &text[at..];
            if rest.starts_with("//") || (rest.starts_with('#') && !rest.starts_with("#[")) {
                at += rest.find('\n').unwrap_or(rest.len());
            } else if let Some(after) = rest.strip_prefix("/*") {
                at += 2 + after.find("*/").map_or(after.len(), |end| end + 2);
            } else if rest.starts_with('\'') || rest.starts_with('"') {
                let (quoted, length) = Self::quoted(rest);
                tokens.push(Self::Quoted(quoted));
                at += length;
            } else if rest.starts_with("=>") {
                tokens.push(Self::Arrow);
                at += 2;
            } else if rest.starts_with('[') {
                tokens.push(Self::Open);
                at += 1;
            } else if rest
                .get(..6)
                .is_some_and(|word| word.eq_ignore_ascii_case("array("))
            {
                tokens.push(Self::Open);
                at += 6;
            } else if rest.starts_with(']') || rest.starts_with(')') {
                tokens.push(Self::Close);
                at += 1;
            } else if rest.starts_with('(') {
                // A call's parentheses balance the `)` above without opening an array.
                tokens.push(Self::Open);
                at += 1;
            } else {
                let next = rest.chars().next().map_or(1, char::len_utf8);
                if !rest.starts_with(char::is_whitespace) {
                    tokens.push(Self::Other);
                }
                at += next;
            }
        }
        tokens
    }

    /// A string literal at the start of `rest`, unescaped as far as a key needs, and its length.
    fn quoted(rest: &str) -> (String, usize) {
        let quote = rest.as_bytes()[0];
        let mut out = String::new();
        let mut escaped = false;
        for (index, letter) in rest.char_indices().skip(1) {
            if escaped {
                out.push(letter);
                escaped = false;
            } else if letter == '\\' {
                escaped = true;
            } else if letter as u32 == u32::from(quote) {
                return (out, index + 1);
            } else {
                out.push(letter);
            }
        }
        (out, rest.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nested_list_is_an_array_key_and_a_line_is_not() {
        let text = r#"<?php
// A comment with a 'quoted' => [ bracket.
return [
    'failed' => 'These credentials do not match. See https://example.com/#help',
    'between' => [
        'numeric' => 'Between :min and :max.',
        'deeper' => array('one' => 'x'),
    ],
    "it's" => 'fine',
    'language_select' => ['ar' => 'العربية', 'he' => 'עברית'],
    'list' => ['a', 'b'],
    'custom' => [
        'attribute-name' => [
            'rule-name' => 'custom-message',
        ],
    ],
];
"#;
        assert_eq!(
            nested_arrays(text),
            [
                "between",
                "between.deeper",
                "language_select",
                "list",
                "custom",
                "custom.attribute-name"
            ]
        );
    }

    #[test]
    fn the_groups_of_the_fallback_locale_are_filed() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let lang = dir.path().join("lang").join("de");
        std::fs::create_dir_all(&lang).expect("the language directory");
        std::fs::write(
            lang.join("auth.php"),
            "<?php return ['failed' => 'x', 'two' => ['a' => 'b']];",
        )
        .expect("a group");
        std::fs::create_dir_all(dir.path().join("config")).expect("the config directory");
        std::fs::write(
            dir.path().join("config").join("app.php"),
            "<?php return ['fallback_locale' => env('APP_FALLBACK_LOCALE', 'de')];",
        )
        .expect("app.php");
        let keys: Vec<String> = array_keys(dir.path())
            .into_iter()
            .map(|entry| entry.key)
            .collect();
        assert_eq!(keys, ["auth", "auth.two"]);
    }
}
