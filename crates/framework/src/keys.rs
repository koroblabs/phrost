//! Top-level keys of a YAML file, and one field read out of each.
//!
//! The shape three of Drupal's configuration files share: a key at column zero, and an indented
//! block under it. A route name, a permission id, a library name -- all declared that way, and none
//! of them a name PHP has, so nothing but a reader like this one can find them.
//!
//! A line scanner rather than a YAML parser, for the reason the service reader gives: what is needed
//! is a key and one field, both written flat, and every one of these files is machine-shaped enough
//! that the flat reading is exact. What a parser would buy is anchors and multi-line scalars, which
//! none of these use for the two things read here.

/// One top-level key, where it is written, and how to describe it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyedEntry {
    pub key: String,
    /// Zero-based, as the protocol counts them.
    ///
    /// A line and a column rather than a byte offset, because nothing downstream holds this file's
    /// text: it is not PHP and the index does not keep it, so an offset could never be turned into a
    /// position later. Columns are in bytes, which is the same as characters here -- a route name and
    /// a permission id are ASCII, and a key that is not simply describes itself one column short.
    pub line: u32,
    pub character: u32,
    /// The value of the field asked for, when the block has one.
    pub detail: Option<String>,
}

/// Every top-level key in `text`, with `field` read from each one's block.
///
/// A top-level key is a line whose first character is not a space and which ends in a colon. That
/// excludes a document's own `---`, a comment, and every nested line -- and it is what tells a route
/// name apart from the `path:` under it without knowing what either means.
pub fn keys_in(text: &str, field: &str) -> Vec<KeyedEntry> {
    let mut found: Vec<KeyedEntry> = Vec::new();

    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
            continue;
        }

        if line.starts_with([' ', '\t']) {
            // Inside the current block. Only the field asked for is read, and only the first one:
            // the same field written twice is a file that does not load, and taking the first is the
            // reading that does not depend on how far the walk got.
            if let Some(entry) = found.last_mut()
                && entry.detail.is_none()
                && let Some(value) = field_value(trimmed, field)
            {
                entry.detail = Some(value);
            }
            continue;
        }

        let Some(key) = top_level_key(trimmed) else {
            continue;
        };
        // The column the key's first character is at, which is what a jump should land on: past an
        // opening quote when the key is quoted.
        let character = u32::try_from(trimmed.len() - trimmed.trim_start().len()).unwrap_or(0)
            + u32::from(quoted(trimmed.trim_start()));
        found.push(KeyedEntry {
            key,
            line: u32::try_from(number).unwrap_or(0),
            character,
            detail: None,
        });
    }

    found
}

/// The key a top-level line declares, or nothing when the line is not one.
fn top_level_key(line: &str) -> Option<String> {
    let key = line.strip_suffix(':')?;
    if key.is_empty() {
        return None;
    }
    let key = key
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            key.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .unwrap_or(key);
    // A key with a colon in it is not one of these files' keys, and reading it as one would claim a
    // route named after half a URL.
    (!key.is_empty() && !key.contains(':')).then(|| key.to_owned())
}

/// Whether a key is written in quotes, which shifts where the name itself starts.
fn quoted(key: &str) -> bool {
    key.starts_with('\'') || key.starts_with('"')
}

/// The value of `field` on an indented line, unquoted.
fn field_value(line: &str, field: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix(field)?.strip_prefix(':')?;
    let value = rest.trim();
    if value.is_empty() {
        return None;
    }
    let value = value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            value
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .unwrap_or(value);
    Some(value.to_owned())
}

/// The value of a top-level `field: value` line, for a file that *is* one declaration.
///
/// A config entity's file has no key of its own -- the whole file is the entity, and its id is in the
/// file's name. So there is no block to look inside, only lines at column zero.
pub fn field_at_root(text: &str, field: &str) -> Option<String> {
    for line in text.lines() {
        if line.starts_with([' ', '\t']) {
            continue;
        }
        // `continue` and not `?`: the field is somewhere among these lines, not necessarily first.
        let Some(rest) = line.strip_prefix(field) else {
            continue;
        };
        if let Some(value) = rest.strip_prefix(':') {
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            return Some(unquote(value).to_owned());
        }
    }
    None
}

/// A value without the quotes around it, if it had any.
fn unquote(value: &str) -> &str {
    value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            value
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_read_with_the_field_asked_for() {
        let text = "\
app.health:
  path: '/health'
  defaults:
    _controller: '\\Drupal\\X::y'

app.other:
  path: /admin/other
";
        let found = keys_in(text, "path");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].key, "app.health");
        assert_eq!(found[0].line, 0);
        assert_eq!(found[0].character, 0);
        assert_eq!(found[0].detail.as_deref(), Some("/health"));
        assert_eq!(found[1].key, "app.other");
        assert_eq!(found[1].line, 5);
        assert_eq!(found[1].detail.as_deref(), Some("/admin/other"));
    }

    #[test]
    fn a_key_with_spaces_and_quotes_keeps_its_name_and_loses_the_quotes() {
        // Both spellings are in real permission files, and the position has to land on the name
        // rather than on the quote a jump would otherwise select.
        let text = "\
administer warehouse:
  title: 'Administer Warehouse'
'access something':
  title: Access
";
        let found = keys_in(text, "title");
        assert_eq!(found[0].key, "administer warehouse");
        assert_eq!(found[0].character, 0);
        assert_eq!(found[1].key, "access something");
        assert_eq!(found[1].character, 1);
        assert_eq!(found[1].detail.as_deref(), Some("Access"));
    }

    #[test]
    fn nested_keys_and_comments_are_not_top_level_keys() {
        let text = "\
# A comment:
---
app.one:
  requirements:
    _access: 'TRUE'
  options:
    no_cache: TRUE
";
        let found = keys_in(text, "path");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].key, "app.one");
        assert_eq!(found[0].detail, None, "the block has no `path`");
    }

    #[test]
    fn a_root_level_field_is_read_from_a_file_that_is_one_declaration() {
        // A config entity's file has no key of its own: the whole file is the entity and its id is in
        // the file's name, so there is no block to look inside.
        let text = "langcode: en\nid: node.field_amount\nfield_name: field_amount\ntype: decimal\nsettings:\n  type: not_this_one\n";
        assert_eq!(field_at_root(text, "type").as_deref(), Some("decimal"));
        assert_eq!(
            field_at_root(text, "field_name").as_deref(),
            Some("field_amount")
        );
        assert_eq!(field_at_root(text, "nothing_like_it"), None);
    }

    #[test]
    fn a_field_a_block_does_not_have_is_left_absent() {
        // Absent rather than blank: a caller shows the key alone, which is honest, where a blank
        // detail reads as a route whose path is empty.
        let found = keys_in("app.one:\n  defaults:\n    _title: X\n", "path");
        assert_eq!(found[0].detail, None);
    }
}
