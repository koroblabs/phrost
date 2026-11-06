//! `LineIndex` and `LineCol` live in `lspkit-text`, shared with orthra: the byte-column API this
//! crate's call sites use, plus the UTF-8/16/32 column forms the protocol layers may want.

pub use lspkit_text::{LineCol, LineIndex};
