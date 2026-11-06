use std::{iter::Peekable, str::Chars};

/// One lexical token from a composer-generated `autoload_*.php` file.
///
/// Deliberately minimal: this file's shape never needs a real PHP lexer, only enough to walk
/// `'key' => array($anchor . 'suffix', ...)` entries. Everything else in the file — the `<?php`
/// tag, the `$vendorDir = dirname(__DIR__);` preamble, comments — is never turned into a token at
/// all; [`tokenize`] just skips past it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Token {
    /// The unescaped contents of a single-quoted string literal.
    Str(String),
    /// A `$name` variable, without the leading `$`.
    Var(String),
    /// A bare word: `array`, `return`, `dirname`, `__DIR__`, ...
    Word(String),
    Dot,
    Comma,
    LParen,
    RParen,
    FatArrow,
}

/// Lexes `source` into tokens, silently skipping any byte that starts none of the shapes above.
///
/// Silent skipping rather than an error is deliberate: this file's syntax has plenty of PHP this
/// parser has no reason to understand, and failing on it would make the parser as fragile as if
/// it tried to understand all of PHP — exactly what it exists to avoid.
pub(crate) fn tokenize(source: &str) -> Vec<Token> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            c if c.is_whitespace() => {
                chars.next();
            }
            '/' => {
                chars.next();
                match chars.peek() {
                    Some('/') => skip_while(&mut chars, |c| c != '\n'),
                    Some('*') => {
                        chars.next();
                        skip_block_comment(&mut chars);
                    }
                    // A lone `/` is not a token this grammar ever needs.
                    _ => {}
                }
            }
            '#' => skip_while(&mut chars, |c| c != '\n'),
            '\'' => {
                chars.next();
                tokens.push(Token::Str(read_single_quoted(&mut chars)));
            }
            '$' => {
                chars.next();
                tokens.push(Token::Var(read_word(&mut chars)));
            }
            '.' => {
                chars.next();
                tokens.push(Token::Dot);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::FatArrow);
                }
                // A bare `=` (the `$vendorDir = dirname(__DIR__);` preamble) needs no token.
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                tokens.push(Token::Word(read_word(&mut chars)))
            }
            _ => {
                chars.next();
            }
        }
    }

    tokens
}

fn skip_while(chars: &mut Peekable<Chars<'_>>, mut keep_going: impl FnMut(char) -> bool) {
    while let Some(&c) = chars.peek() {
        if !keep_going(c) {
            break;
        }
        chars.next();
    }
}

fn skip_block_comment(chars: &mut Peekable<Chars<'_>>) {
    let mut previous = None;
    for c in chars.by_ref() {
        if previous == Some('*') && c == '/' {
            return;
        }
        previous = Some(c);
    }
    // An unterminated block comment is malformed input; running to end of file is a safe
    // fallback rather than a panic.
}

fn read_word(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut word = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
            chars.next();
        } else {
            break;
        }
    }
    word
}

/// Reads a single-quoted PHP string literal's contents, given an iterator already past the
/// opening `'`.
///
/// Applies PHP's single-quote escape rules exactly: `\\` folds to one backslash, `\'` folds to a
/// literal quote, and any other backslash is kept as a literal backslash — PHP does not treat it
/// as an escape introducer inside single quotes. This is what turns a doubled namespace separator
/// (`Psr\\Log\\`, as composer always writes one) back into the real `Psr\Log\`.
fn read_single_quoted(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut text = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some('\\') => {
                    text.push('\\');
                    chars.next();
                }
                Some('\'') => {
                    text.push('\'');
                    chars.next();
                }
                _ => text.push('\\'),
            },
            '\'' => break,
            other => text.push(other),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_a_doubled_backslash_into_one_namespace_separator() {
        // Composer's generator always doubles namespace separators inside its single-quoted
        // string literals, matching PHP's own escaping rule. Getting this backward leaves every
        // namespaced PSR-4 prefix with its separators still doubled.
        let tokens = tokenize(r#"'Psr\\Log\\'"#);
        assert_eq!(tokens, vec![Token::Str("Psr\\Log\\".to_string())]);
    }

    #[test]
    fn a_backslash_not_followed_by_a_quote_or_backslash_is_kept_literally() {
        // PHP's single-quote escaping only recognizes `\\` and `\'`; any other backslash,
        // including `\n`, stays as a literal backslash followed by the next character.
        let tokens = tokenize(r#"'C:\new'"#);
        assert_eq!(tokens, vec![Token::Str("C:\\new".to_string())]);
    }

    #[test]
    fn skips_line_and_block_comments_between_meaningful_tokens() {
        let tokens = tokenize("// header comment\n'a' /* inline */ => 'b',\n# trailing comment\n");
        assert_eq!(
            tokens,
            vec![
                Token::Str("a".to_string()),
                Token::FatArrow,
                Token::Str("b".to_string()),
                Token::Comma,
            ]
        );
    }

    #[test]
    fn recognizes_variables_words_and_structural_punctuation() {
        let tokens = tokenize("$vendorDir . array(dirname(__DIR__))");
        assert_eq!(
            tokens,
            vec![
                Token::Var("vendorDir".to_string()),
                Token::Dot,
                Token::Word("array".to_string()),
                Token::LParen,
                Token::Word("dirname".to_string()),
                Token::LParen,
                Token::Word("__DIR__".to_string()),
                Token::RParen,
                Token::RParen,
            ]
        );
    }
}
