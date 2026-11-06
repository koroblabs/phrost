//! What a docblock for a declaration would have to say.
//!
//! The parts rather than the text: how a project lays a docblock out is a house style, and composing
//! text is the feature layer's business. What is here is what the index knows -- the parameters, the
//! type each takes, and what the declaration returns -- plus the two things a text has to line up
//! with: where it goes, and how far it is indented.

use phrost_model::{Name, TextRange, TextSize};
use phrost_sema::MemberKind;

use crate::view::IndexView;

/// A declaration a docblock could be written for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Documentable {
    /// The declaration's own name, which is what a summary is derived from.
    pub name: Name,
    /// The first byte of the declaration, which is where a docblock goes above it.
    ///
    /// Past the indentation, because the text written here supplies its own -- and the range being
    /// inserted at has to start where a line's content does or the block lands half a line over.
    pub at: TextSize,
    /// The declaration's own indentation, so the block lines up with what it describes.
    pub indent: String,
    pub params: Vec<DocumentedParam>,
    /// What it returns, rendered and fully qualified. `None` when it returns nothing worth a tag --
    /// `void`, `never`, or a declaration that says nothing at all.
    pub returns: Option<String>,
    /// Every class the body throws with a `throw new`, fully qualified, first occurrence first.
    ///
    /// What a `@throws` names. Only what is written in this body: a callee's own throws are its
    /// docblock's business, and reading them here would document the whole call tree.
    pub throws: Vec<Name>,
    /// Whether it already has a docblock. Offering to write a second one would be offering to break
    /// the file.
    pub documented: bool,
}

/// One parameter, as a `@param` would name it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedParam {
    /// Without the leading `$`, as names are stored everywhere else.
    pub name: Name,
    /// Rendered and fully qualified. `None` for a parameter nothing declares a type for, where a
    /// generated tag has to say `mixed` rather than invent one.
    pub ty: Option<String>,
}

impl IndexView<'_> {
    /// The declaration at `offset`, when the cursor is on one a docblock could describe.
    ///
    /// A method or a function, and nothing else: a property's docblock is one tag and a class's is
    /// prose, and neither is something a generator can fill in from what is declared.
    pub fn documentable_at(&self, offset: TextSize) -> Option<Documentable> {
        let target = self.cursor_target(offset)?;
        let (owner, name, kind) = match target {
            crate::CursorTarget::MemberDeclaration {
                owner, name, kind, ..
            } => (owner, name, kind),
            crate::CursorTarget::Declaration {
                fqn,
                kind: phrost_model::SymbolKind::Function,
                ..
            } => return self.documentable_function(&fqn),
            _ => return None,
        };
        if kind != MemberKind::Method {
            return None;
        }

        let table = self.members(owner.as_str());
        let member = table.find(kind, name.as_str())?;
        let types = self.skeleton.types();
        let row = &member.row;

        // The body is in the open buffer's HIR, joined on the name range -- the one field both
        // tiers keep, see `IndexView::bodies_by_name`.
        let hir = self.current()?.hir();
        let body = hir
            .classes()
            .flat_map(|(_, class)| hir.member_list(class.members).iter())
            .map(|&id| hir.member(id))
            .find(|member| member.name_range == row.name_range)
            .and_then(|member| member.body);

        let returns = self.documented_return(hir, row.effective_ty(types), body);
        let throws = body
            .map(|body| self.thrown_classes(hir, body))
            .unwrap_or_default()
            .into_iter()
            .map(|(class, _)| class)
            .fold(Vec::new(), |mut distinct, class| {
                if !distinct.contains(&class) {
                    distinct.push(class);
                }
                distinct
            });

        Some(Documentable {
            name: row.name.clone(),
            at: row.full_range.start(),
            indent: self.indentation_at(row.full_range.start()),
            params: row
                .params
                .iter()
                .map(|param| DocumentedParam {
                    name: param.name.clone(),
                    ty: param
                        .effective_ty(types)
                        .map(|ty| types.render_qualified(ty)),
                })
                .collect(),
            returns,
            throws,
            documented: row.docblock_range.is_some(),
        })
    }

    /// A free function of the current file, by its qualified name.
    ///
    /// The other half of what a method gets, and the one a Drupal module is made of: a `.module`
    /// file is functions, so with this missing every docblock offer in one was silent while the
    /// rule reporting the gap fired -- a report with no fix, on exactly the files the rule was
    /// written for. Types come from the skeleton's row, which has them interned; ranges and the
    /// body from the buffer's HIR, joined on the name range as a method's are.
    fn documentable_function(&self, fqn: &Name) -> Option<Documentable> {
        let file = self.skeleton.file(self.current_file()?)?;
        let index = file
            .scope()
            .symbols()
            .iter()
            .position(|symbol| symbol.fqn == *fqn)?;
        let symbol = &file.scope().symbols()[index];
        let row = file.function_of(u32::try_from(index).ok()?)?;
        let types = self.skeleton.types();
        let hir = self.current()?.hir();
        let declared = hir
            .functions()
            .map(|(_, function)| function)
            .find(|function| function.name_range == symbol.name_range)?;

        let returns = self.documented_return(hir, row.effective_ty(types), declared.body);
        let throws = declared
            .body
            .map(|body| self.thrown_classes(hir, body))
            .unwrap_or_default()
            .into_iter()
            .map(|(class, _)| class)
            .fold(Vec::new(), |mut distinct, class| {
                if !distinct.contains(&class) {
                    distinct.push(class);
                }
                distinct
            });

        Some(Documentable {
            name: declared.name.clone(),
            at: declared.full_range.start(),
            indent: self.indentation_at(declared.full_range.start()),
            params: row
                .params()
                .iter()
                .map(|param| DocumentedParam {
                    name: param.name.clone(),
                    ty: param
                        .effective_ty(types)
                        .map(|ty| types.render_qualified(ty)),
                })
                .collect(),
            returns,
            throws,
            documented: declared.docblock.is_some(),
        })
    }

    /// What a `@return` should say, from the declaration and from what the body hands back.
    ///
    /// Fully qualified, which is what a docblock is for: it is read where the file's `use` lines
    /// are not in view -- by a generator, by another editor, by whoever inherits the class -- and a
    /// short name there is ambiguous in a way it is not in the signature above it.
    ///
    /// One place for both a method and a free function, because they answer the same question and
    /// a Drupal module is made of the second kind -- two copies is how one of them comes to be
    /// missing what the other has.
    fn documented_return(
        &self,
        hir: &phrost_hir::FileHir,
        declared: Option<phrost_ty::TypeId>,
        body: Option<phrost_hir::BodyId>,
    ) -> Option<String> {
        let types = self.skeleton.types();
        let declared = declared?;
        let rendered = types.render_qualified(declared);
        // `void` and `never` are what a declaration says when there is nothing to describe, and a
        // `@return void` is a line that repeats the signature and says nothing.
        if matches!(rendered.as_str(), "void" | "never") {
            return None;
        }
        // A declaration that already says what is in the array is left alone: `array<int, Row>` and
        // `array{id: int}` are claims their author made, and this is a *refinement* of the one
        // spelling that makes no claim at all rather than a second opinion about the others.
        let Some(nullable) = bare_array(types, declared) else {
            return Some(rendered);
        };
        // A body this cannot read keeps the word the signature wrote. The refinement is worth
        // having only where it is certain, and its absence must cost nothing.
        let Some(shape) = body.and_then(|body| self.returned_shape(hir, body)) else {
            return Some(rendered);
        };
        // The `?` is the declaration's, not the body's: what the body returns says what is in the
        // array, and whether there may be no array at all is a thing the signature already settled.
        Some(match nullable {
            true => format!("?{shape}"),
            false => shape,
        })
    }

    /// Every class a body throws with a `throw new` and does not catch on the spot, resolved, with
    /// where each throw is written.
    ///
    /// Resolved at the throw rather than at the declaration, because that is where the name was
    /// written and a `self` inside a closure means something else -- though the walk stays out of
    /// closures anyway. A `new` whose class does not resolve is left out rather than written as a
    /// name nothing declares.
    ///
    /// **Caught means not thrown.** `try { throw new X(); } catch (X $e) {}` throws nothing out of
    /// the declaration, and a docblock that declared `@throws X` for it would be documenting a
    /// thing that cannot happen. Left out for a year, so every such throw was reported -- and
    /// Drupal code wraps a throw in a handler constantly.
    pub(crate) fn thrown_classes(
        &self,
        hir: &phrost_hir::FileHir,
        body: phrost_hir::BodyId,
    ) -> Vec<(Name, TextRange)> {
        let statements = hir.body(body).statements;
        let scopes = phrost_body::catch_scopes(hir, statements);
        phrost_body::throw_sites(hir, statements)
            .into_iter()
            .filter_map(|site| {
                let class = self.class_of_target(site.range.start(), hir, site.class)?;
                (!self.caught_by(hir, &scopes, &class, site.range)).then_some((class, site.range))
            })
            .collect()
    }

    /// Whether some `try` this throw sits inside catches it.
    ///
    /// Through [`Self::throw_documented`], which is the same rule asked in the other direction: a
    /// `catch` covers a subclass of what it names exactly as a `@throws` does, and writing a second
    /// walk of the hierarchy here would be two constructors of one rule waiting to disagree. The
    /// names are compared as written, which is also that rule's decision and its reason.
    pub(crate) fn caught_by(
        &self,
        hir: &phrost_hir::FileHir,
        scopes: &[phrost_body::CatchScope],
        class: &Name,
        at: TextRange,
    ) -> bool {
        scopes
            .iter()
            .filter(|scope| scope.guarded.contains_range(at))
            .any(|scope| {
                let caught: Vec<String> = scope
                    .caught
                    .iter()
                    .map(|&name| hir.name_ref(name).path.to_string())
                    .collect();
                self.throw_documented(class, &caught)
            })
    }

    /// The whitespace at the start of the line `offset` is on.
    pub(crate) fn indentation_at(&self, offset: TextSize) -> String {
        let Some(open) = self.current() else {
            return String::new();
        };
        let text = open.text();
        let at = usize::from(offset).min(text.len());
        let line = text[..at].rfind('\n').map_or(0, |at| at + 1);
        text[line..at]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .collect()
    }
}

/// Whether a declared return says `array` and nothing whatever about what is in it.
///
/// `Some(true)` for `?array`, `Some(false)` for `array`, `None` for everything else -- including
/// `array<string, mixed>`, which names its key and element types and so is a claim rather than a
/// blank. Asked of the interned node rather than of the rendered text: two spellings reach the
/// same node, and a text match would answer for one of them.
fn bare_array(types: &phrost_ty::TypeTable, ty: phrost_ty::TypeId) -> Option<bool> {
    match types.get(ty) {
        phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Array) => Some(false),
        phrost_ty::TypeRef::Nullable(inner) => matches!(
            types.get(*inner),
            phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Array)
        )
        .then_some(true),
        _ => None,
    }
}
