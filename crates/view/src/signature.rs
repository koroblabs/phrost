//! The call the cursor is inside, and what the callee's declaration says.
//!
//! The same resolution parameter hints need, asked the other way round: hints start from an
//! argument and want its parameter, and this starts from a position and wants the whole
//! declaration. So it shares [`IndexView::parameters_of`] and cannot drift from it.

use phrost_body::{CallSite, CallTarget};
use phrost_model::{Name, TextSize};

use crate::view::IndexView;

/// What to show while a call is being written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CallHelp {
    /// The callee as declared, rendered as PHP: `find(int $id, bool $strict = false)`.
    pub label: String,
    /// Each parameter as declared, in order. A client highlights one of these, so they are handed
    /// over separately rather than left to be found again inside the label.
    pub parameters: Vec<String>,
    /// Which parameter the cursor is filling.
    ///
    /// A named argument says which one outright, which is the whole reason somebody writes one.
    /// Otherwise it is the argument's index, and `None` once that has stopped meaning anything --
    /// past a spread, or past a named argument, which PHP allows nothing positional to follow.
    ///
    /// Not the parameter hints' rule, though the two look alike. A hint asks whether naming an
    /// argument adds anything, and a name already written answers no; this asks which parameter is
    /// being filled, and a name written there is the most certain answer there is.
    pub active: Option<usize>,
}

impl IndexView<'_> {
    /// The innermost call `offset` sits inside, if its callee can be resolved.
    ///
    /// Innermost, because `outer(inner($x))` is two calls containing the cursor and only the inner
    /// one is being written.
    pub fn call_at(&self, offset: TextSize) -> Option<CallHelp> {
        let open = self.current()?;
        let hir = open.hir();

        let call = phrost_body::call_site_at(hir, offset)?;

        let params = self.parameters_of(hir, &call)?;
        let types = self.skeleton.types();

        Some(CallHelp {
            label: format!(
                "{}({})",
                callee_name(hir, &call),
                params
                    .iter()
                    .map(|param| {
                        let mut out = String::new();
                        param.write(&mut out, types, hir);
                        out
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            parameters: params
                .iter()
                .map(|param| {
                    let mut out = String::new();
                    param.write(&mut out, types, hir);
                    out
                })
                .collect(),
            active: active_parameter(&call, &params, offset),
        })
    }
}

/// One parameter a call has not been given yet.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FreeParameter {
    /// Without the leading `$`, which is not written at a call site: `find(strict: true)`.
    pub name: Name,
    /// The type it takes, rendered. `None` when the declaration names none.
    pub ty: Option<String>,
}

impl IndexView<'_> {
    /// The parameters of the call at `offset` that nothing has filled in yet.
    ///
    /// What named-argument completion offers. Already-filled ones are left out because a second
    /// value for the same parameter is a fatal error, so offering it is offering a mistake -- and
    /// "already filled" means either named outright or covered by a positional argument before the
    /// cursor.
    ///
    /// A variadic is never offered: PHP has no way to name one.
    ///
    /// Empty when the callee cannot be resolved, which is the rule signature help follows too: the
    /// parameter names come from the declaration or from nowhere.
    pub fn free_parameters(&self, offset: TextSize) -> Vec<FreeParameter> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let Some(call) = phrost_body::call_site_at(hir, offset) else {
            return Vec::new();
        };
        let Some(params) = self.parameters_of(hir, &call) else {
            return Vec::new();
        };

        // The argument the cursor is inside does not count as filling anything: it is the one being
        // written, and leaving its parameter out would leave the list empty at the moment it is
        // wanted.
        let written = call
            .arguments
            .iter()
            .filter(|argument| !argument.range.contains_inclusive(offset));
        let mut named = Vec::new();
        let mut positional = 0_usize;
        for argument in written {
            match &argument.name {
                Some(name) => named.push(name.clone()),
                // A spread fills an unknown number of them, so nothing after it can be called free.
                None if argument.spread => return Vec::new(),
                None => positional += 1,
            }
        }

        params
            .iter()
            .enumerate()
            .filter(|(position, param)| {
                !param.is_variadic() && *position >= positional && !named.contains(param.name())
            })
            .map(|(_, param)| FreeParameter {
                name: param.name().clone(),
                ty: param.render(self.skeleton.types(), hir),
            })
            .collect()
    }
}

/// What the call writes before its parentheses, for the label.
fn callee_name(hir: &phrost_hir::FileHir, call: &CallSite) -> String {
    match &call.target {
        CallTarget::Member { name, .. } => name.as_str().to_owned(),
        CallTarget::Function(reference) => hir.name_ref(*reference).path.as_str().to_owned(),
        // What is written is the class; what has the parameters is its constructor, and naming the
        // class is what a reader is looking at.
        CallTarget::Constructor(_) => "__construct".to_owned(),
        CallTarget::Invoked(_) => "callable".to_owned(),
    }
}

/// The parameter the cursor is filling: the one its argument names, or else the one at its index.
fn active_parameter(
    call: &CallSite,
    params: &[crate::parameters::Parameter],
    offset: TextSize,
) -> Option<usize> {
    let inside = call
        .arguments
        .iter()
        .position(|argument| argument.range.contains_inclusive(offset));
    // Not inside any argument: just after the `(`, or after a comma with nothing typed yet. The
    // parameter being filled is the one after everything already written.
    let position = inside.unwrap_or_else(|| {
        call.arguments
            .iter()
            .filter(|argument| argument.range.end() <= offset)
            .count()
    });

    // A name settles it wherever the argument sits: `store(overwrite: true)` fills the third
    // parameter from the first position, and reading the index instead is how this pointed at the
    // first. A name no parameter answers to is a mistake in the call, and pointing at none of them
    // is what that deserves.
    if let Some(name) = inside
        .and_then(|at| call.arguments.get(at))
        .and_then(|argument| argument.name.as_ref())
    {
        return params.iter().position(|param| *param.name() == *name);
    }

    // Otherwise the index is the claim, and it only means something while everything before the
    // cursor was positional too -- PHP allows nothing positional after a named argument, so a call
    // being written into that state has no parameter for this position at all.
    let lines_up = call
        .arguments
        .iter()
        .take(position)
        .all(|argument| argument.name.is_none() && !argument.spread);
    if !lines_up {
        return None;
    }

    if position < params.len() {
        return Some(position);
    }
    // A variadic takes everything from its own position onward, so an argument past the end is
    // still filling it. Without one, there is no parameter for this argument at all.
    params
        .last()
        .is_some_and(|last| last.is_variadic())
        .then(|| params.len() - 1)
}
