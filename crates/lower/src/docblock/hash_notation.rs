//! WordPress's hash notation for an array a function returns.
//!
//! ```php
//! /**
//!  * @return array {
//!  *     Values for the extension and mime type.
//!  *
//!  *     @type string|false $ext  File extension, or false if the file doesn't match a mime type.
//!  *     @type string|false $type File mime type, or false if the file doesn't match a mime type.
//!  * }
//!  */
//! ```
//!
//! The draft PSR-5 spelling of a shape, and how WordPress documents 134 of its returns. mago reads
//! `array {` as the start of `array{...}` with a space in it, fails on the prose inside, and the tag
//! said nothing at all -- so `wp_check_filetype()` and `wp_get_current_commenter()` had no return
//! type, not even `array`.
//!
//! The docblock is rewritten before mago reads it, and **without changing its length**: the braces
//! that open and close a hash become spaces, and `@type` inside one becomes `@var ` -- five bytes for
//! five. mago then parses an ordinary `@return array` and one `@var` tag per key, and every span it
//! hands back is a span of the text as written, so a class named in a key's type still has its
//! goto and its references. What the `@var` tags say is gathered back into a shape by
//! [`Hashed::shape_of`].
//!
//! Only a `@return`. A `@param array $args {` parses already, because a name stands between the type
//! and the brace -- and its keys are the ones a caller *may* pass, which a shape would turn into the
//! ones it must.

use mago_phpdoc_syntax::cst::{Document, Element, TagValue};
use mago_span::HasSpan;
use phrost_hir::{TypeRefData, TypeRefId};
use phrost_model::BuiltinTy;

use super::type_alias::TypeAliasTable;
use crate::lowerer::Lowerer;

/// A docblock with its hashes rewritten, and where each one was.
pub(crate) struct Hashed {
    pub(crate) text: String,
    /// As written, which is where a key is read: the rewrite spells a positional key `$0` as `$n`,
    /// because mago takes no variable whose name starts with a digit.
    written: String,
    blocks: Vec<Block>,
}

/// One hash: the offsets of its opening and closing brace in the docblock.
#[derive(Clone, Copy)]
struct Block {
    open: usize,
    close: usize,
}

impl Hashed {
    /// The docblock rewritten, or `None` when it holds no hash on a `@return` line -- or one whose
    /// braces do not balance, which is prose with a brace in it rather than a hash.
    ///
    /// A hash is told from a shape written across lines by two things, and both are required:
    /// a space before the brace -- `array {` where PHPStan's spelling is `array{` -- and a `@type`
    /// line directly inside. Either alone would read `@return array{` above `key: T,` lines as a
    /// hash and blank its braces, which took every multi-line shape on the reference project down
    /// to a bare `array` the first time this was measured.
    pub(crate) fn of(text: &str) -> Option<Self> {
        if !text.contains('{') || !text.contains("@return") {
            return None;
        }
        let mut bytes = text.as_bytes().to_vec();
        let mut open = Vec::new();
        let mut blocks = Vec::new();
        let mut start = 0;
        for line in text.split_inclusive('\n') {
            let at = start;
            start += line.len();
            let lead = line.len()
                - line
                    .trim_start_matches(|found: char| {
                        found.is_whitespace() || found == '*' || found == '/'
                    })
                    .len();
            let content = line[lead..].trim_end();
            let inside = !open.is_empty();
            // A key inside a hash is a `@type` line, which mago reads as a type alias; spelled
            // `@var ` it is a type and a name, which is what it says.
            if let Some((_, keyed)) = open.last_mut()
                && content.starts_with("@type ")
            {
                *keyed = true;
                bytes[at + lead..at + lead + 5].copy_from_slice(b"@var ");
                // `@type int $0 The width.` -- a positional key, which PHP gives every list.
                if let Some(dollar) = content.find('$') {
                    let digits = content[dollar + 1..]
                        .bytes()
                        .take_while(u8::is_ascii_digit)
                        .count();
                    let from = at + lead + dollar + 1;
                    bytes[from..from + digits].fill(b'n');
                }
            }
            let opens = content.ends_with('{')
                && content[..content.len() - 1].ends_with(char::is_whitespace)
                && match inside {
                    false => content.starts_with("@return "),
                    true => content.starts_with("@type "),
                };
            if opens {
                let brace = at + lead + content.len() - 1;
                bytes[brace] = b' ';
                open.push((brace, false));
            } else if inside && content == "}" {
                let brace = at + lead;
                bytes[brace] = b' ';
                let (opened, keyed) = open.pop()?;
                if !keyed {
                    return None;
                }
                blocks.push(Block {
                    open: opened,
                    close: brace,
                });
            }
        }
        if !open.is_empty() || blocks.is_empty() {
            return None;
        }
        Some(Self {
            text: String::from_utf8(bytes).ok()?,
            written: text.to_owned(),
            blocks,
        })
    }

    /// The hash written on the line a tag's type ends on, if one was.
    fn block_after(&self, end: usize) -> Option<Block> {
        let line_end = self.text[end..]
            .find('\n')
            .map_or(self.text.len(), |at| end + at);
        self.blocks
            .iter()
            .copied()
            .find(|block| block.open >= end && block.open < line_end)
    }

    /// The returned type with the array it names replaced by the shape its hash lists.
    ///
    /// The hash describes the array's own keys whatever the type before the brace said: `int[] {
    /// @type int $0 ... @type int $1 ... }` is a width and a height, so `array{0: int, 1: int}`,
    /// which is narrower than `int[]` and agrees with it. In a union only the array operand is
    /// replaced -- `array|false {` keeps its `false` -- and a type with no array in it, such as an
    /// `object {` describing properties, is kept as it was.
    pub(crate) fn shape_of(
        &self,
        lowerer: &mut Lowerer<'_, '_>,
        document: &Document<'_>,
        returned: &mago_phpdoc_syntax::cst::r#type::Type<'_>,
        lowered: TypeRefId,
        base: phrost_model::TextSize,
        aliases: &TypeAliasTable,
    ) -> TypeRefId {
        let Some(block) = self.block_after(returned.span().end.offset as usize) else {
            return lowered;
        };
        let Some(shape) = self.fields_of(lowerer, document, block, base, aliases) else {
            return lowered;
        };
        lowerer.with_array_replaced(lowered, shape)
    }

    /// The shape one hash lists: a field per `@type` line directly inside it, with a nested hash
    /// read the same way.
    fn fields_of(
        &self,
        lowerer: &mut Lowerer<'_, '_>,
        document: &Document<'_>,
        block: Block,
        base: phrost_model::TextSize,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        let mut fields = Vec::new();
        for element in document.elements.iter() {
            let Element::Tag(entry) = element else {
                continue;
            };
            let TagValue::Var(value) = &entry.value else {
                continue;
            };
            let at = entry.at.start.offset as usize;
            // Directly inside this hash, and not inside one nested in it -- that one is its own
            // field's shape.
            let directly = at > block.open
                && at < block.close
                && !self.blocks.iter().any(|nested| {
                    nested.open > block.open
                        && nested.close < block.close
                        && at > nested.open
                        && at < nested.close
                });
            if !directly {
                continue;
            }
            let Some(variable) = value.variable.as_ref() else {
                continue;
            };
            let span = variable.span();
            let Some(key) = self
                .written
                .get(span.start.offset as usize..span.end.offset as usize)
                .map(|written| written.trim_start_matches('$'))
            else {
                continue;
            };
            let Some(ty) = lowerer.lower_doc_type(value.r#type, base, &self.text, aliases) else {
                continue;
            };
            let ty = match self.block_after(value.r#type.span().end.offset as usize) {
                Some(nested) => match self.fields_of(lowerer, document, nested, base, aliases) {
                    Some(shape) => lowerer.with_array_replaced(ty, shape),
                    None => ty,
                },
                None => ty,
            };
            fields.push((key.to_owned(), ty));
        }
        (!fields.is_empty()).then(|| lowerer.shape_from(&fields))
    }
}

impl Lowerer<'_, '_> {
    /// A shape of required keys, as a written `array{key: T, ...}` lowers.
    fn shape_from(&mut self, fields: &[(String, TypeRefId)]) -> TypeRefId {
        let fields = fields
            .iter()
            .map(|(key, ty)| {
                self.builder.add_type_ref(TypeRefData::ShapeField {
                    key: self.interner.name(key),
                    optional: false,
                    ty: *ty,
                })
            })
            .collect::<Vec<_>>();
        let fields = self.builder.alloc_type_ref_list(&fields);
        // Open: prose documenting a return lists the keys worth explaining, and nothing in the
        // notation says they are all of them.
        self.builder.add_type_ref(TypeRefData::Shape {
            fields,
            open: true,
            list: false,
        })
    }

    /// `ty` with every array operand replaced by `shape`, or `ty` itself where it names no array.
    fn with_array_replaced(&mut self, ty: TypeRefId, shape: TypeRefId) -> TypeRefId {
        match self.builder.type_ref(ty).clone() {
            TypeRefData::Builtin(BuiltinTy::Array) => shape,
            TypeRefData::Generic { base, .. }
                if matches!(
                    self.builder.type_ref(base),
                    TypeRefData::Builtin(BuiltinTy::Array)
                ) =>
            {
                shape
            }
            TypeRefData::Nullable(inner) => {
                let inner = self.with_array_replaced(inner, shape);
                self.builder.add_type_ref(TypeRefData::Nullable(inner))
            }
            TypeRefData::Union(operands) => {
                let operands = self.builder.type_ref_list(operands).to_vec();
                let replaced = operands
                    .iter()
                    .map(|&operand| self.with_array_replaced(operand, shape))
                    .collect::<Vec<_>>();
                if replaced == operands {
                    return ty;
                }
                let replaced = self.builder.alloc_type_ref_list(&replaced);
                self.builder.add_type_ref(TypeRefData::Union(replaced))
            }
            _ => ty,
        }
    }
}
