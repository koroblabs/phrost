//! Every place one local name is written, and whether each place reads it or writes it.
//!
//! The question a rename, a highlight, and an unused-variable check all ask, and the only one in
//! this crate whose answer must be *complete* rather than merely correct: a rename that misses an
//! occurrence leaves the file broken, and one that includes an occurrence of a different variable
//! breaks it differently. So the two shadowing rules are enforced in both directions -- which scope
//! owns the name, and which nested scopes it reaches into.

use std::collections::{BTreeMap, BTreeSet};

use lspkit_arena::IdSlice;
use phrost_hir::{FileHir, StmtId, body::ExprData};
use phrost_model::{Name, TextRange, TextSize};

use crate::{
    binding::bound_by,
    scope::Scope,
    walk::{visit_catches, visit_foreach_targets, visit_statements},
};

/// One place a local name is written in the source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occurrence {
    pub range: TextRange,
    pub kind: OccurrenceKind,
}

/// What the code does with the value there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OccurrenceKind {
    /// The value is taken: `echo $x`, `f($x)`, `use ($x)`.
    Read,
    /// The value is put there: an assignment target, a `foreach` target, a `catch` clause, a
    /// parameter.
    ///
    /// `$rows['a'] = 1` counts as a write of `$rows`, which is PHP's rule -- it creates the array --
    /// and it is why writing to an element does not make a variable used.
    Write,
    /// Both at once: the target of a compound assignment, `$n += 1` or `$s .= 'x'`.
    ///
    /// Its own kind rather than one of the two, because the two readers here want opposite answers
    /// from it. "Is this variable ever read" has to say yes -- `+=` takes the old value -- and "how
    /// many times is it written" has to count it, which is what makes a variable unsafe to inline.
    /// Folding it into either one gets the other wrong, silently.
    ReadWrite,
}

/// Every occurrence of the local called `name` at `offset`, in source order.
///
/// Includes the place it is bound -- a parameter, a `catch` clause, a `foreach` target -- because
/// that is what an editor lists first and what a rename has to change too.
///
/// Empty when the offset is not inside any declaration. Never crosses a file: a local is one
/// function's business, and a project-wide search for its spelling would find a different variable
/// in every file that happens to use the same one.
pub fn local_occurrences(hir: &FileHir, offset: TextSize, name: &str) -> Vec<Occurrence> {
    let chain = Scope::chain_at(hir, offset);
    let Some(mut owner) = chain.len().checked_sub(1) else {
        return Vec::new();
    };

    // Climb out of every scope the name merely passes through. An arrow function's `$x` is the
    // enclosing function's, and a closure's is too when it says `use ($x)` -- but a parameter by the
    // same name is a different variable, and that stops the climb.
    while owner > 0
        && chain[owner].reaches_outward(hir, name)
        && !chain[owner].declares_parameter(hir, name)
    {
        owner -= 1;
    }

    in_scope(hir, &chain[owner], name)
}

/// A local that nothing ever reads: `$unused = $this->work();`.
///
/// Only names an *assignment* binds. A parameter is not reported -- an unused one is usually a
/// signature the interface fixed -- and neither is a `foreach` target or a caught exception, where
/// PHP had no way to leave the name out until recently and plenty of code predates that.
///
/// Silent for a whole scope that could be reading a variable somewhere this cannot see: an
/// `extract()` or a `compact()` names variables with a string, an `include` hands the whole scope to
/// another file, and `$$name` reaches a variable whose name is a value. Each of those makes every
/// answer here a guess, so each of them stops the check rather than being reported around.
pub fn locals_never_read(hir: &FileHir, private_file_scope: bool) -> Vec<NeverRead> {
    let mut found = Vec::new();

    for scope in Scope::every_scope(hir) {
        // A file's own top-level variables belong to whoever included the file, unless nobody can:
        // see `private_file_scope`. `settings.php` is nothing but that shape -- `$databases = [];`
        // is read by the bootstrap that included it -- and no answer here could see the reader.
        if (scope.is_file_scope(hir) && !private_file_scope) || hides_variables(hir, &scope) {
            continue;
        }

        // Collected first so each name is walked once: a body with fifteen locals would otherwise be
        // walked fifteen times.
        let mut assigned: BTreeMap<Name, TextRange> = BTreeMap::new();
        let mut aliases: BTreeSet<Name> = BTreeSet::new();
        visit_statements(hir, hir.body(scope.body()).statements, &mut |hir, expr| {
            let ExprData::Assign { target, value, .. } = hir.expr(expr) else {
                return;
            };
            let Some(ExprData::Variable { name, range }) = plain_target(hir, *target) else {
                return;
            };
            // `$alias = &$this->rows;` binds a name to somewhere else, and the binding is the whole
            // effect -- Symfony writes exactly that, with a comment saying so. Once a name is an
            // alias every write through it is a write to the other place, so the *name* is excluded
            // and not just this assignment.
            if matches!(hir.expr(*value), ExprData::Reference { .. }) {
                aliases.insert(name.clone());
                return;
            }
            // Writing `$_SESSION` writes global state, which the rest of the program reads.
            if phrost_model::SUPERGLOBALS.contains(&name.as_str()) {
                return;
            }
            // The first assignment, which is where the variable comes into being.
            assigned.entry(name.clone()).or_insert(*range);
        });

        for (name, range) in assigned {
            // A name this scope does not own is the enclosing scope's business, and that scope is
            // in this list too: `function () use (&$rows) { $rows = ...; }` writes the outer
            // `$rows`, and asking about it here would see the write and none of the reads.
            if scope.reaches_outward(hir, name.as_str())
                && !scope.declares_parameter(hir, name.as_str())
            {
                continue;
            }
            // A parameter assigned and not read again is not a finding. Sometimes it is not even
            // true: `function alter(float &$value)` exists to write its parameter, and the caller is
            // the reader.
            if scope.declares_parameter(hir, name.as_str()) || aliases.contains(&name) {
                continue;
            }
            if in_scope(hir, &scope, name.as_str())
                .iter()
                .all(|occurrence| occurrence.kind == OccurrenceKind::Write)
            {
                found.push(NeverRead { name, range });
            }
        }
    }

    found.sort_by_key(|entry| entry.range.start());
    found
}

/// One local nothing reads, and where it is first written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NeverRead {
    pub name: Name,
    pub range: TextRange,
}

/// The variable an assignment target is, when the target is one and nothing else.
///
/// `$rows['a'] = 1` and `$this->rows = 1` are excluded: the first creates `$rows` but says nothing
/// about it being unused, and the second is not a local at all.
fn plain_target(hir: &FileHir, target: phrost_hir::ExprId) -> Option<&ExprData> {
    match hir.expr(target) {
        data @ ExprData::Variable { .. } => Some(data),
        _ => None,
    }
}

/// Whether anything in this scope could reach a variable by a name this cannot see.
pub(crate) fn hides_variables(hir: &FileHir, scope: &Scope) -> bool {
    let mut hidden = false;

    visit_statements(hir, hir.body(scope.body()).statements, &mut |hir, expr| {
        hidden = hidden || hides_a_variable(hir, expr);
    });

    hidden
}

/// Whether one expression is a way of reaching a variable by a name this cannot see.
///
/// The rule apart from the walk, so a reader already traversing the scope for its own reasons can
/// ask it per expression instead of paying for a second traversal.
pub(crate) fn hides_a_variable(hir: &FileHir, expr: phrost_hir::ExprId) -> bool {
    match hir.expr(expr) {
        // `$$name` and `${$expr}`: the name is a value, so no answer about any name is safe.
        ExprData::IndirectVariable { .. } => true,
        // The included file sees this scope's variables and can add its own.
        ExprData::Include { .. } => true,
        ExprData::FunctionCall { name, args, .. } => {
            let called = hir.name_ref(*name).path.as_str();
            // Unqualified, because those are the ones PHP resolves to the global functions. A
            // `compact()` that spells every name it collects reaches nothing unseen: each of those
            // strings is an occurrence of its variable, see `compacted_names`.
            called.eq_ignore_ascii_case("extract")
                || (called.eq_ignore_ascii_case("compact")
                    && compacted_names(hir, expr).count() != hir.expr_list(*args).len())
        }
        _ => false,
    }
}

/// Every occurrence of `name` inside one scope, reads and writes told apart.
fn in_scope(hir: &FileHir, scope: &Scope, name: &str) -> Vec<Occurrence> {
    let mut writes = Vec::new();
    // Kept apart from `writes` so the classification below can tell the two spellings of "the value
    // is put here" from each other: only one of them also takes the old value.
    let mut read_writes = Vec::new();
    let mut found = Vec::new();

    for &id in hir.param_list(scope.params()) {
        let param = hir.param(id);
        if param.name.as_str() == name {
            writes.push(param.name_range);
            found.push(Occurrence {
                range: param.name_range,
                kind: OccurrenceKind::Write,
            });
        }
    }

    // Bodies rather than one walk, because descending into a nested closure is a decision: the same
    // rule that decided which scope owns the name decides which closures it reaches into.
    let mut bodies = vec![scope.body()];
    while let Some(body) = bodies.pop() {
        let statements = hir.body(body).statements;

        // Writes first, so the classification below can consult them. Three shapes, because a
        // binding is not always an expression the walk would recognise on its own.
        visit_statements(hir, statements, &mut |hir, expr| {
            let (ExprData::Assign { target, .. } | ExprData::CompoundAssign { target, .. }) =
                hir.expr(expr)
            else {
                return;
            };
            if !bound_by(hir, *target).is_some_and(|bound| bound.name().as_str() == name) {
                return;
            }
            let Some(range) = written_range(hir, *target) else {
                return;
            };
            match matches!(hir.expr(expr), ExprData::CompoundAssign { .. }) {
                true => read_writes.push(range),
                false => writes.push(range),
            }
        });
        // A closure's `use (&$x)` hands the *variable* over, so whatever the closure does to it is
        // done here. That makes the capture a write as much as a read -- and a reader that saw only
        // a read offered to inline `$shouldStop` into `use ($output, &false)`.
        visit_statements(hir, statements, &mut |hir, expr| {
            let ExprData::Closure { captures, .. } = hir.expr(expr) else {
                return;
            };
            for &id in hir.expr_list(*captures) {
                let ExprData::Reference { value, .. } = hir.expr(id) else {
                    continue;
                };
                if let ExprData::Variable {
                    name: captured,
                    range,
                } = hir.expr(*value)
                    && captured.as_str() == name
                {
                    read_writes.push(*range);
                }
            }
        });
        // Every one of them: the offset gate exists for a cursor query, and this is about a whole
        // scope.
        visit_foreach_targets(
            hir,
            statements,
            TextSize::from(u32::MAX),
            &mut |hir, expr| {
                if let ExprData::Variable {
                    name: bound, range, ..
                } = hir.expr(expr)
                    && bound.as_str() == name
                {
                    writes.push(*range);
                }
            },
        );
        visit_catches(hir, statements, &mut |hir, catch| {
            let data = hir.catch(catch);
            if data
                .variable
                .as_ref()
                .is_some_and(|bound| bound.as_str() == name)
                && let Some(range) = data.variable_range
            {
                writes.push(range);
                found.push(Occurrence {
                    range,
                    kind: OccurrenceKind::Write,
                });
            }
        });

        visit_statements(hir, statements, &mut |hir, expr| match hir.expr(expr) {
            ExprData::Variable {
                name: written,
                range,
            } if written.as_str() == name => found.push(Occurrence {
                range: *range,
                kind: match (writes.contains(range), read_writes.contains(range)) {
                    (true, _) => OccurrenceKind::Write,
                    (false, true) => OccurrenceKind::ReadWrite,
                    (false, false) => OccurrenceKind::Read,
                },
            }),
            // A closure's `use ($x)` is written in the body that made it, so the capture is an
            // occurrence here -- and a read, because making the closure takes the value. Whether the
            // closure's own body holds more of them is the same question as above.
            ExprData::Closure {
                kind,
                params,
                captures,
                body,
                ..
            } => {
                // An arrow function captures every outer variable by value; a `function` takes
                // only what its `use` clause names. `static` changes nothing about *variables* --
                // it is only about `$this`.
                let reaches = kind.is_arrow() || Scope::captures(hir, *captures, name);
                let shadowed = hir
                    .param_list(*params)
                    .iter()
                    .any(|&id| hir.param(id).name.as_str() == name);
                if reaches && !shadowed {
                    bodies.push(*body);
                }
            }
            // `compact('title')` reads `$title` by its name, so the string is an occurrence -- and a
            // rename that left it behind would make the call quietly stop collecting the variable.
            ExprData::FunctionCall { .. } => found.extend(
                compacted_names(hir, expr)
                    .filter(|(collected, _)| collected.as_str() == name)
                    .map(|(_, range)| Occurrence {
                        range,
                        kind: OccurrenceKind::Read,
                    }),
            ),
            _ => {}
        });
    }

    found.sort_by_key(|occurrence| occurrence.range.start());
    found.dedup_by_key(|occurrence| occurrence.range.start());
    found
}

/// Every name a `compact()` call collects, each with the range inside its quotes.
///
/// Inside the quotes, because that is the name: a rename replaces this range with the new
/// spelling, and the quotes have to stay. Empty for any other call.
pub(crate) fn compacted_names(
    hir: &FileHir,
    call: phrost_hir::ExprId,
) -> impl Iterator<Item = (&Name, TextRange)> {
    let (args, literals) = match hir.expr(call) {
        ExprData::FunctionCall {
            name,
            args,
            literal_arguments,
            ..
        } if hir
            .name_ref(*name)
            .path
            .as_str()
            .eq_ignore_ascii_case("compact") =>
        {
            (hir.expr_list(*args), &literal_arguments[..])
        }
        _ => (&[][..], &[][..]),
    };
    args.iter().enumerate().filter_map(move |(position, &arg)| {
        let ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::String,
            range,
        } = hir.expr(arg)
        else {
            return None;
        };
        let literal = literals.iter().find(|literal| {
            usize::from(literal.position) == position
                && literal.kind == phrost_hir::body::ArgumentLiteral::Text
        })?;
        Some((
            &literal.value,
            TextRange::new(
                range.start() + TextSize::from(1),
                range.end() - TextSize::from(1),
            ),
        ))
    })
}

/// The range of the variable an assignment target names, however it is wrapped.
fn written_range(hir: &FileHir, target: phrost_hir::ExprId) -> Option<TextRange> {
    match hir.expr(target) {
        ExprData::Variable { range, .. } => Some(*range),
        ExprData::Reference { value, .. } | ExprData::Index { base: value, .. } => {
            written_range(hir, *value)
        }
        _ => None,
    }
}

/// One local that is read where nothing in scope binds it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NeverBound {
    pub name: Name,
    /// The first read, which is where the program would warn.
    pub range: TextRange,
}

/// Locals read in a scope that nothing in that scope ever binds.
///
/// The mirror of [`locals_never_read`], and the failure modes are mirrored with it: there, a missed
/// *read* produces a false finding, so the walk has to see every way PHP can read a name. Here a
/// missed *binding* does, so it has to see every way PHP can create one -- which is the longer list,
/// because PHP creates variables in places that do not look like writing:
///
/// - `preg_match($p, $s, $matches)` fills `$matches` through a by-reference parameter, and nothing at
///   the call site says so. Whether a parameter is by reference is the callee's business, and the
///   callee may be anywhere -- so **any** name passed as a bare argument counts as bound. That gives
///   up reporting `f($typo)`, which is a real defect, and keeps the check from reporting the whole
///   standard library's out-parameters.
/// - `global $x;` and `static $x;` introduce a name without writing to it.
/// - `$rows['a'] = 1` creates `$rows`, which is why bindings come from [`bound_by`] rather than from
///   plain assignment targets.
/// - `[$monthly, $annual] = $this->totals()` and
///   `['grid' => $grid, 'basePrice' => $rate] = $options` bind every name written inside the target.
///   `foreach ($rows as [$a, $b])` binds them the same way. Destructuring is how a PHP method returns
///   two things and how a test writes a table of cases, and it was the whole of the first
///   measurement's false positives: 70 findings on a real project, every one of them this.
///
/// Everything [`locals_never_read`] refuses to answer about is refused here too, and for the same
/// reason: a scope holding `extract()`, `compact()`, `$$name` or an `include` can reach a name this
/// cannot see.
pub fn locals_never_bound(hir: &FileHir, private_file_scope: bool) -> Vec<NeverBound> {
    let mut found = Vec::new();

    for scope in Scope::every_scope(hir) {
        // A file's own top level is included by whoever wrote the `include`, and its variables can
        // come from there -- unless nobody can include it. See `private_file_scope`.
        if (scope.is_file_scope(hir) && !private_file_scope) || hides_variables(hir, &scope) {
            continue;
        }

        let statements = hir.body(scope.body()).statements;
        let mut bound: BTreeSet<Name> = BTreeSet::new();
        for &id in hir.param_list(scope.params()) {
            bound.insert(hir.param(id).name.clone());
        }
        let destructured = collect_bindings(hir, statements, &mut bound);

        let mut reads: Vec<(Name, TextRange)> = Vec::new();
        visit_statements(hir, statements, &mut |hir, expr| {
            if let ExprData::Variable { name, range } = hir.expr(expr) {
                reads.push((name.clone(), *range));
            }
            // `compact('typo')` reads a variable by name, and PHP warns about it the same way.
            reads.extend(compacted_names(hir, expr).map(|(name, range)| (name.clone(), range)));
        });

        // A name written inside a destructuring target is bound by it, whatever shape the target
        // has. By range rather than by walking the target's own structure: a key-value item is not
        // modelled as one thing, and every variable inside a target is a name the assignment creates.
        for (name, range) in &reads {
            if destructured
                .iter()
                .any(|target| target.contains_range(*range))
            {
                bound.insert(name.clone());
            }
        }

        let mut reported: BTreeSet<Name> = BTreeSet::new();
        for (name, range) in reads {
            if bound.contains(&name) || reported.contains(&name) {
                continue;
            }
            // `$this` is bound by the call, and a superglobal by the request.
            if name.as_str() == "this" || phrost_model::SUPERGLOBALS.contains(&name.as_str()) {
                continue;
            }
            // A name the enclosing scope owns: a closure's `use ($outer)`, an arrow function's
            // implicit capture. The scope that owns it is in this list too and answers there.
            if scope.reaches_outward(hir, name.as_str()) {
                continue;
            }
            reported.insert(name.clone());
            found.push(NeverBound { name, range });
        }
    }

    found.sort_by_key(|entry| entry.range.start());
    found
}

/// Every name a scope's own statements can bring into being, and the ranges of its destructuring
/// targets.
///
/// The ranges come back rather than the names inside them, because the caller has already walked
/// every variable in the scope and can say which fall inside one -- and a destructuring target's own
/// structure is not modelled item by item.
fn collect_bindings(
    hir: &FileHir,
    statements: IdSlice<StmtId>,
    into: &mut BTreeSet<Name>,
) -> Vec<TextRange> {
    let mut destructured = Vec::new();

    visit_statements(hir, statements, &mut |hir, expr| match hir.expr(expr) {
        ExprData::Assign { target, .. } => {
            if let ExprData::ArrayLiteral { range, .. } = hir.expr(*target) {
                destructured.push(*range);
            }
            if let Some(bound) = bound_by(hir, *target) {
                into.insert(bound.name().clone());
            }
        }
        // Any argument that is a bare variable. A by-reference parameter fills it and the call site
        // does not say which are: `preg_match`, `sscanf`, `settype`, `array_push`, and every
        // hand-written `&$out` in the project.
        ExprData::MethodCall { args, .. }
        | ExprData::StaticCall { args, .. }
        | ExprData::FunctionCall { args, .. }
        | ExprData::New { args, .. } => {
            for &argument in hir.expr_list(*args) {
                let value = match hir.expr(argument) {
                    ExprData::Argument { value, .. } => *value,
                    _ => argument,
                };
                if let Some(bound) = bound_by(hir, value) {
                    into.insert(bound.name().clone());
                }
            }
        }
        _ => {}
    });

    // `foreach ($rows as $key => $row)` and `catch (T $e)`, which are bindings the expression walk
    // does not reach on its own.
    visit_foreach_targets(
        hir,
        statements,
        TextSize::from(u32::MAX),
        &mut |hir, expr| {
            // `foreach ($rows as [$a, $b])` destructures too, and it is the commoner half: a test's
            // table of cases is written that way, and so is every loop over a list of pairs.
            if let ExprData::ArrayLiteral { range, .. } = hir.expr(expr) {
                destructured.push(*range);
            }
            if let Some(bound) = bound_by(hir, expr) {
                into.insert(bound.name().clone());
            }
        },
    );
    visit_catches(hir, statements, &mut |hir, catch| {
        if let Some(name) = hir.catch(catch).variable.as_ref() {
            into.insert(name.clone());
        }
    });

    // `global $x;` and `static $x;`, the two shapes that introduce a name without writing to it.
    for &id in hir.stmt_list(statements) {
        if let phrost_hir::body::StmtData::Bind { bindings, .. } = hir.stmt(id) {
            for &expr in hir.expr_list(*bindings) {
                if let Some(bound) = bound_by(hir, expr) {
                    into.insert(bound.name().clone());
                }
            }
        }
    }

    destructured
}

/// Every expression in the file, closures included.
///
/// The scope walk's, handed out so a reader above the boundary can ask a question about a *shape*
/// -- `clone $x`, `$x()` -- without one of its own. What the expression *holds* is the reader's
/// question and needs an index; where the expressions are is this layer's.
pub fn visit_every_expression(hir: &FileHir, visit: &mut impl FnMut(&FileHir, phrost_hir::ExprId)) {
    crate::scope::Scope::visit_every_expression(hir, visit);
}
