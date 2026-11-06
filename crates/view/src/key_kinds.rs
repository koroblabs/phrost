//! A literal key no key of its container can be.
//!
//! An `array<string, int>` holds no key `0`: PHP turns every numeric string key into an integer, so
//! a string-keyed array is keyed by strings that are not numbers, and `$map[0]` -- or `[$a, $b] =
//! $map`, which reads `0` and `1` -- is an undefined key on every run. The mirror holds for an
//! `array<int, V>` or a `list<V>` read at `'name'`. Only what the declaration states outright: a
//! `V[]` says nothing about its keys, nor does a shape, which lists the ones it has.

use phrost_hir::{
    FileHir, TypeRefData,
    body::{ArrayElementKind, ExprData},
};
use phrost_model::{BuiltinTy, TextRange};
use phrost_ty::TypeRef;

use crate::{IndexView, resolve::Held, variables::Naming, view::FOLLOW_LIMIT};

/// One read of a key the container's declared keys exclude.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImpossibleKey {
    /// The read, or the destructuring element that makes it.
    pub range: TextRange,
    /// The key as PHP stores it: an integer bare, a string quoted.
    pub key: String,
    /// The container's type, rendered.
    pub container: String,
}

/// Which keys a declaration says its array has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyKind {
    Int,
    String,
}

impl KeyKind {
    /// Whether a literal key is one no key of this kind can be.
    fn refuses(self, key: &str) -> bool {
        match self {
            Self::String => is_integer_key(key),
            // A float key is truncated to an integer, so only a string that is no number at all is
            // out of an int-keyed array.
            Self::Int => !is_integer_key(key) && key.parse::<f64>().is_err(),
        }
    }
}

/// PHP's rule for a key it stores as an integer: a decimal with no leading zero, sign allowed.
fn is_integer_key(key: &str) -> bool {
    let digits = key.strip_prefix('-').unwrap_or(key);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && !(key.starts_with('-') && digits == "0")
}

impl IndexView<'_> {
    /// Every literal key read in the current file that its container's declared keys exclude.
    pub fn impossible_keys(&self) -> Vec<ImpossibleKey> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for site in phrost_body::keyed_subscript_sites(hir) {
            let Some(key) = site.key.as_ref() else {
                continue;
            };
            let at = site.range.start();
            let Some(held) = self.resolve_path(at, hir, &site.base, FOLLOW_LIMIT) else {
                continue;
            };
            // A guard proved the array empty -- `if ($rows === [])`, `if (empty($rows))` -- and an
            // empty array has no key at all.
            if (matches!(&held, Held::Record(fields) if fields.is_empty())
                && self.guarded_empty_where_read(hir, at, &site.base))
                || self
                    .declared_key_kind(hir, &held)
                    .is_some_and(|kind| kind.refuses(key.as_str()))
            {
                out.extend(self.impossible(hir, site.range, key.as_str(), &held));
            }
        }

        for (_, expr) in hir.expressions() {
            let ExprData::Assign { target, value, .. } = expr else {
                continue;
            };
            let ExprData::ArrayLiteral { elements, .. } = hir.expr(*target) else {
                continue;
            };
            let Some(path) = phrost_body::value_path_of(hir, *value) else {
                continue;
            };
            let at = hir.expr(*value).range().start();
            let Some(held) = self.resolve_path(at, hir, &path, FOLLOW_LIMIT) else {
                continue;
            };
            let Some(kind) = self.declared_key_kind(hir, &held) else {
                continue;
            };
            let mut next = 0_i64;
            for &element in hir.expr_list(*elements) {
                let (key, range) = match hir.expr(element) {
                    ExprData::ArrayElement {
                        kind: ArrayElementKind::LiteralKey { name, .. },
                        range,
                        ..
                    } => (name.as_str().to_owned(), *range),
                    // A computed key or a spread says nothing this can number past.
                    ExprData::ArrayElement { .. } => break,
                    other => {
                        let key = next.to_string();
                        next += 1;
                        (key, other.range())
                    }
                };
                if kind.refuses(&key) {
                    out.extend(self.impossible(hir, range, &key, &held));
                }
            }
        }

        out.sort_by_key(|found| found.range.start());
        out
    }

    /// Whether an empty array was proved by a guard on the very local being read.
    ///
    /// The type alone is not enough, because a call is read as a slot: `$ret = $this->find(); if
    /// (!$ret) { $again = $this->find(); }` gives the second call the first one's guard, which is
    /// the right reading for a getter and the wrong one to report a missing key on. So the empty
    /// array has to come from the local's own binding being a guard about that local -- through
    /// any guards stacked on it, never through a value that was assigned.
    fn guarded_empty_where_read(
        &self,
        hir: &FileHir,
        at: phrost_model::TextSize,
        base: &phrost_body::ValuePath,
    ) -> bool {
        let (phrost_body::PathRoot::Local(name), []) = (&base.root, base.steps.as_slice()) else {
            return false;
        };
        let Some(bound) = phrost_body::local_binding(
            hir,
            at,
            name.as_str(),
            &|hir, expr| self.call_never_returns(at, hir, expr),
            &self.asserting(),
        ) else {
            return false;
        };
        if bound.kind != phrost_body::BindingKind::Narrowing {
            return false;
        }
        let mut root = &bound.value.root;
        while let Some((of, _)) = root.narrows() {
            if !of.steps.is_empty() {
                return false;
            }
            root = &of.root;
        }
        matches!(root, phrost_body::PathRoot::Local(local) if local == name)
    }

    fn impossible(
        &self,
        hir: &FileHir,
        range: TextRange,
        key: &str,
        held: &Held,
    ) -> Option<ImpossibleKey> {
        Some(ImpossibleKey {
            range,
            key: if is_integer_key(key) {
                key.to_owned()
            } else {
                format!("'{key}'")
            },
            container: self.render_held(range.start(), hir, held, Naming::Short)?,
        })
    }

    /// The keys a value's declared type states, when it states them as one scalar kind.
    pub(crate) fn declared_key_kind(&self, hir: &FileHir, held: &Held) -> Option<KeyKind> {
        let of_builtin = |builtin: BuiltinTy| match builtin {
            BuiltinTy::Int => Some(KeyKind::Int),
            string if string.is_string() => Some(KeyKind::String),
            _ => None,
        };
        match held {
            Held::Stored { ty, .. } => {
                let types = self.skeleton.types();
                let ty = match types.get(*ty) {
                    TypeRef::Nullable(inner) => *inner,
                    _ => *ty,
                };
                let TypeRef::Generic { base, args, .. } = types.get(ty) else {
                    return None;
                };
                let (TypeRef::Builtin(BuiltinTy::Array), [key, _]) =
                    (types.get(*base), types.operands(*args))
                else {
                    return None;
                };
                match types.get(*key) {
                    TypeRef::Builtin(builtin) => of_builtin(*builtin),
                    _ => None,
                }
            }
            Held::Written(id) => {
                let TypeRefData::Generic { base, args, .. } = hir.type_ref(*id) else {
                    return None;
                };
                let (TypeRefData::Builtin(BuiltinTy::Array), [key, _]) =
                    (hir.type_ref(*base), hir.type_ref_list(*args))
                else {
                    return None;
                };
                match hir.type_ref(*key) {
                    TypeRefData::Builtin(builtin) => of_builtin(*builtin),
                    _ => None,
                }
            }
            Held::MapOf { key, .. } => match key.as_ref() {
                Held::Builtin(builtin) => of_builtin(*builtin),
                _ => None,
            },
            _ => None,
        }
    }
}
