//! The class a syntactic position names: `new Foo`, `Foo::`, `$obj::`.
//!
//! Read off the source rather than off a type, which is why it is not [`super::class_of`]. A
//! `new $class` holding a `Foo::class` is recovered by following the *path* a binding at a time --
//! the magic constant is a string at runtime and no type survives it -- and the `new` position and
//! the `::` position deliberately read the same expression differently. Keeping both readings
//! here is what stops either from being "simplified" into the other.

use phrost_hir::{ExprId, FileHir, body::ClassTarget};
use phrost_model::{Name, RelativeTy, TextSize};

use crate::{resolve::HIERARCHY_DEPTH, view::IndexView};

impl IndexView<'_> {
    /// The class a written class reference names: `new Foo`, `Foo::`, `self::`.
    pub(crate) fn class_of_target(
        &self,
        offset: TextSize,
        hir: &FileHir,
        target: ClassTarget,
    ) -> Option<Name> {
        match target {
            ClassTarget::Named(id) => {
                let reference = hir.name_ref(id);
                self.resolve_written_name(offset, reference.path.as_str(), reference.anchor)
                    .map(|location| location.fqn)
            }
            ClassTarget::Relative {
                relative: RelativeTy::SelfTy | RelativeTy::Static,
                ..
            } => self.enclosing_class(offset),
            ClassTarget::Relative {
                relative: RelativeTy::Parent,
                ..
            } => self.parent_of_enclosing_class(offset),
            // `new $class` -- the class is a value, and the only value that names one is a
            // class-string somebody wrote as `Foo::class`. An *instance* written here is legal PHP
            // and answered by [`Self::class_of_scope`] instead; see there for why the two positions
            // are read differently.
            ClassTarget::Expr(expr) => self.class_a_value_names(hir, expr),
        }
    }

    /// The class a value names, when what it holds is a `Foo::class` somebody wrote.
    ///
    /// `$f = Foo::class; new $f;` -- a plugin registry, a factory keyed by class name, a container
    /// binding. The magic constant is a string at runtime, so nothing in the type layer keeps which
    /// class it named; what does keep it is the *path*, which ends at `Foo` and a `::class` step.
    ///
    /// Read syntactically for exactly that reason. Following the value one binding at a time is what
    /// makes `$f` work as well as `Foo::class` written in place, and stopping at a path that ends
    /// anywhere else is what keeps a guess out.
    fn class_a_value_names(&self, hir: &FileHir, expr: ExprId) -> Option<Name> {
        let path = phrost_body::value_path_of(hir, expr)?;
        // Asked as of where the value was written, not where the cursor is. `$f = Foo::class; $f =
        // new $f;` reassigns the very name it reads, and asking at the cursor would answer the read
        // with the write it is part of.
        let at = hir.expr(expr).range().start();
        self.class_a_path_names(at, hir, &path, HIERARCHY_DEPTH)
    }

    /// The class a `::` written on this target reaches.
    ///
    /// Everything [`Self::class_of_target`] answers, and an instance besides: `$obj::CONST` and
    /// `$obj::method()` take the class the value has, which is legal PHP and written wherever a
    /// constant belongs to a service rather than to the class reading it.
    ///
    /// Kept apart from the `new` reading deliberately. `new $x` is written almost only with a
    /// class-string, and a docblock calling one an *instance* is common enough to matter: one
    /// project has `/** @var Notification $class */ $class = ...; new $class($app)` where the tag
    /// should say `class-string<Notification>`, and reading it as an instance reported a concrete
    /// plugin as instantiating its abstract base.
    pub(crate) fn class_of_scope(
        &self,
        offset: TextSize,
        hir: &FileHir,
        target: ClassTarget,
    ) -> Option<Name> {
        if let Some(named) = self.class_of_target(offset, hir, target) {
            return Some(named);
        }
        let ClassTarget::Expr(expr) = target else {
            return None;
        };
        self.class_of_instance(hir, expr)
    }

    /// The class of the instance `expr` holds, for a class-string read off it -- never `$this`.
    fn class_of_an_instance_named(&self, hir: &FileHir, expr: ExprId) -> Option<Name> {
        if matches!(hir.expr(expr), phrost_hir::body::ExprData::Variable { name, .. } if name.as_str() == "this")
        {
            return None;
        }
        self.class_of_instance(hir, expr)
    }

    /// The class an instance written as a class target has.
    fn class_of_instance(&self, hir: &FileHir, expr: ExprId) -> Option<Name> {
        let path = phrost_body::value_path_of(hir, expr)?;
        // Where the value was written, for the reason `class_a_value_names` gives.
        let at = hir.expr(expr).range().start();
        let held = self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT)?;
        self.class_of_held(at, hir, &held)
    }

    pub(crate) fn class_a_path_names(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        depth: u8,
    ) -> Option<Name> {
        self.class_spelled_by_path(offset, hir, path, depth, false)
    }

    /// The class a value names when it names that class and nothing below it: `Foo::class` or
    /// `'App\Foo'` written somewhere its bindings lead, never a `class-string<Foo>`.
    ///
    /// For a check that is wrong when the class is only an upper bound. `new $class` over a
    /// `class-string<Shape>` makes a concrete shape, and reading the bound as the class reported
    /// it as instantiating the abstract one -- so a reader that judges the class itself asks this.
    pub(crate) fn class_a_value_names_exactly(&self, hir: &FileHir, expr: ExprId) -> Option<Name> {
        let path = phrost_body::value_path_of(hir, expr)?;
        let at = hir.expr(expr).range().start();
        self.class_spelled_by_path(at, hir, &path, HIERARCHY_DEPTH, true)
    }

    /// One walk for both questions; `exactly` refuses every arm whose answer is an upper bound --
    /// an instance's class, which a subclass may be, and a `class-string<T>` from a declaration.
    fn class_spelled_by_path(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        depth: u8,
        exactly: bool,
    ) -> Option<Name> {
        let depth = depth.checked_sub(1)?;
        match (&path.root, path.steps.as_slice()) {
            // `$item::class` and `get_class($item)`: the class the instance has. Not of `$this`,
            // for the reason `static::class` is refused below -- it is the class the call was made
            // on, which a subclass inheriting the code makes something other than the enclosing one.
            (phrost_body::PathRoot::ClassName(ClassTarget::Expr(expr)), []) => {
                (!exactly).then(|| self.class_of_an_instance_named(hir, *expr))?
            }
            (phrost_body::PathRoot::FunctionReturn { name, call, .. }, [])
                if !exactly
                    && name
                        .as_str()
                        .trim_start_matches('\\')
                        .eq_ignore_ascii_case("get_class") =>
            {
                let phrost_hir::body::ExprData::FunctionCall { args, .. } = hir.expr(call.expr()?)
                else {
                    return None;
                };
                let first = *hir.expr_list(*args).first()?;
                let value = match hir.expr(first) {
                    phrost_hir::body::ExprData::Argument { value, .. } => *value,
                    _ => first,
                };
                self.class_of_an_instance_named(hir, value)
            }
            // `Foo::class` written where it is used, and `self::class` and `parent::class`, which
            // name a class exactly.
            //
            // `static::class` is refused: it means whatever class the call was made on, which for
            // `new $class` is by definition not the one it was written in. Reading it as the
            // enclosing class reported `new $class` inside an abstract class as instantiating the
            // abstract class -- which is the shape every Drupal plugin's `create()` is written in.
            (phrost_body::PathRoot::ClassName(target), [])
                if !matches!(
                    target,
                    ClassTarget::Relative {
                        relative: RelativeTy::Static,
                        ..
                    }
                ) =>
            {
                self.class_of_target(offset, hir, *target)
            }
            // `'App\Repo'`, the spelling from before `::class`, and exactly as certain of its class.
            (phrost_body::PathRoot::StringLiteral(literal), []) => {
                self.class_a_literal_names(hir.expr(*literal).range())
            }
            // A local holding one. Followed a binding at a time, so `$a = Foo::class; $b = $a;`
            // reaches the same answer as writing it in place.
            (phrost_body::PathRoot::Local(name), []) => {
                let bound = phrost_body::local_binding(
                    hir,
                    offset,
                    name.as_str(),
                    &|hir, expr| self.call_never_returns(offset, hir, expr),
                    &self.asserting(),
                )?;
                let at = bound.written_at.unwrap_or(offset);
                self.class_spelled_by_path(at, hir, &bound.value, depth, exactly)
            }
            // `/** @var class-string<Rule> */ $className = ...;` -- the class comes from an
            // annotation rather than from a `Foo::class` literal anywhere in the value's own path,
            // which is what `local_binding` above resolves an annotated local to. A plain
            // `@var string $x` reaches here too and correctly answers nothing: only a
            // `class-string<T>` shape does.
            (phrost_body::PathRoot::Written(id), []) => {
                (!exactly).then(|| self.class_named_by_class_string(offset, hir, *id))?
            }
            // A guard stands between the annotation and the use. `class_exists($class)` is how
            // every project checks a class name before instantiating it, and `is_string` and
            // `=== NULL` are the same shape -- so this walk stopping at the first guard meant a
            // `@var class-string<Rule>` lost its argument to the most ordinary defensive line
            // there is. What the guard proves is irrelevant here: none of them says which class a
            // string names, and none of them can take the annotation away. So the subject is
            // followed at the guard's own position, where the guard does not reach.
            //
            // `narrows` answers `None` for every root that is not a guard, which is what makes
            // this the last arm rather than one more shape to enumerate.
            (root, []) if root.narrows().is_some() => {
                let (of, at) = root.narrows()?;
                self.class_spelled_by_path(at, hir, of, depth.checked_sub(1)?, exactly)
            }
            // Anything else whose *type* is a `class-string<T>`: a table of `Foo::class` read by
            // key, a call declared to return one. The factory's own spelling --
            // `$class = static::TYPES[$kind]; return new $class($node);` -- when the table names one
            // class; with several, no single one is the class every use of the value means.
            _ if exactly => None,
            _ => {
                let held = self.resolve_path(offset, hir, path, depth)?;
                self.class_named_by_held_class_string(offset, hir, &held)
            }
        }
    }

    /// Every class a value names when it names one of several, for a `new` that makes whichever it
    /// is: `$c ? User::class : Admin::class` written into the value, or a `class-string<A|B>` its
    /// type states.
    ///
    /// Empty for anything but a value target, and for a value naming one class, which
    /// [`Self::class_of_target`] already answers.
    pub(crate) fn classes_a_value_names(&self, hir: &FileHir, class: ClassTarget) -> Vec<Name> {
        let ClassTarget::Expr(expr) = class else {
            return Vec::new();
        };
        let at = hir.expr(expr).range().start();
        let Some(path) = phrost_body::value_path_of(hir, expr) else {
            return Vec::new();
        };
        let written = self.classes_spelled_by_path(at, hir, &path, HIERARCHY_DEPTH);
        if written.len() > 1 {
            return written;
        }
        let Some(held) = self.resolve_path(at, hir, &path, crate::view::FOLLOW_LIMIT) else {
            return Vec::new();
        };
        let crate::resolve::Held::Stored { ty, scope } = held else {
            return Vec::new();
        };
        let types = self.skeleton.types();
        let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(ty) else {
            return Vec::new();
        };
        if !matches!(
            types.get(*base),
            phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::String)
        ) {
            return Vec::new();
        }
        let [argument] = types.operands(*args) else {
            return Vec::new();
        };
        let phrost_ty::TypeRef::Union(operands) = types.get(*argument) else {
            return Vec::new();
        };
        types
            .operands(*operands)
            .iter()
            .map(|&operand| self.class_of_stored(operand, scope.as_ref()))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// Every class a value names where the code wrote which: one class through
    /// [`Self::class_spelled_by_path`], or several where the value is one of several operands or
    /// writes that each name one.
    ///
    /// Read off the path for the reason a single `Foo::class` is -- see
    /// [`Self::class_a_value_names`] -- since a ternary of two class constants is a union of two
    /// strings to the type layer, and the classes are gone by the time it is one.
    ///
    /// Every operand has to name a class, or the value names none: `$c ? User::class : null` is not
    /// a class `new` can make on one of its paths, and claiming `User` for it would be half a
    /// union. A write in a branch counts the value before the branch as an operand where a path may
    /// have skipped every write, as resolving it does.
    fn classes_spelled_by_path(
        &self,
        offset: TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
        depth: u8,
    ) -> Vec<Name> {
        let Some(depth) = depth.checked_sub(1) else {
            return Vec::new();
        };
        let operands: Vec<(TextSize, &phrost_body::ValuePath)> =
            match (&path.root, path.steps.as_slice()) {
                (phrost_body::PathRoot::EitherOf { operands, .. }, []) => operands
                    .iter()
                    .map(|operand| (operand.at, &operand.value))
                    .collect(),
                (
                    phrost_body::PathRoot::MaybeWritten {
                        writes,
                        read,
                        before,
                        otherwise,
                    },
                    [],
                ) => writes
                    .iter()
                    .map(|write| (write.at, &write.value))
                    .chain(
                        (!matches!(otherwise, phrost_body::Otherwise::Impossible))
                            .then_some((*before, &**read)),
                    )
                    .collect(),
                (phrost_body::PathRoot::Local(name), []) => {
                    let Some(bound) = phrost_body::local_binding(
                        hir,
                        offset,
                        name.as_str(),
                        &|hir, expr| self.call_never_returns(offset, hir, expr),
                        &self.asserting(),
                    ) else {
                        return Vec::new();
                    };
                    let at = bound.written_at.unwrap_or(offset);
                    return self.classes_spelled_by_path(at, hir, &bound.value, depth);
                }
                // A guard between the write and the `new`, followed where it does not reach -- see
                // the same arm in `class_spelled_by_path`.
                (root, []) if root.narrows().is_some() => {
                    let Some((of, at)) = root.narrows() else {
                        return Vec::new();
                    };
                    return self.classes_spelled_by_path(at, hir, of, depth);
                }
                _ => {
                    return self
                        .class_spelled_by_path(offset, hir, path, depth, false)
                        .into_iter()
                        .collect();
                }
            };
        let mut named = Vec::new();
        for (at, operand) in operands {
            let classes = self.classes_spelled_by_path(at, hir, operand, depth);
            if classes.is_empty() {
                return Vec::new();
            }
            for class in classes {
                if !named.contains(&class) {
                    named.push(class);
                }
            }
        }
        named
    }

    /// The class a held `class-string<T>` names, however it was spelled.
    pub(crate) fn class_named_by_held_class_string(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &crate::resolve::Held,
    ) -> Option<Name> {
        match held {
            crate::resolve::Held::Written(id) => self.class_named_by_class_string(offset, hir, *id),
            crate::resolve::Held::Nullable(inner) => {
                self.class_named_by_held_class_string(offset, hir, inner)
            }
            crate::resolve::Held::Stored { ty, scope } => {
                let types = self.skeleton.types();
                let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(*ty) else {
                    return None;
                };
                if !matches!(
                    types.get(*base),
                    phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::String)
                ) {
                    return None;
                }
                let [argument] = types.operands(*args) else {
                    return None;
                };
                // One class, and not the ancestor several share: `Types::MAP[$k]::PARAGRAPH` reads
                // a constant every class in the table declares and their parent does not, so the
                // parent is the wrong class to look it up on -- for `::` as much as for `new`.
                if matches!(types.get(*argument), phrost_ty::TypeRef::Union(_)) {
                    return None;
                }
                self.class_of_stored(*argument, scope.as_ref())
            }
            _ => None,
        }
    }
}
