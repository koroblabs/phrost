//! What a variable at the cursor is.
//!
//! The one projection that answers about something no file declares. A local has no entry in any
//! catalog: it exists because a parameter, an assignment, a `foreach`, or a `catch` put it there,
//! and what it holds is whatever that binding said.

use phrost_body::{BindingKind, NeverBound, NeverRead, Occurrence, VariableSite};
use phrost_model::{FileId, Name, TextRange, TextSize, short_name};

use crate::{
    resolve::{Held, RelativeScope},
    view::{FOLLOW_LIMIT, IndexView},
};

/// How far a rendered type has to travel, which is what decides how its class names are spelled.
///
/// A hover card is read beside the `use` lines that make a short name unambiguous, and repeating
/// `\Drupal\Core\Entity\EntityInterface` there spends a line of screen saying what the top of the
/// file already said. A generated docblock is read anywhere -- by another editor, by whoever
/// inherits the class, by this index reading the file back -- and a short name there names nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Naming {
    /// The last segment: `EntityInterface`.
    Short,
    /// Written out in full, with the leading separator a docblock needs: `\Drupal\Core\...`.
    Qualified,
}

impl Naming {
    /// One class name, spelled this way.
    pub(crate) fn write(self, fqn: &str) -> String {
        match self {
            Self::Short => short_name(fqn).to_owned(),
            // Leading, because a docblock is read against the *file's* namespace and a name written
            // without it resolves relatively -- the trap §8 records for the completion scanner,
            // where a stripped `\Drupal` became `Drupal\app_core\Drupal` and named nothing.
            Self::Qualified => format!("\\{}", fqn.trim_start_matches('\\')),
        }
    }
}

/// A variable the cursor is on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VariableInfo {
    /// Without the leading `$`, as names are stored everywhere else.
    pub name: Name,
    /// The file the variable is in, which is always the one the request is about.
    pub file: FileId,
    pub range: TextRange,
    /// Where the name is bound: the parameter, the assignment target, the `foreach` or `catch`
    /// target. `None` when nothing in scope declares it.
    ///
    /// A different answer from the type, and the two can disagree: after `if ($x instanceof Repo)`
    /// the value comes from the guard while the binding is still the parameter, and that is where
    /// goto belongs -- so this carries the construct at the *site*, which for that case is the
    /// parameter rather than the guard.
    pub declared_at: Option<(TextRange, BindingKind)>,
    /// For a template's variable, which nothing in the template binds: the key that names it in a
    /// call rendering the template -- `'post'` in `view('posts.show', ['post' => $post])` -- and the
    /// file that call is in. The nearest thing to a declaration such a variable has.
    pub rendered_at: Option<(FileId, TextRange)>,
    /// The type as the declaration says, rendered. `None` when nothing says.
    ///
    /// Worth reporting as absent rather than as `mixed`: it is the reason completion offers nothing
    /// on this variable, and a reader who is told that can fix it by writing a type.
    pub ty: Option<String>,
    /// What introduced it, or `None` at the parameter's own declaration -- where the construct is
    /// what the reader is already looking at.
    pub binding: Option<BindingKind>,
    /// The class the same type names, for a reader that wants to jump to it rather than show it.
    ///
    /// Beside [`Self::ty`] rather than found again: what a local holds depends on *which site* the
    /// cursor is on -- an assignment target is typed by its own right-hand side and a use is typed
    /// by searching backwards -- and that is the knowledge this function has and a second reader
    /// does not. Goto-type-definition searched backwards at an assignment target and got nothing,
    /// on a line where hover beside it named the class.
    pub class: Option<Name>,
    /// The class the type's *element* names, when the type itself names none.
    ///
    /// `@var Rule[]` names no class -- there is no `Rule[]` to jump to -- and the only declaration a
    /// reader asking where this type is written could want is `Rule`. Beside [`Self::class`] for
    /// the reason that one is beside [`Self::ty`]: the element is read at the site, where the
    /// resolution knows what the value is, and a caller holding a name cannot ask again.
    pub element_class: Option<Name>,
    /// Every class a union or an intersection the value holds names.
    ///
    /// Beside [`Self::class`], which for a union is their nearest common ancestor and so the class
    /// the members can be looked up on -- but not where the *type* is declared. `if
    /// (!$app->isHomeDelivery() && !$app->isWholesale()) { return; }` leaves a
    /// `HomeDelivery|BusinessLine`, the card says so, and type-definition landed on `Application`. An
    /// intersection reduces to no one class at all, so every PHPUnit mock -- `MockObject&Repo` --
    /// had nowhere to jump until its parts were listed here too.
    pub classes: Vec<Name>,
}

impl IndexView<'_> {
    /// The variable at `offset`, if the cursor is on one.
    pub fn variable_at(&self, offset: TextSize) -> Option<VariableInfo> {
        let mut found = self.variable_at_as_typed(offset)?;
        // A string that names a class says so, which is what the value is for: `$class =
        // $item::class` renders `class-string<Item>` rather than `string`, from the same reading
        // `new $class` goes through, so the card and the instantiation cannot disagree.
        if matches!(
            found.ty.as_deref(),
            Some("string" | "non-empty-string" | "non-falsy-string")
        )
            && let Some(open) = self.current()
            // At an assignment the value is what names the class; anywhere else the name is
            // followed back to where it was written.
            && let Some(class) = self.class_a_path_names(
                offset,
                open.hir(),
                &match phrost_body::variable_at(open.hir(), offset) {
                    Some(VariableSite::Assigned { value, .. }) => value,
                    _ => phrost_body::ValuePath {
                        root: phrost_body::PathRoot::Local(found.name.clone()),
                        steps: Vec::new(),
                        may_be_absent: false,
                    },
                },
                FOLLOW_LIMIT,
            )
        {
            found.ty = Some(format!("class-string<{}>", short_name(class.as_str())));
        }
        Some(found)
    }

    fn variable_at_as_typed(&self, offset: TextSize) -> Option<VariableInfo> {
        let file = self.current_file()?;
        let open = self.current()?;
        let hir = open.hir();
        let site = phrost_body::variable_at(hir, offset)?;
        let declared_at = phrost_body::local_declaration(hir, offset, site.name().as_str());

        // `$this` is bound by the language rather than by anything in the body, so it needs no
        // inference -- but it can still be narrowed. A base class asking `if ($this instanceof
        // Subclass)` is what PHP is written instead of pattern matching, and inside that branch the
        // enclosing class is not the answer.
        if site.name().as_str() == "this" {
            let narrowed = self.narrowed_this(offset);
            let class = narrowed.clone().or_else(|| self.enclosing_class(offset));
            return Some(VariableInfo {
                name: site.name().clone(),
                file,
                range: site.range(),
                // `$this` is declared by nothing: it is the language's, not the file's.
                declared_at: None,
                rendered_at: None,
                ty: class
                    .clone()
                    .map(|class| short_name(class.as_str()).to_owned()),
                binding: narrowed.map(|_| BindingKind::Narrowing),
                class,
                // `$this` is the enclosing class and nothing else; it holds no elements.
                element_class: None,
                classes: Vec::new(),
            });
        }

        match site {
            // A parameter with nothing written: what a use of it reads, which for a closure handed
            // to a function is what the function calls it with. Hovering the name said "no declared
            // type" where the first use of it, one line down, named the class -- the declaration
            // answering less than its own uses.
            VariableSite::Parameter {
                name,
                range,
                declared: None,
            } if let Some(held) =
                self.resolve_local_held(range.start(), name.as_str(), FOLLOW_LIMIT) =>
            {
                Some(VariableInfo {
                    ty: self.render_held(offset, hir, &held, Naming::Short),
                    class: self.class_of_held(offset, hir, &held),
                    element_class: self.element_class_of(offset, hir, &held),
                    classes: self.union_classes(offset, hir, &held),
                    name,
                    file,
                    range,
                    declared_at: Some((range, BindingKind::Parameter)),
                    rendered_at: None,
                    binding: None,
                })
            }
            // A parameter's own name: the type is written right there, so no search backwards.
            VariableSite::Parameter {
                name,
                range,
                declared,
            } => Some(VariableInfo {
                name,
                file,
                range,
                declared_at: Some((range, BindingKind::Parameter)),
                rendered_at: None,
                ty: declared.and_then(|id| {
                    self.render_held(
                        offset,
                        hir,
                        &crate::resolve::Held::Written(id),
                        Naming::Short,
                    )
                }),
                binding: None,
                class: declared.and_then(|id| {
                    self.class_of_held(offset, hir, &crate::resolve::Held::Written(id))
                }),
                element_class: declared.and_then(|id| {
                    self.element_class_of(offset, hir, &crate::resolve::Held::Written(id))
                }),
                classes: declared
                    .map(|id| self.union_classes(offset, hir, &crate::resolve::Held::Written(id)))
                    .unwrap_or_default(),
            }),
            // The assignment the cursor is standing on is the binding being asked about.
            VariableSite::Assigned { name, range, value } => {
                let held = self.resolve_path(offset, hir, &value, FOLLOW_LIMIT);
                Some(VariableInfo {
                    name,
                    file,
                    range,
                    declared_at,
                    rendered_at: None,
                    ty: held
                        .as_ref()
                        .and_then(|held| self.render_held(offset, hir, held, Naming::Short)),
                    binding: Some(BindingKind::Assignment),
                    class: held
                        .as_ref()
                        .and_then(|held| self.class_of_held(offset, hir, held)),
                    element_class: held
                        .as_ref()
                        .and_then(|held| self.element_class_of(offset, hir, held)),
                    classes: held
                        .as_ref()
                        .map(|held| self.union_classes(offset, hir, held))
                        .unwrap_or_default(),
                })
            }
            VariableSite::Use { name, range } => {
                let bound = phrost_body::local_binding(
                    hir,
                    offset,
                    name.as_str(),
                    &|hir, expr| self.call_never_returns(offset, hir, expr),
                    &self.asserting(),
                );
                let held = bound.as_ref().and_then(|bound| {
                    // Resolved where the value was written and refined where it is read, which are
                    // two questions that used to be answered as one.
                    //
                    // The write position is for the names *inside* the path: they mean what they
                    // meant there, and `LocalBinding::written_at` exists because reading them at the
                    // asking position sends `$one = $one->find()` round again.
                    //
                    // The refinement is asked about the **local**, not about what its binding holds,
                    // and that is the whole of why this card used to disagree with every check about
                    // one variable. `contract_narrowed` walks the calls that were passed a path and
                    // matches them by comparing written paths, so handing it `bound.value` -- the
                    // path of the *value* -- matches nothing and returns what it was given. Hover
                    // said `string` for a variable a later `fills(array &$out)` had filled while the
                    // checks said `array`. Measured: refining at the cursor and keeping the value's
                    // path changes nothing, so the position was never the cause.
                    let at = bound.written_at.unwrap_or(offset);
                    let held = self.resolve_path(at, hir, &bound.value, FOLLOW_LIMIT)?;
                    let local = phrost_body::local_path(&name);
                    Some(self.contract_narrowed(offset, hir, &local, FOLLOW_LIMIT, held))
                });
                // Nothing in the file bound it, which for a variable the engine binds is the
                // normal state rather than a gap -- `$_SERVER` is declared by the runtime the way
                // a parameter is declared by a signature. Asked here as well as in
                // `resolve_local_held` because this card reads the binding itself rather than
                // going through it.
                let held = held.or_else(|| self.unbound_held(name.as_str(), FOLLOW_LIMIT));
                let rendered_at = match bound {
                    None => self.template_variable_named_at(name.as_str()),
                    Some(_) => None,
                };

                Some(VariableInfo {
                    name,
                    file,
                    range,
                    declared_at,
                    rendered_at,
                    ty: held
                        .as_ref()
                        .and_then(|held| self.render_held(offset, hir, held, Naming::Short)),
                    binding: bound.map(|bound| bound.kind),
                    class: held
                        .as_ref()
                        .and_then(|held| self.class_of_held(offset, hir, held)),
                    element_class: held
                        .as_ref()
                        .and_then(|held| self.element_class_of(offset, hir, held)),
                    classes: held
                        .as_ref()
                        .map(|held| self.union_classes(offset, hir, held))
                        .unwrap_or_default(),
                })
            }
        }
    }

    /// The class a value's *element* names, when the value's own type names none.
    ///
    /// Asked only where the type is not a class itself, which is what keeps `Rule` answering `Rule`
    /// rather than whatever `Rule` happens to iterate as: a class that implements `IteratorAggregate`
    /// has an element and is still the answer to "where is this type declared".
    fn element_class_of(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        held: &crate::resolve::Held,
    ) -> Option<Name> {
        if self.class_of_held(offset, hir, held).is_some() {
            return None;
        }
        let element = self.element_of_held(offset, hir, held)?;
        self.class_of_held(offset, hir, &element)
    }

    /// The classes a union of classes names, one per operand, or the parts of an intersection;
    /// empty for anything else.
    fn union_classes(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        held: &crate::resolve::Held,
    ) -> Vec<Name> {
        let Some(operands) = self.union_operands(offset, hir, held) else {
            let parts = self.intersection_of_held(offset, hir, held);
            return match parts.len() {
                0 | 1 => Vec::new(),
                _ => parts,
            };
        };
        let classes = operands
            .into_iter()
            .filter(|operand| operand.kind != Some(phrost_model::BuiltinTy::Null))
            .map(|operand| operand.class)
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        match classes.len() {
            0 | 1 => Vec::new(),
            _ => classes,
        }
    }

    /// Whether a rendered type already says it admits null.
    ///
    /// Both spellings, because both are produced here: `?T` for a type that takes the marker and
    /// `T|null` for the unions and the builtins that cannot.
    fn already_nullable(rendered: &str) -> bool {
        rendered.starts_with('?')
            || rendered.ends_with("|null")
            // A null anywhere among the top-level operands of a plain pipe list: an interned
            // `int|null` joined beside a `string|false` renders its null in the middle.
            || (!rendered.contains(['<', '(', '{'])
                && rendered.split('|').any(|operand| operand == "null"))
    }

    /// A stored type as hover shows it.
    ///
    /// Almost all of it is the type table's own rendering. The one substitution is
    /// `self`/`static`/`parent`, which are rendered as the class they mean here: the reader is
    /// asking what this value is, and `static` is an answer about the declaration.
    ///
    /// Only the relative node is substituted, and only where the scope answers for it -- so `?static`
    /// keeps its `?`, which is the one fact a reader is looking at this popup to find, and a word
    /// nothing can resolve stands as written rather than taking the whole type down with it.
    pub(crate) fn render_stored(
        &self,
        ty: phrost_ty::TypeId,
        scope: Option<&RelativeScope>,
        naming: Naming,
    ) -> String {
        self.skeleton
            .types()
            .render_relative(ty, naming == Naming::Qualified, &|relative| {
                let class = self.class_of_relative(relative, scope?)?;
                Some(naming.write(class.as_str()))
            })
    }

    /// The parameter list a closure literal declares, as a reader would write it.
    ///
    /// Empty for anything that is not a closure -- which an id from another file's arena can look
    /// like -- so a signature is never assembled out of the wrong declaration.
    fn closure_params(&self, hir: &phrost_hir::FileHir, expr: phrost_hir::ExprId) -> String {
        match hir.expr(expr) {
            phrost_hir::body::ExprData::Closure { params, .. } => {
                phrost_hir::render::render_params_short(hir, *params)
            }
            _ => String::new(),
        }
    }

    /// A held type as a person would write it, spelled the way the reader of the text needs.
    ///
    /// [`Naming::Short`] for a hover card, which sits in the file that wrote the `use` lines;
    /// [`Naming::Qualified`] for text that leaves it -- a generated docblock is read by another
    /// editor, by whoever inherits the class, and by this index reading the file back.
    ///
    /// One renderer with a parameter rather than two, for the reason §8 records four times over: a
    /// second constructor of one rule is how one of them comes to be missing what the other has.
    pub(crate) fn render_held(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        held: &Held,
        naming: Naming,
    ) -> Option<String> {
        match held {
            Held::Class(name) => Some(naming.write(name.as_str())),
            // As a docblock writes it, which is where the arguments were written.
            Held::Applied { class, arguments } => Some(format!(
                "{}<{}>",
                naming.write(class.as_str()),
                arguments
                    .iter()
                    .map(|argument| self.render_held(offset, hir, argument, naming))
                    .collect::<Option<Vec<_>>>()?
                    .join(", ")
            )),
            Held::Builtin(builtin) => Some(builtin.as_str().to_owned()),
            // As a docblock would have written it, so a hover over a literal and a hover over
            // an `array{...}` tag read the same.
            Held::Record(fields) => Some(format!(
                "array{{{}}}",
                fields
                    .iter()
                    .map(|(key, value)| format!(
                        "{}: {}",
                        phrost_ty::shape_key(key.as_str()),
                        self.render_held(offset, hir, value, naming)
                            .unwrap_or_else(|| "mixed".to_owned())
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            // As a docblock writes one: a property name needs no quoting.
            Held::Object(fields) => Some(format!(
                "object{{{}}}",
                fields
                    .iter()
                    .map(|(name, value)| format!(
                        "{name}: {}",
                        self.render_held(offset, hir, value, naming)
                            .unwrap_or_else(|| "mixed".to_owned())
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            // Class names are left as this file spelled them, whichever naming is asked for. A
            // written type ref is an id into *this* file's arena, so the name in it is the one the
            // author typed against this file's imports -- there is no qualified spelling to render,
            // and resolving one would be answering a different question from "what does the
            // declaration say".
            //
            // `self`, `static` and `parent` are the exception, and the same exception
            // [`Self::render_stored`] already makes: they are not names anybody chose, so repeating
            // one tells a reader nothing they cannot see. Written and stored are two spellings of
            // one type, and this rule living on only one of them is what let a card say `self`
            // where type-definition landed on `Profile`.
            Held::Written(id) => Some(phrost_hir::render::render_type_ref_short_relative(
                hir,
                *id,
                &|relative| {
                    let class = match relative {
                        phrost_model::RelativeTy::SelfTy | phrost_model::RelativeTy::Static => {
                            self.enclosing_class(offset)?
                        }
                        phrost_model::RelativeTy::Parent => {
                            self.parent_of_enclosing_class(offset)?
                        }
                    };
                    Some(naming.write(class.as_str()))
                },
            )),
            Held::Stored { ty, scope } => Some(self.render_stored(*ty, scope.as_ref(), naming)),
            // The null is what the ternary added, so it is rendered the way the language writes it --
            // which for a boolean literal is `false|null`, because `?false` is not a type PHP can
            // parse. See `BuiltinTy::takes_a_question_mark`.
            Held::Nullable(inner) => Some(match &**inner {
                Held::Builtin(kind) if !kind.takes_a_question_mark() => {
                    format!("{}|null", kind.as_str())
                }
                // Already nullable, so adding a second marker spells nothing: `??array<string, int>`
                // was on a hover card, the outer `?` from here and the inner one from inside a
                // `Held::Stored` holding an interned `?array`. `Held::nullable` folds the doubling it
                // can see and cannot see that one -- the type table would be needed, and this is the
                // only place the difference is observable, because every other reader strips a
                // nullable by recursing. So it is asked of the *text*, which is what a renderer is
                // for.
                inner if Self::already_nullable(&self.render_held(offset, hir, inner, naming)?) => {
                    self.render_held(offset, hir, inner, naming)?
                }
                // A `?` binds tighter than `|` and `&`, so `?string|array<int, string>` is read as
                // `(?string)|array<int, string>` -- a different type, and one PHP cannot spell at
                // all. The same precedence rule the bracket suffix below states, and the same shape
                // §8 records for `string|?int`, which turned out to be an inference bug wearing a
                // render bug's clothes. This one really is the render.
                Held::OneOf(_) => {
                    format!("{}|null", self.render_held(offset, hir, inner, naming)?)
                }
                // In parentheses, as PHP's own grammar writes one: `A&B|null` is not a type it can
                // parse, `(A&B)|null` is.
                Held::AllOf(_) => {
                    format!("({})|null", self.render_held(offset, hir, inner, naming)?)
                }
                _ => format!("?{}", self.render_held(offset, hir, inner, naming)?),
            }),
            // `Row[]` for a plain element, `array<int|string>` for a composite one. The suffix binds
            // tighter than `|`, `&` and `?`, so `int|string[]` is read by PHP as `int|(string[])` --
            // a different type, and one the value is not. The same rule the two type renderers
            // already state as `brackets_bind`.
            Held::ListOf(inner) => Some(match &**inner {
                Held::OneOf(_) | Held::AllOf(_) | Held::Nullable(_) => {
                    format!("array<{}>", self.render_held(offset, hir, inner, naming)?)
                }
                _ => format!("{}[]", self.render_held(offset, hir, inner, naming)?),
            }),
            Held::MapOf { key, value } => Some(format!(
                "array<{}, {}>",
                self.render_held(offset, hir, key, naming)?,
                self.render_held(offset, hir, value, naming)?
            )),
            // `Closure(...)` and not `callable(...)`, because every way this value is built makes a
            // `Closure` -- a closure literal, a first-class callable, `Closure::fromCallable()` --
            // and `$fn->bindTo()` already resolves as one. PHPStan spells it the same way; the
            // `callable` it used to read as is the wider type a parameter would accept.
            //
            // With the parameters, because leaving them out does not read as "unknown" -- it reads
            // as "takes none", which is a claim about the value and usually a false one. Where
            // there is no literal to read them from -- a first-class callable naming a member
            // declared in another file -- they are `...`, which is both PHP's own spelling for
            // "whatever the callee takes" and the syntax that built the value.
            Held::Callable(callable) => Some(format!(
                "Closure({}): {}",
                match callable.literal {
                    Some(expr) => self.closure_params(hir, expr),
                    None => "...".to_owned(),
                },
                self.render_held(offset, hir, &callable.returns, naming)?
            )),
            // Both rendered as PHP writes them, which is also how the author wrote the types they
            // came from.
            Held::OneOf(operands) => {
                let mut rendered = operands
                    .iter()
                    .map(|operand| self.render_held(offset, hir, operand, naming))
                    // An operand that is itself a plain pipe list holds operands of its own, and one
                    // of them can repeat a sibling: `$s .= ...` in one branch and `saveHTML()` in the
                    // other join a `string` beside a `string|false`, which read whole spell the
                    // `string` twice. Opened out one level so the fold below can see them, and only
                    // where the text carries no brackets -- a `|` inside `array<int|string, Row>` is
                    // not an operand separator, and splitting there would print a type nobody wrote.
                    .map(|one| {
                        let one = one?;
                        match one.contains(['<', '(', '&', '?']) {
                            true => Some(vec![one]),
                            false => Some(one.split('|').map(str::to_owned).collect()),
                        }
                    })
                    .collect::<Option<Vec<Vec<_>>>>()?
                    .concat();
                // `?string|string` says `?string` twice over: an operand whose nullable is also an
                // operand adds nothing, and PHP has no spelling for the pair. Folded as text,
                // because the two arrive as different values -- an interned `?string` from one
                // branch beside a written `string` from the other -- and the rendering is the one
                // place both are in a vocabulary that can compare them. The weaker claim is kept.
                let covered: Vec<String> = rendered
                    .iter()
                    .filter_map(|one| {
                        one.strip_prefix('?')
                            .or_else(|| one.strip_suffix("|null"))
                            .map(str::to_owned)
                    })
                    .collect();
                // And a string another operand already admits: `?string|non-falsy-string` is a
                // `?string`, the refinement being one of the strings it holds. The same fold the
                // constructors make, for the pair they could not see -- an interned `?string` is not
                // a builtin to them.
                let weakest_string = ["string", "non-empty-string", "non-falsy-string"]
                    .iter()
                    .position(|string| {
                        rendered
                            .iter()
                            .chain(&covered)
                            .any(|one| one.as_str() == *string)
                    });
                // A class-string is a non-falsy one that also names its class, which a plain string
                // beside it no longer says anything about.
                let admitted = |one: &str| {
                    let rank = match one.starts_with("class-string") {
                        true => Some(2),
                        false => ["string", "non-empty-string", "non-falsy-string"]
                            .iter()
                            .position(|string| one == *string),
                    };
                    rank.zip(weakest_string)
                        .is_some_and(|(rank, weakest)| rank > weakest)
                };
                let mut seen: Vec<String> = Vec::new();
                rendered.retain(|one| {
                    if covered.contains(one) || seen.contains(one) || admitted(one) {
                        return false;
                    }
                    seen.push(one.clone());
                    true
                });
                Some(rendered.join("|"))
            }
            Held::AllOf(operands) => Some(
                operands
                    .iter()
                    .map(|operand| self.render_held(offset, hir, operand, naming))
                    .collect::<Option<Vec<_>>>()?
                    .join("&"),
            ),
        }
    }
}

impl IndexView<'_> {
    /// Every range in the current file where the local at `offset` is written.
    ///
    /// A thin projection over the body, because the scope rules are the body's and the file
    /// boundary is this layer's: a local never leaves its file, and a caller that swept the project
    /// for the spelling would rename a different variable in every file that shares it.
    pub fn local_occurrences(&self, offset: TextSize, name: &str) -> Vec<Occurrence> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        phrost_body::local_occurrences(open.hir(), offset, name)
    }

    /// The class whose property the local at `offset` also declares, when it is a promoted
    /// constructor parameter.
    ///
    /// `__construct(public int $across)` is one declaration PHP reads as two, so a request standing
    /// on the parameter is standing on the property as well. Answering with the class turns the
    /// question into the one the member tables already answer project-wide, which is the half a
    /// local can never reach.
    pub fn promoted_property_owner(&self, offset: TextSize, name: &str) -> Option<Name> {
        let open = self.current()?;
        phrost_body::promoting_parameter(open.hir(), offset, name)?;
        self.enclosing_class(offset)
    }

    /// Every range in this file where the parameter that promotes `property` on `class` is written.
    ///
    /// The other half of [`Self::promoted_property_owner`], and empty for an ordinary property --
    /// a promoted property's declaration range *is* its parameter's, so asking the body what binds
    /// that offset is what tells the two apart, with no flag stored anywhere to say so.
    ///
    /// Empty as well when the declaration is in another file, which is what keeps a project-wide
    /// sweep honest: the constructor body's `$across` exists in the declaring file only, and
    /// occurrences read at an offset that means something else elsewhere would be a different
    /// variable entirely.
    pub fn promoted_parameter_occurrences(&self, class: &str, property: &str) -> Vec<TextRange> {
        let Some(location) = self.member_location(class, crate::MemberKind::Property, property)
        else {
            return Vec::new();
        };
        if Some(location.file) != self.current_file_id() {
            return Vec::new();
        }

        let at = location.name_range.start();
        let Some(open) = self.current() else {
            return Vec::new();
        };
        if phrost_body::promoting_parameter(open.hir(), at, property).is_none() {
            return Vec::new();
        }

        let mut found: Vec<TextRange> = phrost_body::local_occurrences(open.hir(), at, property)
            .into_iter()
            .map(|occurrence| occurrence.range)
            .collect();
        // The constructor's `@param` names the parameter, so it is one more place the property is
        // spelled -- the same reason a plain parameter's docblock moves with it.
        let mentions = self.docblock_mentions(property, &found);
        found.extend(mentions);
        found
    }

    /// Locals in the current file that nothing ever reads.
    ///
    /// Whole-file, because the question is about every scope rather than about a cursor -- and empty
    /// when the file is not open, which is the shape of the two tiers rather than a fallback: this
    /// reads a body, and bodies exist for open buffers only.
    pub fn locals_never_read(&self) -> Vec<NeverRead> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        phrost_body::locals_never_read(open.hir(), self.file_scope_is_private())
    }

    /// Locals in the current file that are read where nothing binds them.
    ///
    /// The mirror of [`Self::locals_never_read`], and a whole-file question for the same reason.
    pub fn locals_never_bound(&self) -> Vec<NeverBound> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        phrost_body::locals_never_bound(open.hir(), self.file_scope_is_private())
    }

    /// Whether the current file's own top level is a scope nothing outside it can share.
    ///
    /// PHP shares a scope through `include`, so a file's top-level variables are ordinarily the
    /// includer's -- which is why a check about them says nothing there. The exception is generic
    /// and not about any framework: an autoloaded file is reached through composer's `includeFile()`,
    /// a *function*, so its top level is local to that call and nobody outside can reach it.
    pub(crate) fn file_scope_is_private(&self) -> bool {
        self.current_file()
            .is_some_and(|file| self.skeleton.file_scope_is_private(file))
    }
}

/// One assignment, and the type it gave the name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssignedType {
    /// Without the leading `$`.
    pub name: Name,
    pub range: TextRange,
    /// The right-hand side, so a report can show which expression could not be typed.
    pub value_range: TextRange,
    /// The type, rendered. `None` is the interesting case: the variable has none.
    pub ty: Option<String>,
}

impl IndexView<'_> {
    /// Every assignment in the current file, with the type it produced.
    ///
    /// For measuring inference over a real project rather than guessing at it. Which shapes give a
    /// variable no type is not something anyone can reason out from the code; it is a list, and the
    /// list is ranked by how often each shape is actually written.
    pub fn assigned_types(&self) -> Vec<AssignedType> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::assignments(hir)
            .into_iter()
            .map(|assigned| AssignedType {
                ty: assigned.value.as_ref().and_then(|path| {
                    let at = assigned.range.start();
                    let held = self.resolve_path(at, hir, path, FOLLOW_LIMIT)?;
                    self.render_held(at, hir, &held, Naming::Short)
                }),
                name: assigned.name,
                range: assigned.range,
                value_range: assigned.value_range,
            })
            .collect()
    }
}
