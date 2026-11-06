//! A receiver as the scanner read it, as the steps the view walks.
//!
//! The translation between two descriptions of the same thing: [`crate::context`] says what was
//! *written* before the operator, and the view answers questions about a *chain*. Shared because
//! members and shape keys both start from a receiver and ask different questions of it.

use phrost_view::{ChainKey, ChainRoot, ChainStep, MemberKind};

use crate::context::{Receiver, ReceiverRoot, ReceiverStep, ScannedKey, StepKind};

/// A written receiver as the steps the view walks.
pub(super) fn chain_steps(receiver: &Receiver) -> Vec<ChainStep<'_>> {
    receiver
        .steps
        .iter()
        .map(|step| match step {
            ReceiverStep::Member {
                name,
                kind: StepKind::Constant,
                ..
            } => ChainStep::Constant(name.as_str()),
            ReceiverStep::Member {
                name,
                kind,
                arguments,
            } => ChainStep::Member {
                name: name.as_str(),
                kind: match kind {
                    // Static and instance properties share one table and are told apart by a flag,
                    // so reaching one is the same lookup either way.
                    StepKind::Property | StepKind::StaticProperty => MemberKind::Property,
                    StepKind::MethodReturn => MemberKind::Method,
                    StepKind::Constant => unreachable!("matched above"),
                },
                arguments,
            },
            ReceiverStep::Invoked => ChainStep::Invoked,
            // Both kinds travel: a literal answers a shape's key, a variable identifies the
            // slot. Throwing the second away is what left a guard about `$rows[$id]` unread.
            ReceiverStep::Element(key) => ChainStep::Element(key.as_ref().map(|key| match key {
                ScannedKey::Literal(text) => ChainKey::Literal(text),
                ScannedKey::Variable(name) => ChainKey::Variable(name),
            })),
        })
        .collect()
}

/// What the chain starts from. `None` for a receiver the scanner could not read, which is a
/// deliberate miss and not an empty answer.
pub(super) fn chain_root(root: &ReceiverRoot) -> Option<ChainRoot<'_>> {
    match root {
        ReceiverRoot::This => Some(ChainRoot::This),
        ReceiverRoot::SelfClass => Some(ChainRoot::SelfClass),
        ReceiverRoot::Parent => Some(ChainRoot::Parent),
        ReceiverRoot::Local(name) => Some(ChainRoot::Local(name)),
        ReceiverRoot::Class(name) => Some(ChainRoot::Class(name)),
        ReceiverRoot::Function { name, arguments } => Some(ChainRoot::Function {
            name: name.as_str(),
            arguments,
        }),
        ReceiverRoot::Unknown => None,
    }
}
