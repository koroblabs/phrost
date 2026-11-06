//! A property no declaration writes.
//!
//! `__get` makes every name legal, which is why a class with one cannot be reported on -- and also
//! why nothing could say what reading any of those names gives back. A framework can: it knows which
//! of its classes have an accessor, which names the accessor really answers for, and what comes out.
//!
//! Asked where PHP asks it. The accessor runs exactly when no accessible declared property answered,
//! so the member table is consulted first and this second -- the language's own order, and not a
//! second attempt at the first question. The two read different sources and at most one of them can
//! be right about any given name.

use phrost_index::{PropertyBinding, PropertyNames};
use phrost_sema::MemberKind;

use crate::{
    location::SymbolLocation,
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

impl<'a> IndexView<'a> {
    /// What such a property holds, when a framework said the receiver answers for this name.
    ///
    /// Resolved through [`IndexView::class_location`] rather than handed back as a bare name, for
    /// the reason a bound argument is: a [`Held`] for a class the index never saw would report every
    /// member reached through it as missing.
    pub(crate) fn magic_property_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Held> {
        let binding = self.magic_property(class_fqn, kind, name)?;
        let held = Held::Class(self.class_location(&binding.holds)?.fqn);
        Some(match binding.nullable {
            true => Self::nullable(held),
            false => held,
        })
    }

    /// What PHP's own accessor hands back for a name no declaration answers, when the accessor says.
    ///
    /// `__get(string $k): Foo` is a claim about every property the class does not declare, and
    /// `__call(...): Foo` about every method -- which is exactly what PHP runs for one. Asked after
    /// the member table and after a framework's binding, the language's order: a declared member
    /// or a tag answers first, and a framework knows more per name than the accessor's one type.
    ///
    /// A method reaches `__call` first and `__callStatic` second, because a lookup here does not
    /// know which arrow the call was written with, and when a class declares both the instance
    /// accessor is the one that also answers `parent::` and a call from inside the class.
    pub(crate) fn magic_accessor_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Held> {
        let accessors: &[&str] = match kind {
            MemberKind::Property => &["__get"],
            MemberKind::Method => &["__call", "__callStatic"],
            MemberKind::ClassConstant | MemberKind::EnumCase => return None,
        };
        let table = self.members(class_fqn);
        // Found before it is asked, so an accessor that is itself missing ends here rather than
        // coming back through the lookup that sent it.
        let accessor = accessors.iter().find_map(|accessor| {
            (*accessor != name)
                .then(|| table.find(MemberKind::Method, accessor))
                .flatten()
        })?;
        // Only what the accessor *declares*, natively or in a docblock. Its body is a dispatcher --
        // AWS's `__call` returns a `Result` down one branch and a `Promise` down the other, and
        // Laravel's hands back `$this` for a `with*` prefix and throws for the rest -- so what a
        // body happens to return is a claim about one name and not about every name. A class that
        // means its accessor to answer for all of them says so in the signature.
        if accessor.row.doc_ty_from_body
            || accessor.row.effective_ty(self.skeleton.types()).is_none()
        {
            return None;
        }
        // `if (method_exists($this, 'headers')) { $this->headers(); }` is how PHP says a method is
        // declared on a subclass, and then that declaration runs rather than the accessor --
        // Laravel's `Mailable` writes exactly this beside a `__call` that returns `$this`.
        if self
            .existence_tests()
            .is_some_and(|tests| tests.tests_member(name))
        {
            return None;
        }
        let accessor = accessor.name.clone();
        self.member_held_on(class_fqn, MemberKind::Method, accessor.as_str(), None, &[])
    }

    /// Where to go from such a property, which is where the class it holds is declared.
    ///
    /// There is nowhere else. The property is declared in no PHP at all -- a Drupal field's only
    /// declaration is a block of YAML, and the accessor that answers for it names nothing -- so the
    /// class it hands back is the one thing at that position with a definition to read. The same
    /// answer a catalog key gets, and for the same reason.
    pub(crate) fn magic_property_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        let binding = self.magic_property(class_fqn, kind, name)?;
        self.class_location(&binding.holds)
    }

    /// The binding that speaks for this name on this receiver.
    ///
    /// Three gates, cheapest first, which is the ordering the bound-argument lookup measured. A
    /// project with no framework has nothing to scan at all; then the name has to match, which is a
    /// comparison against a handful of short strings or a single hash; and only then is the
    /// receiver's hierarchy walked, because that climb is the expensive half and asking it first
    /// would charge it to every property access in the project.
    /// The names a binding says the receiver's accessor answers, for a list being built.
    ///
    /// A name a binding writes, and every key of a catalog a binding answers for -- those last at
    /// most `limit` of them and matched against `typed` the way a catalog key is, because on an
    /// entity they are every field the project has.
    pub(crate) fn bound_properties_of(
        &self,
        class_fqn: &str,
        typed: &str,
        limit: usize,
    ) -> Vec<crate::UndeclaredProperty> {
        let contracts = self.skeleton.returns();
        let mut found: Vec<crate::UndeclaredProperty> = Vec::new();
        for binding in contracts.magic_properties() {
            if !class_fqn.eq_ignore_ascii_case(&binding.class)
                && !self.descends_from(class_fqn, &binding.class, HIERARCHY_DEPTH)
            {
                continue;
            }
            // What the read gives back, as a list writes a type: the class's own name, and a `?`
            // where the binding says the accessor may find nothing.
            let holds = phrost_model::short_name(&binding.holds);
            let ty: Box<str> = match binding.nullable {
                true => format!("?{holds}").into(),
                false => holds.into(),
            };
            let (names, keyed) = match &binding.name {
                PropertyNames::Exactly(one) => (vec![&**one], false),
                PropertyNames::KeyOf(catalog) => {
                    (contracts.catalog_keys(catalog, typed, limit), true)
                }
            };
            for name in names {
                if !found.iter().any(|known| &*known.name == name) {
                    found.push(crate::UndeclaredProperty {
                        name: name.into(),
                        ty: Some(ty.clone()),
                        declarer: None,
                        keyed,
                    });
                }
            }
        }
        found
    }

    fn magic_property(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<&'a PropertyBinding> {
        if kind != MemberKind::Property {
            return None;
        }
        let contracts = self.skeleton.returns();
        contracts.magic_properties().iter().find(|binding| {
            let named = match &binding.name {
                // Exactly, because PHP compares a property name exactly -- unlike a method name,
                // and unlike the class names either side of this.
                PropertyNames::Exactly(one) => &**one == name,
                PropertyNames::KeyOf(catalog) => contracts.catalog_holds(catalog, name),
            };
            named
                && (class_fqn.eq_ignore_ascii_case(&binding.class)
                    || self.descends_from(class_fqn, &binding.class, HIERARCHY_DEPTH))
        })
    }
}
