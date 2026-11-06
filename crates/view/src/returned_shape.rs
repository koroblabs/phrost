//! The keys an array a body returns actually holds.
//!
//! `public function getStoreSales(): ?array` says the value is an array and nothing about what is
//! in it, and on the reference project **866 of 2 088** declarations returning `array` or `?array`
//! hand back a keyed literal written out three lines below the signature. Everything a reader
//! wants -- which keys exist, what each holds -- is in that literal, and the generated `@return`
//! was throwing all of it away and writing the word the signature already said.
//!
//! # Why it is here and not where the body's return type is read
//!
//! [`phrost_lower`]'s `returned_from_body` answers the same question at ingest, syntactically, for
//! a declaration that names *no* return. It cannot answer this one: `'payment' => $this->pay()`
//! names a type only after resolution, and resolution is a whole index away from lowering. So this
//! is the resolved half, asked at the request rather than stored -- which is also what makes it
//! free. It runs when somebody asks for a docblock on one declaration, so the cost is a handful of
//! path resolutions on a gesture, and nothing is held.
//!
//! # All or nothing
//!
//! The rule `phrost_lower`'s module states, for the same reason: a body whose returns this cannot
//! read settles nothing rather than settling from the ones it can. `return ['a' => 1];` beside
//! `return $this->rows();` is not a shape, and writing the first would state a claim the code does
//! not make. So one unreadable return gives the whole declaration back to what it said before.

use phrost_hir::{
    BodyId, ExprId, FileHir,
    body::{ArrayElementKind, ExprData, LiteralKind},
};

use crate::{variables::Naming, view::FOLLOW_LIMIT, view::IndexView};

/// How many keys a generated shape may name.
///
/// Past this the tag is no longer something a person reads -- a framework's render array is a
/// literal with dozens of entries, and a `@return` spanning a screen is worse than the word
/// `array`. The declaration keeps what it said rather than being given a shape nobody can scan.
const MOST_KEYS: usize = 24;

impl IndexView<'_> {
    /// The shape every `return` of `body` agrees on, written as a docblock writes it.
    ///
    /// `None` unless every return hands back either `null` -- which the declaration's own `?`
    /// already carries -- or an array literal keyed entirely by string literals, and every such
    /// literal names the same keys. Two literals that disagree describe two shapes, and a caller
    /// told about one of them would be told something false about the other.
    ///
    /// Class names are qualified: the text is going into a file to be read anywhere, and this
    /// index reads it back.
    pub(crate) fn returned_shape(&self, hir: &FileHir, body: BodyId) -> Option<String> {
        // A `return` inside a generator hands its value to `Generator::getReturn()` rather than to
        // the caller, so it says nothing about what the declaration produces. The same exclusion
        // `incompatible_returns` makes, for the same reason.
        if hir.body(body).is_generator {
            return None;
        }

        let mut shape: Option<String> = None;
        for value in phrost_body::returned_values(hir, hir.body(body).statements) {
            // A bare `return;` hands back no value. PHP refuses one in a function declaring a type,
            // so this is a file that does not run -- and there is nothing to read either way.
            let value = value?;
            match hir.expr(value) {
                ExprData::Literal {
                    kind: LiteralKind::Null,
                    ..
                } => continue,
                ExprData::ArrayLiteral { elements, .. } => {
                    let written = self.shape_of_literal(hir, hir.expr_list(*elements))?;
                    match &shape {
                        // Two literals naming two shapes: see the module's all-or-nothing rule.
                        Some(first) if *first != written => return None,
                        Some(_) => {}
                        None => shape = Some(written),
                    }
                }
                _ => return None,
            }
        }
        shape
    }

    /// `array{date: string, file: ?\App\File}` for one literal, or nothing when it is not a record.
    ///
    /// Every element has to be a literal key with a value: a bare element means the array is a
    /// list and not a record, a computed key names a slot this layer cannot identify, and a spread
    /// contributes keys from somewhere else entirely. Each of the three is a reason the literal
    /// says less than a shape would claim.
    fn shape_of_literal(&self, hir: &FileHir, elements: &[ExprId]) -> Option<String> {
        if elements.is_empty() || elements.len() > MOST_KEYS {
            return None;
        }

        let mut fields = Vec::with_capacity(elements.len());
        for &element in elements {
            let ExprData::ArrayElement {
                kind: ArrayElementKind::LiteralKey { name, .. },
                value,
                ..
            } = hir.expr(element)
            else {
                return None;
            };
            fields.push(format!(
                "{}: {}",
                phrost_ty::shape_key(name.as_str()),
                self.element_type(hir, *value)
            ));
        }
        Some(format!("array{{{}}}", fields.join(", ")))
    }

    /// What one element of the literal holds, resolved where it is written.
    ///
    /// `mixed` where resolution has no answer, which is the same word `@param` writes for a
    /// parameter nothing declares a type for: the key is *there*, written by the author, and the
    /// only thing in doubt is what goes in it. Dropping the key instead would state a shape the
    /// code contradicts, and declining the whole tag would throw away the keys that did resolve --
    /// on real code most of them do.
    fn element_type(&self, hir: &FileHir, value: ExprId) -> String {
        let at = hir.expr(value).range().start();
        phrost_body::value_path_of(hir, value)
            .and_then(|path| self.resolve_path(at, hir, &path, FOLLOW_LIMIT))
            .and_then(|held| self.render_held(at, hir, &held, Naming::Qualified))
            .unwrap_or_else(|| "mixed".to_owned())
    }
}
