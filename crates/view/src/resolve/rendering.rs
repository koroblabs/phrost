//! A template whose `$this` is the instance that renders it.
//!
//! A Livewire component renders its own template, inside a closure bound to itself: `$this` is the
//! component, its scope is the component's class, and every public property the component declares
//! is a variable. Which class a template belongs to is said in one of two places -- a
//! `view('livewire.counter')` written in a class's body, which the index records with the class it
//! is written in (`phrost_index::RenderSite::class`), or for a class with no `render()`, the
//! template its name spells below the rule's namespace.

use phrost_model::Name;
use phrost_sema::{MemberKind, MemberProvenance};

use crate::{
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

impl IndexView<'_> {
    /// The class whose instance renders this template, when a framework's rule says one does.
    pub(crate) fn template_class(&self) -> Option<Name> {
        self.template_class
            .get_or_init(|| self.find_template_class())
            .clone()
    }

    fn find_template_class(&self) -> Option<Name> {
        let rules = self.skeleton.returns().rendering_classes();
        if rules.is_empty() {
            return None;
        }
        let path = self.skeleton.file(self.current_file()?)?.path();
        let template = self.skeleton.template_name(path)?;
        // A call in a class's body says it outright.
        for (_, site) in self.skeleton.renderers_of(&template) {
            if let Some(class) = &site.class
                && self.rendering_rule(class.as_str()).is_some()
            {
                return Some(class.clone());
            }
        }
        // Else the class's name spells the template, where the class says nothing itself.
        let separator = self.skeleton.template_separator(path)?;
        rules.iter().find_map(|(base, namespace, templates)| {
            let below = template
                .strip_prefix(&**templates)?
                .strip_prefix(separator)?;
            let mut fqn = String::from(&**namespace);
            for segment in below.split(separator) {
                fqn.push('\\');
                fqn.push_str(&studly(segment));
            }
            let class = self.class_location(&fqn)?.fqn;
            (self.descends_from(class.as_str(), base, HIERARCHY_DEPTH)
                && self
                    .members(class.as_str())
                    .find(MemberKind::Method, "render")
                    .is_none())
            .then_some(class)
        })
    }

    /// The class a rule names that `class` descends from.
    fn rendering_rule(&self, class: &str) -> Option<&str> {
        self.skeleton
            .returns()
            .rendering_classes()
            .iter()
            .map(|(base, _, _)| &**base)
            .find(|base| self.descends_from(class, base, HIERARCHY_DEPTH))
    }

    /// What the public property `name` the template's class hands it holds -- where `declared`,
    /// only when its declaration or its docblock says so. A type read off a default is a starting
    /// value, and a tag rendering the component says better what it was handed.
    pub(crate) fn template_property(&self, name: &str, declared: bool) -> Option<Held> {
        let class = self.template_class()?;
        let row = self.handed_property(class.as_str(), name)?;
        if declared && (row.ty.is_none() || row.ty_from_value) && row.doc_ty.is_none() {
            return None;
        }
        self.member_held(class.as_str(), MemberKind::Property, name)
    }

    /// Where the public property `name` the template's class hands it is declared.
    pub(crate) fn template_property_at(
        &self,
        name: &str,
    ) -> Option<crate::location::SymbolLocation> {
        let class = self.template_class()?;
        self.handed_property(class.as_str(), name)?;
        self.member_location(class.as_str(), MemberKind::Property, name)
    }

    /// The names of the public properties the template's class hands it.
    pub(crate) fn template_property_names(&self) -> Vec<Box<str>> {
        let Some(class) = self.template_class() else {
            return Vec::new();
        };
        let Some(base) = self.rendering_rule(class.as_str()) else {
            return Vec::new();
        };
        self.members(class.as_str())
            .members()
            .iter()
            .filter(|member| self.is_handed(base, member))
            .map(|member| Box::from(member.name.as_str()))
            .collect()
    }

    /// The property `name` `class` hands its template, when it does.
    fn handed_property(&self, class: &str, name: &str) -> Option<phrost_sema::MemberRow> {
        let base = self.rendering_rule(class)?;
        let table = self.members(class);
        let member = table.find(MemberKind::Property, name)?;
        self.is_handed(base, member).then(|| member.row.clone())
    }

    /// A public property declared below the rule's class: the ones the framework's own class
    /// declares are not handed over.
    fn is_handed(&self, base: &str, member: &phrost_sema::ResolvedMember) -> bool {
        member.row.kind == MemberKind::Property
            && member.visibility == phrost_model::Visibility::Public
            && !member.row.modifiers.is_static()
            && !member.forwarded
            && match &member.provenance {
                MemberProvenance::Own | MemberProvenance::Trait(_) => true,
                MemberProvenance::Inherited(owner) => {
                    !owner.as_str().eq_ignore_ascii_case(base)
                        && !self.descends_from(base, owner.as_str(), HIERARCHY_DEPTH)
                }
                MemberProvenance::Interface(_) => false,
            }
    }
}

/// A template name's segment as a class name spells it: `create-post` is `CreatePost`, as
/// Laravel's `Str::studly` writes it.
fn studly(segment: &str) -> String {
    segment
        .split(['-', '_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(letters).collect()
            })
        })
        .collect()
}
