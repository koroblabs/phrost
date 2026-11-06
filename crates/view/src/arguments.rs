//! Calls that cannot run: the wrong number of arguments for the callee's declaration.
//!
//! The narrowest claim a checker can make about a call, and one PHP settles at runtime with an
//! `ArgumentCountError`. It needs the callee's declaration, which is in another file -- so unlike
//! nearly everything a per-file linter can say, this one is only answerable here.
//!
//! Two shapes, both fatal on every run that reaches the line: too few arguments, and now too many.
//! Both decline past a spread (`f(...$args)`), because an array's length is not known here and
//! guessing would turn "the count is unknowable" into a wrong count. Both decline once a call writes
//! a named argument, too: past one, position has stopped lining up with the declaration at all, the
//! same place [`crate::signature::active_parameter`] gives up for the same reason -- there is no
//! second rule here for what that comment already settled. And a variadic parameter has no upper
//! bound to check against at all: it takes any number of arguments from its own position on.
//!
//! What "too many" also does not check, and why: a body that calls `func_get_args()` treats its
//! declared parameter list as a lower bound rather than a full contract, and PHP raises nothing
//! when a caller supplies more. Whether a declaration does that is not visible from here -- the
//! index keeps a declaration's *signature*, never its body, which is also why a closed or vendor
//! file's statements are never retained -- so lowering records it and
//! [`IndexView::callee_takes_more_than_it_names`](crate::view::IndexView) reads it back. Asked only of a
//! call this has already decided is too long, because it costs a second resolution of the callee
//! and almost no call needs the answer.
//!
//! Laravel's `PendingRequest::throwIf($condition)` is the shape:
//! `return $condition ? $this->throw(func_get_args()[1] ?? null) : $this;`, called with two
//! arguments by its own tests. This module doc used to call that a theoretical risk on the strength
//! of one corpus where it did not appear -- and typing a closure's parameter made two such sites
//! reachable, which was reason enough to record the fact and is not the size of the problem.
//! Measured once it existed: **373 findings removed** -- 222 on Laravel, 140 on Symfony, 11 on
//! WordPress, every one an `extra-argument` and nothing added anywhere. Almost all of them are one
//! idiom, `$keys = is_array($keys) ? $keys : func_get_args();`, which is how a framework spells "an
//! array or a list of arguments" and which no signature can express. The reference project has
//! none, which is exactly why the first corpus said the risk was theoretical: it is a fact about a
//! *dialect*, and a single corpus cannot tell one of those from a fact about PHP.

use crate::parameters::Parameter;
use phrost_body::{ArgumentSite, CallSite, CallTarget};
use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, TextRange};

use crate::variables::Naming;
use crate::view::IndexView;

/// One call with nothing supplied for a parameter that has no default.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MissingArguments {
    /// The whole call, which is what a report underlines: the mistake is the call rather than any
    /// one argument, and there is no argument at the place where the missing one would go.
    pub range: TextRange,
    /// The callee as written, for the message.
    pub callee: String,
    /// The parameters left with nothing, in declared order.
    pub missing: Vec<phrost_model::Name>,
    pub required: usize,
    pub given: usize,
}

impl IndexView<'_> {
    /// Every call in the current file that supplies too few arguments.
    ///
    /// Silent about a call whose callee it cannot resolve, and about one it cannot count: a spread
    /// stands for however many values are in an array, and nothing here knows how many that is.
    pub fn calls_missing_arguments(&self) -> Vec<MissingArguments> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        // A call naming a parameter wrongly is missing the right one only because of it -- `find(idd:
        // 5)` leaves `$id` empty -- and the label is the mistake a reader has to fix. One report.
        let misnamed = self.misnamed_arguments();

        self.file_calls()
            .iter()
            .filter(|call| {
                !misnamed
                    .iter()
                    .any(|wrong| call.range.contains_range(wrong.range))
            })
            .filter_map(|call| {
                let params = self.parameters_of(hir, call)?;
                missing_in(hir, call, &params)
            })
            .collect()
    }
}

/// What one call leaves unfilled, or nothing when it fills everything or cannot be counted.
fn missing_in(hir: &FileHir, call: &CallSite, params: &[Parameter]) -> Option<MissingArguments> {
    // Neither count is claimed about a call through a value, and PHP is why in both directions.
    // Extra arguments to anything user-defined are *accepted* -- measured on 8.5.10, a function, a
    // closure and a method all ignore them -- so the upper bound was never a runtime claim, and for
    // a closure it is a claim about a callee identified through a chain of assignments, which is
    // the least certain callee there is. `$response = $callback; while ($response instanceof
    // Closure) { $response = $response($request, $options); }` in Laravel's own `Http\Factory` is
    // the shape: the literal the chain lands on declares no parameters and the call passes two.
    // The type checks degrade to silence on a wrong identification; a count check asserts.
    if matches!(call.target, CallTarget::Invoked(_)) {
        return None;
    }

    // A spread is however many values the array holds, which is not knowable here. Declining is the
    // whole difference between this check and a guess.
    if call.arguments.iter().any(|argument| argument.spread) {
        return None;
    }

    let positional = call
        .arguments
        .iter()
        .filter(|argument| argument.name.is_none())
        .count();
    let named = call
        .arguments
        .iter()
        .filter_map(|argument| argument.name.as_ref())
        .collect::<Vec<_>>();

    // Declared order, because that is the order a reader will read the message in -- and because a
    // positional argument fills the parameter at its own index.
    let missing = params
        .iter()
        .enumerate()
        .filter(|(_, param)| !param.has_default() && !param.is_variadic())
        .filter(|(at, param)| {
            *at >= positional
                && !named
                    .iter()
                    .any(|given| given.as_str() == param.name().as_str())
        })
        .map(|(_, param)| param.name().clone())
        .collect::<Vec<_>>();

    if missing.is_empty() {
        return None;
    }

    Some(MissingArguments {
        range: call.range,
        callee: callee_of(hir, call),
        required: params
            .iter()
            .filter(|param| !param.has_default() && !param.is_variadic())
            .count(),
        given: call.arguments.len(),
        missing,
    })
}

/// One call with more arguments than the callee's declaration has room for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtraArguments {
    /// The extra arguments themselves: from the first one the declaration has no parameter for
    /// through the last one written. Not the call as a whole -- everything up to here is fine, and
    /// underlining the call would blame arguments that are correct along with the ones that are not.
    pub range: TextRange,
    /// The callee as written, for the message.
    pub callee: String,
    /// How many the declaration takes, at most.
    pub allowed: usize,
    pub given: usize,
}

impl IndexView<'_> {
    /// Every call in the current file that supplies more arguments than its callee has parameters
    /// for.
    ///
    /// Silent about everything [`Self::calls_missing_arguments`] is silent about, and for the same
    /// reasons -- an unresolved callee, a spread, a named argument -- plus one more that only this
    /// direction has: a variadic parameter takes any number of arguments from its own position on,
    /// so a declaration ending in one has no upper bound to check against at all.
    pub fn calls_extra_arguments(&self) -> Vec<ExtraArguments> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        self.file_calls()
            .iter()
            .filter_map(|call| {
                let params = self.parameters_of(hir, call)?;
                let extra = extra_in(hir, call, &params)?;
                // Asked only of a call that would otherwise be reported, which is what makes a
                // second resolution of the callee affordable -- see `callee_takes_more_than_it_names`.
                match self.callee_takes_more_than_it_names(hir, call) {
                    true => None,
                    false => Some(extra),
                }
            })
            .collect()
    }
}

/// What one call supplies past the callee's last parameter, or nothing when it fits, cannot be
/// counted, or has no upper bound to exceed.
fn extra_in(hir: &FileHir, call: &CallSite, params: &[Parameter]) -> Option<ExtraArguments> {
    // See `missing_in`: neither count is claimed about a call through a value.
    if matches!(call.target, CallTarget::Invoked(_)) {
        return None;
    }

    // A spread's length is not knowable here, same as for the lower bound. A named argument means
    // position has stopped corresponding to a parameter at all -- the same place
    // `active_parameter` gives up, and for the same reason: past one, there is no second rule to
    // invent for what a bare position means.
    if call
        .arguments
        .iter()
        .any(|argument| argument.spread || argument.name.is_some())
    {
        return None;
    }

    // A variadic parameter absorbs everything from its own position on, so there is no count of
    // arguments that could be "too many".
    if params.iter().any(|param| param.is_variadic()) {
        return None;
    }

    let given = call.arguments.len();
    let allowed = params.len();
    if given <= allowed {
        return None;
    }

    // `given > allowed`, so at least one argument sits at index `allowed` or past it.
    let first_extra = &call.arguments[allowed];
    let last_extra = call.arguments.last().expect("given > allowed >= 0");

    Some(ExtraArguments {
        range: TextRange::new(first_extra.range.start(), last_extra.range.end()),
        callee: callee_of(hir, call),
        allowed,
        given,
    })
}

/// An argument that may be a sentinel where the parameter cannot take one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SentinelArgument {
    /// The argument as written, which is the expression to go and guard.
    pub range: TextRange,
    pub parameter: phrost_model::Name,
    /// The type the parameter declares, rendered.
    pub declared: String,
    /// Whether the value is the sentinel on every run or only on some.
    pub certain: bool,
    /// Which sentinel: [`BuiltinTy::Null`] or [`BuiltinTy::False`].
    ///
    /// The vocabulary the reach walk already speaks -- see `IndexView::type_admits` -- rather than
    /// a second enum saying the same two things. A third value is not expressible and should not
    /// be: these two are what PHP's own library hands back to mean *not there*.
    pub sentinel: BuiltinTy,
}

impl IndexView<'_> {
    /// Every argument in the current file passed where the parameter does not admit null.
    ///
    /// The same claim as a null receiver, from the other side of a call: PHP raises a `TypeError`,
    /// so it is the branch nobody tested rather than a style note. Liveable for the same reason --
    /// the layer below narrows, so a guarded value has already had the null taken off it.
    ///
    /// Silent where the answer is not knowable or not a fact:
    ///
    /// - a parameter with no declared type, or `mixed`, takes anything;
    /// - a parameter that already admits null has nothing to say about;
    /// - a parameter *with a default* is declined outright, because `int $x = null` is implicitly
    ///   nullable in PHP and the default's value is not stored here -- so this cannot tell that case
    ///   from `int $x = 0`, and guessing would report the idiom older framework code is full of;
    /// - a named argument, a spread, or a variadic breaks the pairing, which [`bind`] already stops at.
    ///
    /// # The same question about `false`
    ///
    /// `strpos()`, `reset()`, `file_get_contents()` and their neighbours hand back `false` to mean
    /// *not there*, exactly as another half of the library hands back null, and an unguarded one
    /// reaching a parameter is the same mistake. Asked here rather than in its own walk, so the two
    /// halves of one rule cannot drift -- the shape §8 names, and the receiver check already reads
    /// both reaches off one site for the same reason.
    ///
    /// Null is asked first and wins, because a value that may be both is one mistake and not two.
    ///
    /// What a `false` costs that a null does not is that weak mode converts it: `false` reaches a
    /// `string` parameter as `""` and an `int` one as `0`, with no error at all. That conversion is
    /// exactly what makes it worth reporting rather than a reason not to -- `base64_encode(false)`
    /// encodes the empty string and says nothing -- and it is why the judgement in
    /// [`Self::incompatible_arguments`] cannot answer here: it asks what PHP rejects, and PHP
    /// accepts this.
    pub fn sentinel_arguments(&self) -> Vec<SentinelArgument> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let types = self.skeleton.types();
        let mut out = Vec::new();

        for call in self.file_calls().iter() {
            let Some((params, templates)) = self.parameters_and_templates_of(hir, call) else {
                continue;
            };

            for (argument, param) in paired(&call.arguments, &params) {
                // A spread is however many values an array holds, and what any one of them is is
                // not knowable here.
                if argument.spread {
                    break;
                }
                // A variadic ends the pairing; a *defaulted* parameter does not. It used to end
                // it here and nowhere else -- `incompatible_arguments` pairs the same arguments
                // against the same parameters and has never stopped at one -- so every argument
                // from the first optional parameter onwards went unchecked for a sentinel.
                // `date('F jS', strtotime($when))` is the shape: `strtotime` gives `int|false`,
                // `date`'s second parameter is `?int $timestamp = null`, and nothing was said.
                // Measured against PHPStan on the reference checkout, 13 of its `argument.type`
                // findings are exactly that pair.
                if param.is_variadic() {
                    break;
                }
                // Overwritten, not read -- see `overwritten_by_the_runtime`.
                if self.overwritten_by_the_runtime(hir, call, param) {
                    continue;
                }
                // With the receiver's template arguments put in place, and `None` -- which is
                // silence -- where nothing bound them. A bare `T` is not a type to judge against:
                // `@template TValue` on an interface with `@param TValue|null $value` on its method
                // reads as "takes null and not false", so `transform(FALSE)` was reported as
                // passing a `false` where a class nothing declares was wanted. The judgement in
                // `incompatible_arguments` has asked this since templates existed and this one
                // never did -- two constructors of one rule, with `bound_expectation` as the seam
                // that now serves both.
                //
                // And where the docblock that won puts a template inside a union or a
                // `class-string<T>` -- `@param class-string<T>|T` beside `object|string` on
                // `ReflectionClass` -- the hint beside it: no substitution can rebuild that as one
                // type, and the hint is never narrower than a docblock it lost to, so a sentinel
                // it admits is one the docblock admits too. Silence there cost every
                // `new ReflectionClass($maybeNull)`.
                let written = param.expected(types);
                let Some(expected) = self.sentinel_expectation(param, templates.as_ref()) else {
                    continue;
                };
                let takes = |sentinel| self.parameter_takes(hir, param, expected, sentinel);
                if takes(BuiltinTy::Null) && takes(BuiltinTy::False) {
                    continue;
                }

                let at = argument.range.start();
                let Some(path) = phrost_body::value_path_of(hir, argument.value) else {
                    continue;
                };
                let Some(held) = self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT)
                else {
                    continue;
                };

                // Null first, so a value that may be either is one report about the shape a reader
                // will guard anyway.
                let found = [BuiltinTy::Null, BuiltinTy::False]
                    .into_iter()
                    .filter(|sentinel| !takes(*sentinel))
                    .find_map(|sentinel| {
                        let reach = match sentinel {
                            BuiltinTy::Null => self.null_reach_of(hir, &held),
                            _ => self.false_reach_of(hir, &held),
                        };
                        match reach {
                            crate::references::SentinelReach::Impossible => None,
                            crate::references::SentinelReach::Possible => Some((sentinel, false)),
                            crate::references::SentinelReach::Certain => Some((sentinel, true)),
                        }
                    });
                let Some((sentinel, certain)) = found else {
                    continue;
                };

                // What the parameter takes *here*, where a receiver bound a template in it -- the
                // judgement above was made against that, and `$key takes \TKey` names nothing a
                // reader can act on. The declaration as written everywhere else, which keeps a
                // default's implied null and a docblock beside the hint in the sentence.
                let declared = match written != Some(expected) {
                    true => expected.render(types, hir),
                    false => match param.render(types, hir) {
                        Some(declared) => declared,
                        None => continue,
                    },
                };
                out.push(SentinelArgument {
                    range: argument.range,
                    parameter: param.name().clone(),
                    declared,
                    certain,
                    sentinel,
                });
            }
        }

        out
    }

    /// What a parameter is judged against for a sentinel: the docblock or hint that won, with the
    /// receiver's template arguments put in place, and the hint where the docblock cannot be
    /// rebuilt as one type. `None` -- silence -- where a template stays unbound.
    fn sentinel_expectation(
        &self,
        param: &Parameter,
        templates: Option<&crate::hints::CalleeTemplates>,
    ) -> Option<crate::compat::Expectation> {
        let types = self.skeleton.types();
        param
            .expected(types)
            .and_then(|expected| self.bound_expectation(templates, expected))
            .or_else(|| {
                param
                    .native()
                    .and_then(|native| self.bound_expectation(templates, native))
            })
    }

    /// Whether a parameter takes a sentinel, which decides whether passing one is a finding at all.
    ///
    /// One rule for both checks that judge an argument, because they divide the work along it: a
    /// value that may be null or false where the parameter takes neither belongs to
    /// [`Self::sentinel_arguments`], and everything else to [`Self::incompatible_arguments`]. Each
    /// used to ask its own question, and a `?int` handed to a `?string` fell between them -- the
    /// sentinel check had nothing to say because the null was welcome, and the type check declined
    /// anything null-ish as the other check's.
    ///
    /// PHP makes a parameter that defaults to `null` implicitly nullable whatever its hint says --
    /// deprecated on 8.4 and still in force, and written that way all over the bundled stubs. So the
    /// default is part of what the parameter takes.
    fn parameter_takes(
        &self,
        hir: &FileHir,
        param: &Parameter,
        expected: crate::compat::Expectation,
        sentinel: BuiltinTy,
    ) -> bool {
        self.expectation_admits(hir, expected, sentinel)
            || (sentinel == BuiltinTy::Null
                && param
                    .default()
                    .is_some_and(|written| written.trim().eq_ignore_ascii_case("null")))
    }
}

/// Which parameter each argument fills, for as far as the correspondence can be followed.
///
/// One rule and one place, because there were four: three type-judging walks each wrote
/// `if argument.name.is_some() { break; }` and stopped at the first named argument, which means
/// every check that judges what a call passes went blind at `deleteRecursive(path: $x)`. PHP has
/// paired arguments by name since 8.0 and the pairing is not hard -- positional arguments fill
/// parameters in order, a named one fills the parameter it names, and PHP's grammar forbids a
/// positional argument after a named one, so the two never have to be reconciled.
///
/// Measured before it existed: three of the sentinel arguments the reference project's own PHPStan
/// reports, `fileUris:` twice and `clientId:` once, and the direction PHP 8 code is moving.
///
/// What it still declines, and each is an absence of knowledge rather than a policy:
///
/// - a **spread**, which stands for however many values an array holds. The pair is yielded, so a
///   caller with a rule for `...$rest` into a variadic can apply it, and nothing after it is --
///   every position past a spread is unknown.
/// - a **name no parameter has**, which PHP collects into a variadic or raises an `Error` for.
///   Nothing is yielded for it and the arguments after it are still paired, because a name carries
///   its own position and one bad name does not move the others.
fn paired<'a>(
    arguments: &'a [ArgumentSite],
    params: &'a [Parameter],
) -> Vec<(&'a ArgumentSite, &'a Parameter)> {
    let mut out = Vec::new();
    let mut position = 0;
    for argument in arguments {
        let param = match &argument.name {
            // Exactly, and not ignoring case: PHP matches a named argument to a parameter by a
            // case-sensitive name, which is the one place in the language where a declaration's
            // spelling is load-bearing.
            Some(name) => params
                .iter()
                .find(|param| param.name().as_str() == name.as_str()),
            None => {
                let found = params.get(position);
                position += 1;
                found
            }
        };
        let Some(param) = param else {
            // A name nothing declares says nothing about this call; a position past the last
            // parameter ends the pairing, and `extra_in` is the check that speaks about it.
            match argument.name {
                Some(_) => continue,
                None => return out,
            }
        };
        let spread = argument.spread;
        out.push((argument, param));
        if spread {
            return out;
        }
    }
    out
}

/// One `&` argument whose place cannot hold what the callee may write back.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WrittenBack {
    /// The argument as written, which is the expression a reader has to go and look at.
    pub range: TextRange,
    /// The class declaring the property, and the property -- which promise is being broken.
    pub owner: phrost_model::Name,
    pub property: phrost_model::Name,
    /// What the property is declared to hold, rendered.
    pub declared: String,
    /// The `&` parameter, and what it may put there.
    pub parameter: phrost_model::Name,
    pub writes: String,
}

/// An argument whose type cannot be what the parameter declares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncompatibleArgument {
    /// The argument as written, which is the expression to go and change.
    pub range: TextRange,
    pub parameter: phrost_model::Name,
    /// The type the parameter declares, rendered.
    pub declared: String,
    /// The type the value has, rendered.
    pub given: String,
    /// Whether the argument was spread into a variadic, so a message can say which end is wrong.
    pub spread: bool,
}

impl IndexView<'_> {
    /// Every argument in the current file whose type no value of the parameter's type can be.
    ///
    /// Only what the judgement can *prove* wrong. `Fit::Unknown` -- which is most of what it says
    /// about real code -- is silence: see the policy in `crates/ty/src/compat/mod.rs`, which refuses
    /// to reject anything broader than what was asked for.
    ///
    /// Silent, additionally, where another check owns the finding or the pairing has broken:
    ///
    /// - **a null or false the parameter does not take is left to [`Self::sentinel_arguments`]**.
    ///   The judgement rejects a nullable against a non-nullable too, and that check already reports
    ///   202 of them on one real project -- so without this the two would double every one of them
    ///   on the first run. A null the parameter *does* take is set aside and the rest judged, since
    ///   that check has nothing to say about it;
    /// - a parameter with no declared type, or `mixed`, takes anything;
    /// - a named argument, past which position means nothing, which [`bind`] also stops at;
    /// - a spread against a *non*-variadic, where how many values arrive is unknown.
    ///
    /// A parameter *with a default* stays in scope, unlike in `sentinel_arguments`: that check declines
    /// one because `int $x = null` is implicitly nullable and the default is not stored, which is a
    /// fact about null and does not extend to `needs_string($anInt)`.
    ///
    /// What a parameter accepts is [`ParamRow::rejects`]'s question, not this one's: a `@param`
    /// wider than the native hint states what the callee handles, and an argument fitting either is
    /// compatible.
    pub fn incompatible_arguments(&self) -> Vec<IncompatibleArgument> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let types = self.skeleton.types();
        let mut out = Vec::new();

        for call in self.file_calls().iter() {
            let Some((params, templates)) = self.parameters_and_templates_of(hir, call) else {
                continue;
            };

            for (argument, param) in paired(&call.arguments, &params) {
                // A spread feeds the variadic and nothing after it; anywhere else, how many values
                // it carries is not known and the pairing is over.
                if argument.spread && !param.is_variadic() {
                    break;
                }
                if self.overwritten_by_the_runtime(hir, call, param) {
                    continue;
                }
                let Some(expected) = param.expected(types) else {
                    continue;
                };

                let at = argument.range.start();
                let Some(path) = phrost_body::value_path_of(hir, argument.value) else {
                    continue;
                };
                let Some(held) = self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT)
                else {
                    continue;
                };

                // The other check's territory: a sentinel the parameter does not take. Checked
                // before the judgement rather than after, because a value carrying a sentinel is
                // rejected for *carrying* it and the message would then be about the wrong thing.
                // Both sentinels, so that a `string|false` reaching an `array` parameter is reported
                // once and by the check that names what is wrong with it. Whether it takes one is
                // `parameter_takes`, the same question that check asks.
                let may_be_null =
                    self.null_reach_of(hir, &held) != crate::references::SentinelReach::Impossible;
                let may_be_false =
                    self.false_reach_of(hir, &held) != crate::references::SentinelReach::Impossible;
                let sentinel_owned = |sentinel| {
                    self.sentinel_expectation(param, templates.as_ref())
                        .is_some_and(|owned| !self.parameter_takes(hir, param, owned, sentinel))
                };
                if (may_be_null && sentinel_owned(BuiltinTy::Null))
                    || (may_be_false && sentinel_owned(BuiltinTy::False))
                {
                    continue;
                }
                // A null the parameter takes has nowhere wrong to land, so what is judged is the
                // rest: `?int` reaching a `?string` is the `int`. Taken off rather than left to the
                // judgement, because a parameter that takes null only through its `= null` default
                // declares a type the null half would be judged against and fail.
                let given = held.clone();
                let held = match may_be_null {
                    true => match self.without_null(held) {
                        Some(rest) => rest,
                        None => continue,
                    },
                    false => held,
                };

                // Spread into a variadic: what is judged is the element, not the array around it.
                let judged = match argument.spread {
                    true => match self.element_of_held(at, hir, &held) {
                        Some(element) => element,
                        // An array whose element type is unknown says nothing about what arrives.
                        None => continue,
                    },
                    false => held,
                };

                // Against everything the parameter accepts, not only against what it declares: a
                // `@param` wider than the hint is the callee promising to handle the extra shapes.
                // See [`ParamRow::rejects`], which owns that rule.
                // Each expectation with a template the receiver bound put in its place, and
                // `Unknown` -- which is silence -- where nothing bound it. A bare `T` is not a type
                // to judge against: see `substituted_expectation`.
                let rejected = param.rejects(types, |expected| {
                    // Against the *written* argument as well as against what it resolves to: weak
                    // mode converts a string where an int is declared, and a literal that is not a
                    // number is the one string it cannot convert. Inside the closure so it is asked
                    // of every alternative the parameter accepts -- a `@param int|string` beside an
                    // `int` hint takes the string as a string.
                    let Some(expected) = self.bound_expectation(templates.as_ref(), expected)
                    else {
                        return phrost_ty::compat::Fit::Unknown;
                    };
                    self.fit_written_value(at, hir, argument.value, &judged, expected)
                });
                // What the callback will be handed, which the type above does not hold: a
                // signature's parameters are kept beside it.
                let handed = match param {
                    Parameter::Declared(row) => row.callback_params_ty,
                    Parameter::Closure(_) => None,
                };
                if !rejected
                    && !handed.is_some_and(|handed| {
                        self.callback_parameter_refused(at, hir, &judged, handed, || {
                            self.callee_is_strict(call)
                        })
                    })
                {
                    continue;
                }

                out.push(IncompatibleArgument {
                    range: argument.range,
                    parameter: param.name().clone(),
                    // What the parameter takes *here*, which for a template is what the receiver
                    // bound it to: a message naming `T` tells a reader nothing they can act on, and
                    // the judgement above was made against the substitution rather than the name.
                    declared: match self.bound_expectation(templates.as_ref(), expected) {
                        Some(bound) => handed
                            .and_then(|handed| self.signature_rendered(handed, bound))
                            .unwrap_or_else(|| bound.render(types, hir)),
                        None => continue,
                    },
                    // What was passed, null included: the judgement set the null aside, the message
                    // describes the value.
                    given: self
                        .render_held(
                            argument.range.start(),
                            hir,
                            match argument.spread {
                                true => &judged,
                                false => &given,
                            },
                            Naming::Short,
                        )
                        .unwrap_or_else(|| "a value of another type".to_owned()),
                    spread: argument.spread,
                });
            }
        }

        out
    }
}

impl IndexView<'_> {
    /// Whether the function or method a call reaches is declared in a file under
    /// `strict_types=1`, which is what decides how *its* calls convert a scalar -- the question a
    /// callback it is handed has. An internal function has no such file and calls the way a file
    /// without the directive does.
    fn callee_is_strict(&self, call: &CallSite) -> bool {
        let at = call.callee_range.start();
        self.cursor_target(at)
            .and_then(|target| self.declaration_of(at, &target))
            .and_then(|location| self.skeleton.file(location.file))
            .is_some_and(|file| file.strict_types())
    }
}

/// A `new` on something PHP will not instantiate.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UninstantiableClass {
    /// The whole `new` expression, which is what a report underlines.
    pub range: TextRange,
    pub class: phrost_model::Name,
    /// What is wrong with it, as the word PHP's own error uses.
    pub because: Uninstantiable,
}

/// Why a class cannot be instantiated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Uninstantiable {
    Abstract,
    Interface,
    Trait,
}

impl IndexView<'_> {
    /// Every `new` in the current file naming something PHP will refuse to instantiate.
    ///
    /// A fatal error, and one only the index can see: whether a name is abstract is decided by a
    /// declaration in another file.
    ///
    /// `new static()` is exempt, and it is the exemption that matters: inside an abstract class it is
    /// legal and it is the standard factory idiom -- `static` is the class the call was made on, which
    /// by definition was instantiable. `new self()` is *not* exempt, because `self` is the class the
    /// code is written in, so in an abstract class it is a real error. Reporting the first and missing
    /// the second was the first thing this check did.
    ///
    /// # Only a class named outright
    ///
    /// `new $class()` is exempt too, and for a different reason: the class is a *value*, so what the
    /// type layer knows about it is a **bound** rather than an identity. `@param class-string<Product>
    /// $class` says "`Product` or any subclass of it", which is exactly how a factory over an abstract
    /// base is written -- and resolving that bound to `Product` and reporting it turned the correct
    /// annotation of a real factory into `Cannot instantiate abstract class`. PHPStan reads the same
    /// tag the same way and says nothing.
    ///
    /// What this gives up is `$c = Product::class; new $c();`, which is a real error. It cannot be
    /// kept: a `::class` literal and a docblock's `class-string<X>` are the same type by the time
    /// anything here can ask, so there is no spelling of "exactly this class" to test against -- and
    /// a check that reports a working factory to catch a line that fails on its first run is the
    /// wrong trade. See §8 on two source spellings interning to one type.
    /// Every `&` argument naming a property that cannot hold what the callee may write.
    ///
    /// The other direction of [`Self::incompatible_arguments`], and the one §8.1 names without
    /// having had a rule: "a declaration on the *input* side is a lower bound ... a by-reference
    /// parameter's hint". An ordinary argument is judged value-against-parameter because the call
    /// reads it; a `&` parameter is also *written*, so the same pair has a second question with the
    /// sides swapped -- can the place hold what the callee may put there.
    ///
    /// `setValueAtPath(?array &$data, ...)` called as `$this->setValueAtPath($this->result, ...)`
    /// on a `protected array $result` is the shape, six times in one file on a real project.
    /// Nothing is wrong with the argument going *in* -- an `array` is a fine `?array` -- so the
    /// read-side judgement accepts it, and the one thing the call can do that the class forbids is
    /// exactly what its signature says it may.
    ///
    /// **Only a property.** A place has to *declare* a type for there to be a promise to break, and
    /// that is what a property is: `private array $result` is a promise about every write. A local
    /// holds whatever the last assignment put there, which is a description -- writing a `?array`
    /// into a local that held an `array` is how a local's type changes, not a mistake.
    ///
    /// Its own walk rather than a question inside the loop above, and cheap by construction: a `&`
    /// parameter is rare, so the prefilter runs before anything is resolved and almost every pair
    /// stops there.
    pub fn written_back_arguments(&self) -> Vec<WrittenBack> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for call in self.file_calls().iter() {
            let Some((params, _)) = self.parameters_and_templates_of(hir, call) else {
                continue;
            };
            for (argument, param) in paired(&call.arguments, &params) {
                if argument.spread && !param.is_variadic() {
                    break;
                }
                if !param.by_reference() || self.written_by_convention(hir, call) {
                    continue;
                }
                let Some(expected) = param.expected(self.skeleton.types()) else {
                    continue;
                };
                if let Some(found) = self.write_refused(
                    argument.range.start(),
                    hir,
                    argument.value,
                    argument.range,
                    param,
                    expected,
                ) {
                    out.push(found);
                }
            }
        }
        out
    }

    /// Whether the runtime overwrites this argument without reading it: a stub's `&` beside a
    /// `@param-out`, which is how PHP spells an output parameter -- `exec`'s `$output`,
    /// `preg_match`'s `$matches`.
    ///
    /// The type the stub writes on such a parameter is what comes *out*, and the engine does not
    /// check what goes in: `$output = 'log'; exec($cmd, $output);` runs on PHP 8.5.10 and leaves
    /// `$output` a list, where reading the hint as a demand reported the string as the wrong type.
    /// Code anybody wrote enforces its hint on the way in, which is why this is the stub set's rule
    /// and nobody else's.
    fn overwritten_by_the_runtime(
        &self,
        hir: &phrost_hir::FileHir,
        call: &phrost_body::CallSite,
        param: &crate::parameters::Parameter,
    ) -> bool {
        param.filled_by_the_callee() && self.written_by_convention(hir, call)
    }

    /// Whether the callee's `&` is the engine's calling convention rather than a promise.
    ///
    /// The whole of the standard library's array cursor -- `reset`, `end`, `current`, `next`, `key`
    /// -- takes `array|object &$array`, and not one of them replaces the array with an object: the
    /// reference is there to move an internal pointer without copying a million elements. Reading
    /// those signatures as writes reported `reset($this->items)` on a `string[]` property, and on
    /// three of the four corpora that shape was *every* finding but one -- 57 of 58 on WordPress.
    ///
    /// So the line is the **stub set**, and it is a line about what a signature means rather than a
    /// list of names: a stub is the engine's ABI written down, where a `&` says how the value is
    /// passed. Code anybody wrote -- the project's own or a dependency's -- says with the same `&`
    /// what it intends to put there, and that is the promise this rule is about.
    fn written_by_convention(
        &self,
        hir: &phrost_hir::FileHir,
        call: &phrost_body::CallSite,
    ) -> bool {
        let phrost_body::CallTarget::Function(reference) = &call.target else {
            return false;
        };
        let written = hir.name_ref(*reference);
        let Some(location) = self.resolve_name(
            call.range.start(),
            written.path.as_str(),
            written.anchor,
            phrost_hir::ImportKind::Function,
        ) else {
            return false;
        };
        self.skeleton
            .file(location.file)
            .is_some_and(|file| file.origin() == phrost_model::SourceOrigin::Stub)
    }

    /// One `&` pair, when the property it names is declared to hold less than the parameter.
    fn write_refused(
        &self,
        at: phrost_model::TextSize,
        hir: &FileHir,
        argument: phrost_hir::ExprId,
        range: TextRange,
        param: &Parameter,
        expected: crate::compat::Expectation,
    ) -> Option<WrittenBack> {
        let phrost_hir::body::ExprData::PropertyAccess {
            receiver,
            name: Some(property),
            ..
        } = hir.expr(argument)
        else {
            return None;
        };

        let path = phrost_body::value_path_of(hir, *receiver)?;
        let held = self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT)?;
        let owner = self.declaring_class(
            at,
            hir,
            &held,
            phrost_sema::MemberKind::Property,
            property.as_str(),
        )?;
        let promised = self.member_held(
            owner.as_str(),
            phrost_sema::MemberKind::Property,
            property.as_str(),
        )?;

        // Sides swapped: what the callee may write is the value, and the property's declaration is
        // the expectation. Only a proven refusal -- `Unknown` is silence, as everywhere here.
        let writes = match expected {
            crate::compat::Expectation::Stored(ty) => {
                crate::resolve::Held::Stored { ty, scope: None }
            }
            crate::compat::Expectation::Written(id) => crate::resolve::Held::Written(id),
        };
        if self.fit_held_against_held(at, hir, &writes, &promised)
            != phrost_ty::compat::Fit::Rejected
        {
            return None;
        }

        Some(WrittenBack {
            range,
            owner: owner.clone(),
            property: property.clone(),
            declared: self.render_held(at, hir, &promised, crate::variables::Naming::Short)?,
            parameter: param.name().clone(),
            writes: param.render(self.skeleton.types(), hir)?,
        })
    }

    pub fn uninstantiable_news(&self) -> Vec<UninstantiableClass> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        self.file_calls()
            .iter()
            .filter_map(|call| {
                let phrost_body::CallTarget::Constructor(target) = &call.target else {
                    return None;
                };
                let at = call.range.start();
                let class = match target {
                    phrost_hir::body::ClassTarget::Relative {
                        relative: phrost_model::RelativeTy::Static,
                        ..
                    } => return None,
                    // The class is a value, so what is known of it is usually an upper bound; see
                    // above. Only one that names the class itself -- `Foo::class` or `'Foo'` --
                    // says which class this makes.
                    phrost_hir::body::ClassTarget::Expr(value) => {
                        self.class_a_value_names_exactly(hir, *value)?
                    }
                    _ => self.class_of_target(at, hir, *target)?,
                };
                let header = self.class_header(class.as_str())?;

                let because = match header.kind {
                    phrost_sema::ClassKind::Interface => Uninstantiable::Interface,
                    phrost_sema::ClassKind::Trait => Uninstantiable::Trait,
                    // An enum cannot be instantiated either, but `new` on one is not something
                    // anybody writes -- and PHP's own message is about the enum rather than about
                    // instantiation, so it is left to say it.
                    _ if header.modifiers.contains(&"abstract") => Uninstantiable::Abstract,
                    _ => return None,
                };

                Some(UninstantiableClass {
                    range: call.range,
                    class,
                    because,
                })
            })
            .collect()
    }
}

/// The callee as a person would name it in a message.
///
/// A free function is named the way a method already was. It used to read "the function needs 2
/// arguments", which in a file that calls forty of them says nothing about which -- and the same
/// message for a method has always said `write()`. The name is the one written at the call site
/// rather than the resolved one, because that is what the reader is looking at.
fn callee_of(hir: &FileHir, call: &CallSite) -> String {
    match &call.target {
        phrost_body::CallTarget::Member { name, .. } => format!("{name}()"),
        phrost_body::CallTarget::Function(reference) => {
            format!("{}()", hir.name_ref(*reference).path)
        }
        phrost_body::CallTarget::Constructor(_) => "the constructor".to_owned(),
        // No name is written: what a reader is looking at is the value on the left of the
        // parentheses, and there is no spelling of it that is shorter than the line itself.
        phrost_body::CallTarget::Invoked(_) => "the value being called".to_owned(),
    }
}
