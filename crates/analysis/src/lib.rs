//! Editor features over one index snapshot, with no protocol types in sight.
//!
//! This is where an indexed fact becomes an answer a person reads: where to jump, what to show on
//! hover, what to offer as a completion. The results deliberately contain no LSP types, so the
//! protocol layer is a translation and nothing more, and so a feature can be tested without a
//! server, a transport, or a client.
//!
//! # What belongs here, and what does not
//!
//! Ranking, filtering, wording, and presentation. Not PHP semantics, not workspace lookups, not
//! text scanning. If a feature here needs a fact it cannot get from `phrost-view`, the projection
//! is missing — reaching past the view is how the previous attempt grew a second, disagreeing copy
//! of the semantics.
//!
//! The store crates and the parser are not in this crate's dependency graph, and that is checked.

mod actions;
mod calls;
mod completion;
mod context;
mod diagnostics;
mod docblock;
mod hints;
mod hover;
mod navigation;
mod references;
mod render;
mod signature;
mod stub;
mod symbols;
mod tokens;
mod type_width;

#[cfg(test)]
mod tests;

/// Re-exported because it appears in this crate's own public types.
///
/// Same reason the view re-exports what it hands out: the protocol layer has to be able to name a
/// member kind to put one on the wire, and reaching past this crate for it would put a projection's
/// dependency in the protocol's graph.
pub use phrost_view::{MemberKind, OccurrenceKind};

pub use self::{
    actions::{ActionKind, ActionRequest, CodeAction, DocStyle, TextEdit, code_actions},
    calls::{
        CallSubject, IncomingCall, OutgoingCall, call_hierarchy_head, callable_declaration,
        incoming_calls, outgoing_calls, subject_of,
    },
    completion::{
        Call, CallShape, CompletionItem, CompletionList, Describe, Described, MemberRef,
        completions, describe_completion,
    },
    diagnostics::{
        AskedFor, CheckCost, Diagnostic, Severity, check_costs, config_diagnostics, diagnostics,
        diagnostics_asked_for, template::template_diagnostics,
    },
    hints::{InlayHint, InlayKind, inlay_hints},
    hover::{HoverBlock, HoverInfo, hover},
    navigation::{
        ExternalTarget, NavigationTarget, goto_definition, goto_external, goto_implementation,
        goto_type_definition, hierarchy_head, subtypes_of, supertypes_of,
    },
    references::{
        ConstructionQuery, ReferenceTarget, SweepQuery, is_reserved_word, is_valid_name,
        reference_target, references_in, rename_range, rename_refusal, rename_span, rename_target,
        rename_target_above,
    },
    signature::{SignatureHelp, signature_help},
    symbols::{SearchHit, SymbolEntry, document_symbols, workspace_symbols},
    tokens::semantic_tokens,
};
