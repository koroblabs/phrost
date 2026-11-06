//! Flattening tests, run against a hand-built hierarchy.
//!
//! No project, no files, no indexing — which is the point of [`ClassSource`](crate::ClassSource)
//! being a trait. PHP's precedence rules are subtle enough that they deserve tests that fail for
//! exactly one reason, and a fixture-driven test would also be exercising parsing, lowering,
//! resolution, and the catalog.

mod enums;
mod precedence;
mod robustness;
mod traits;

use expect_test::Expect;
use phrost_model::{Interner, Name, TextRange, TextSize, Visibility};
use rustc_hash::FxHashMap;

use crate::{
    ClassFacts, ClassKind, ClassRef, ClassSource, MemberKind, MemberModifiers, MemberRow,
    TraitAdaptation, TraitAdaptationKind, flatten_class, render_member_table,
};

/// A hierarchy assembled in a test, standing in for the index.
#[derive(Debug, Default)]
pub(crate) struct Source {
    interner: Interner,
    /// Empty in these tests: precedence does not depend on types, and the renderer needs a table to
    /// resolve the ids a member row would carry.
    types: phrost_ty::TypeTable,
    /// Keyed by folded name, because PHP looks classes up case-insensitively.
    classes: FxHashMap<String, Entry>,
}

#[derive(Debug)]
struct Entry {
    fqn: Name,
    kind: ClassKind,
    is_abstract: bool,
    enum_backing: Option<phrost_ty::TypeId>,
    extends: Vec<ClassRef>,
    implements: Vec<ClassRef>,
    uses: Vec<ClassRef>,
    adaptations: Vec<TraitAdaptation>,
    mixins: Vec<ClassRef>,
    members: Vec<MemberRow>,
}

impl Source {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn class(self, fqn: &str, build: impl FnOnce(Decl<'_>) -> Decl<'_>) -> Self {
        self.declare(ClassKind::Class, fqn, build)
    }

    pub(crate) fn interface(self, fqn: &str, build: impl FnOnce(Decl<'_>) -> Decl<'_>) -> Self {
        self.declare(ClassKind::Interface, fqn, build)
    }

    pub(crate) fn php_trait(self, fqn: &str, build: impl FnOnce(Decl<'_>) -> Decl<'_>) -> Self {
        self.declare(ClassKind::Trait, fqn, build)
    }

    pub(crate) fn php_enum(self, fqn: &str, build: impl FnOnce(Decl<'_>) -> Decl<'_>) -> Self {
        self.declare(ClassKind::Enum, fqn, build)
    }

    /// An enum with a backing type, which is what decides its implicit interface.
    pub(crate) fn backed_enum(
        mut self,
        fqn: &str,
        build: impl FnOnce(Decl<'_>) -> Decl<'_>,
    ) -> Self {
        self = self.declare(ClassKind::Enum, fqn, build);
        let backing = self.types.builtin(phrost_model::BuiltinTy::String);
        if let Some(entry) = self.classes.get_mut(&fqn.to_ascii_lowercase()) {
            entry.enum_backing = Some(backing);
        }
        self
    }

    fn declare(
        mut self,
        kind: ClassKind,
        fqn: &str,
        build: impl FnOnce(Decl<'_>) -> Decl<'_>,
    ) -> Self {
        let name = self.interner.name(fqn);
        let entry = Entry {
            fqn: name,
            kind,
            is_abstract: false,
            enum_backing: None,
            extends: Vec::new(),
            implements: Vec::new(),
            uses: Vec::new(),
            adaptations: Vec::new(),
            mixins: Vec::new(),
            members: Vec::new(),
        };

        let Decl { entry, .. } = build(Decl {
            entry,
            interner: &mut self.interner,
        });
        self.classes.insert(fqn.to_ascii_lowercase(), entry);
        self
    }
}

impl ClassSource for Source {
    fn class(&self, fqn: &str) -> Option<ClassFacts<'_>> {
        let entry = self.classes.get(&fqn.to_ascii_lowercase())?;
        Some(ClassFacts {
            fqn: &entry.fqn,
            kind: entry.kind,
            is_abstract: entry.is_abstract,
            enum_backing: entry.enum_backing,
            extends: &entry.extends,
            implements: &entry.implements,
            uses: &entry.uses,
            adaptations: &entry.adaptations,
            mixins: &entry.mixins,
            implied_mixins: &[],
            opened: &[],
            tagged_from: &[],
            members: &entry.members,
        })
    }
}

/// Builder for one declaration inside a [`Source`].
pub(crate) struct Decl<'a> {
    entry: Entry,
    interner: &'a mut Interner,
}

impl Decl<'_> {
    pub(crate) fn abstract_class(mut self) -> Self {
        self.entry.is_abstract = true;
        self
    }

    pub(crate) fn extends(mut self, fqn: &str) -> Self {
        let fqn = self.interner.name(fqn);
        self.entry.extends.push(ClassRef {
            fqn,
            range: range(),
        });
        self
    }

    pub(crate) fn implements(mut self, fqn: &str) -> Self {
        let fqn = self.interner.name(fqn);
        self.entry.implements.push(ClassRef {
            fqn,
            range: range(),
        });
        self
    }

    pub(crate) fn uses(mut self, fqn: &str) -> Self {
        let fqn = self.interner.name(fqn);
        self.entry.uses.push(ClassRef {
            fqn,
            range: range(),
        });
        self
    }

    pub(crate) fn method(self, name: &str, visibility: Visibility) -> Self {
        self.member(MemberKind::Method, name, visibility)
    }

    pub(crate) fn abstract_method(mut self, name: &str, visibility: Visibility) -> Self {
        self = self.member(MemberKind::Method, name, visibility);
        self.entry
            .members
            .last_mut()
            .expect("just pushed")
            .modifiers
            .set_abstract(true);
        self
    }

    pub(crate) fn static_method(mut self, name: &str, visibility: Visibility) -> Self {
        self = self.member(MemberKind::Method, name, visibility);
        self.entry
            .members
            .last_mut()
            .expect("just pushed")
            .modifiers
            .set_static(true);
        self
    }

    pub(crate) fn property(self, name: &str, visibility: Visibility) -> Self {
        self.member(MemberKind::Property, name, visibility)
    }

    pub(crate) fn constant(self, name: &str, visibility: Visibility) -> Self {
        self.member(MemberKind::ClassConstant, name, visibility)
    }

    pub(crate) fn enum_case(self, name: &str) -> Self {
        self.member(MemberKind::EnumCase, name, Visibility::Public)
    }

    /// `A::method insteadof B, C;`
    pub(crate) fn insteadof(mut self, winner: &str, method: &str, losers: &[&str]) -> Self {
        let trait_fqn = self.interner.name(winner);
        let method = self.interner.name(method);
        let excluded = losers
            .iter()
            .map(|loser| self.interner.name(loser))
            .collect();

        self.entry.adaptations.push(TraitAdaptation {
            trait_fqn: Some(trait_fqn),
            method,
            method_range: range(),
            kind: TraitAdaptationKind::Precedence { excluded },
        });
        self
    }

    /// `Trait::method as [visibility] [alias];`
    pub(crate) fn alias_from(
        mut self,
        trait_fqn: &str,
        method: &str,
        alias: Option<&str>,
        visibility: Option<Visibility>,
    ) -> Self {
        let trait_fqn = self.interner.name(trait_fqn);
        let method = self.interner.name(method);
        let alias = alias.map(|alias| self.interner.name(alias));

        self.entry.adaptations.push(TraitAdaptation {
            trait_fqn: Some(trait_fqn),
            method,
            method_range: range(),
            kind: TraitAdaptationKind::Alias { alias, visibility },
        });
        self
    }

    /// `method as [visibility] [alias];`
    pub(crate) fn alias(
        mut self,
        method: &str,
        alias: Option<&str>,
        visibility: Option<Visibility>,
    ) -> Self {
        let method = self.interner.name(method);
        let alias = alias.map(|alias| self.interner.name(alias));

        self.entry.adaptations.push(TraitAdaptation {
            trait_fqn: None,
            method,
            method_range: range(),
            kind: TraitAdaptationKind::Alias { alias, visibility },
        });
        self
    }

    fn member(mut self, kind: MemberKind, name: &str, visibility: Visibility) -> Self {
        let name = self.interner.name(name);
        self.entry.members.push(MemberRow {
            doc_ty_from_body: false,
            doc_ty_refused: false,
            reads_own_arguments: false,
            name,
            kind,
            visibility,
            set_visibility: None,
            modifiers: MemberModifiers::default(),
            ty: None,
            doc_ty: None,
            ty_from_value: false,
            params: Box::default(),
            name_range: range(),
            full_range: range(),
            docblock_range: None,
            contract: None,
            from_docblock: false,
        });
        self
    }
}

/// Ranges are irrelevant to precedence, so tests use one placeholder rather than inventing
/// positions that would make every snapshot noisier without checking anything.
fn range() -> TextRange {
    TextRange::empty(TextSize::from(0))
}

pub(crate) fn check(source: &Source, class: &str, expect: Expect) {
    expect.assert_eq(&render_member_table(
        &flatten_class(source, class),
        &source.types,
    ));
}
