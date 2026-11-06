//! A property read through the method of the same name.
//!
//! Eloquent's relations are the case: `$post->author` is `$post->author()->getResults()`, because
//! `__get` finds the method, sees that it returns a relation, and loads it. A framework states the
//! rule; the method is the project's own declaration, so it says which class comes back and it is
//! where goto lands. Asked after the member table, as every accessor is: a declared property of the
//! same name is what PHP reads.

use phrost_model::{Name, TextSize};
use phrost_sema::{MemberKind, MemberProvenance};

use phrost_index::MethodProperty;

use crate::{
    location::SymbolLocation,
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

impl IndexView<'_> {
    /// What such a property holds: the backing method's result, then the rule's `through` on it.
    pub(crate) fn method_backed_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Held> {
        if let Some(rule) = self.backing_rule(class_fqn, kind, name) {
            return self.held_through(rule, class_fqn, name);
        }
        // `$post->comments_count`: what a query's `withCount('comments')` puts beside the relation,
        // and nothing until one did.
        self.counted_relation(class_fqn, kind, name)?;
        Some(Self::nullable(Held::Builtin(phrost_model::BuiltinTy::Int)))
    }

    /// The relation a property counts, where the rule names a suffix and the property is a relation's
    /// name in snake case with it: `comments_count` for `comments()`.
    ///
    /// The suffix first, which nearly no property has; and a column of the same name is the table's
    /// and not a count -- a denormalised counter is the one thing spelled like this on purpose.
    fn counted_relation(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<(&MethodProperty, Name)> {
        if kind != MemberKind::Property {
            return None;
        }
        let rules = self.skeleton.returns().method_properties();
        let (rule, stem) = rules.iter().find_map(|rule| {
            let stem = name.strip_suffix(rule.counted.as_deref()?)?;
            Some((rule, stem))
        })?;
        if stem.is_empty()
            || !self.is_or_below(class_fqn, &rule.class)
            || self.model_column(class_fqn, name).is_some()
        {
            return None;
        }
        let method = self
            .members(class_fqn)
            .members()
            .iter()
            .find(|member| {
                member.row.kind == MemberKind::Method
                    && super::accessors::snake(member.name.as_str()) == stem
            })?
            .name
            .clone();
        self.backing_value(rule, class_fqn, method.as_str())?;
        Some((rule, method))
    }

    /// What `rule` makes the property `name` on `class_fqn` hold, the rule already known to speak
    /// for the class.
    fn held_through(&self, rule: &MethodProperty, class_fqn: &str, name: &str) -> Option<Held> {
        let (backing, arguments) = self.backing_value(rule, class_fqn, name)?;
        let held = self.member_held_on(
            backing.as_str(),
            MemberKind::Method,
            &rule.through,
            None,
            &arguments,
        )?;
        Some(self.keyed_by_a_column(rule, class_fqn, name, &backing, held))
    }

    /// What a relation loads, with the null taken off where the key it reads cannot be empty: a
    /// column that cannot be null and is a foreign key the database enforces always points at a
    /// row. See the seam's `KeyedRelation`.
    ///
    /// Only where the migrations say both. A key no migration defines, or one named other than the
    /// convention, finds no column and keeps the null the relation declares.
    fn keyed_by_a_column(
        &self,
        rule: &MethodProperty,
        class_fqn: &str,
        name: &str,
        backing: &Name,
        held: Held,
    ) -> Held {
        let enforced = rule.keyed.iter().any(|(relation, suffix)| {
            self.is_or_below(backing.as_str(), relation)
                && self
                    .model_column(
                        class_fqn,
                        &format!("{}{suffix}", super::accessors::snake(name)),
                    )
                    .is_some_and(|column| !column.nullable && column.constrained)
        });
        match enforced {
            true => self.without_null(held.clone()).unwrap_or(held),
            false => held,
        }
    }

    /// Where such a property is declared, which is the method that backs it.
    pub(crate) fn method_backed_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        if let Some((_, method)) = self.counted_relation(class_fqn, kind, name) {
            return self.member_location(class_fqn, MemberKind::Method, method.as_str());
        }
        let rule = self.backing_rule(class_fqn, kind, name)?;
        self.backing_value(rule, class_fqn, name)?;
        self.member_location(class_fqn, MemberKind::Method, name)
    }

    /// The properties a list should offer on `class_fqn`: every method of the project's own whose
    /// result makes its name one.
    ///
    /// Only methods the class declares or takes from a trait or a parent the *project* wrote --
    /// never the framework class the rule names, whose own methods return builders and relations
    /// for reasons that are not properties. And only ones a caller could make without arguments,
    /// because that is how `__get` calls them.
    pub(crate) fn method_backed_properties_of(
        &self,
        class_fqn: &str,
    ) -> Vec<crate::UndeclaredProperty> {
        let rules = self.skeleton.returns().method_properties();
        let Some(rule) = rules
            .iter()
            .find(|rule| self.is_or_below(class_fqn, &rule.class))
        else {
            return Vec::new();
        };
        let table = self.members(class_fqn);
        let candidates = table
            .members()
            .iter()
            .filter(|member| {
                member.row.kind == MemberKind::Method
                    && member.row.params.iter().all(|param| param.has_default)
                    && match &member.provenance {
                        MemberProvenance::Own | MemberProvenance::Trait(_) => true,
                        MemberProvenance::Inherited(owner) => {
                            !self.is_or_above(owner.as_str(), &rule.class)
                        }
                        MemberProvenance::Interface(_) => false,
                    }
                    && self.hint_admits(member.row.ty, &rule.returns)
            })
            .map(|member| member.name.clone())
            .collect::<Vec<Name>>();
        let declarer = self.class_location(class_fqn).map(|location| location.fqn);
        let hir = self.current().map(|open| open.hir());
        let mut found = Vec::new();
        for name in candidates {
            // The rule found once above, rather than again for every method: the climb that
            // settles it is the same one each time.
            let Some((backing, arguments)) = self.backing_value(rule, class_fqn, name.as_str())
            else {
                continue;
            };
            // What it loads, where its relation says -- and a relation that does not say is still a
            // relation, with a count.
            if let Some(held) = self
                .member_held_on(
                    backing.as_str(),
                    MemberKind::Method,
                    &rule.through,
                    None,
                    &arguments,
                )
                .map(|held| self.keyed_by_a_column(rule, class_fqn, name.as_str(), &backing, held))
            {
                found.push(crate::UndeclaredProperty {
                    ty: hir
                        .and_then(|hir| {
                            self.render_held(
                                phrost_model::TextSize::default(),
                                hir,
                                &held,
                                crate::variables::Naming::Short,
                            )
                        })
                        .map(Into::into),
                    name: name.as_str().into(),
                    declarer: declarer.clone(),
                    keyed: false,
                });
            }
            // And the count a query's `withCount()` puts beside it.
            if let Some(suffix) = rule.counted.as_deref() {
                found.push(crate::UndeclaredProperty {
                    ty: Some("?int".into()),
                    name: format!("{}{suffix}", super::accessors::snake(name.as_str())).into(),
                    declarer: declarer.clone(),
                    keyed: false,
                });
            }
        }
        found
    }

    /// The rule's `through`, when one speaks for a property `name` on `class_fqn`.
    ///
    /// Cheapest first: a project with no such rule asks nothing, and a name with no method of the
    /// same name is settled by one lookup before any hierarchy is climbed.
    fn backing_rule(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<&MethodProperty> {
        if kind != MemberKind::Property {
            return None;
        }
        let rules = self.skeleton.returns().method_properties();
        if rules.is_empty() {
            return None;
        }
        self.members(class_fqn).find(MemberKind::Method, name)?;
        rules
            .iter()
            .find(|rule| self.is_or_below(class_fqn, &rule.class))
    }

    /// What the backing method returns on this receiver, when it is something the rule reads.
    fn backing_value(
        &self,
        rule: &MethodProperty,
        class_fqn: &str,
        name: &str,
    ) -> Option<(Name, Vec<Held>)> {
        let returned = self.member_held_on(class_fqn, MemberKind::Method, name, None, &[])?;
        // Whichever spelling the return arrived in -- built with its arguments, or the declared type
        // as it stands -- read the same way a call on it is.
        let hir = self.current()?.hir();
        let class = self.class_of_held(TextSize::default(), hir, &returned)?;
        let arguments = self.arguments_of_held(hir, &returned);
        self.is_or_below(class.as_str(), &rule.returns)
            .then_some((class, arguments))
    }

    /// Whether a method's native return hint lets it hand back a `returns` -- false only where the
    /// runtime would refuse to: a scalar, `void`, `array`, `self`, or a class that is not one.
    ///
    /// The cheap half of the question every method of a model is asked when its properties are
    /// listed, and the half PHP settles: a hint is enforced, so a method declaring `: string` returns
    /// no relation whatever its body says. Only what the hint leaves open -- no hint, `mixed`,
    /// `object`, a union -- is resolved, bodies and all.
    fn hint_admits(&self, hint: Option<phrost_ty::TypeId>, returns: &str) -> bool {
        let Some(hint) = hint else {
            return true;
        };
        let types = self.skeleton.types();
        let mut ty = types.get(hint);
        if let phrost_ty::TypeRef::Nullable(inner) = ty {
            ty = types.get(*inner);
        }
        if let phrost_ty::TypeRef::Generic { base, .. } = ty {
            ty = types.get(*base);
        }
        match ty {
            phrost_ty::TypeRef::Named(class) => self.is_or_below(class.as_str(), returns),
            phrost_ty::TypeRef::Builtin(builtin) => matches!(
                builtin,
                phrost_model::BuiltinTy::Mixed | phrost_model::BuiltinTy::Object
            ),
            // The model itself, which is no relation.
            phrost_ty::TypeRef::Relative(_) => false,
            _ => true,
        }
    }

    fn is_or_below(&self, class_fqn: &str, ancestor: &str) -> bool {
        class_fqn.eq_ignore_ascii_case(ancestor)
            || self.descends_from(class_fqn, ancestor, HIERARCHY_DEPTH)
    }

    fn is_or_above(&self, class_fqn: &str, descendant: &str) -> bool {
        self.is_or_below(descendant, class_fqn)
    }
}
