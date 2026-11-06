//! Where a variable is asked about: the entry points that take a name and a position.
//!
//! The way in, and the only part of this module a caller names. Each finds the binding the body
//! recorded for the name and hands its value to the walk in [`super::path`]; every source of a
//! type that walk consults is a sibling, so a new one lands without touching this file.

use phrost_model::{Name, TextSize};

use crate::{
    resolve::Held,
    view::{FOLLOW_LIMIT, IndexView},
};

impl IndexView<'_> {
    /// The class-like type of a local variable at `offset`, as its declaration says.
    ///
    /// Declared, not inferred: today that means a parameter's type hint, which on type-hinted code
    /// is most variables. Assignments (`$x = new Foo`), properties reached through `$this`, and
    /// docblock `@var` all need expression bodies, which are not lowered — and each of those will
    /// arrive behind this same projection rather than beside it, so nothing above here has to know
    /// which of them answered.
    ///
    /// `None` means the type is not knowable here, never a guess. A feature that offers a guessed
    /// class's members is worse than one that offers nothing: the wrong list reads as authoritative.
    pub fn variable_class(&self, offset: TextSize, name: &str) -> Option<Name> {
        self.resolve_local(offset, name, FOLLOW_LIMIT)
    }

    /// The classes a local holds, which is more than one when its declared type is an intersection.
    ///
    /// `function run(Application&MockObject $mock)` is a parameter with both APIs, and a reader that
    /// wants what can be reached through it wants both tables.
    pub fn variable_classes(&self, offset: TextSize, name: &str) -> Vec<Name> {
        let Some(hir) = self.current().map(|open| open.hir()) else {
            return Vec::new();
        };
        let Some(held) = self.resolve_local_held(offset, name, FOLLOW_LIMIT) else {
            return Vec::new();
        };
        match self.class_of_held(offset, hir, &held) {
            Some(one) => vec![one],
            None => self.intersection_of_held(offset, hir, &held),
        }
    }

    /// Follows a local back to a class, through however many assignments it takes.
    ///
    /// `budget` bounds the chase. `$a = $b; $b = $a;` is not valid PHP that anyone means, but it is
    /// text an editor sees while it is being written, and a language server that hangs on it is
    /// worse than one that gives up.
    pub(crate) fn resolve_local(&self, offset: TextSize, name: &str, budget: u8) -> Option<Name> {
        let hir = self.current()?.hir();
        let held = self.resolve_local_held(offset, name, budget)?;
        self.class_of_held(offset, hir, &held)
    }

    /// The same, stopping at the type rather than the class it names.
    pub(crate) fn resolve_local_held(
        &self,
        offset: TextSize,
        name: &str,
        budget: u8,
    ) -> Option<Held> {
        if budget == 0 {
            return None;
        }

        let hir = self.current()?.hir();
        let bound = match phrost_body::local_binding(
            hir,
            offset,
            name,
            &|hir, expr| self.call_never_returns(offset, hir, expr),
            &self.asserting(),
        ) {
            Some(bound) => bound,
            None => return self.unbound_held(name, budget),
        };

        // Where the value was written, not where it was asked for. See `LocalBinding::written_at`:
        // `$one = $one->find()` reads its own receiver, and reading it here would find this same
        // assignment again.
        let at = bound.written_at.unwrap_or(offset);
        let held = self.resolve_path(at, hir, &bound.value, budget - 1)?;

        // A literal is what the variable holds until something writes into it, and an element
        // write is not a binding -- so `$args = ['-f']; $args[] = $path;` would otherwise read as
        // the one entry it started with. See `filled`.
        //
        // And so is a list a call built for this one variable -- `array_fill(0, $n, null)`, whose
        // contract hands back a list of what it was given. It is as much this variable's own as a
        // literal is, and reading it without the writes said `null[]` of a list every slot of which
        // was then given a string: narrower than the truth, which reports correct code.
        let built = match bound.value.root {
            phrost_body::PathRoot::ArrayLiteral(_) => bound.value.steps.is_empty(),
            _ => matches!(held, Held::ListOf(_)),
        };
        if built
            && let Some(since) = bound.written_at
            && matches!(held, Held::Record(_) | Held::ListOf(_))
        {
            let fills = phrost_body::slots_filled_since(hir, offset, name, since, &|hir, expr| {
                self.call_never_returns(offset, hir, expr)
            });
            if !fills.is_empty() {
                return Some(self.filled_in(hir, held, &fills, budget - 1));
            }
            return Some(held);
        }
        // So is a sealed shape, which names every key it has: a parameter declared `array{id: int}`
        // and given `$row['name'] = ...` holds two keys afterwards, and judged by its declaration
        // alone it lacks one. Read as the record it is, the writes fold in the same way -- and only
        // where something was written, because a shape keeps what a record cannot say, the keys it
        // may not have.
        if bound.value.steps.is_empty()
            && let Some(record) = self.sealed_shape_as_record(hir, &held)
        {
            let fills = phrost_body::slots_filled_in_scope(
                hir,
                offset,
                name,
                bound.written_at,
                &|hir, expr| self.call_never_returns(offset, hir, expr),
            );
            if !fills.is_empty() {
                return Some(self.filled_in(hir, record, &fills, budget - 1));
            }
        }
        Some(held)
    }

    /// A sealed `array{...}` as the record of what each key holds, or `None` for anything else.
    ///
    /// An optional key is kept as present: what a record claims about a key that may be gone is the
    /// claim a write in a branch makes too.
    fn sealed_shape_as_record(&self, hir: &phrost_hir::FileHir, held: &Held) -> Option<Held> {
        let fields: Vec<(Name, Held)> = match held {
            Held::Stored { ty, scope } => {
                let types = self.skeleton.types();
                if !matches!(
                    types.get(*ty),
                    phrost_ty::TypeRef::Shape { open: false, .. }
                ) {
                    return None;
                }
                types
                    .shape_keys(*ty)
                    .into_iter()
                    .map(|(key, ty, _)| {
                        (
                            key.clone(),
                            Held::Stored {
                                ty,
                                scope: scope.clone(),
                            },
                        )
                    })
                    .collect()
            }
            Held::Written(id) => {
                let phrost_hir::TypeRefData::Shape {
                    fields,
                    open: false,
                    ..
                } = hir.type_ref(*id)
                else {
                    return None;
                };
                hir.type_ref_list(*fields)
                    .iter()
                    .filter_map(|&field| match hir.type_ref(field) {
                        phrost_hir::TypeRefData::ShapeField { key, ty, .. } => {
                            Some((key.clone(), Held::Written(*ty)))
                        }
                        _ => None,
                    })
                    .collect()
            }
            _ => return None,
        };
        Some(Held::Record(fields.into_boxed_slice()))
    }

    /// What a variable nothing in the file binds holds: a superglobal, which the engine bound before
    /// any of this code ran (see `super::engine`), or a template's variable, which the calls
    /// rendering it bound (see `resolve::rendered`). One place, because the card on a variable and a
    /// path through it both ask, and a rule taught to one of them only is a card that disagrees with
    /// the member access beside it.
    pub(crate) fn unbound_held(&self, name: &str, budget: u8) -> Option<Held> {
        self.engine_variable(name)
            .or_else(|| self.template_variable(name, budget))
    }

    /// Local variables in scope at `offset`, plus PHP's superglobals.
    ///
    /// Names without their `$`, nearest binding first, then the superglobals — which exist in every
    /// scope, are declared by no file, and are most of what a `$_` prefix is reaching for.
    ///
    /// Plain strings rather than interned names, unlike every other projection here: a superglobal is
    /// not declared anywhere, so there is no symbol to hand back. Interning one to look like a
    /// declaration would be describing something that does not exist.
    pub fn locals_in_scope(&self, offset: TextSize) -> Vec<Box<str>> {
        let Some(open) = self.current() else {
            return Vec::new();
        };

        let mut names = phrost_body::locals_in_scope(open.hir(), offset)
            .into_iter()
            .map(|name| Box::from(name.as_str()))
            .collect::<Vec<Box<str>>>();

        // A template's variables after its own: what the calls rendering it hand it.
        for name in self.template_variable_names() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        for superglobal in phrost_model::SUPERGLOBALS {
            let name = Box::from(*superglobal);
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    /// Whether `offset` is inside a string `compact()` reads as the name of a local.
    ///
    /// `compact('from', 'to')` builds an array out of `$from` and `$to`, so each string is a variable
    /// written without its `$` -- goto and rename already read it that way, and what can be written
    /// there is exactly the names in scope.
    pub fn inside_a_compacted_name(&self, offset: TextSize) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        let hir = open.hir();
        let Some(call) = phrost_body::call_site_at(hir, offset) else {
            return false;
        };
        let phrost_body::CallTarget::Function(name) = call.target else {
            return false;
        };
        hir.name_ref(name)
            .path
            .as_str()
            .eq_ignore_ascii_case("compact")
            && call.arguments.iter().any(|argument| {
                argument.range.contains_inclusive(offset)
                    && matches!(
                        hir.expr(argument.value),
                        phrost_hir::body::ExprData::Literal {
                            kind: phrost_hir::body::LiteralKind::String,
                            ..
                        }
                    )
            })
    }
}
