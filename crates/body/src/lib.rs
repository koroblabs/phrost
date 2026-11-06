//! Reading a function body: local scopes, and what a local's declaration says it is.
//!
//! Only for open buffers, because only they have bodies. Nothing here resolves a name to a class or
//! reaches another file: it answers questions that a single file's own text settles, and hands back
//! a written type reference for a layer with a catalog to resolve.
//!
//! # Why scopes are a chain and not a lookup
//!
//! PHP's local scope is a function, not a block — `if ($x) { $y = 1; }` leaves `$y` defined after
//! the block. So the only nesting that matters is function nesting, and closures are where it
//! happens. Their capture rules differ in a way that changes the answer:
//!
//! - A **closure** sees only its own parameters and what its `use (...)` clause names. A variable it
//!   neither declares nor captures is undefined inside it, and answering from the method around it
//!   would describe something that is not there.
//! - An **arrow function** captures by value automatically, so an outer variable really is the outer
//!   one and the search continues outward.
//!
//! Getting this backwards produces the worst kind of wrong answer: a name that exists outside with a
//! different type, described confidently.

mod assignment_sites;
mod binding;
mod bound_closures;
mod calls;
mod clone_with;
mod discarded_calls;
mod early_reads;
mod elements;
mod existence;
mod extractable;
mod facts;
mod filled_slots;
mod flow;
mod iteration;
mod keyed_elements;
mod locals;
mod matches;
mod members;
mod narrowing;
mod object_context;
mod occurrences;
mod path;
mod proved_memo;
mod range_nest;
mod reach;
mod reference_loops;
mod relative;
mod returns;
mod scope;
mod skipped_paths;
mod slot_value;
mod subscripts;
mod throws;
mod variables;
mod walk;

#[cfg(test)]
mod tests;

pub use self::{
    assignment_sites::{
        AssignmentSite, AssignmentTarget, assignment_sites, cloned_property_writes,
    },
    bound_closures::{ClosureThis, bound_closure_scope, bound_scope_names, closure_this_at},
    calls::{
        ArgumentSite, CallSite, CallTarget, a_call_is_named_at, call_site_at, call_sites,
        call_sites_in, enclosing_call,
    },
    discarded_calls::discarded_calls,
    early_reads::{EarlyRead, locals_read_before_bound},
    elements::{ElementWrite, element_written},
    existence::{ExistenceTests, existence_tests},
    extractable::{Extractable, extractable_at, literal_at},
    filled_slots::{FilledSlot, SlotFill, slots_filled_in_scope, slots_filled_since},
    flow::{falls_off_end, unreachable_regions},
    iteration::{IteratedSubject, iterated_subjects},
    keyed_elements::{KeyedElement, keyed_element_at, keyed_elements, keyed_siblings_at},
    locals::{
        BindingKind, LocalBinding, element_key_name, local_binding, local_name, locals_in_scope,
    },
    matches::{MatchSite, match_sites},
    members::{
        existence_checked, member_site_at, member_sites, null_suppressed_ranges, string_literal_at,
    },
    narrowing::{
        Callee, PassedAs, PassedPaths, PassedTo, narrowed_at, narrowed_this, passed_by_reference,
        passed_to_any_call, passed_to_calls,
    },
    object_context::{OutOfContext, Written, out_of_context},
    occurrences::visit_every_expression,
    occurrences::{
        NeverBound, NeverRead, Occurrence, OccurrenceKind, local_occurrences, locals_never_bound,
        locals_never_read,
    },
    path::{
        CallSource, ElementKey, MemberSite, OperandSite, Otherwise, PathRoot, PathStep, Refused,
        SiteKind, Unless, ValuePath, WriteSite, local_path, value_path_of,
    },
    proved_memo::{Asserts, NoAssertions, ProvedMemo},
    relative::relative_class_at,
    returns::{return_sites, returned_values},
    slot_value::{SlotValue, assigned_properties, slot_value},
    subscripts::{SubscriptSite, TestedKey, keyed_subscript_sites, subscript_sites, tested_keys},
    throws::{CatchScope, ThrowSite, catch_scopes, throw_sites},
    variables::{
        Assigned, VariableSite, assignments, local_declaration, promoting_parameter, variable_at,
    },
    walk::{AssertsArguments, NeverReturns, visit_every_statement},
};

/// Whether `at` is in the condition of an `if`, an `elseif` or a ternary rather than in any arm --
/// where a write can be the very thing the condition tests.
pub fn written_in_a_condition(hir: &phrost_hir::FileHir, at: phrost_model::TextSize) -> bool {
    facts::BodyFacts::of(hir).condition_of(at).is_some()
}
