//! What a call wrote, as the contracts need to read it.
//!
//! One struct and two free functions, apart from both the lookup and the contracts because it is
//! neither: it is the *question*, assembled once at the call site and handed to whatever answers.
//! Both halves of what settles a return arrive through it -- the arguments the call wrote and the
//! arguments the receiver was written with -- which is the seam a union bound two ways needs.

use phrost_hir::FileHir;

use crate::resolve::Held;

/// What a call proves about one of the values it was passed.
pub(crate) struct ArgumentAssertion {
    /// The type the tag names. Meaningless when [`Self::from_class_string`] is set: the name is a
    /// `@template` and the class is written at the call.
    pub(crate) ty: phrost_ty::TypeId,
    /// `!T` -- the value is *not* that once the call returns.
    pub(crate) negated: bool,
    /// `None` for an unconditional `@assert`, which holds however the call returned.
    pub(crate) when_true: Option<bool>,
    /// Which argument names the class, when the declaration only says where to look.
    pub(crate) from_class_string: Option<u16>,
}

/// The literal a call wrote for one parameter, positionally or under the parameter's name.
///
/// A positional argument answers for its own position, which is sound even beside named ones: PHP
/// requires every positional argument to come before the first named one. A named argument answers
/// for whichever parameter it names, and the declaration's list is the only thing that says which
/// position that is.
pub(crate) fn arguments_from<'c>(
    arguments: &'c [phrost_hir::body::LiteralArgument],
    params: &[phrost_sema::ParamRow],
    position: u16,
) -> Vec<&'c phrost_hir::body::LiteralArgument> {
    if !params
        .get(usize::from(position))
        .is_some_and(|param| param.is_variadic)
    {
        return argument_for(arguments, params, position)
            .into_iter()
            .collect();
    }
    arguments
        .iter()
        .filter(|argument| argument.name.is_none() && argument.position >= position)
        .collect()
}

pub(super) fn argument_for<'c>(
    arguments: &'c [phrost_hir::body::LiteralArgument],
    params: &[phrost_sema::ParamRow],
    position: u16,
) -> Option<&'c phrost_hir::body::LiteralArgument> {
    let wanted = params.get(usize::from(position)).map(|param| &param.name);
    arguments.iter().find(|argument| match &argument.name {
        None => argument.position == position,
        Some(written) => wanted.is_some_and(|name| name == written),
    })
}

/// The single expression an arrow function's body is, when the expression is a body of one.
pub(super) fn only_expression(
    hir: &FileHir,
    body: phrost_hir::BodyId,
) -> Option<phrost_hir::ExprId> {
    let [only] = hir.stmt_list(hir.body(body).statements) else {
        return None;
    };
    match hir.stmt(*only) {
        phrost_hir::body::StmtData::Return { value, .. } => *value,
        _ => None,
    }
}

/// What a call site says about itself, for a contract that reads the call rather than the signature.
///
/// One struct because the four travel together and mean nothing apart: the literals answer a
/// `class-string` test, the call answers what was passed and how much of it, and the budget is what
/// keeps resolving an argument from resolving forever.
#[derive(Clone, Copy)]
pub(crate) struct CallFacts<'a> {
    pub(crate) offset: phrost_model::TextSize,
    pub(crate) hir: &'a FileHir,
    pub(crate) literals: &'a [phrost_hir::body::LiteralArgument],
    pub(crate) call: phrost_body::CallSource,
    pub(crate) budget: u8,
    /// What the *receiver* was written with, for a template the declaring class supplies rather than
    /// the call.
    ///
    /// The bridge, and the reason it is on this struct: `@return TDefault|TInput` needs an argument's
    /// type for one operand and `InputBag<string>` for the other, and until both were in one place
    /// each resolver was complete for its own kind and blind to the other's -- so a union spanning
    /// them resolved in neither. Empty for a reader with no receiver, which is every free function.
    pub(crate) receiver: &'a [Held],
    /// The arguments as the *text* wrote them, for a reader that has no call expression.
    ///
    /// The completion chain scans its receiver out of characters -- the statement being typed does
    /// not parse -- so there is no call node to read, and a conditional return turns on exactly
    /// what a call wrote: `($key is null ? InputBag : mixed)` is settled by the parameter having
    /// been left out, and `($something is string ? Expectation : HigherOrderMessage)` by a quoted
    /// argument being there.
    ///
    /// Each entry is what that position was written with when it is a string -- a quoted literal or
    /// the class a `::class` named, both of which are strings at runtime -- and `None` for a
    /// position the scanner could not read. The whole field is `None` for a reader that has a call
    /// and for a step that is not one, which is how "a call with no arguments" stays apart from
    /// "not a call".
    pub(crate) written: Option<&'a [Option<Box<str>>]>,
}

impl CallFacts<'_> {
    /// How many arguments the call wrote, and `None` for a reader with no call expression.
    ///
    /// Named and spread arguments are counted too: what the count decides is that a parameter was
    /// *left out*, and one written under a name still fills it.
    pub(crate) fn arity(&self) -> Option<u16> {
        match self.arguments() {
            Some(arguments) => u16::try_from(arguments.len()).ok(),
            None => u16::try_from(self.written?.len()).ok(),
        }
    }

    /// The string one parameter was written with, for a reader that read the call out of text.
    ///
    /// Positional only: the scanner splits an argument list at its commas and refuses one written
    /// under a parameter's name, so a position is the whole of what it knows.
    pub(crate) fn written_string(&self, position: u16) -> Option<&str> {
        self.written?.get(usize::from(position))?.as_deref()
    }

    /// The strings a parameter was written with, for a reader that read the call out of text: the
    /// one at its position, or every one from there on for a variadic.
    ///
    /// Empty for a reader with a call expression, whose arguments are its literals instead -- two
    /// spellings of one call, and a reader of either asks both.
    pub(crate) fn written_strings_from(
        &self,
        params: &[phrost_sema::ParamRow],
        position: u16,
    ) -> Vec<&str> {
        let Some(written) = self.written else {
            return Vec::new();
        };
        let variadic = params
            .get(usize::from(position))
            .is_some_and(|param| param.is_variadic);
        written
            .iter()
            .skip(usize::from(position))
            .take(if variadic { usize::MAX } else { 1 })
            .filter_map(|argument| argument.as_deref())
            .collect()
    }

    /// The expression the call wrote for one parameter, positionally or under the parameter's name.
    pub(crate) fn written_argument(
        &self,
        params: &[phrost_sema::ParamRow],
        position: u16,
    ) -> Option<phrost_hir::ExprId> {
        let arguments = self.arguments()?;
        let wanted = params.get(usize::from(position)).map(|param| &param.name);
        for (index, &argument) in arguments.iter().enumerate() {
            // A named or spread argument is wrapped; a plain positional one is the value itself.
            let (name, value) = match self.hir.expr(argument) {
                phrost_hir::body::ExprData::Argument { name, value, .. } => (name.as_ref(), *value),
                _ => (None, argument),
            };
            let fills = match name {
                Some(written) => wanted.is_some_and(|name| name == written),
                None => index == usize::from(position),
            };
            if fills {
                return Some(value);
            }
        }
        None
    }

    /// Every argument one parameter takes, which for a variadic is all of them from its position on.
    ///
    /// `@param T ...$values` beside `@return T` is how `max` and `min` are written, and how a
    /// `class-string<T> ...$names` factory is: the declaration binds one template to a *list* of
    /// arguments, and what comes back is one of them. Reading only the argument at the parameter's
    /// own position answered from the first and ignored the rest, so `max(1, 'a')` was `int`.
    ///
    /// One argument for an ordinary parameter, so nothing that is not variadic changes.
    ///
    /// Each with whether it was written `...$values`. A spread passes what the value *holds*, one argument per element, so a reader binding a
    /// template to it wants the element: `array_merge(...$groups)` merges the groups, not the list
    /// of them.
    pub(super) fn written_arguments_from(
        &self,
        params: &[phrost_sema::ParamRow],
        position: u16,
    ) -> Vec<(phrost_hir::ExprId, bool)> {
        if !params
            .get(usize::from(position))
            .is_some_and(|param| param.is_variadic)
        {
            return self
                .written_argument(params, position)
                .into_iter()
                .map(|value| (value, false))
                .collect();
        }

        let Some(arguments) = self.arguments() else {
            return Vec::new();
        };
        // Positional only. A variadic's arguments have no names of their own -- a named argument
        // filling one is `...$values` spread, which names no single value.
        arguments
            .iter()
            .enumerate()
            .filter(|(index, _)| *index >= usize::from(position))
            .filter_map(|(_, &argument)| match self.hir.expr(argument) {
                phrost_hir::body::ExprData::Argument { name: Some(_), .. } => None,
                phrost_hir::body::ExprData::Argument { value, spread, .. } => {
                    Some((*value, *spread))
                }
                _ => Some((argument, false)),
            })
            .collect()
    }

    /// The arguments the call was given, when there is a call expression to read them from.
    pub(crate) fn arguments(&self) -> Option<&[phrost_hir::ExprId]> {
        let args = match self.hir.expr(self.call.expr()?) {
            phrost_hir::body::ExprData::FunctionCall { args, .. }
            | phrost_hir::body::ExprData::MethodCall { args, .. }
            | phrost_hir::body::ExprData::StaticCall { args, .. }
            | phrost_hir::body::ExprData::IndirectCall { args, .. }
            // A `new` is a call to a constructor, and its arguments are what bind a class's
            // templates.
            | phrost_hir::body::ExprData::New { args, .. } => *args,
            _ => return None,
        };
        Some(self.hir.expr_list(args))
    }
}
