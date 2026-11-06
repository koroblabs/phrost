use lspkit_arena::IdSlice;
use phrost_hir::{BodyId, ExprId, FileHir, ParamId, body::ExprData};
use phrost_model::{TextRange, TextSize};

use crate::walk::visit_statements;

/// One function-like scope: what it declares, and whether a name can come from outside it.
pub(crate) struct Scope {
    params: IdSlice<ParamId>,
    body: BodyId,
    kind: ScopeKind,
}

pub(crate) enum ScopeKind {
    /// A method or a free function. Nothing encloses it.
    Declaration,
    /// `function () use ($x) {}` — sees its parameters and its captures, and nothing else.
    Closure { captures: IdSlice<ExprId> },
    /// `fn () => ...` — captures every outer variable by value, so the search continues outward.
    ArrowFunction,
}

impl Scope {
    /// Every body in the file: methods, functions, and file scope.
    ///
    /// Closure bodies are reached by the walk inside each one rather than listed here, because a
    /// closure is an expression and lives in the body that wrote it.
    pub(crate) fn every_body(hir: &FileHir) -> Vec<BodyId> {
        let mut bodies = hir.root_body().into_iter().collect::<Vec<_>>();
        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                bodies.extend(member.body);
                // A property hook is function-like: `set(Item $value) => $this->x = $value` has a
                // body and a parameter, and leaving it out makes every name written in one invisible.
                for &hook in hir.hook_list(member.hooks) {
                    bodies.extend(hir.hook(hook).body);
                }
            }
        }
        for (_, function) in hir.functions() {
            bodies.extend(function.body);
        }
        bodies
    }

    /// Every function-like scope in the file: methods, functions, the file itself, and every
    /// closure inside any of them.
    ///
    /// The scope-rule counterpart of [`Self::visit_every_expression`], which hands over expressions
    /// and loses which scope each came from. Anything asking a question *per scope* -- whether a
    /// local is ever read, which names a body binds -- needs this one, and confusing the two fails
    /// silently: a closure is a scope of its own, and folding its locals into the function around it
    /// makes one function's `$row` look like another's.
    pub(crate) fn every_scope(hir: &FileHir) -> Vec<Self> {
        let mut scopes = Vec::new();

        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if let Some(body) = member.body {
                    scopes.push(Self {
                        params: member.params,
                        body,
                        kind: ScopeKind::Declaration,
                    });
                }
                // A hook is a scope of its own, not part of the property's declaration: its `$value`
                // is bound by the hook and nothing outside it can see the name.
                for &id in hir.hook_list(member.hooks) {
                    let hook = hir.hook(id);
                    if let Some(body) = hook.body {
                        scopes.push(Self {
                            params: hook.params,
                            body,
                            kind: ScopeKind::Declaration,
                        });
                    }
                }
            }
        }
        for (_, function) in hir.functions() {
            if let Some(body) = function.body {
                scopes.push(Self {
                    params: function.params,
                    body,
                    kind: ScopeKind::Declaration,
                });
            }
        }
        if let Some(body) = hir.root_body() {
            scopes.push(Self {
                params: IdSlice::EMPTY,
                body,
                kind: ScopeKind::Declaration,
            });
        }

        // Closures are found on the way rather than listed up front: a closure is an expression, so
        // the only way to know one exists is to walk the body that wrote it.
        let mut next = 0;
        while next < scopes.len() {
            let body = scopes[next].body;
            next += 1;
            visit_statements(hir, hir.body(body).statements, &mut |hir, expr| {
                if let ExprData::Closure {
                    kind,
                    params,
                    captures,
                    body,
                    ..
                } = hir.expr(expr)
                {
                    scopes.push(Self {
                        params: *params,
                        body: *body,
                        kind: match kind.is_arrow() {
                            false => ScopeKind::Closure {
                                captures: *captures,
                            },
                            true => ScopeKind::ArrowFunction,
                        },
                    });
                }
            });
        }

        scopes
    }

    /// Visits every expression in the file, closures included.
    ///
    /// The walk a *position* lookup needs, as opposed to [`Self::every_body`], which is the one
    /// scope rules need. The two differ over closures and confusing them fails silently: from
    /// outside, a closure is not a scope a name resolves against, so `every_body` leaves it out --
    /// and that is why goto on a variable written inside one found nothing at all while the member
    /// call beside it resolved perfectly.
    ///
    /// Closures are found on the way rather than listed up front, because a closure is an
    /// expression and the only way to know one exists is to walk the body that wrote it. Which is
    /// also why this hands the caller expressions instead of bodies: discovering the closures in a
    /// body and looking at that body are the same pass, and splitting them walks everything twice.
    pub(crate) fn visit_every_expression(hir: &FileHir, visit: &mut impl FnMut(&FileHir, ExprId)) {
        let mut queue = Self::every_body(hir);

        while let Some(body) = queue.pop() {
            let mut nested = Vec::new();
            visit_statements(hir, hir.body(body).statements, &mut |hir, expr| {
                if let ExprData::Closure { body, .. } = hir.expr(expr) {
                    nested.push(*body);
                }
                visit(hir, expr);
            });
            queue.append(&mut nested);
        }
    }

    pub(crate) fn params(&self) -> IdSlice<ParamId> {
        self.params
    }

    pub(crate) fn body(&self) -> BodyId {
        self.body
    }

    /// Whether this scope is a closure of either spelling, which is what a binding can be applied
    /// to -- an arrow function included, since `Closure::bind` takes one as readily.
    pub(crate) fn is_closure(&self) -> bool {
        matches!(
            self.kind,
            ScopeKind::Closure { .. } | ScopeKind::ArrowFunction
        )
    }

    pub(crate) fn kind(&self) -> &ScopeKind {
        &self.kind
    }

    /// Every function-like scope containing `offset`, outermost first.
    ///
    /// Starts from the method or function the offset is in, then descends through the closures
    /// inside it. An offset outside every declaration gives an empty chain.
    pub(crate) fn chain_at(hir: &FileHir, offset: TextSize) -> Vec<Self> {
        let mut chain = Vec::new();

        let Some((params, body)) = Self::enclosing_declaration(hir, offset) else {
            return chain;
        };
        chain.push(Self {
            params,
            body,
            kind: ScopeKind::Declaration,
        });

        let mut body = body;
        while let Some((scope, inner)) = Self::closure_in(hir, body, offset) {
            chain.push(scope);
            body = inner;
        }

        chain
    }

    /// How narrowly a function-like declaration reaches `offset`, or `None` where it does not
    /// reach it at all.
    ///
    /// Two ways in, and the second is the one that is easy to miss. The body's own width, where the
    /// cursor is inside the body; and the parameter's, where the cursor is on a parameter's *name*.
    /// A signature is not inside the body, which is the right answer for a type hint -- nothing
    /// there is a local -- and the wrong one for the name beside it: that name is the local's
    /// binding site, the one position a person renames a parameter from. Answering nothing there
    /// left rename, document highlight, references and goto-type-definition all silently dead at
    /// it, and alive one word to the right.
    ///
    /// The width is what decides which declaration wins, so a parameter's name -- a handful of
    /// characters -- beats every body that encloses it.
    fn reach_of(
        hir: &FileHir,
        params: IdSlice<ParamId>,
        body: Option<TextRange>,
        offset: TextSize,
    ) -> Option<TextSize> {
        // `None` for a declaration with no body, which an `interface`'s method and an `abstract`
        // one both are. They still bind parameters, so only the second test can reach them -- and
        // that is the right answer: nothing between the braces they do not have is a local.
        if let Some(body) = body
            && body.contains_inclusive(offset)
        {
            return Some(body.len());
        }
        hir.param_list(params)
            .iter()
            .map(|&id| hir.param(id).name_range)
            .find(|range| range.contains_inclusive(offset))
            .map(|range| range.len())
    }

    /// The innermost method or free function whose body contains `offset`, or whose signature binds
    /// a parameter named at it.
    fn enclosing_declaration(
        hir: &FileHir,
        offset: TextSize,
    ) -> Option<(IdSlice<ParamId>, BodyId)> {
        // Over the flat list the facts keep rather than the classes, members and hooks each time:
        // this runs once per position resolved. Methods and hooks first, then functions, and file
        // scope last, because it contains everything and only wins when nothing narrower did.
        let mut best: Option<(TextSize, IdSlice<ParamId>, BodyId)> = None;
        for declaration in crate::facts::BodyFacts::of(hir).declarations_reaching(offset) {
            let Some(width) = Self::reach_of(hir, declaration.params, declaration.range, offset)
            else {
                continue;
            };
            if best.is_some_and(|(current, _, _)| current <= width) {
                continue;
            }
            best = Some((width, declaration.params, declaration.body));
        }

        best.map(|(_, params, body)| (params, body))
    }

    /// The innermost closure directly inside `body` whose own body contains `offset` -- or whose
    /// parameter list binds a name at it, for the reason [`Self::reach_of`] gives.
    fn closure_in(hir: &FileHir, body: BodyId, offset: TextSize) -> Option<(Self, BodyId)> {
        // From the file's own index of closures rather than a walk of the body's statements: this
        // runs once per position resolved, and the walk was most of what a scope lookup cost.
        let mut found = None;
        for expr in hir.closures_directly_in(body) {
            let ExprData::Closure {
                kind,
                params,
                captures,
                body,
                ..
            } = hir.expr(expr)
            else {
                continue;
            };
            // The closure's own span holds its parameters and its body, so outside it neither
            // test can pass -- and it is one comparison where the test reads every parameter.
            if !hir.expr(expr).range().contains_inclusive(offset) {
                continue;
            }
            if Self::reach_of(hir, *params, Some(hir.body(*body).range), offset).is_none() {
                continue;
            }
            found = Some((
                Self {
                    params: *params,
                    body: *body,
                    kind: match kind.is_arrow() {
                        false => ScopeKind::Closure {
                            captures: *captures,
                        },
                        true => ScopeKind::ArrowFunction,
                    },
                },
                *body,
            ));
        }
        found
    }

    /// The file's own top level as a scope, when the file has one.
    ///
    /// Reachable on its own because a `global $x;` inside a function is about *that* scope and
    /// nothing else: the name it imports is the one the script's top level wrote.
    pub(crate) fn file_scope(hir: &FileHir) -> Option<Self> {
        Some(Self {
            params: IdSlice::EMPTY,
            body: hir.root_body()?,
            kind: ScopeKind::Declaration,
        })
    }

    /// Whether this is the file's own top level rather than a declaration or a closure.
    ///
    /// Its variables are not private to it: an `include` hands them to the included file, and being
    /// included hands them to whoever included this one.
    pub(crate) fn is_file_scope(&self, hir: &FileHir) -> bool {
        matches!(self.kind, ScopeKind::Declaration) && hir.root_body() == Some(self.body)
    }

    /// Whether any outer name can reach into this scope at all.
    ///
    /// An arrow function captures everything; a closure captures only what it names, and those are
    /// already listed as its own bindings.
    pub(crate) fn reaches_anything(&self) -> bool {
        matches!(self.kind, ScopeKind::ArrowFunction)
    }

    /// Whether this scope declares a parameter by that name.
    ///
    /// What stops an outer name from reaching in even where the scope would otherwise let it:
    /// `fn ($x) => $x` inside a function that also has an `$x` is two different variables, and an
    /// arrow function reaches outward for everything else.
    /// Whether `name` is the *first* parameter this scope declares.
    /// Which of this scope's parameters a name is, counted in written order.
    ///
    /// The index and not merely whether it is the first, because a declared callable signature says
    /// something about every position: `@param callable(string, int): void` types both parameters
    /// of a closure written `function ($key, $count)`. The heuristic beside it -- the element type
    /// of a container passed in the same call -- can only ever answer the first, which is why that
    /// one asks [`Self::declares_first_parameter`] instead.
    pub(crate) fn parameter_position(&self, hir: &FileHir, name: &str) -> Option<usize> {
        hir.param_list(self.params)
            .iter()
            .position(|&id| hir.param(id).name.as_str() == name)
    }

    pub(crate) fn declares_parameter(&self, hir: &FileHir, name: &str) -> bool {
        hir.param_list(self.params)
            .iter()
            .any(|&id| hir.param(id).name.as_str() == name)
    }

    /// Whether a name not declared here can be the enclosing scope's.
    pub(crate) fn reaches_outward(&self, hir: &FileHir, name: &str) -> bool {
        match &self.kind {
            ScopeKind::Declaration => false,
            ScopeKind::ArrowFunction => true,
            // Through the `&` of a by-reference capture, which wraps the name the way every other
            // by-reference target is wrapped: `use (&$found)` reaches outward as surely as
            // `use ($found)` does, and more so -- it writes back.
            ScopeKind::Closure { captures } => Self::captures(hir, *captures, name),
        }
    }

    /// Whether a closure's `use (...)` list names `name`, by value or by reference.
    ///
    /// Through the `&` of a by-reference capture, which wraps the name the way every other
    /// by-reference target is wrapped: `use (&$found)` reaches outward as surely as `use ($found)`
    /// does, and more so -- it writes back. One reading for every walk that asks, because the walk
    /// that decided which closure bodies a local's occurrences reach into matched a bare variable
    /// only, and so listed `use (&$fail)` and never the `$fail++` inside.
    pub(crate) fn captures(hir: &FileHir, captures: IdSlice<ExprId>, name: &str) -> bool {
        hir.expr_list(captures)
            .iter()
            .any(|&id| Self::captured(hir, id).is_some_and(|captured| captured.as_str() == name))
    }

    /// The name one entry of a `use (...)` list captures, with a by-reference `&` looked through.
    pub(crate) fn captured(hir: &FileHir, capture: ExprId) -> Option<&phrost_model::Name> {
        let id = match hir.expr(capture) {
            ExprData::Reference { value, .. } => *value,
            _ => capture,
        };
        match hir.expr(id) {
            ExprData::Variable { name, .. } => Some(name),
            _ => None,
        }
    }
}
