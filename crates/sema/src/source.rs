use phrost_model::{Name, TextRange, Visibility};

use crate::member::MemberRow;

/// What flattening needs to know about one class, wherever it is stored.
///
/// Implemented by the index. Keeping it a trait is what lets the precedence rules be tested
/// against a hand-built hierarchy — and keeps the dependency arrow pointing from storage to
/// semantics rather than the other way round.
pub trait ClassSource {
    /// The facts for a fully-qualified class name, or `None` if nothing declares it.
    ///
    /// Returning `None` is a normal answer, not an error: real projects reference classes that are
    /// not installed, and flattening has to produce a useful partial table anyway.
    fn class(&self, fqn: &str) -> Option<ClassFacts<'_>>;
}

/// One class as a single declaration site describes it.
#[derive(Clone, Copy, Debug)]
pub struct ClassFacts<'a> {
    pub fqn: &'a Name,
    pub kind: ClassKind,
    pub is_abstract: bool,
    /// `enum Suit: string` -- the backing type, and `None` for a pure enum.
    ///
    /// The type and not just a flag, though the flag is most of what it is for: which implicit
    /// interface the enum gets. The type answers the other half, which `BackedEnum` cannot -- it has
    /// to declare `$value` as `int|string` for every enum at once, and this enum said which.
    pub enum_backing: Option<phrost_ty::TypeId>,
    /// Resolved parents. A class has at most one; an interface may extend several.
    pub extends: &'a [ClassRef],
    pub implements: &'a [ClassRef],
    /// Traits pulled in by `use`, in source order — which matters, because a later `use` can
    /// conflict with an earlier one.
    pub uses: &'a [ClassRef],
    pub adaptations: &'a [TraitAdaptation],
    /// `@mixin Other` -- classes whose members are reachable through this one though PHP says
    /// nothing about them.
    pub mixins: &'a [ClassRef],
    /// `@mixin` tags a framework says this class behaves as though it wrote -- read exactly as the
    /// written ones are.
    pub implied_mixins: &'a [ClassRef],
    /// Methods of this class a framework says its `__call` hands to the declaration of the same
    /// name, so a scope PHP would refuse still reaches them -- see `ResolvedMember::opened`.
    pub opened: &'a [Box<str>],
    /// Classes a generator wrote this class's `@method` tags from, which a framework names: where
    /// one declares the method, its declaration is what the tag summarised, templates and all.
    pub tagged_from: &'a [ClassRef],
    pub members: &'a [MemberRow],
}

/// A reference from one class to another: where it points, and where it was written.
///
/// The range is why this is not just a `Name`: goto on `extends Base` should land on `Base`, not on
/// the class that wrote it. Defined here rather than in the index so storage can hand out slices of
/// these directly instead of copying them into a shape flattening happens to want.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassRef {
    pub fqn: Name,
    pub range: TextRange,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ClassKind {
    Class,
    Interface,
    Trait,
    Enum,
}

impl ClassKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Enum => "enum",
        }
    }
}

/// One `insteadof` or `as` clause from a `use SomeTrait { ... }` block.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraitAdaptation {
    /// The trait the method is taken from, when written as `A::m`. `None` for a bare `m`, which is
    /// only valid when exactly one used trait declares it.
    pub trait_fqn: Option<Name>,
    pub method: Name,
    pub method_range: TextRange,
    pub kind: TraitAdaptationKind,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TraitAdaptationKind {
    /// `A::m insteadof B, C;` — the listed traits lose this method.
    Precedence { excluded: Vec<Name> },
    /// `A::m as [visibility] [alias];` — either or both parts may be present. An alias *adds* a
    /// name rather than renaming, so the original stays reachable.
    Alias {
        alias: Option<Name>,
        visibility: Option<Visibility>,
    },
}

impl TraitAdaptationKind {
    /// The clause as a short label, for a dump the index invariants measure against.
    pub fn label(&self) -> String {
        match self {
            Self::Precedence { excluded } => format!(
                "insteadof {}",
                excluded
                    .iter()
                    .map(|name| name.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Alias { alias, visibility } => {
                let mut out = String::from("as");
                if let Some(visibility) = visibility {
                    out.push(' ');
                    out.push_str(visibility.as_str());
                }
                if let Some(alias) = alias {
                    out.push(' ');
                    out.push_str(alias.as_str());
                }
                out
            }
        }
    }
}
