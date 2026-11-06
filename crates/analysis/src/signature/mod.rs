//! What to show while a call is being written.
//!
//! A thin feature on purpose. Everything hard -- which call the cursor is in, whose declaration it
//! names, which parameter is being filled -- is resolution, and resolution belongs behind the view.
//! What is left here is the shape a client expects.

use phrost_model::TextSize;
use phrost_view::IndexView;

/// The callee's declaration, and which parameter is being filled.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SignatureHelp {
    /// The whole declaration, for the client to show.
    pub label: String,
    /// Each parameter as declared, in order.
    pub parameters: Vec<String>,
    /// Which of them the cursor is filling, when position still says.
    pub active: Option<usize>,
}

/// The signature of the call `offset` sits inside.
///
/// Nothing when the call cannot be resolved, which is the same refusal every other feature makes:
/// the parameters of some plausible method are worse than no parameters, because they are read as
/// authoritative and silently disagree with the code.
pub fn signature_help(view: &IndexView<'_>, offset: TextSize) -> Option<SignatureHelp> {
    let help = view.call_at(offset)?;
    Some(SignatureHelp {
        label: help.label,
        parameters: help.parameters,
        active: help.active,
    })
}
