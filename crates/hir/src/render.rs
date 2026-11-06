//! Canonical text rendering of a [`FileHir`].
//!
//! This is not only a test helper. Two things need one agreed textual form of the IR: snapshot
//! tests, and the `incremental == from-scratch` invariant, which compares an index built by
//! applying edits against one built from the final state. That comparison is only meaningful if
//! the rendering is *canonical* — same content, same bytes, regardless of how it was built.
//!
//! So the rules are: source order everywhere (never hash order), ranges as one-based line:column
//! so a human can check them against an editor, and every optional piece either printed or
//! omitted — never printed as an empty placeholder that two builds could disagree about.

use std::fmt::Write as _;

use phrost_model::{BuiltinTy, LineIndex, RelativeTy, TextRange};

use crate::{
    FileHir, NameAnchor, NameRefData, TypeRefData,
    ids::{ClassId, DocblockId, NameRefId, NamespaceId, TypeRefId},
    item::{ClassData, Hooks, MemberKind, TraitAdaptationKind},
};

/// Renders the whole file's declarations, grouped by the namespace they were declared in.
///
/// Grouping rather than a flat list because a file can declare several namespaces, and which one a
/// class belongs to decides how every name inside it resolves. A flat list would make a lowering
/// bug that loses namespace attribution invisible in the snapshot.
pub fn render_file_hir(hir: &FileHir, lines: &LineIndex) -> String {
    let mut out = String::new();
    let renderer = Renderer {
        hir,
        lines: Some(lines),
        spelling: Spelling::Verbatim,
        relative: None,
    };

    // Printed only when set, the way every other optional piece here is: a `strict_types=0` line
    // on the thirty thousand files that do not declare it would be noise in every snapshot.
    if hir.strict_types() {
        let _ = writeln!(out, "declare(strict_types=1)");
    }

    // The global namespace has no header, so its declarations sit at the top level. Declaring
    // `namespace X;` after global code is legal PHP, so this bucket can be non-empty either way.
    renderer.declarations(&mut out, None, "");

    for (id, namespace) in hir.namespaces() {
        let name = namespace
            .name
            .as_ref()
            .map_or("<global>", |name| name.as_str());
        let _ = writeln!(
            out,
            "namespace {name} @ {}",
            renderer.range(namespace.name_range)
        );
        renderer.declarations(&mut out, Some(id), "  ");
    }

    if out.is_empty() {
        return "<no declarations>\n".to_owned();
    }

    out
}

/// One written type, as text. For a caller that has a type reference and no declaration around it.
pub fn render_type_ref(hir: &FileHir, id: crate::ids::TypeRefId) -> String {
    Renderer {
        hir,
        lines: None,
        spelling: Spelling::Verbatim,
        relative: None,
    }
    .type_ref(id)
}

/// The same type with class names cut to their last segment: `?OrderManager`.
///
/// For hover, which is the one reader that wants a *shape* rather than the author's spelling. The
/// shape has to survive -- dropping the `?` from `?Repo` would hide nullability from the one place
/// a reader looks for it -- and the fully qualified name has to go, because everything else hover
/// shows is short and two styles in one popup read as two different facts.
pub fn render_type_ref_short(hir: &FileHir, id: crate::ids::TypeRefId) -> String {
    Renderer {
        hir,
        lines: None,
        spelling: Spelling::Reader,
        relative: None,
    }
    .type_ref(id)
}

/// The same again, with `self`, `static` and `parent` rendered as the class each one names.
///
/// A class name in a written type is the author's spelling and is left alone; a relative keyword is
/// not a name anybody chose, it is the language's way of writing "the class this is in". A reader
/// asking what a value *is* learns nothing from being told `self`, and the two halves of one card
/// disagreed over it: Symfony declares `Profile::addChild(self $child)`, where go-to-type-definition
/// landed on `Profile` while hover said `self`.
///
/// `resolve` answers `None` for a keyword nothing here can settle -- a `self` in a file with no
/// class around it -- and then the word stands as written rather than taking the type down with it.
/// The same contract [`phrost_ty::TypeTable::render_relative`] states, deliberately: they are two
/// spellings of one type and this is the rule they have to share.
pub fn render_type_ref_short_relative(
    hir: &FileHir,
    id: crate::ids::TypeRefId,
    resolve: &dyn Fn(RelativeTy) -> Option<String>,
) -> String {
    Renderer {
        hir,
        lines: None,
        spelling: Spelling::Reader,
        relative: Some(resolve),
    }
    .type_ref(id)
}

/// A parameter list, as text: `int $id, ?string $label = ...`.
pub fn render_params(hir: &FileHir, params: lspkit_arena::IdSlice<crate::ids::ParamId>) -> String {
    Renderer {
        hir,
        lines: None,
        spelling: Spelling::Verbatim,
        relative: None,
    }
    .params(params)
}

/// The same list as a reader wants it: `int $n, string $s`, class names cut to their last segment.
///
/// For a closure's signature in hover, which is the only place a parameter list describes a *value*
/// rather than a declaration somebody is looking at.
pub fn render_params_short(
    hir: &FileHir,
    params: lspkit_arena::IdSlice<crate::ids::ParamId>,
) -> String {
    Renderer {
        hir,
        lines: None,
        spelling: Spelling::Reader,
        relative: None,
    }
    .params(params)
}

/// How far a rendering may depart from what the author wrote.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Spelling {
    /// The author's names and the author's nesting, exactly. What the canonical snapshot and the
    /// `incremental == from-scratch` invariant compare, so a lowering bug that puts a union inside
    /// a union stays visible as `A|(B|C)` rather than being tidied away.
    Verbatim,
    /// What a reader wants: class names cut to their last segment, and a union flattened into the
    /// one list PHP would write.
    Reader,
}

struct Renderer<'a> {
    hir: &'a FileHir,
    /// `None` when rendering something positionless, such as a signature on its own.
    lines: Option<&'a LineIndex>,
    spelling: Spelling,
    /// What a relative keyword names here, for a reader who wants the class rather than the word.
    /// `None` leaves `self`, `static` and `parent` as they were written, which is what a canonical
    /// snapshot of the tree needs.
    relative: Option<&'a dyn Fn(RelativeTy) -> Option<String>>,
}

impl Renderer<'_> {
    /// Everything declared in one namespace, at the given indent.
    fn declarations(&self, out: &mut String, namespace: Option<NamespaceId>, indent: &str) {
        for (_, import) in self.hir.imports().filter(|(_, i)| i.namespace == namespace) {
            let _ = write!(out, "{indent}use {}", import.path);
            if let Some(alias) = &import.alias {
                let _ = write!(out, " as {alias}");
            }
            let _ = writeln!(
                out,
                "  [{}] @ {}",
                import.kind.as_str(),
                self.range(import.path_range)
            );
        }

        for (id, class) in self.hir.classes().filter(|(_, c)| c.namespace == namespace) {
            self.class(out, id, class, indent);
        }

        for (_, function) in self
            .hir
            .functions()
            .filter(|(_, f)| f.namespace == namespace)
        {
            let _ = writeln!(
                out,
                "{indent}function {}{}({}){}{}{} @ {}{}",
                if function.returns_by_reference {
                    "&"
                } else {
                    ""
                },
                function.name,
                self.params(function.params),
                self.return_type(function.return_type),
                // Beside it, exactly as a method's is: a docblock's return is a field a test at
                // this layer has to read, and it was rendered for a method and not for a free
                // function -- so a snapshot about a function's `@return` could not see the thing it
                // was about. §8's rule, met a third time in one session.
                self.doc_return_type(function.doc_return_type),
                Self::conditional(function.is_conditional),
                self.range(function.name_range),
                self.doc(function.docblock),
            );
        }

        for (_, constant) in self
            .hir
            .constants()
            .filter(|(_, c)| c.namespace == namespace)
        {
            let _ = writeln!(
                out,
                "{indent}const {}{} @ {}{}",
                constant.name,
                Self::conditional(constant.is_conditional),
                self.range(constant.name_range),
                self.doc(constant.docblock),
            );
        }
    }

    /// The marker a declaration only some runs reach carries in every snapshot.
    ///
    /// One word rather than a line of its own, so the declaration it belongs to stays one line --
    /// and printed only when set, the way every other optional piece here is.
    fn conditional(is_conditional: bool) -> &'static str {
        if is_conditional { " [conditional]" } else { "" }
    }

    fn class(&self, out: &mut String, _id: ClassId, class: &ClassData, indent: &str) {
        let mut modifiers = String::new();
        if class.modifiers.is_abstract {
            modifiers.push_str(" abstract");
        }
        if class.modifiers.is_final {
            modifiers.push_str(" final");
        }
        if class.modifiers.is_readonly {
            modifiers.push_str(" readonly");
        }
        if class.modifiers.is_deprecated {
            modifiers.push_str(" deprecated");
        }

        let _ = write!(
            out,
            "{indent}{} {}{modifiers}",
            class.kind.as_str(),
            class.name
        );
        if let Some(backing) = class.enum_backing {
            let _ = write!(out, ": {}", self.type_ref(backing));
        }
        let _ = writeln!(
            out,
            "{} @ {}{}",
            Self::conditional(class.is_conditional),
            self.range(class.name_range),
            self.doc(class.docblock)
        );

        for &template in self.hir.template_list(class.templates) {
            let data = self.hir.template(template);
            let bound = data
                .bound
                .map(|id| format!(" of {}", self.type_ref(id)))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "{indent}  template {}{bound} @ {}",
                data.name,
                self.range(data.name_range)
            );
        }

        for (label, list) in [
            ("extends", class.extends),
            ("implements", class.implements),
            ("uses", class.uses),
        ] {
            let names = self.hir.name_ref_list(list);
            if !names.is_empty() {
                let _ = writeln!(out, "{indent}  {label} {}", self.name_list(names));
            }
        }

        for &adaptation in self.hir.adaptation_list(class.trait_adaptations) {
            let data = self.hir.trait_adaptation(adaptation);
            let owner = data
                .trait_name
                .map(|id| format!("{}::", self.name_ref(id)))
                .unwrap_or_default();

            match &data.kind {
                TraitAdaptationKind::Precedence { excluded } => {
                    let _ = writeln!(
                        out,
                        "{indent}  adapt {owner}{} insteadof {}",
                        data.method,
                        self.name_list(self.hir.name_ref_list(*excluded))
                    );
                }
                TraitAdaptationKind::Alias {
                    alias, visibility, ..
                } => {
                    let mut rest = String::new();
                    if let Some(visibility) = visibility {
                        let _ = write!(rest, " {}", visibility.as_str());
                    }
                    if let Some(alias) = alias {
                        let _ = write!(rest, " {alias}");
                    }
                    let _ = writeln!(out, "{indent}  adapt {owner}{} as{rest}", data.method);
                }
            }
        }

        for &member in self.hir.member_list(class.members) {
            let data = self.hir.member(member);
            let _ = write!(out, "{indent}  {} ", data.kind.as_str());

            match data.kind {
                MemberKind::Method => {
                    let _ = write!(
                        out,
                        "{}({}){}{}",
                        data.name,
                        self.params(data.params),
                        self.return_type(data.type_ref),
                        // What the signature does not say and something else does: a docblock's
                        // `@return`, or the body's own `return` statements. Shown for the reason
                        // `@param-out` is shown beside a parameter -- it is the only place either
                        // can be seen, and a type that does not reach the lowering is inert without
                        // failing anything.
                        self.doc_return_type(data.doc_type_ref)
                    );
                }
                MemberKind::Property => {
                    let _ = write!(out, "${}{}", data.name, self.declared_type(data.type_ref));
                }
                MemberKind::ClassConstant | MemberKind::EnumCase => {
                    let _ = write!(out, "{}{}", data.name, self.declared_type(data.type_ref));
                }
            }

            let _ = write!(out, "  {}", data.visibility.as_str());
            if let Some(set) = data.set_visibility {
                let _ = write!(out, " {}(set)", set.as_str());
            }
            for (flag, label) in [
                (data.modifiers.is_static(), "static"),
                (data.modifiers.is_abstract(), "abstract"),
                (data.modifiers.is_final(), "final"),
                (data.modifiers.is_readonly(), "readonly"),
                // Each state its own word: which one it is decides whether a write is legal,
                // and a dump that spells them all "hooks" cannot tell them apart.
                (data.modifiers.hooks == Hooks::Settable, "hooks"),
                (data.modifiers.hooks == Hooks::BackedGet, "backed-get"),
                (data.modifiers.hooks == Hooks::DeclaredGet, "declared-get"),
                (data.modifiers.hooks == Hooks::VirtualGet, "virtual-get"),
                (data.modifiers.is_no_discard(), "no-discard"),
                (data.modifiers.is_deprecated(), "deprecated"),
            ] {
                if flag {
                    let _ = write!(out, " {label}");
                }
            }

            let _ = writeln!(
                out,
                " @ {}{}",
                self.range(data.name_range),
                self.doc(data.docblock)
            );

            // Under the property, because a hook only exists as part of one and a flat line would
            // lose which property it belongs to. Rendered even in skeleton mode: the declaration is
            // lowered there, only the body is not.
            for &id in self.hir.hook_list(data.hooks) {
                let hook = self.hir.hook(id);
                let _ = writeln!(
                    out,
                    "{indent}    hook {}{}({}) @ {}",
                    if hook.returns_by_reference { "&" } else { "" },
                    hook.kind.as_str(),
                    self.params(hook.params),
                    self.range(hook.name_range)
                );
            }
        }
    }

    fn params(&self, params: lspkit_arena::IdSlice<crate::ids::ParamId>) -> String {
        self.hir
            .param_list(params)
            .iter()
            .map(|&id| {
                let param = self.hir.param(id);
                let mut text = String::new();
                if let Some(promotion) = &param.promotion {
                    let _ = write!(text, "{} ", promotion.visibility.as_str());
                    if promotion.modifiers.is_readonly() {
                        text.push_str("readonly ");
                    }
                }
                if let Some(type_ref) = param.type_ref {
                    let _ = write!(text, "{} ", self.type_ref(type_ref));
                }
                if param.by_reference {
                    text.push('&');
                }
                if param.is_variadic {
                    text.push_str("...");
                }
                let _ = write!(text, "${}", param.name);
                if param.has_default {
                    text.push_str(" = ...");
                }
                // What `@param-out` says the callee leaves behind, which is a different type from
                // the one beside the `&` and is the only place it can be seen: a tag that does not
                // reach the lowering is inert without failing anything, and this dump is what a test
                // at this layer has to read.
                if let Some(out) = param.out_type_ref {
                    let _ = write!(text, " -> {}", self.type_ref(out));
                }
                // The same rule as the tag above it: a `@param callable(A, B)` that does not reach
                // the lowering is inert without failing anything, so the dump has to show it. As
                // `calls(...)` rather than as a type, because the slot is what gives the arguments
                // their meaning -- printed as a type it would read as `callable(): A` and say the
                // opposite of what it holds.
                if let Some(params) = param.callback_params_type_ref {
                    let rendered = match self.hir.type_ref(params) {
                        crate::TypeRefData::Generic { args, .. } => self
                            .hir
                            .type_ref_list(*args)
                            .iter()
                            .map(|&id| self.type_ref(id))
                            .collect::<Vec<_>>()
                            .join(", "),
                        other => format!("{other:?}"),
                    };
                    let _ = write!(text, " calls({rendered})");
                }
                if let Some(this) = param.closure_this_type_ref {
                    let _ = write!(text, " this({})", self.type_ref(this));
                }
                text
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `: T` for a function or method return type, empty when unannotated.
    fn return_type(&self, type_ref: Option<TypeRefId>) -> String {
        type_ref.map_or_else(String::new, |id| format!(": {}", self.type_ref(id)))
    }

    /// ` doc: T` for a return the signature does not carry: a docblock's, or the body's.
    ///
    /// One marker for both, because the slot is one -- see `phrost_lower::returns`. The day a
    /// reader has to tell a docblock's claim from what the body actually does is the day the slot
    /// splits, and this is where that would show first.
    fn doc_return_type(&self, type_ref: Option<TypeRefId>) -> String {
        type_ref.map_or_else(String::new, |id| format!(" doc: {}", self.type_ref(id)))
    }

    /// Same shape as [`Self::return_type`], named separately because a property or constant type
    /// is a declared type rather than a result.
    fn declared_type(&self, type_ref: Option<TypeRefId>) -> String {
        self.return_type(type_ref)
    }

    fn type_ref(&self, id: TypeRefId) -> String {
        match self.hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => builtin.as_str().to_owned(),
            TypeRefData::Relative(relative) => self
                .relative
                .and_then(|resolve| resolve(*relative))
                .unwrap_or_else(|| relative.as_str().to_owned()),
            TypeRefData::Named(name) => self.name_ref(*name),
            // `false|null` rather than `?false`, which PHP cannot parse -- see
            // `BuiltinTy::takes_a_question_mark`.
            TypeRefData::Nullable(inner) => match self.hir.type_ref(*inner) {
                TypeRefData::Builtin(kind) if !kind.takes_a_question_mark() => {
                    format!("{}|null", kind.as_str())
                }
                _ => format!("?{}", self.type_ref(*inner)),
            },
            // `class-string<T>` is the only generic a docblock ever writes with a plain `string`
            // base -- see `phrost_lower::docblock::type_lowering::Lowerer::class_string` -- so
            // spelling it that way rather than as `string<T>`, which nobody wrote, needs no flag
            // of its own.
            TypeRefData::Generic { base, args, .. }
                if matches!(
                    self.hir.type_ref(*base),
                    TypeRefData::Builtin(BuiltinTy::String)
                ) =>
            {
                // A bare `class-string` is lowered as one of `object`; see the lowering.
                match self.hir.type_ref_list(*args) {
                    [only]
                        if matches!(
                            self.hir.type_ref(*only),
                            TypeRefData::Builtin(BuiltinTy::Object)
                        ) =>
                    {
                        "class-string".to_owned()
                    }
                    _ => format!("class-string<{}>", self.type_arguments(*args)),
                }
            }
            // `callable(...): T` -- a docblock's `callable(Foo): Bar` keeps only its return, so the
            // parameters are written as an ellipsis rather than invented. The same spelling a
            // first-class callable already renders with, and `callable<Bar>` is not a type PHP can
            // write.
            TypeRefData::Generic { base, args, .. }
                if matches!(
                    self.hir.type_ref(*base),
                    TypeRefData::Builtin(BuiltinTy::Callable)
                ) && self.hir.type_ref_list(*args).len() == 1 =>
            {
                format!("callable(...): {}", self.type_arguments(*args))
            }
            // `Item[]`, which is the spelling PHP writes most and the shorter one in a signature.
            // Only for a single element and only when it is a plain name: `Foo|Bar[]` binds the
            // brackets to `Bar` alone and `?Foo[]` to the array rather than the element, so a
            // composite keeps the unambiguous `array<...>` rather than gaining parentheses nobody
            // writes.
            TypeRefData::Generic {
                base,
                args,
                non_empty: false,
            } if matches!(
                self.hir.type_ref(*base),
                TypeRefData::Builtin(BuiltinTy::Array)
            ) && self.hir.type_ref_list(*args).len() == 1
                && self.brackets_bind(self.hir.type_ref_list(*args)[0]) =>
            {
                format!("{}[]", self.type_arguments(*args))
            }
            TypeRefData::Generic {
                args,
                non_empty: true,
                ..
            } => format!("non-empty-array<{}>", self.type_arguments(*args)),
            TypeRefData::Generic { base, args, .. } => {
                format!("{}<{}>", self.type_ref(*base), self.type_arguments(*args))
            }
            TypeRefData::Union(_) if self.spelling == Spelling::Reader => self.flat_union(id),
            TypeRefData::Union(parts) => self.type_list(*parts, "|"),
            TypeRefData::Intersection(parts) => self.type_list(*parts, "&"),
            TypeRefData::Shape { fields, open, list } => format!(
                "{}{{{}{}}}",
                if *list { "list" } else { "array" },
                self.type_arguments(*fields),
                if *open { ", ..." } else { "" }
            ),
            TypeRefData::ObjectShape { fields } => {
                format!("object{{{}}}", self.type_arguments(*fields))
            }
            TypeRefData::ShapeField { key, optional, ty } => format!(
                "{key}{}: {}",
                if *optional { "?" } else { "" },
                self.type_ref(*ty)
            ),
        }
    }

    /// A union as PHP writes it: one flat list, with any null at the end.
    ///
    /// [`Spelling::Reader`] only. The docblock grammar's `|` is binary, so `string|int|null` is
    /// lowered as `string|(int|null)` -- the same type the author wrote, and a tree the canonical
    /// rendering has to keep showing as it is. A person reading hover does not: the parentheses are
    /// an artefact of the parse, and `?` inside a union is a spelling PHP has no way to express.
    fn flat_union(&self, id: TypeRefId) -> String {
        let mut operands = Vec::new();
        let mut nullable = false;
        self.spread(id, &mut operands, &mut nullable);
        Self::fold_bool(&mut operands);

        match (operands.as_slice(), nullable) {
            ([], _) => "null".to_owned(),
            // The `?` shortcut only for the operands that can take one: `?false` is not a type PHP
            // can parse, so a boolean literal falls through to the union spelling below. See
            // `BuiltinTy::takes_a_question_mark`.
            ([only], true)
                if BuiltinTy::from_name(only).is_none_or(BuiltinTy::takes_a_question_mark) =>
            {
                format!("?{only}")
            }
            (_, true) => format!("{}|null", operands.join("|")),
            (_, false) => operands.join("|"),
        }
    }

    /// `true|false` is `bool`, and so is `bool|true`.
    ///
    /// The fold the two type constructors make -- `Held::union` and the interned table's -- applied
    /// here rather than in lowering, and that is the one decision in this function worth stating: the
    /// arena keeps the type as the *author wrote it*, and the canonical spelling exists to show
    /// exactly that. So a declaration reading `@param true|false` still renders verbatim where a
    /// declaration is being shown, and reads as `bool` only here, where a `?` inside a union is
    /// already rewritten for the same reason.
    fn fold_bool(operands: &mut Vec<String>) {
        let held = |wanted: &str| operands.iter().any(|text| text == wanted);
        let (has_true, has_false, has_bool) = (held("true"), held("false"), held("bool"));
        if !((has_true && has_false) || (has_bool && (has_true || has_false))) {
            return;
        }

        let mut stood_in = has_bool;
        operands.retain_mut(|text| match text.as_str() {
            "true" | "false" if stood_in => false,
            "true" | "false" => {
                stood_in = true;
                *text = "bool".to_owned();
                true
            }
            _ => true,
        });
    }

    /// One operand's contribution to a flattened union, rendered.
    ///
    /// Deduplicated by the text rather than by the id: the same type written twice in one docblock
    /// is two arena nodes, and what a reader would notice is the repeated word.
    fn spread(&self, id: TypeRefId, out: &mut Vec<String>, nullable: &mut bool) {
        match self.hir.type_ref(id) {
            TypeRefData::Builtin(BuiltinTy::Null) => *nullable = true,
            TypeRefData::Nullable(inner) => {
                *nullable = true;
                self.spread(*inner, out, nullable);
            }
            TypeRefData::Union(parts) => {
                for &part in self.hir.type_ref_list(*parts) {
                    self.spread(part, out, nullable);
                }
            }
            _ => {
                let text = self.type_operand(id);
                if !out.contains(&text) {
                    out.push(text);
                }
            }
        }
    }

    /// Whether a trailing `[]` would bind to the whole of this type.
    ///
    /// It binds tighter than `|`, `&` and `?`, so only a type with no operator at its top level can
    /// take the suffix and still mean what it meant.
    fn brackets_bind(&self, ty: TypeRefId) -> bool {
        !matches!(
            self.hir.type_ref(ty),
            TypeRefData::Union(_)
                | TypeRefData::Intersection(_)
                | TypeRefData::Nullable(_)
                | TypeRefData::Shape { .. }
                | TypeRefData::ObjectShape { .. }
        )
    }

    fn type_list(&self, parts: lspkit_arena::IdSlice<TypeRefId>, separator: &str) -> String {
        self.hir
            .type_ref_list(parts)
            .iter()
            .map(|&id| self.type_operand(id))
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// The arguments of a generic, or the fields of a shape.
    ///
    /// Unparenthesized, unlike an operand of a union: `<`, `,` and `>` already say where each one
    /// ends, so `array<(A|B)>` writes a bracket the reader has to undo and `array<A|B>` cannot be
    /// read any other way.
    fn type_arguments(&self, parts: lspkit_arena::IdSlice<TypeRefId>) -> String {
        self.hir
            .type_ref_list(parts)
            .iter()
            .map(|&id| self.type_ref(id))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// One operand of a union or intersection, parenthesized if it is itself composite.
    ///
    /// PHP only allows this nesting in DNF (`(A&B)|C`), and without the parens the rendering of
    /// `(A&B)|C` and a hypothetical `A&(B|C)` would be identical -- so a flattening bug would not
    /// show up in any snapshot.
    fn type_operand(&self, id: TypeRefId) -> String {
        match self.hir.type_ref(id) {
            TypeRefData::Union(_) | TypeRefData::Intersection(_) => {
                format!("({})", self.type_ref(id))
            }
            _ => self.type_ref(id),
        }
    }

    /// A written name, with its anchor made visible.
    ///
    /// `\` prefixes a fully-qualified name and `namespace\` an explicitly-current-namespace one,
    /// exactly as the source would. Losing that distinction in the snapshot would hide the class
    /// of bug where lowering forgets an anchor and the def-map then resolves against imports it
    /// should have skipped.
    fn name_ref(&self, id: NameRefId) -> String {
        let NameRefData { path, anchor, .. } = self.hir.name_ref(id);
        if self.spelling == Spelling::Reader {
            // The anchor goes with the namespace it anchors. `\\` and `namespace\\` say where to
            // start looking for a name that is no longer being shown.
            return phrost_model::short_name(path.as_str()).to_owned();
        }
        match anchor {
            NameAnchor::FullyQualified => format!("\\{path}"),
            NameAnchor::Relative => path.to_string(),
            NameAnchor::CurrentNamespace => format!("namespace\\{path}"),
        }
    }

    fn name_list(&self, ids: &[NameRefId]) -> String {
        ids.iter()
            .map(|&id| self.name_ref(id))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn doc(&self, docblock: Option<DocblockId>) -> &'static str {
        match docblock {
            Some(_) => "  [doc]",
            None => "",
        }
    }

    fn range(&self, range: TextRange) -> String {
        let Some(lines) = self.lines else {
            return format!("{}..{}", u32::from(range.start()), u32::from(range.end()));
        };

        let (start, end) = lines.range_line_cols(range);
        format!(
            "{}:{}-{}:{}",
            start.line + 1,
            start.col + 1,
            end.line + 1,
            end.col + 1
        )
    }
}
