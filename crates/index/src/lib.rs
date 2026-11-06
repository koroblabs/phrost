//! The project-wide skeleton: which file declares which fully-qualified name.
//!
//! This is the lean, always-resident half of the index. It knows what every file declares, what it
//! imports, and how its classes relate — and deliberately nothing else. No source text, no parse
//! trees, no expression bodies, no diagnostics. Resident memory is then a function of how much code
//! the project contains, not how large its `vendor/` directory is.
//!
//! # Why leanness is what makes "no fallbacks" possible
//!
//! Every declaration in every file keeps its name range, vendor included. That is the fact the old
//! fallback paths existed to reconstruct: when a range was dropped for a closed file, goto had to
//! guess by scanning text, and guessing is what made behavior depend on whether a file happened to
//! be open. Keeping the range for everything is cheap, and it turns "the index has no answer" into
//! a statement about the code rather than about the index.
//!
//! # Determinism
//!
//! Two properties are load-bearing and both are checked by tests rather than argued for:
//!
//! - **Per-file updates are local.** Re-ingesting one file touches only its own entries.
//! - **The result does not depend on the order files arrived.** Conflicting declarations are ranked
//!   by origin and path, never by `FileId`, because a file id reflects discovery order. Without
//!   this, an index built by applying edits could disagree with one built from scratch.

mod bulk;
pub mod cache;
pub mod class_defaults;
mod configuration;
mod configured_this;
pub mod constant;
pub mod constructions;
mod declared_inputs;
mod dispatch;
mod dump;
mod function;
pub mod macros;
mod open;
mod owners;
mod reached;
mod renders;
mod returned_paths;
mod returns;
pub mod schema;
mod skeleton;
mod state;
mod stated;
mod superglobals;
mod tables;
mod throws;
mod types;
mod words;

#[cfg(test)]
mod tests;

pub use self::{
    bulk::{BatchFile, INGEST_BATCH, Reused},
    configuration::{ConfigReference, ConfigSite, ConfigTarget, TwigLoader},
    configured_this::{ConfiguredThis, ConfiguredThisRule},
    declared_inputs::DeclaredInputsRule,
    dispatch::DispatchRow,
    dump::dump_skeleton,
    function::FunctionRow,
    open::{OpenFile, OpenSet},
    owners::{Declarer, OwnerCatalog},
    reached::{Reached, Reaches},
    renders::{RenderArrayRule, RenderKind, RenderSite, SHARED, enclosing_body, written_key},
    returned_paths::{ReturnedKey, ReturnedRow},
    returns::{
        AccessorRule, ArgumentBinding, ArgumentClass, ChainedBranch, ChainedOperandRule,
        CompleteTable, ContainerReachRule, DefaultBranch, Dispatch, DispatchReceiver, DispatchRule,
        DocumentedByRule, DocumentedKey, DocumentedKeysRule, FilledArgumentRule, FilledReturnRule,
        FluentPrefixRule, Forwarded, ForwardedKey, FunctionArgument, KeyedReturnRule,
        MethodProperty, NEAR_MISS_EDITS, NEAR_MISS_FLOOR, NamedAfterRule, NamedArgumentRule, Place,
        PrefixedMethodRule, PropertyBinding, PropertyNames, PropertyRows, ReturnContracts,
        SharedTemplateRule, TabledBinding, UserFacingDocblockRule, edits_within,
    },
    skeleton::{
        BuiltKey, ClassSkeleton, FileSkeleton, ProjectSkeleton, PropertyShape, SkeletonMemory,
        StandInMethod, TemplateDialect, TemplateRow, TemplateToPhp,
    },
    state::IndexState,
    stated::{
        StatedGetter, StatedKeyRule, StatedKind, StatedPropertiesRule, StatedRow, StatedTable,
        StatedValue,
    },
    tables::{AttributeArgument, DeclaredTableRule, FoundTable, TabledProperty},
    throws::ThrownRow,
    words::{WordNeedles, WordSet},
};

/// Re-exported because installing it is part of setting an index up, and the layer that knows which
/// attributes matter has no other reason to name the lowering crate.
pub use phrost_lower::KeyedAttributes;
