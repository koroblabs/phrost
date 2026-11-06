//! Framework knowledge, behind typed extension points.
//!
//! The rule this crate exists to enforce: **no framework's name appears outside it.** Not in
//! discovery, not in resolution, not in a feature. The previous attempt started that way too, and
//! then Drupal knowledge leaked into a hover implementation and a hardcoded list of return types —
//! after which adding Laravel meant editing generic features, and the two frameworks could break
//! each other.
//!
//! So a framework contributes through [`FrameworkProvider`], the generic layer calls the registry,
//! and adding a framework is a new directory under `providers/` plus one line in [`registry`].
//! Nothing else should have to change; if it does, the seam is missing a method.
//!
//! # Why a framework needs a seam at all
//!
//! Composer is not the only thing that decides where PHP lives. Drupal registers a namespace per
//! installed module at runtime, reading `*.info.yml` — so `Drupal\node\` maps to a directory that
//! nothing in `composer.json` mentions. A generic layer cannot know that, and cannot be taught it
//! without becoming a Drupal layer.

pub mod keys;
pub mod providers;
mod registry;
mod rule;
pub mod services;

pub use self::{
    registry::{FrameworkProject, FrameworkProvider, detect, known_ids, named},
    rule::{
        AccessorProperty, AnnotationRewrite, AnnotationValueClass, ArgumentBinding,
        ArrayKeyBinding, AttributeArgument, AttributeKey, AutoloadRule, BuilderCalls, BuiltKey,
        CatalogEntry, CatalogKind, CatalogName, ChainedOperand, ChainedReturn, ClassAlias,
        ComponentTag, ConstructedEntry, ContainerReach, ContainerRule, DeclaredInputs,
        DeclaredTable, DerivedKey, DerivedName, DispatchedName, DocumentedBy, DocumentedKeys,
        FilledArgument, FilledReturn, FluentPrefix, Forwarded, ForwardedArgument, FunctionBinding,
        ImpliedMixin, InputDeclarations, InvokedCallback, KeyedRelation, KeyedReturn,
        MacroRegistrar, MagicName, MagicProperty, MethodBackedProperty, MethodsTaggedFrom,
        MixinReturnedBy, NamedAfterCaller, NamedTemplateArgument, NonNullReturn, NullFact,
        OpenedByCall, PlaceEntry, PrefixedMethod, PropertyTable, RenderArray, RenderedTemplate,
        RenderingClass, SharedTemplateData, StandInMethod, StatedKey, StatedProperties, StatedType,
        TabledAccess, TemplateDialect, TemplateProblems, TwigLoader, UnlistedKey,
        UserFacingDocblock, WrittenSchema, YamlReference, YamlTarget,
    },
};
