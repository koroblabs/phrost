//! A property read through the accessor method its name spells.
//!
//! Eloquent's two spellings: `getExcerptAttribute()` is `$post->excerpt`, and so is `excerpt():
//! Attribute`. `__get` studly-cases the name between a prefix and a suffix for the first, and
//! camel-cases it for the second, where the method has to return the framework's attribute class. A
//! framework states the correspondence; the method is the project's own declaration, so it is where
//! goto lands and what says what comes back. Asked after the member table, as every accessor is.

use phrost_model::Name;
use phrost_sema::{MemberKind, MemberProvenance};

use crate::{
    location::SymbolLocation,
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

/// Which of the two spellings answered, and the method's own name.
enum Accessor {
    /// `getExcerptAttribute()`, which returns what the property holds.
    Method(Name),
    /// `excerpt(): Attribute`, whose first type argument -- where a docblock writes one -- is what
    /// the property holds.
    Attribute(Name),
}

impl IndexView<'_> {
    /// What such a property holds.
    pub(crate) fn accessor_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Held> {
        match self.accessor_of(class_fqn, kind, name)? {
            Accessor::Method(method) => {
                self.member_held(class_fqn, MemberKind::Method, method.as_str())
            }
            // A docblock's `Attribute<T, ...>` first, as a declaration outranks a body; then the
            // getter closure the body hands over, which ingest read where the body still was.
            Accessor::Attribute(method) => {
                let returned = self.member_held(class_fqn, MemberKind::Method, method.as_str())?;
                let hir = self.current()?.hir();
                self.arguments_of_held(hir, &returned)
                    .into_iter()
                    .next()
                    .or_else(|| self.getter_of(class_fqn, method.as_str()))
            }
        }
    }

    /// Where such a property is declared, which is the accessor.
    pub(crate) fn accessor_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        let (Accessor::Method(method) | Accessor::Attribute(method)) =
            self.accessor_of(class_fqn, kind, name)?;
        self.member_location(class_fqn, MemberKind::Method, method.as_str())
    }

    /// The properties a list should offer on `class_fqn`: one per accessor the project wrote, in
    /// either spelling, by the snake-cased name `__get` is usually asked for.
    pub(crate) fn accessor_properties_of(&self, class_fqn: &str) -> Vec<crate::UndeclaredProperty> {
        let rules = self.skeleton.returns().accessors();
        let Some(rule) = rules
            .iter()
            .find(|rule| self.is_at_or_under(class_fqn, &rule.class))
        else {
            return Vec::new();
        };
        let declarer = self.class_location(class_fqn).map(|location| location.fqn);
        let hir = self.current().map(|open| open.hir());
        let mut out: Vec<crate::UndeclaredProperty> = Vec::new();
        for member in self.members(class_fqn).members() {
            if member.row.kind != MemberKind::Method
                || matches!(&member.provenance, MemberProvenance::Inherited(owner)
                    if self.is_at_or_under(&rule.class, owner.as_str()))
            {
                continue;
            }
            let method = member.name.as_str();
            let property = match spelled_between(method, &rule.prefix, &rule.suffix) {
                Some(studly) => snake(studly),
                None if self.returns_attribute(class_fqn, method, &rule.attribute) => snake(method),
                None => continue,
            };
            if property.is_empty() || out.iter().any(|known| *known.name == *property) {
                continue;
            }
            let ty = self
                .accessor_held(class_fqn, MemberKind::Property, &property)
                .zip(hir)
                .and_then(|(held, hir)| {
                    self.render_held(
                        phrost_model::TextSize::default(),
                        hir,
                        &held,
                        crate::variables::Naming::Short,
                    )
                });
            out.push(crate::UndeclaredProperty {
                name: property.into(),
                ty: ty.map(Into::into),
                declarer: declarer.clone(),
                keyed: false,
            });
        }
        out
    }

    /// The accessor a rule finds for a property `name` on `class_fqn`.
    ///
    /// Cheapest first: a project with no rule asks nothing, and the two lookups by name come before
    /// any hierarchy is climbed.
    fn accessor_of(&self, class_fqn: &str, kind: MemberKind, name: &str) -> Option<Accessor> {
        if kind != MemberKind::Property || name.is_empty() {
            return None;
        }
        let rules = self.skeleton.returns().accessors();
        if rules.is_empty() {
            return None;
        }
        let table = self.members(class_fqn);
        rules.iter().find_map(|rule| {
            let spelled = format!("{}{}{}", rule.prefix, studly(name), rule.suffix);
            let method = table
                .find(MemberKind::Method, &spelled)
                .map(|member| member.name.clone());
            let attribute = || {
                let camel = camel(name);
                table
                    .find(MemberKind::Method, &camel)
                    .filter(|_| self.returns_attribute(class_fqn, &camel, &rule.attribute))
                    .map(|member| member.name.clone())
            };
            let found = match method {
                Some(method) => Accessor::Method(method),
                None => Accessor::Attribute(attribute()?),
            };
            self.is_at_or_under(class_fqn, &rule.class).then_some(found)
        })
    }

    /// The getter an accessor method's body hands the framework, from the class that declares the
    /// method -- which may be a parent of the one the property is read on.
    fn getter_of(&self, class_fqn: &str, method: &str) -> Option<Held> {
        let location = self.member_location(class_fqn, MemberKind::Method, method)?;
        let declarer = location.container.as_ref()?;
        let getter = self
            .skeleton
            .stated_by(location.file, declarer.as_str())?
            .getters
            .iter()
            .find(|getter| getter.method.as_str().eq_ignore_ascii_case(method))?;
        Some(Held::Stored {
            ty: getter.ty,
            scope: None,
        })
    }

    /// Whether a method's *native* return type names the framework's attribute class.
    ///
    /// Laravel's rule, read off `hasAttributeMutator`: reflection's return type, a single named one,
    /// and that exact class -- so a docblock saying so, a body handing one back or a union holding it
    /// make no accessor at runtime, and make none here. Reading the hint alone is also what keeps a
    /// list of a model's properties affordable: asking what every method of the model returns,
    /// bodies and all, was most of a 17 ms completion on Monica's `User`.
    fn returns_attribute(&self, class_fqn: &str, method: &str, attribute: &str) -> bool {
        let table = self.members(class_fqn);
        let Some(hint) = table
            .find(MemberKind::Method, method)
            .and_then(|member| member.row.ty)
        else {
            return false;
        };
        let types = self.skeleton.types();
        let named = match types.get(hint) {
            phrost_ty::TypeRef::Nullable(inner) => types.get(*inner),
            other => other,
        };
        // A hint a docblock refined, `Attribute<string, never>`, is still the one class it names.
        let named = match named {
            phrost_ty::TypeRef::Generic { base, .. } => types.get(*base),
            other => other,
        };
        matches!(named, phrost_ty::TypeRef::Named(class) if class.as_str().eq_ignore_ascii_case(attribute))
    }

    fn is_at_or_under(&self, class_fqn: &str, ancestor: &str) -> bool {
        class_fqn.eq_ignore_ascii_case(ancestor)
            || self.descends_from(class_fqn, ancestor, HIERARCHY_DEPTH)
    }
}

/// The text a method name holds between a prefix and a suffix, when it has both and something
/// between them starting with a capital: `Excerpt` in `getExcerptAttribute`.
fn spelled_between<'n>(method: &'n str, prefix: &str, suffix: &str) -> Option<&'n str> {
    let inner = method.strip_prefix(prefix)?.strip_suffix(suffix)?;
    inner
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        .then_some(inner)
}

/// `published_at` and `publishedAt` as `PublishedAt` -- Laravel's `Str::studly`, for the names a
/// property is written in.
pub(super) fn studly(name: &str) -> String {
    name.split(['_', '-', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + letters.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// `published_at` as `publishedAt` -- Laravel's `Str::camel`.
fn camel(name: &str) -> String {
    let studly = studly(name);
    let mut letters = studly.chars();
    letters
        .next()
        .map(|first| first.to_ascii_lowercase().to_string() + letters.as_str())
        .unwrap_or_default()
}

/// `PublishedAt` and `publishedAt` as `published_at` -- Laravel's `Str::snake`.
pub(super) fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (index, letter) in name.chars().enumerate() {
        if letter.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(letter.to_ascii_lowercase());
        } else {
            out.push(letter);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{camel, snake, spelled_between, studly};

    #[test]
    fn the_names_laravel_spells_an_accessor_with() {
        assert_eq!(studly("published_at"), "PublishedAt");
        assert_eq!(studly("publishedAt"), "PublishedAt");
        assert_eq!(camel("published_at"), "publishedAt");
        assert_eq!(snake("PublishedAt"), "published_at");
        assert_eq!(snake("headline"), "headline");
        assert_eq!(
            spelled_between("getExcerptAttribute", "get", "Attribute"),
            Some("Excerpt")
        );
        assert_eq!(spelled_between("getAttribute", "get", "Attribute"), None);
        assert_eq!(
            spelled_between("getattributeAttribute", "get", "Attribute"),
            None
        );
    }
}
