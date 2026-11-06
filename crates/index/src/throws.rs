//! What a declaration's `@throws` tags named, for the callers that cannot read its docblock.
//!
//! The index stores docblock *ranges* and not text -- every docblock of every vendor class resident
//! is tens of megabytes -- so a caller asking "what can this call throw" would be one disk read per
//! call site. Which is fine for a hover and impossible for a check: 34 000 calls on one real
//! project, judged in a 30 ms budget.
//!
//! So the answer is derived once, where the text is already in hand, and stored. A list per file
//! rather than a field per declaration, which is what `Lowerer::doc_throws` said it would need and
//! what `ScopedAttributeLiteral` is kept as for the same reason: 9 500 declarations of a quarter of
//! a million carry the tag, and a field would be two words on every row to say "none" 96 % of the
//! time.
//!
//! Keyed by the declaration and looked up by file, because a caller has both: resolving a call
//! answers with a `SymbolLocation`, which carries the file it is declared in and the class that
//! declares it.

use phrost_model::Name;

/// One declaration's `@throws` tags, resolved against the file that wrote them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ThrownRow {
    /// The declaring class, for a method. `None` for a free function.
    pub owner: Option<Name>,
    /// The method or function name, as declared.
    pub name: Name,
    /// The classes the tags named, fully qualified with no leading separator, in source order.
    ///
    /// Never empty -- a declaration with no tag has no row at all, which is what makes the table
    /// sparse.
    pub classes: Box<[Name]>,
}

impl lspkit_memsize::MemSize for ThrownRow {
    /// The class list's buffer. Every `Name` in it is a pointer into the interner, which counts
    /// those bytes itself.
    fn heap_size(&self) -> usize {
        lspkit_memsize::flat(&self.classes)
    }
}
