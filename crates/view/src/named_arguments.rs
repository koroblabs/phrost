//! A parameter a call names: `$repo->find(id: 5)`.
//!
//! Since PHP 8.0 a parameter's name is part of how a function is called, and a call naming one the
//! callee does not have stops with `Unknown named parameter $id`. A rename of the parameter that
//! edited the declaration and the body and left the label at every call site turned working code
//! into that error -- and the reference project writes some thirteen hundred named arguments.
//!
//! The label is matched by the declaration the call reaches, as an access is: `find(id:)` on a
//! `Repo` and on an unrelated class with its own `find` are two parameters.

use phrost_body::BindingKind;
use phrost_model::{Name, TextRange, TextSize};
use phrost_sema::ParamRow;

use crate::{location::SymbolLocation, view::IndexView};

/// A function's or a method's parameter, identified as a call reaching it resolves the callee.
///
/// By name and not by position or range: the label a call writes is the name, and the declaration
/// is found again in whichever file a reader is over -- which is what lets a search start from a
/// label in one file and edit the parameter in another without the index storing where any
/// parameter is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NamedParameter {
    /// The class declaring the method, `None` for a free function.
    pub container: Option<Name>,
    /// The method's name, or the function's fully qualified one.
    pub callable: Name,
    /// Without the `$`, as a call's label writes it.
    pub name: Name,
}

impl IndexView<'_> {
    /// The parameter the local under the cursor is, when it is a function's or a method's own.
    ///
    /// Not a closure's: nothing calls a closure by a name it could check a label against, and its
    /// parameters are locals like any other.
    pub fn named_parameter_at(&self, offset: TextSize) -> Option<NamedParameter> {
        let variable = self.variable_at(offset)?;
        let (declared, kind) = variable.declared_at?;
        if kind != BindingKind::Parameter {
            return None;
        }
        let at = self
            .signature_parameters()
            .find(|at| at.intersect(declared).is_some())?;
        let owner = self.enclosing_declaration(at.start())?;
        Some(NamedParameter {
            container: owner.container,
            callable: owner.fqn,
            name: variable.name,
        })
    }

    /// The parameter a call's label under the cursor names, and the label: `id` in `find(id: 5)`.
    ///
    /// Only where the callee has a parameter of that name. A label naming none is the call's own
    /// mistake -- PHP stops on it -- and answering for it would send goto to a declaration that does
    /// not take what the call passes.
    pub fn named_argument_at(&self, offset: TextSize) -> Option<(NamedParameter, TextRange)> {
        let open = self.current()?;
        let hir = open.hir();
        // From the characters first, because hover, goto and a highlight all ask this at every
        // position and the calls of a file are a walk over all of it: a label is a word with a
        // single `:` after it, which almost nothing under a cursor is.
        let text = open.text();
        let at = usize::from(offset).min(text.len());
        let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
        let end = at
            + text.as_bytes()[at..]
                .iter()
                .take_while(|byte| is_word(**byte))
                .count();
        let after = text[end..].trim_start();
        if end == at && !text.as_bytes()[..at].last().copied().is_some_and(is_word)
            || !after.starts_with(':')
            || after.starts_with("::")
        {
            return None;
        }
        let calls = self.file_calls();
        let (call, name, label) = calls.iter().find_map(|call| {
            call.arguments.iter().find_map(|argument| {
                let name = argument.name.as_ref()?;
                let label = TextRange::at(argument.range.start(), TextSize::of(name.as_str()));
                label
                    .contains_inclusive(offset)
                    .then(|| (call, name.clone(), label))
            })
        })?;
        let callee = self.callee_of(hir, call)?;
        let parameter = NamedParameter {
            container: callee.container,
            callable: callee.fqn,
            name,
        };
        self.declared_parameter(&parameter)?;
        Some((parameter, label))
    }

    /// The class whose property the parameter declares, when its constructor promotes it -- which
    /// makes the label a spelling of the *property*: renaming it renames `$money->amount` too.
    pub fn promoted_by(&self, parameter: &NamedParameter) -> Option<Name> {
        if !parameter
            .callable
            .as_str()
            .eq_ignore_ascii_case("__construct")
        {
            return None;
        }
        self.declared_parameter(parameter)?.promotion?;
        parameter.container.clone()
    }

    /// Where the parameter's function or method is declared.
    pub fn parameter_declaration(&self, parameter: &NamedParameter) -> Option<SymbolLocation> {
        match &parameter.container {
            Some(class) => self.member_location(
                class.as_str(),
                phrost_sema::MemberKind::Method,
                parameter.callable.as_str(),
            ),
            None => self.function_location(parameter.callable.as_str()),
        }
    }

    /// The parameter as its declaration writes it, `bool $fresh = false`.
    pub fn parameter_signature(&self, parameter: &NamedParameter) -> Option<String> {
        let mut out = String::new();
        self.declared_parameter(parameter)?
            .write(&mut out, self.skeleton.types());
        Some(out)
    }

    /// Every place in the current file meaning `parameter`: its own occurrences where it is
    /// declared, docblock included, and every label naming it at a call that reaches the
    /// declaration.
    pub fn parameter_uses(&self, parameter: &NamedParameter) -> Vec<TextRange> {
        let mut found = Vec::new();
        // The declaration, when it is in this file: found again by name and by the declaration it
        // belongs to, the way a sweep reading this file without the cursor's has to.
        let declared_here = self.current().and_then(|open| {
            let text = open.text();
            self.signature_parameters().find(|at| {
                text.get(usize::from(at.start())..usize::from(at.end()))
                    .is_some_and(|written| {
                        written.trim_start_matches('$') == parameter.name.as_str()
                    })
                    && self.enclosing_declaration(at.start()).is_some_and(|owner| {
                        declared_as(
                            &owner,
                            parameter.container.as_ref().map(Name::as_str),
                            parameter.callable.as_str(),
                        )
                    })
            })
        });
        if let Some(at) = declared_here {
            found = self
                .local_occurrences(at.start(), parameter.name.as_str())
                .into_iter()
                .map(|occurrence| occurrence.range)
                .collect();
            let mentions = self.docblock_mentions(parameter.name.as_str(), &found);
            found.extend(mentions);
        }
        found.extend(self.labels_naming(
            parameter.container.as_ref().map(Name::as_str),
            parameter.callable.as_str(),
            parameter.name.as_str(),
        ));
        found.sort_by_key(|range| range.start());
        found.dedup();
        found
    }

    /// Where every parameter of a method or a function in the current file is written -- a
    /// signature's, never a closure's.
    fn signature_parameters(&self) -> impl Iterator<Item = TextRange> + '_ {
        let hir = self.current().map(|open| open.hir());
        let methods = hir.into_iter().flat_map(|hir| {
            hir.classes()
                .flat_map(|(_, class)| hir.member_list(class.members).iter().copied())
                .map(|id| hir.member(id))
                .filter(|member| member.kind == phrost_hir::MemberKind::Method)
                .flat_map(|member| hir.param_list(member.params).iter().copied())
                .map(|param| hir.param(param).name_range)
        });
        let functions = hir.into_iter().flat_map(|hir| {
            hir.functions()
                .flat_map(|(_, function)| hir.param_list(function.params).iter().copied())
                .map(|param| hir.param(param).name_range)
        });
        methods.chain(functions)
    }

    /// The stored row of the parameter, when its declaration has one by that name.
    fn declared_parameter(&self, parameter: &NamedParameter) -> Option<ParamRow> {
        let params = match &parameter.container {
            Some(class) => self
                .members(class.as_str())
                .find(phrost_sema::MemberKind::Method, parameter.callable.as_str())?
                .row
                .params
                .clone(),
            None => self.function_params(parameter.callable.as_str())?,
        };
        params
            .iter()
            .find(|param| param.name.as_str() == parameter.name.as_str())
            .cloned()
    }

    /// The same for a property `class`'s constructor promotes: `amount` in `new Money(amount: 5)`
    /// names the parameter that declares the property, so renaming the property renames the label.
    /// Empty for a property no constructor promotes. Read off the member table, so it answers in
    /// every file a sweep reads and not only the declaring one.
    pub fn named_arguments_to_promoting(&self, class: &str, property: &str) -> Vec<TextRange> {
        let Some(constructor) =
            self.member_location(class, phrost_sema::MemberKind::Method, "__construct")
        else {
            return Vec::new();
        };
        let promotes = self
            .members(class)
            .find(phrost_sema::MemberKind::Method, "__construct")
            .is_some_and(|entry| {
                entry
                    .row
                    .params
                    .iter()
                    .any(|param| param.promotion.is_some() && param.name.as_str() == property)
            });
        if !promotes {
            return Vec::new();
        }
        self.labels_naming(
            constructor.container.as_ref().map(Name::as_str),
            constructor.fqn.as_str(),
            property,
        )
    }

    fn labels_naming(
        &self,
        container: Option<&str>,
        callable: &str,
        wanted: &str,
    ) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let text = open.text();
        self.file_calls()
            .iter()
            .filter_map(|call| {
                // By the label first, which almost no call writes; the callee is the expensive half.
                let argument = call.arguments.iter().find(|argument| {
                    argument
                        .name
                        .as_ref()
                        .is_some_and(|name| name.as_str() == wanted)
                })?;
                let callee = self.callee_of(hir, call)?;
                if !declared_as(&callee, container, callable) {
                    return None;
                }
                let label = TextRange::at(argument.range.start(), TextSize::of(wanted));
                (text.get(usize::from(label.start())..usize::from(label.end())) == Some(wanted))
                    .then_some(label)
            })
            .collect()
    }
}

/// Whether a call resolving to `callee` is a call of the declaration `container::callable` -- by
/// the rules PHP compares those names by, case aside for both.
fn declared_as(callee: &SymbolLocation, container: Option<&str>, callable: &str) -> bool {
    let same = |a: &str, b: &str| {
        a.trim_start_matches('\\')
            .eq_ignore_ascii_case(b.trim_start_matches('\\'))
    };
    same(callee.fqn.as_str(), callable)
        && match (&callee.container, container) {
            (Some(called), Some(declared)) => same(called.as_str(), declared),
            (None, None) => true,
            _ => false,
        }
}

/// A label PHP refuses at the call: `Unknown named parameter $idd`, or `Named parameter $id
/// overwrites previous argument`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MisnamedArgument {
    /// The label, which is the word that is wrong.
    pub range: TextRange,
    pub name: Name,
    pub problem: Misnamed,
    /// For an unknown label, the parameter it is one edit from that nothing else filled -- the one
    /// it plainly meant. Nothing when two are as near, which would be a guess.
    pub nearest: Option<Name>,
}

/// Which of PHP's two refusals a label meets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Misnamed {
    /// The callee has no parameter of that name and no variadic to collect it.
    Unknown,
    /// A positional argument or an earlier label already filled it.
    Twice,
}

impl IndexView<'_> {
    /// Every label in the current file that PHP refuses when the call runs.
    ///
    /// Only where the names compared are the ones PHP compares. A stub's parameter names are the
    /// stub author's and may not be the engine's, so a built-in callee is never judged; neither is
    /// one a `@method` tag declares or a `@mixin` forwards, which a `__call` answers whatever the
    /// label says; nor a call past a spread, which may supply names of its own. `missing-argument`
    /// speaks about such a call too, with the real cause one step away -- `find(idd: 5)` is missing
    /// `$id` only because the label is wrong -- so a call reported here is left to this.
    pub fn misnamed_arguments(&self) -> Vec<MisnamedArgument> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let text = open.text();
        let mut found = Vec::new();
        for call in self.file_calls().iter() {
            // Almost no call names an argument, and nothing below is asked of one that does not.
            if call
                .arguments
                .iter()
                .all(|argument| argument.name.is_none())
                || call.arguments.iter().any(|argument| argument.spread)
                || matches!(call.target, phrost_body::CallTarget::Invoked(_))
            {
                continue;
            }
            let Some(callee) = self.callee_of(hir, call) else {
                continue;
            };
            let judged =
                callee.container.is_some() || callee.kind == phrost_model::SymbolKind::Function;
            let stubbed = self
                .skeleton
                .file(callee.file)
                .is_some_and(|file| file.origin() == phrost_model::SourceOrigin::Stub);
            if !judged
                || stubbed
                || callee.from_docblock
                || self.callee_takes_more_than_it_names(hir, call)
            {
                continue;
            }
            let Some(params) = self.parameters_of(hir, call) else {
                continue;
            };
            let collects = params.iter().any(|param| param.is_variadic());

            let mut positional = 0_usize;
            let mut named = Vec::<&Name>::new();
            for argument in &call.arguments {
                let Some(name) = argument.name.as_ref() else {
                    positional += 1;
                    continue;
                };
                let filled = params
                    .iter()
                    .position(|param| param.name().as_str() == name.as_str());
                let problem = match filled {
                    Some(at) if at < positional || named.contains(&name) => Some(Misnamed::Twice),
                    Some(_) => None,
                    None if collects => None,
                    None => Some(Misnamed::Unknown),
                };
                named.push(name);
                let nearest = match problem {
                    Some(Misnamed::Unknown) => {
                        nearest_unfilled(&params, positional, &call.arguments, name)
                    }
                    _ => None,
                };
                let label = TextRange::at(argument.range.start(), TextSize::of(name.as_str()));
                if let Some(problem) = problem
                    && text.get(usize::from(label.start())..usize::from(label.end()))
                        == Some(name.as_str())
                {
                    found.push(MisnamedArgument {
                        range: label,
                        name: name.clone(),
                        problem,
                        nearest,
                    });
                }
            }
        }
        found
    }
}

/// The parameter a misspelt label was one edit from, among those the call left unfilled.
///
/// Unfilled, because a label renamed onto a parameter something already filled is the other error.
/// And the same near-miss rule a misspelt shape key follows: one edit, and nothing when two tie.
fn nearest_unfilled(
    params: &[crate::parameters::Parameter],
    positional: usize,
    arguments: &[phrost_body::ArgumentSite],
    written: &Name,
) -> Option<Name> {
    let mut best: Option<(&Name, usize)> = None;
    let mut tied = false;
    for (at, param) in params.iter().enumerate() {
        let filled = at < positional
            || arguments.iter().any(|argument| {
                argument
                    .name
                    .as_ref()
                    .is_some_and(|name| name.as_str() == param.name().as_str())
            });
        if filled {
            continue;
        }
        let Some(distance) = phrost_index::edits_within(
            written.as_str(),
            param.name().as_str(),
            phrost_index::NEAR_MISS_EDITS,
        ) else {
            continue;
        };
        match best {
            Some((_, held)) if distance > held => {}
            Some((_, held)) if distance == held => tied = true,
            _ => {
                best = Some((param.name(), distance));
                tied = false;
            }
        }
    }
    best.filter(|_| !tied).map(|(name, _)| name.clone())
}
