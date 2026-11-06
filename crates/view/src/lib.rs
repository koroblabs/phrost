//! The only index API a feature may see.
//!
//! Everything above this crate — navigation, hover, completion, diagnostics — reads the index
//! through these projections and nothing else. The store crates are not in its dependency graph,
//! which is checked rather than agreed: the previous attempt at this project had an API module too,
//! and features imported the index directly anyway.
//!
//! # What belongs here
//!
//! A view describes an **indexed fact**, not a feature's workflow. `members_visible_from` belongs
//! here; `completion_items_after_arrow` does not. The test is whether the method would still make
//! sense if the feature that wanted it were deleted.
//!
//! A feature that needs a fact it cannot get from here is telling you a projection is missing —
//! not that it should reach past this layer.
//!
//! # No fallbacks
//!
//! Every method either answers from the index or returns nothing, deterministically. There is no
//! "try the index, else re-parse, else scan the text, else guess". That rule is what makes the
//! answer independent of whether a file happens to be open, and it is affordable only because the
//! skeleton keeps a name range for every declaration in every file, vendor included.
//!
//! # Two tiers, and why openness cannot change an answer
//!
//! Position queries — "what is under the cursor" — read the open buffer, because the file being
//! looked at is by definition open. Cross-file answers — "where is that declared" — read the
//! skeleton, which knows nothing about openness. So a target file being open cannot change where
//! goto lands. That is structural, not a convention, and there is a test that says so.

mod aliases;
mod annotations;
mod arguments;
pub mod assignments;
mod assumed_non_null;
mod built_keys;
mod callable_strings;
mod calls;
mod candidates;
mod catalog_uses;
mod chain;
mod class_body;
mod closure_this;
mod closures;
mod comments;
mod compat;
mod completion;
mod configuration;
mod constant_references;
mod container_reach;
mod cursor;
mod declared_input_uses;
mod declared_inputs;
mod deprecated_uses;
mod discarded_results;
mod dispatched;
mod documentable;
mod documentation;
mod documented_by;
mod documented_keys;
mod early_reads;
mod extract;
mod filtered_elements;
mod flow;
mod function_references;
mod generic_fit;
mod generics;
mod hints;
mod implementers;
mod imports;
mod impossible_use;
mod inline;
mod iteration;
mod key_kinds;
mod keys;
mod literal_returns;
mod locals;
mod location;
mod named_arguments;
mod names;
mod namespace_paths;
mod namespaces;
mod not_callable;
mod not_iterable;
mod out_of_context;
mod outline;
mod overridable;
mod overrides;
mod parameters;
mod placeholders;
mod position;
mod promotable;
mod psr4;
mod reached;
mod readonly_writes;
mod redeclared_member;
mod references;
mod refused_declaration;
mod refused_return;
mod resolve;
mod results_here;
mod returned_by_body;
mod returned_shape;
mod returns;
mod ruled_out_failures;
mod sealed_parent;
mod search;
mod signature;
mod stand_ins;
mod string_classes;
mod subscripts;
mod symbols;
mod syntax;
mod tabled;
mod trait_adaptations;
mod trait_collision;
mod trait_hosts;
mod twice_declared;
pub mod twig;
mod undocumented_throws;
mod unhandled_cases;
mod unimplemented;
mod unused_imports;
mod unused_members;
mod variables;
mod view;
mod word_evidence;

#[cfg(test)]
mod tests;

/// Re-exported because they appear in this crate's own return types.
///
/// A feature layer must be able to name what a projection hands it, and reaching past the view to
/// the crate that defined it would put a store crate in its dependency graph — which is the thing
/// the graph is checked to prevent.
pub use phrost_body::{
    BindingKind, EarlyRead, NeverBound, NeverRead, Occurrence, OccurrenceKind, Written,
};
pub use phrost_hir::{ImportKind, NameAnchor, display_class_name, is_anonymous_class_name};
/// Re-exported because a cursor target carries one: a catalog key can name a place in a file the
/// index does not hold, and the feature layer has to be able to say where.
pub use phrost_index::{DocumentedKey, Place};
pub use phrost_sema::{ClassKind, MemberKind, MemberProvenance};

pub use self::{
    aliases::{AliasBinding, AliasCandidate},
    annotations::DeclaredClass,
    arguments::WrittenBack,
    arguments::{MissingArguments, SentinelArgument, Uninstantiable, UninstantiableClass},
    assumed_non_null::AssumedNonNull,
    built_keys::BuiltKeyEntry,
    calls::ResolvedCall,
    catalog_uses::CatalogUse,
    chain::{ChainKey, ChainRoot, ChainStep, ReceiverClasses},
    class_body::{ClassBody, DeclaredProperty},
    comments::CommentPlace,
    completion::{
        Candidate, CatalogCandidate, FluentCandidate, MemberCandidate, MemberDeclaration,
        NameKinds, PrefixedCandidate, UndeclaredProperty,
    },
    configuration::{
        ConfigCandidate, ConfigCandidateKind, ConfigClassSite, ConfigProblem, ConfigReference,
        ConfigReferent,
    },
    container_reach::ContainerReach,
    cursor::CursorTarget,
    declared_input_uses::UndeclaredInput,
    declared_inputs::DeclaredInputEntry,
    deprecated_uses::{DeprecatedKind, DeprecatedUse},
    discarded_results::DiscardedResult,
    dispatched::DispatchSite,
    documentable::{Documentable, DocumentedParam},
    documentation::{
        BodyRef, Docblock, DocblockScope, DocumentedDeclaration, DocumentedItem,
        DocumentedProperty, Parameter,
    },
    documented_by::DocumentationClaim,
    documented_keys::DocumentedKeys,
    extract::Extraction,
    flow::{MissingReturn, UnreachableCode},
    function_references::FunctionReference,
    hints::ArgumentBinding,
    implementers::{Implementer, Inherits},
    imports::ImportEdit,
    impossible_use::{ImpossibleUse, UsedAs},
    inline::Inlining,
    key_kinds::ImpossibleKey,
    keys::ShapeKey,
    location::{ClassHeader, MemberModifiers, SymbolLocation},
    named_arguments::{Misnamed, MisnamedArgument, NamedParameter},
    names::NameReference,
    namespace_paths::PathContinuations,
    namespaces::NamespaceDeclaration,
    not_callable::NotCallable,
    not_iterable::{NotIterable, NotIterableLoop},
    out_of_context::OutOfContext,
    outline::OutlineNode,
    overridable::OverridableMethod,
    overrides::{BadOverride, EmptyOverride, Incompatible},
    placeholders::{PlaceholderFault, PlaceholderMismatch},
    promotable::Promotable,
    psr4::Psr4Mismatch,
    readonly_writes::{RefusedBecause, RefusedWrite},
    redeclared_member::RedeclaredMember,
    references::{MemberAccess, MemberAccessOutcome, SentinelReach},
    refused_declaration::{Refused, RefusedDeclaration},
    refused_return::{RefusedReturn, WrongReturn},
    returns::IncompatibleReturn,
    sealed_parent::SealedParent,
    signature::{CallHelp, FreeParameter},
    subscripts::{
        Absence, AbsentKey, BadStringOffset, MisspeltKey, NonArraySubscript, NullSubscript,
    },
    syntax::SyntaxProblem,
    trait_adaptations::TraitAlias,
    trait_collision::TraitCollision,
    twice_declared::TwiceDeclared,
    undocumented_throws::{ThrownByCall, UndeclaredThrow},
    unhandled_cases::UnhandledCases,
    unimplemented::{RequiredMethod, UnimplementedMethods},
    unused_imports::UnusedImport,
    unused_members::UnusedMember,
    variables::{AssignedType, VariableInfo},
    view::IndexView,
};
