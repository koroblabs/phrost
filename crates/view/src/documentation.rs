//! Every function-like in the open file, beside the docblock written above it.
//!
//! A projection for the checks about how code is *documented* rather than about what it means. They
//! need three things the rest of the index has no reason to put together: the parameters as
//! declared, whether the declaration returns anything, and the raw comment above it.
//!
//! Raw, and deliberately: interpreting a docblock is a second type language and belongs to the layer
//! that parses it properly. What a documentation rule compares is the *names* an author wrote against
//! the names they declared, and for that the text is the fact.

use phrost_hir::{FileHir, body::StmtData};
use phrost_model::{Name, TextRange, TextSize};

use crate::view::IndexView;

/// One function, method, or hook, with what documents it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedDeclaration {
    /// What it is called, which is what a rule about the *kind* of declaration this is reads.
    pub name: Name,
    /// The name alone, which is what a report about the declaration underlines.
    pub name_range: TextRange,
    /// The raw comment above it, `/**` and `*/` included, and where it is.
    ///
    /// `None` when there is none, which the rules read differently: an undocumented declaration is a
    /// different finding from one documented wrongly, and only a project that asked for the first
    /// hears about it.
    pub docblock: Option<Docblock>,
    /// Its parameters, in the order written.
    pub parameters: Vec<Parameter>,
    /// Whether it declares a return type that carries a value.
    ///
    /// `void` and `never` do not, and neither does an absent one -- so a `@return` is expected only
    /// where something comes back, which is the rule a person would apply reading it.
    pub returns_a_value: bool,
    /// Whether its body has a `return` with a value in it.
    ///
    /// Read for the declarations that state no return type at all, which on older code is most of
    /// them. Only the body's own statements: a `return` inside a closure written in it belongs to the
    /// closure.
    pub returns_from_body: bool,
    /// Every `throw new X` written in its body and not caught there, resolved, in source order.
    pub throws: Vec<Thrown>,
    /// The class that declares it, fully qualified. `None` for a free function.
    ///
    /// A `String` rather than a `Name`: building an interned name needs the interner open for
    /// writing, and this is a read-only projection. Nothing keyed by it is stored.
    ///
    /// What a rule needs to ask what this declaration *overrides*: a contract is written on the
    /// interface and an implementation inherits it, which is not a fact about the body.
    pub owner: Option<String>,
    /// Its body, for a rule that has to look inside one.
    ///
    /// `None` for an abstract or interface method, which has no body. Opaque: a feature does not
    /// see HIR ids, and what this is for is handing one back to the view that made it.
    pub body: Option<BodyRef>,
}

/// One property the open file declares, with the comment above it.
///
/// Apart from [`DocumentedDeclaration`] rather than a kind of it: every rule over those compares a
/// docblock against a *signature*, and a property has none -- a `@param` above one would read to
/// them as naming a parameter that does not exist.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedProperty {
    pub name: Name,
    pub name_range: TextRange,
    pub docblock: Option<Docblock>,
}

/// A handle to one declaration's body, to be given back to the view that issued it.
///
/// Carries its span as well as its id, because a reader that has to ask "is the cursor inside this
/// declaration" should not have to resolve anything to find out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BodyRef(pub(crate) phrost_hir::BodyId, TextRange);

impl BodyRef {
    /// The span of the body's own statements -- not the braces, and not the signature.
    pub fn span(self) -> TextRange {
        self.1
    }
}

/// One `throw new X(...)` in a body, as a documentation rule sees it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Thrown {
    pub class: Name,
    /// The whole `throw` expression, which is what a report underlines.
    pub range: TextRange,
}

/// One parameter, as far as a documentation rule is concerned.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Parameter {
    pub name: Name,
    pub name_range: TextRange,
}

/// What a docblock documents. See [`IndexView::documented_item_at`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DocumentedItem {
    Function,
    /// A method, a property or a class constant, with the class that declares it -- what asking
    /// about the declaration it overrides needs.
    Member {
        owner: String,
        kind: phrost_sema::MemberKind,
        name: Name,
    },
    ClassLike,
    /// A global constant, which overrides nothing.
    Constant,
    /// A docblock over a statement, which documents a variable: `/** @var Repo $repo */`.
    Statement,
}

/// A docblock a cursor is in, and the parameters of the declaration under it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocblockScope {
    /// The function or method the docblock documents.
    pub declaration: Name,
    pub docblock: Docblock,
    /// Its parameters, in the order written.
    pub parameters: Vec<Parameter>,
}

/// The comment above a declaration, as written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Docblock {
    pub text: Box<str>,
    pub range: TextRange,
}

impl IndexView<'_> {
    /// Whether a framework says a docblock reading like this documents itself somewhere else.
    ///
    /// Drupal's `Implements hook_form_alter().` is the case: the standard asks for that line and
    /// nothing more, because what the parameters mean is documented on the hook. Which prose means it
    /// is a fact about a framework and arrives from one -- nothing here names any.
    pub fn documents_elsewhere(&self, prose: &str) -> bool {
        self.skeleton.returns().documents_elsewhere(prose)
    }

    /// Whether a framework shows the docblock of a declaration called this to a person.
    ///
    /// Drupal's update hooks are the case: `update.php` prints the whole docblock as the pending
    /// update's description, so a `@return` generated into it is words in a sentence somebody
    /// reads. The docblock is required there and closed to tags -- which is the opposite pair from
    /// [`Self::documents_elsewhere`], where the prose names where the tags live instead.
    pub fn user_facing_docblock(&self, name: &str) -> bool {
        self.skeleton.returns().user_facing_docblock(name)
    }

    /// Every function-like the open file declares, with the comment above it.
    ///
    /// Empty when there is no open file, which is every cross-file question: a rule about how a
    /// declaration is written is a rule about the file somebody has in front of them.
    pub fn documented_declarations(&self) -> Vec<DocumentedDeclaration> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let mut found = Vec::new();
        for (_, function) in hir.functions() {
            found.push(DocumentedDeclaration {
                name: function.name.clone(),
                name_range: function.name_range,
                docblock: docblock_of(hir, function.docblock),
                parameters: parameters_of(hir, hir.param_list(function.params)),
                returns_a_value: returns_a_value(hir, function.return_type),
                returns_from_body: returns_from_body(hir, function.body),
                throws: self.thrown_in(hir, function.body),
                body: self.body_ref(hir, function.body),
                owner: None,
            });
        }
        for (_, class) in hir.classes() {
            // Qualified once per class rather than per member: what it is for is asking the index
            // what this declaration overrides, and the index is keyed by the fully-qualified name.
            let owner = self.qualified_name(class);
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.kind != phrost_hir::MemberKind::Method || member.from_docblock {
                    continue;
                }
                found.push(DocumentedDeclaration {
                    name: member.name.clone(),
                    name_range: member.name_range,
                    docblock: docblock_of(hir, member.docblock),
                    parameters: parameters_of(hir, hir.param_list(member.params)),
                    returns_a_value: returns_a_value(hir, member.type_ref),
                    returns_from_body: returns_from_body(hir, member.body),
                    throws: self.thrown_in(hir, member.body),
                    body: self.body_ref(hir, member.body),
                    owner: Some(owner.clone()),
                });
            }
        }

        // Source order, so a client's list reads top to bottom whatever order the arenas hold.
        found.sort_by_key(|declaration| declaration.name_range.start());
        found
    }

    /// Every property the open file's classes declare, with the comment above it -- one entry per
    /// declaration written, so `public $a, $b;` sharing a docblock is two.
    pub fn documented_properties(&self) -> Vec<DocumentedProperty> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut found = hir
            .classes()
            .flat_map(|(_, class)| hir.member_list(class.members).iter())
            .map(|&id| hir.member(id))
            .filter(|member| {
                member.kind == phrost_hir::MemberKind::Property && !member.from_docblock
            })
            .map(|member| DocumentedProperty {
                name: member.name.clone(),
                name_range: member.name_range,
                docblock: docblock_of(hir, member.docblock),
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|property| property.name_range.start());
        found
    }

    /// The function-like whose docblock `offset` is inside, and the parameters that docblock can name.
    ///
    /// The cursor's half of [`Self::documented_declarations`], without what that resolves for the
    /// documentation rules -- every `throw` in every body of the file -- which a keystroke in a
    /// comment has no use for. One answer for the three features that read `$name` in a docblock,
    /// so goto, hover and completion cannot disagree about which parameter it is.
    pub fn docblock_scope_at(&self, offset: TextSize) -> Option<DocblockScope> {
        let hir = self.current()?.hir();
        let inside = |docblock: Option<phrost_hir::DocblockId>| {
            docblock.is_some_and(|id| hir.docblock(id).range.contains(offset))
        };

        let functions = hir
            .functions()
            .map(|(_, function)| (&function.name, function.docblock, function.params));
        let methods = hir
            .classes()
            .flat_map(|(_, class)| hir.member_list(class.members).iter())
            .map(|&id| hir.member(id))
            .filter(|member| member.kind == phrost_hir::MemberKind::Method && !member.from_docblock)
            .map(|member| (&member.name, member.docblock, member.params));

        let (name, docblock, params) = functions
            .chain(methods)
            .find(|(_, docblock, _)| inside(*docblock))?;
        Some(DocblockScope {
            declaration: name.clone(),
            docblock: docblock_of(hir, docblock)?,
            parameters: parameters_of(hir, hir.param_list(params)),
        })
    }

    /// Every `$name` written in the docblock of the function that declares this parameter.
    ///
    /// `occurrences` are the local's own, from the body -- the parameter is the function whose
    /// signature holds one of them, which is what tells `$text` in `translate` from `$text` in the
    /// method below it. The tag line and the prose both count: they name the same parameter, which
    /// is the reading goto and hover give a cursor on either.
    pub fn docblock_mentions(&self, name: &str, occurrences: &[TextRange]) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let functions = hir
            .functions()
            .map(|(_, function)| (function.docblock, function.params));
        let methods = hir
            .classes()
            .flat_map(|(_, class)| hir.member_list(class.members).iter())
            .map(|&id| hir.member(id))
            .filter(|member| member.kind == phrost_hir::MemberKind::Method && !member.from_docblock)
            .map(|member| (member.docblock, member.params));

        let Some(docblock) = functions
            .chain(methods)
            .find(|(_, params)| {
                hir.param_list(*params).iter().any(|&id| {
                    let param = hir.param(id);
                    param.name.as_str() == name && occurrences.contains(&param.name_range)
                })
            })
            .and_then(|(docblock, _)| docblock)
        else {
            return Vec::new();
        };

        let docblock = hir.docblock(docblock);
        let written = format!("${name}");
        let text = &docblock.text;
        text.match_indices(written.as_str())
            .filter(|(at, _)| {
                // The whole name and not the start of a longer one: `$text` is not `$textLength`.
                !text[at + written.len()..]
                    .starts_with(|ch: char| ch.is_alphanumeric() || ch == '_')
            })
            .filter_map(|(at, _)| {
                let start = docblock.range.start() + TextSize::try_from(at).ok()?;
                Some(TextRange::at(start, TextSize::of(written.as_str())))
            })
            .collect()
    }

    /// What the docblock `offset` is inside documents, and where that docblock is.
    ///
    /// For a reader deciding which tags belong there: `@param` is about a function's parameters,
    /// `@var` about a property, `@method` about a class -- and a docblock over a statement, which no
    /// declaration owns, documents a variable.
    pub fn documented_item_at(&self, offset: TextSize) -> Option<(DocumentedItem, TextRange)> {
        let hir = self.current()?.hir();
        let range_of = |docblock: Option<phrost_hir::DocblockId>| {
            docblock
                .map(|id| hir.docblock(id).range)
                .filter(|range| range.contains(offset))
        };

        for (_, function) in hir.functions() {
            if let Some(range) = range_of(function.docblock) {
                return Some((DocumentedItem::Function, range));
            }
        }
        for (_, constant) in hir.constants() {
            if let Some(range) = range_of(constant.docblock) {
                return Some((DocumentedItem::Constant, range));
            }
        }
        for (_, class) in hir.classes() {
            if let Some(range) = range_of(class.docblock) {
                return Some((DocumentedItem::ClassLike, range));
            }
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.from_docblock {
                    continue;
                }
                let Some(range) = range_of(member.docblock) else {
                    continue;
                };
                let kind = match member.kind {
                    phrost_hir::MemberKind::Method => phrost_sema::MemberKind::Method,
                    phrost_hir::MemberKind::Property => phrost_sema::MemberKind::Property,
                    phrost_hir::MemberKind::ClassConstant => phrost_sema::MemberKind::ClassConstant,
                    phrost_hir::MemberKind::EnumCase => phrost_sema::MemberKind::EnumCase,
                };
                let item = DocumentedItem::Member {
                    owner: self.qualified_name(class),
                    kind,
                    name: member.name.clone(),
                };
                return Some((item, range));
            }
        }
        hir.docblocks()
            .map(|(_, docblock)| docblock.range)
            .find(|range| range.contains(offset))
            .map(|range| (DocumentedItem::Statement, range))
    }

    /// A handle to a body, with the span of its own statements.
    ///
    /// The statements and not a stored range, because that is the window every other reader of a
    /// body here works in -- the catch scopes are spanned the same way, and a range that took in
    /// the signature would put a default argument inside the body.
    fn body_ref(&self, hir: &FileHir, body: Option<phrost_hir::BodyId>) -> Option<BodyRef> {
        let body = body?;
        let statements = hir.stmt_list(hir.body(body).statements);
        Some(BodyRef(
            body,
            TextRange::new(
                hir.stmt(*statements.first()?).range().start(),
                hir.stmt(*statements.last()?).range().end(),
            ),
        ))
    }

    fn thrown_in(&self, hir: &FileHir, body: Option<phrost_hir::BodyId>) -> Vec<Thrown> {
        let Some(body) = body else {
            return Vec::new();
        };
        self.thrown_classes(hir, body)
            .into_iter()
            .map(|(class, range)| Thrown { class, range })
            .collect()
    }

    /// Whether a `@throws` written as `documented` covers a throw of `thrown`.
    ///
    /// The class itself, or anything it extends or implements: `@throws \Exception` covers a
    /// `RuntimeException`, and `@throws \Throwable` covers all of them, which is how a docblock is
    /// allowed to summarise. Compared by short name, because a docblock names a class the way its
    /// author saw it -- through the file's imports, or fully qualified -- and this is a rule about
    /// documentation, where two classes sharing a short name in one hierarchy is not a case worth a
    /// resolution per tag. A union `A|B` documents each of its operands.
    pub fn throw_documented(&self, thrown: &Name, documented: &[String]) -> bool {
        let short = |name: &str| {
            name.trim_start_matches('\\')
                .rsplit('\\')
                .next()
                .unwrap_or(name)
                .to_ascii_lowercase()
        };
        let mut covering = vec![short(thrown.as_str())];
        covering.extend(
            self.members(thrown.as_str())
                .contributors()
                .iter()
                .map(|contributor| short(contributor.as_str())),
        );
        documented
            .iter()
            .flat_map(|written| written.split('|'))
            .map(|written| short(written.trim().trim_start_matches('?')))
            // Nothing but a `Throwable` can be thrown, so it covers every class -- including one
            // the index never saw, whose ancestry is empty here: DocuSign's client declares a
            // dev dependency's `InvalidArgumentException`, and a `catch (\Throwable)` around the
            // call was read as not catching it.
            .any(|written| written == "throwable" || covering.contains(&written))
    }
}

fn docblock_of(hir: &FileHir, id: Option<phrost_hir::DocblockId>) -> Option<Docblock> {
    let data = hir.docblock(id?);
    Some(Docblock {
        text: data.text.clone(),
        range: data.range,
    })
}

fn parameters_of(hir: &FileHir, params: &[phrost_hir::ParamId]) -> Vec<Parameter> {
    params
        .iter()
        .map(|&id| hir.param(id))
        .map(|param| Parameter {
            name: param.name.clone(),
            name_range: param.name_range,
        })
        .collect()
}

/// Whether a declared return type carries a value back.
fn returns_a_value(hir: &FileHir, declared: Option<phrost_hir::TypeRefId>) -> bool {
    let Some(id) = declared else {
        return false;
    };
    !matches!(
        hir.type_ref(id),
        phrost_hir::TypeRefData::Builtin(
            phrost_model::BuiltinTy::Void | phrost_model::BuiltinTy::Never
        )
    )
}

/// Whether the body has a `return` with a value, not counting one inside a closure written in it.
fn returns_from_body(hir: &FileHir, body: Option<phrost_hir::BodyId>) -> bool {
    let Some(body) = body else {
        return false;
    };
    let mut returns = false;
    phrost_body::visit_every_statement(hir, hir.body(body).statements, &mut |hir, statement| {
        if let StmtData::Return { value: Some(_), .. } = hir.stmt(statement) {
            returns = true;
        }
    });
    returns
}
