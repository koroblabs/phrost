//! A method or function a string literal names as a callable.
//!
//! PHP's callables are written three ways a cursor could not follow: `[$formatter, 'format']`,
//! `'App\Repo::find'`, and `'App\render'`. The lowering already reads `[$this, 'm']` and
//! `[Foo::class, 'm']` as a member reference; a *variable* beside the string is left an array
//! there, because `[$id, 'asc']` is the same shape holding data and typing it as a callable would
//! change what every reader of the value sees.
//!
//! Here the question is only where a cursor lands, so resolution can settle it without touching the
//! value: the pair names a method exactly when the variable's class declares one by that name, and a
//! string names a function or a static method exactly when the index holds it. An int has no method
//! called `asc`, and a message is not spelled `App\Repo::find`.

use phrost_hir::body::ExprData;
use phrost_model::{TextRange, TextSize};
use phrost_sema::MemberKind;

use crate::{CursorTarget, view::IndexView};

impl IndexView<'_> {
    /// The method or function a string under the cursor names as a callable.
    pub fn callable_named_by_a_string_at(&self, offset: TextSize) -> Option<CursorTarget> {
        let open = self.current()?;
        let quoted = open.hir().string_at(offset)?;
        let inside = self.inside_the_quotes(quoted)?;
        let written = open
            .text()
            .get(usize::from(inside.start())..usize::from(inside.end()))?;
        if written.is_empty() || written.contains('$') {
            return None;
        }
        self.method_of_a_pair(quoted, inside, written)
            .or_else(|| self.static_method_named(inside, written))
            .or_else(|| self.function_named(inside, written))
    }

    /// `[$formatter, 'format']`: the second element of a two-element array whose first is a
    /// variable holding an object that declares the method.
    fn method_of_a_pair(
        &self,
        quoted: TextRange,
        inside: TextRange,
        written: &str,
    ) -> Option<CursorTarget> {
        if !is_a_method_name(written) {
            return None;
        }
        let hir = self.current()?.hir();
        let first = hir.expressions().find_map(|(_, data)| {
            let ExprData::ArrayLiteral { elements, .. } = data else {
                return None;
            };
            let [first, second] = hir.expr_list(*elements) else {
                return None;
            };
            (hir.expr(*second).range() == quoted
                && matches!(hir.expr(*first), ExprData::Variable { .. }))
            .then_some(*first)
        })?;
        let at = hir.expr(first).range().start();
        let path = phrost_body::value_path_of(hir, first)?;
        let held = self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT)?;
        let owner = self.owner_of_member(at, hir, &held, MemberKind::Method, written)?;
        self.method_target(owner, written, inside)
    }

    /// Every `[$variable, 'method']` in the file, as the access it is when the variable's class
    /// declares the method.
    ///
    /// Presumed, the way `[Foo::class, 'bar']` is, and for the reason the module gives: the same
    /// shape holds data, and what tells the two apart is whether the class has the method -- so a
    /// reader keeps the site only where it resolves. Without it a search or a rename from the method
    /// left the pair behind, while goto from the pair reached the method: `[$command, 'handle']`
    /// handed to a mock's `with()` on Laravel was the case that showed it.
    pub(crate) fn pair_sites(&self, hir: &phrost_hir::FileHir) -> Vec<phrost_body::MemberSite> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let text = open.text();
        hir.expressions()
            .filter_map(|(_, data)| {
                let ExprData::ArrayLiteral { elements, .. } = data else {
                    return None;
                };
                let [first, second] = hir.expr_list(*elements) else {
                    return None;
                };
                let ExprData::Variable {
                    range: receiver_range,
                    ..
                } = hir.expr(*first)
                else {
                    return None;
                };
                let ExprData::Literal {
                    kind: phrost_hir::body::LiteralKind::String,
                    range,
                } = hir.expr(*second)
                else {
                    return None;
                };
                let inside = self.inside_the_quotes(*range)?;
                let written = text.get(usize::from(inside.start())..usize::from(inside.end()))?;
                if !is_a_method_name(written) {
                    return None;
                }
                Some(phrost_body::MemberSite {
                    // Read rather than interned, as a double's string is: a name no class declares
                    // is one no receiver has, which is the site a presumed access drops anyway.
                    name: self.skeleton().interner().lookup_name(written)?,
                    receiver: phrost_body::value_path_of(hir, *first)?,
                    kind: phrost_body::SiteKind::Method,
                    name_range: inside,
                    receiver_range: *receiver_range,
                    nullsafe: false,
                    presumed: true,
                    named: true,
                })
            })
            .collect()
    }

    /// Every string in the current file written `'Class::name'`, with the range of the name and the
    /// class it names -- the whole-file counterpart of the cursor's reading, for a search or a rename
    /// that has to list it.
    ///
    /// Only the spelling is matched here, and the class looked up: which declaration it reaches is
    /// the caller's to decide, since the method may be inherited. A name the class does not have is
    /// then simply not a use of anything -- a deprecation message saying `'Old::gone'` stays prose.
    pub fn static_method_strings(&self, name: &str) -> Vec<(TextRange, phrost_model::Name)> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let text = open.text();
        open.hir()
            .string_literals()
            .filter_map(|quoted| self.inside_the_quotes(quoted))
            .filter_map(|inside| {
                let written = text.get(usize::from(inside.start())..usize::from(inside.end()))?;
                let (class, method) = written.split_once("::")?;
                if class.is_empty() || !method.eq_ignore_ascii_case(name) {
                    return None;
                }
                let owner = self.class_location(class.trim_start_matches('\\'))?.fqn;
                let range = TextRange::new(
                    inside.end() - TextSize::try_from(method.len()).ok()?,
                    inside.end(),
                );
                Some((range, owner))
            })
            .collect()
    }

    /// What calling the method a `[$object, 'method']` literal names gives back, when the object's
    /// class declares it.
    ///
    /// The one reading of the pair as a call, for the two places one is made: handed to a parameter
    /// that invokes it -- `call_user_func([$repo, 'find'])` -- and invoked where a variable holds
    /// it, `$find = [$repo, 'find']; $find()`. Both had been one rule written twice waiting to
    /// happen, and the second was simply missing.
    ///
    /// The literal stays an array everywhere else, for the reason the module gives. Being *called*
    /// is what settles it, and then the class of the variable either declares the method or the
    /// pair names nothing -- so `[$id, 'asc']` called is a call of nothing, as it is at run time.
    pub(crate) fn method_a_pair_returns(
        &self,
        hir: &phrost_hir::FileHir,
        literal: phrost_hir::ExprId,
        budget: u8,
    ) -> Option<crate::resolve::Held> {
        let (object, method) = self.pair_written(hir, literal)?;
        let offset = hir.expr(object).range().start();
        let path = phrost_body::value_path_of(hir, object)?;
        let held = self.resolve_path(offset, hir, &path, budget.checked_sub(1)?)?;
        self.member_through(offset, hir, &held, MemberKind::Method, method, None)
    }

    /// What invoking a value gives back where the value is a `[$object, 'method']` literal a local
    /// was assigned -- read off the path, because the type the literal resolves to keeps the
    /// variable's class and not the method name, which is a `string` like any other.
    ///
    /// Followed a binding at a time, as a `Foo::class` is for `new $class`, and through a guard,
    /// which says nothing about which method the pair names. Not past an element written into the
    /// local since: `$pair[1] = 'other'` names another method, and the binding still reads as the
    /// literal.
    pub(crate) fn invoked_pair(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        path: &phrost_body::ValuePath,
        budget: u8,
    ) -> Option<crate::resolve::Held> {
        let budget = budget.checked_sub(1)?;
        if !path.steps.is_empty() {
            return None;
        }
        match &path.root {
            phrost_body::PathRoot::ArrayLiteral(literal) => {
                self.method_a_pair_returns(hir, *literal, budget)
            }
            phrost_body::PathRoot::Local(name) => {
                let never =
                    |hir: &phrost_hir::FileHir, expr| self.call_never_returns(offset, hir, expr);
                let bound = phrost_body::local_binding(
                    hir,
                    offset,
                    name.as_str(),
                    &never,
                    &self.asserting(),
                )?;
                let at = bound.written_at.unwrap_or(offset);
                if bound.written_at.is_some_and(|since| {
                    !phrost_body::slots_filled_since(hir, offset, name.as_str(), since, &never)
                        .is_empty()
                }) {
                    return None;
                }
                self.invoked_pair(at, hir, &bound.value, budget)
            }
            root => {
                let (of, at) = root.narrows()?;
                self.invoked_pair(at, hir, of, budget)
            }
        }
    }

    /// The variable and the method name of a `[$object, 'method']` literal, as written.
    fn pair_written<'t>(
        &'t self,
        hir: &phrost_hir::FileHir,
        literal: phrost_hir::ExprId,
    ) -> Option<(phrost_hir::ExprId, &'t str)> {
        let ExprData::ArrayLiteral { elements, .. } = hir.expr(literal) else {
            return None;
        };
        let [object, name] = hir.expr_list(*elements) else {
            return None;
        };
        if !matches!(hir.expr(*object), ExprData::Variable { .. }) {
            return None;
        }
        let ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::String,
            range,
        } = hir.expr(*name)
        else {
            return None;
        };
        let inside = self.inside_the_quotes(*range)?;
        let written = self
            .current()?
            .text()
            .get(usize::from(inside.start())..usize::from(inside.end()))?;
        is_a_method_name(written).then_some((*object, written))
    }

    /// `'App\Repo::find'`: a class the index holds and a method it declares.
    fn static_method_named(&self, inside: TextRange, written: &str) -> Option<CursorTarget> {
        let (class, method) = written.split_once("::")?;
        if !is_a_method_name(method) {
            return None;
        }
        let owner = self.class_location(class.trim_start_matches('\\'))?.fqn;
        let method_range = TextRange::new(
            inside.end() - TextSize::try_from(method.len()).ok()?,
            inside.end(),
        );
        self.method_target(owner, method, method_range)
    }

    /// `'App\render'`: a function the index holds, written qualified.
    ///
    /// Qualified only. A bare word is the commonest thing a string holds, and `'count'` or `'trim'`
    /// in a list of labels is not a callable because the runtime happens to have a function by
    /// that name; a namespace separator is what nobody writes by accident.
    fn function_named(&self, inside: TextRange, written: &str) -> Option<CursorTarget> {
        let path = written.trim_start_matches('\\');
        if !path.contains('\\') {
            return None;
        }
        let located = self.function_location(path)?;
        Some(CursorTarget::WrittenName {
            path: located.fqn,
            anchor: phrost_hir::NameAnchor::FullyQualified,
            kind: phrost_hir::ImportKind::Function,
            range: inside,
        })
    }

    /// The method as the member table spells it, when `owner` has it.
    fn method_target(
        &self,
        owner: phrost_model::Name,
        name: &str,
        range: TextRange,
    ) -> Option<CursorTarget> {
        let table = self.members(owner.as_str());
        let name = table.find(MemberKind::Method, name)?.name.clone();
        Some(CursorTarget::MemberAccess {
            owner,
            name,
            kind: MemberKind::Method,
            range,
        })
    }
}

/// A PHP identifier, which is all a method name can be.
fn is_a_method_name(written: &str) -> bool {
    !written.is_empty()
        && !written.as_bytes()[0].is_ascii_digit()
        && written
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
