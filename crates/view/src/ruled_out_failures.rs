//! A method whose failure branch a literal the caller wrote out rules out.
//!
//! The sibling of [`crate::literal_returns`], which says the same thing about a free function, and
//! here for the same reason: `DOMDocument::createElement()` is declared `DOMElement|false` and the
//! `false` is what it answers for a tag name that is not a valid XML name. When the name is written
//! at the call site -- `$doc->createElement('p')` -- there is nothing left to be uncertain about.
//!
//! Measured on Symfony after the stub set's lost `@return` tags were recovered: of 236 reports about
//! a `DOMElement|false` receiver, **166 are a `createElement` with a literal tag** and cannot
//! happen. The rest name a variable and stay, which is the whole point -- `createElement($tag)` on a
//! tag read from configuration really can fail.
//!
//! # Only the failure operand, and only for a recognised literal
//!
//! It removes `false` from what the declaration said and nothing else. A literal the recogniser does
//! not accept keeps the union, so a genuine `createElement('not a name')` is still reported -- the
//! recogniser refuses rather than approximates, which is the discipline its free-function sibling
//! states and the reason both are safe. The worst either can do is miss a finding.

use phrost_model::BuiltinTy;

use crate::{resolve::Held, view::IndexView};

/// What a literal has to look like for the failure branch to be impossible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LiteralShape {
    /// A name XML accepts: a letter or `_` first, then letters, digits, `-`, `.` and `_`, with at
    /// most one `:` separating a prefix. Deliberately narrower than the specification, which allows
    /// most of Unicode -- what is recognised is what real code writes.
    XmlName,
}

/// One method, the argument that decides, and the shape that settles it.
struct RuledOut {
    class: &'static str,
    method: &'static str,
    argument: usize,
    shape: LiteralShape,
}

/// The methods whose `false` a well-formed literal rules out.
///
/// All four are DOM's own, and all four fail for the same reason: the name is not one XML allows.
/// `createTextNode`, `createComment` and `createDocumentFragment` are absent because they cannot
/// fail at all and their declarations already say so.
static RULED_OUT: &[RuledOut] = &[
    RuledOut {
        class: "DOMDocument",
        method: "createElement",
        argument: 0,
        shape: LiteralShape::XmlName,
    },
    RuledOut {
        class: "DOMDocument",
        method: "createElementNS",
        argument: 1,
        shape: LiteralShape::XmlName,
    },
    RuledOut {
        class: "DOMDocument",
        method: "createAttribute",
        argument: 0,
        shape: LiteralShape::XmlName,
    },
    RuledOut {
        class: "DOMDocument",
        method: "createAttributeNS",
        argument: 1,
        shape: LiteralShape::XmlName,
    },
];

impl IndexView<'_> {
    /// `held` with the failure operand taken off, when a literal ruled it out.
    ///
    /// The same value back for every other call. The method name is compared first, so a project
    /// that never writes one of these pays one string comparison per member step.
    pub(crate) fn without_a_ruled_out_failure(
        &self,
        class: &str,
        method: &str,
        literals: &[phrost_hir::body::LiteralArgument],
        held: Held,
    ) -> Held {
        let Some(entry) = RULED_OUT.iter().find(|entry| {
            method.eq_ignore_ascii_case(entry.method) && class.eq_ignore_ascii_case(entry.class)
        }) else {
            return held;
        };
        let Some(written) = literals
            .iter()
            .find(|literal| usize::from(literal.position) == entry.argument)
        else {
            return held;
        };
        // A keyword is `null`, `true` or `false` and an interpolated string is a shape rather than
        // a name -- none of them a tag `createElement` could be given.
        if !written.wrote_its_value() {
            return held;
        }
        let settled = match entry.shape {
            LiteralShape::XmlName => is_an_xml_name(written.value.as_str()),
        };
        match settled {
            true => self.without_false(held.clone()).unwrap_or(held),
            false => held,
        }
    }

    /// `held` with the `false` operand removed, or nothing when `false` was the whole of it.
    ///
    /// Written here rather than beside `without_null`, whose shapes are the ones a *guard* produces:
    /// what a stub's `@return DOMElement|false` becomes is an interned union, and that reader only
    /// ever looked through the `?` wrapper the type table lifts a null into. Two sentinels reached
    /// by two readers, and this is the second one's half.
    fn without_false(&self, held: Held) -> Option<Held> {
        Some(match held {
            Held::Builtin(BuiltinTy::False) => return None,
            Held::OneOf(operands) => {
                let rest = operands
                    .into_vec()
                    .into_iter()
                    .filter_map(|held| self.without_false(held))
                    .collect::<Vec<_>>();
                return match rest.is_empty() {
                    true => None,
                    false => Some(Self::one_of(rest)),
                };
            }
            Held::Stored { ty, scope } => match self.skeleton.types().get(ty) {
                phrost_ty::TypeRef::Union(operands) => {
                    let types = self.skeleton.types();
                    let rest = types
                        .operands(*operands)
                        .iter()
                        .filter(|operand| {
                            !matches!(
                                types.get(**operand),
                                phrost_ty::TypeRef::Builtin(BuiltinTy::False)
                            )
                        })
                        .map(|operand| Held::Stored {
                            ty: *operand,
                            scope: scope.clone(),
                        })
                        .collect::<Vec<_>>();
                    return match rest.is_empty() {
                        true => None,
                        false => Some(Self::one_of(rest)),
                    };
                }
                _ => Held::Stored { ty, scope },
            },
            other => other,
        })
    }
}

/// Whether XML certainly accepts this as a name.
///
/// A recogniser and not a parser, the same discipline `literal_returns` states: it says yes only to
/// shapes whose meaning is not in doubt, and no to everything else including names XML would in
/// fact accept. Being wrong in that direction costs a finding nobody gets; being wrong in the other
/// would remove a report from code that really can fail.
fn is_an_xml_name(written: &str) -> bool {
    // A qualified name is a prefix and a local name, each a name in its own right.
    let mut parts = written.split(':');
    let (Some(first), second, None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [Some(first), second]
        .into_iter()
        .flatten()
        .all(is_a_name_part)
}

fn is_a_name_part(text: &str) -> bool {
    let mut bytes = text.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != b'_' {
        return false;
    }
    bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::is_an_xml_name;

    #[test]
    fn the_recogniser_takes_what_real_code_writes_and_refuses_the_rest() {
        for name in ["p", "div", "xsl:template", "my-tag", "ns_1.2", "_x"] {
            assert!(is_an_xml_name(name), "{name} is a name XML accepts");
        }
        // A digit first, a space, an empty prefix, two colons, and the empty string -- every one of
        // which XML refuses, so the failure branch stays.
        for name in ["1p", "a tag", ":p", "a:b:c", "", "p!"] {
            assert!(!is_an_xml_name(name), "{name} is not one");
        }
    }
}
