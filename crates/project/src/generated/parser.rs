use std::{
    iter::Peekable,
    path::{Path, PathBuf},
    vec::IntoIter,
};

use super::lexer::Token;

/// One directory-anchor a composer-generated file's path expressions are built from.
///
/// `$vendorDir` and `$baseDir` are the only two identifiers composer's own generator ever emits
/// here, and they are genuinely different directories whenever a project's layout is not the
/// simplest case. Drupal core is the standing example: it registers paths under `$baseDir` that
/// are not inside `vendor/` at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Anchor {
    VendorDir,
    BaseDir,
}

/// One value from a generated array, before it is resolved against real directories.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum PathValue {
    /// `$vendorDir . '...'` or `$baseDir . '...'`: anchored on whichever token preceded it.
    Anchored { anchor: Anchor, suffix: String },
    /// A bare string literal with no anchor, used as-is.
    Literal(String),
}

impl PathValue {
    /// Resolves this value against the two real directories it could be anchored on.
    ///
    /// The anchor is decided by which token actually preceded the literal in the source text —
    /// never by guessing from the literal's own shape. A heuristic like "starts with `/web/`, so
    /// it must mean `$baseDir`" would happen to be right for Drupal core and wrong the moment a
    /// vendor package's own path also starts with something that looks project-root-ish.
    pub(crate) fn resolve(&self, vendor_dir: &Path, base_dir: &Path) -> PathBuf {
        match self {
            Self::Anchored { anchor, suffix } => {
                let anchor_dir = match anchor {
                    Anchor::VendorDir => vendor_dir,
                    Anchor::BaseDir => base_dir,
                };
                let mut path = anchor_dir.to_path_buf();
                // Composer always writes the suffix with a leading `/` (`$baseDir . '/web/core'`).
                // `PathBuf::push` treats a component starting with `/` as an absolute path and
                // replaces the base entirely on Unix, which would silently drop the anchor; strip
                // it first so the join actually joins.
                path.push(suffix.trim_start_matches('/'));
                path
            }
            Self::Literal(text) => PathBuf::from(text),
        }
    }
}

type Tokens = Peekable<IntoIter<Token>>;

/// Parses the `return array('key' => value, ...);` body out of a composer-generated autoload
/// file's tokens.
///
/// Everything before the first `return` — the `<?php` tag, the `$vendorDir = dirname(__DIR__);`
/// preamble, comments — is never turned into a token by the lexer in the first place, so skipping
/// to `return` is enough; there is nothing meaningful left behind to lose.
pub(crate) fn parse_entries(tokens: Vec<Token>) -> Vec<(String, Vec<PathValue>)> {
    let mut tokens: Tokens = tokens.into_iter().peekable();
    if !skip_to_return_array(&mut tokens) {
        // No `return array(...)` at all: a malformed or unexpected file contributes nothing,
        // which is the same outcome as a missing file. Neither is a broken state worth a panic.
        return Vec::new();
    }

    let mut entries = Vec::new();
    while !matches!(tokens.peek(), Some(Token::RParen) | None) {
        let Some(Token::Str(key)) = tokens.next() else {
            break;
        };
        if !matches!(tokens.next(), Some(Token::FatArrow)) {
            break;
        }
        entries.push((key, parse_value(&mut tokens)));
        if matches!(tokens.peek(), Some(Token::Comma)) {
            tokens.next();
        }
    }
    entries
}

/// Advances past `return`, `array`, `(`, in that order. Returns `false` if the token stream never
/// has one, which the caller treats as "this file declares nothing" rather than an error.
fn skip_to_return_array(tokens: &mut Tokens) -> bool {
    loop {
        match tokens.next() {
            Some(Token::Word(word)) if word == "return" => break,
            Some(_) => continue,
            None => return false,
        }
    }

    matches!(tokens.next(), Some(Token::Word(word)) if word == "array")
        && matches!(tokens.next(), Some(Token::LParen))
}

fn parse_value(tokens: &mut Tokens) -> Vec<PathValue> {
    if matches!(tokens.peek(), Some(Token::Word(word)) if word == "array") {
        tokens.next(); // `array`
        tokens.next(); // `(`
        let mut values = Vec::new();
        while !matches!(tokens.peek(), Some(Token::RParen) | None) {
            values.push(parse_single_value(tokens));
            if matches!(tokens.peek(), Some(Token::Comma)) {
                tokens.next();
            }
        }
        tokens.next(); // consume the `)`
        values
    } else {
        vec![parse_single_value(tokens)]
    }
}

/// Parses one `.`-joined chain — `$vendorDir . '/psr/log/src'` — or a bare `'literal'`.
fn parse_single_value(tokens: &mut Tokens) -> PathValue {
    let mut anchor = None;
    let mut suffix = String::new();

    loop {
        match tokens.next() {
            Some(Token::Var(name)) if name == "vendorDir" => anchor = Some(Anchor::VendorDir),
            Some(Token::Var(name)) if name == "baseDir" => anchor = Some(Anchor::BaseDir),
            Some(Token::Str(text)) => suffix.push_str(&text),
            _ => break,
        }
        if matches!(tokens.peek(), Some(Token::Dot)) {
            tokens.next();
        } else {
            break;
        }
    }

    match anchor {
        Some(anchor) => PathValue::Anchored { anchor, suffix },
        None => PathValue::Literal(suffix),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::lexer::tokenize;

    fn entries(source: &str) -> Vec<(String, Vec<PathValue>)> {
        parse_entries(tokenize(source))
    }

    #[test]
    fn base_dir_and_vendor_dir_resolve_to_different_anchors_in_the_same_array() {
        // The gotcha this exists to prevent: a parser that assumed every entry in this file is
        // relative to `$vendorDir` would route Drupal core's `$baseDir`-anchored entry to
        // `<root>/vendor/web/core/...`, which never exists. The anchor must come from whichever
        // token actually precedes each literal, not from a blanket assumption.
        let parsed = entries(
            r#"<?php
            $vendorDir = dirname(__DIR__);
            $baseDir = dirname($vendorDir);
            return array(
                'Drupal' => $baseDir . '/web/core/lib/Drupal.php',
                'Composer\\InstalledVersions' => $vendorDir . '/composer/InstalledVersions.php',
            );
            "#,
        );

        assert_eq!(
            parsed,
            vec![
                (
                    "Drupal".to_string(),
                    vec![PathValue::Anchored {
                        anchor: Anchor::BaseDir,
                        suffix: "/web/core/lib/Drupal.php".to_string(),
                    }]
                ),
                (
                    "Composer\\InstalledVersions".to_string(),
                    vec![PathValue::Anchored {
                        anchor: Anchor::VendorDir,
                        suffix: "/composer/InstalledVersions.php".to_string(),
                    }]
                ),
            ]
        );

        let vendor_dir = Path::new("/proj/vendor");
        let base_dir = Path::new("/proj");
        assert_eq!(
            parsed[0].1[0].resolve(vendor_dir, base_dir),
            PathBuf::from("/proj/web/core/lib/Drupal.php")
        );
        assert_eq!(
            parsed[1].1[0].resolve(vendor_dir, base_dir),
            PathBuf::from("/proj/vendor/composer/InstalledVersions.php")
        );
    }

    #[test]
    fn a_prefix_array_with_multiple_directories_parses_every_value() {
        let parsed = entries(
            r#"<?php
            return array(
                'Psr\\Log\\' => array($vendorDir . '/psr/log/src', $vendorDir . '/psr/log/src2'),
            );
            "#,
        );

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "Psr\\Log\\");
        assert_eq!(parsed[0].1.len(), 2);
    }

    #[test]
    fn a_leading_slash_on_the_suffix_does_not_replace_the_anchor_on_join() {
        // `PathBuf::push`/`join` treat a component starting with `/` as absolute and discard the
        // existing path entirely on Unix; without stripping that leading slash first, every
        // resolved path here would silently lose its vendor/base directory.
        let value = PathValue::Anchored {
            anchor: Anchor::VendorDir,
            suffix: "/psr/log/src".to_string(),
        };
        assert_eq!(
            value.resolve(Path::new("/proj/vendor"), Path::new("/proj")),
            PathBuf::from("/proj/vendor/psr/log/src")
        );
    }

    #[test]
    fn a_bare_literal_with_no_anchor_is_used_as_is() {
        let parsed = entries(
            r#"<?php
            return array(
                'Foo' => '/already/absolute/Foo.php',
            );
            "#,
        );

        assert_eq!(
            parsed,
            vec![(
                "Foo".to_string(),
                vec![PathValue::Literal("/already/absolute/Foo.php".to_string())]
            )]
        );
    }

    #[test]
    fn a_file_with_no_return_array_at_all_contributes_no_entries() {
        assert_eq!(entries("<?php\n// nothing here\n"), Vec::new());
    }
}
