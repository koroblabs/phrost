use lspkit_arena::IdSlice;
use mago_span::{HasSpan as _, Span};
use mago_syntax::cst::{
    AttributeList, Class, ClassLikeMember, Declare, DeclareBody, Enum, Expression, Extends,
    Identifier, Implements, Interface, Literal, Modifier, Namespace, NamespaceBody,
    PartialArgument, Sequence, Statement, Trait, Use, UseItem, UseItems,
};
use phrost_hir::{
    AttributeArgument, AttributeLiteralData, AttributeLiteralId, ClassData, ClassKind,
    ClassModifiers, ConstantData, DocblockData, FileHir, FileHirBuilder, FunctionData, ImportData,
    ImportKind, NameAnchor, NameRefData, NameRefId, NamespaceData, NamespaceId, TypeRefId,
};
use phrost_model::{FileId, Interner, Name, TextRange, TextSize, Visibility};
use phrost_syntax::{ParsedFile, identifier_text, text_range};

use crate::attributes::KeyedAttributes;

/// Carries everything one file's lowering needs, so the per-node functions stay short.
pub(crate) struct Lowerer<'a, 'p> {
    pub(crate) builder: FileHirBuilder,
    pub(crate) interner: &'a mut Interner,
    parsed: &'a ParsedFile<'p>,
    /// The namespace declarations are lexically nested in; `None` is the global namespace.
    current_namespace: Option<NamespaceId>,
    /// Written paths of allocated name refs, so type lowering can check a name against the
    /// reserved words without re-reading the source.
    name_ref_paths: Vec<Name>,
    /// Whether to lower statement bodies as well as declarations.
    ///
    /// Off for indexing: the skeleton covers every file in the project, and a body is both much
    /// larger than its declaration and never read to answer a cross-file question.
    pub(crate) lower_bodies: bool,
    /// The name reference each attribute's name was recorded as, by the start of that name.
    ///
    /// An attribute is written in a declaration and read as a call, and the two passes see it from
    /// opposite sides: the declaration pass is the only one that reaches the *name*, and the body
    /// pass the only one that lowers the *arguments*. Recording the name twice would put two
    /// find-references results at one position, so the id is carried across instead.
    ///
    /// Only collected when bodies are wanted, because only the body pass reads it.
    attribute_refs: Vec<(TextSize, NameRefId)>,
    /// Whether a `yield` has been seen in the body currently being lowered.
    ///
    /// Saved and cleared around each nested body, so a `yield` inside a closure is the closure's
    /// and not the enclosing function's -- which is the one thing a flag like this gets wrong.
    pub(crate) saw_yield: bool,
    /// Which attributes carry a key somebody above reads: whose positional literals are worth
    /// keeping, and whose older annotation spelling is worth looking for in a docblock.
    ///
    /// Handed down rather than known here -- see [`KeyedAttributes`].
    pub(crate) keyed_attributes: &'a KeyedAttributes,
    /// Whether the declaration walk is currently inside something PHP does not hoist.
    ///
    /// Set for the length of a nested walk and restored after it, so that everything declared
    /// under it is marked and the top-level statement after it is not. See `conditional.rs`.
    pub(crate) conditional: bool,
    /// What tells this file's anonymous classes from another file's, computed at most once.
    ///
    /// See [`Self::file_mark`] for why it is derived from the text.
    file_mark: std::cell::OnceCell<u32>,
    /// Whether the file mentions `func_get_args` at all, computed at most once.
    ///
    /// A file-level fact behind a per-body question, so a project that never writes it pays one
    /// search per file instead of one per declaration. See `own_arguments`.
    may_read_own_arguments: std::cell::OnceCell<bool>,
    /// Whether the file writes `yield` anywhere, computed at most once.
    ///
    /// The same arrangement [`Self::may_read_own_arguments`] has, and for the same reason: reading
    /// what a generator produces means walking a body's whole node tree, and all but a per cent of
    /// bodies are not generators. See `yields`.
    may_yield: std::cell::OnceCell<bool>,
    /// The `@phpstan-type` aliases of the class-like being lowered, for every docblock inside it.
    ///
    /// A member's own tags were handed the table explicitly, but a `/** @var Row $r */` in a method
    /// body, a contract's branch and an assertion's type were lowered with none -- so the alias read
    /// as a class nobody declares there and nowhere else. Saved and restored around each class, so an
    /// anonymous class inside a method has its own and the method gets its own back.
    pub(crate) class_aliases: std::rc::Rc<crate::docblock::type_alias::TypeAliasTable>,
}

impl<'a, 'p> Lowerer<'a, 'p> {
    /// A node's range, held to the text the caller wrote.
    ///
    /// A buffer mid-edit is cut anywhere, and the parse repairs it by writing what the cut took -- a
    /// closing parenthesis, a semicolon -- after the end, so a statement recovered that way ends past
    /// the last byte of the file. Every reader above slices the file's own text with these ranges,
    /// and an inlay hint over the last assignment of a truncated file failed with "end byte index
    /// 4688 is out of bounds for string of length 4686". Held here, where every range lowering hands
    /// out is made, rather than at every reader after it.
    pub(crate) fn within_the_text(&self, span: mago_span::Span) -> phrost_model::TextRange {
        let end = phrost_model::TextSize::try_from(self.parsed.written_len()).unwrap_or_default();
        let range = text_range(span);
        phrost_model::TextRange::new(range.start().min(end), range.end().min(end))
    }

    pub(crate) fn new(
        interner: &'a mut Interner,
        file: FileId,
        parsed: &'a ParsedFile<'p>,
        keyed_attributes: &'a KeyedAttributes,
    ) -> Self {
        Self {
            builder: FileHirBuilder::new(file),
            interner,
            parsed,
            current_namespace: None,
            name_ref_paths: Vec::new(),
            lower_bodies: false,
            attribute_refs: Vec::new(),
            saw_yield: false,
            keyed_attributes,
            conditional: false,
            may_read_own_arguments: std::cell::OnceCell::new(),
            may_yield: std::cell::OnceCell::new(),
            file_mark: std::cell::OnceCell::new(),
            class_aliases: std::rc::Rc::default(),
        }
    }

    /// A number that tells this file apart from every other, for naming an anonymous class.
    ///
    /// The synthesised name used to be the offset of the `class` keyword alone, which is unique
    /// inside one file and not in a project: Laravel generates every migration from one stub, so
    /// the `class` keyword lands on byte 146 in all of them and every migration in an application
    /// claimed one name. A catalog ranks two claimants for a name and the loser is invisible, so
    /// `$this->` inside one migration answered about another, and the reference list of its `up()`
    /// held a declaration from a different file. Measured: 5 of Laravel's own fixtures, 3 of 3 in a
    /// fresh `laravel new`, and one pair each in Symfony and psl.
    ///
    /// From the text rather than from the path or the file id, both of which would be right here
    /// and wrong somewhere else. A path is absolute, so it would put this machine's directories in
    /// a name the index dump prints and the disk cache stores; a file id is assigned by discovery
    /// order, so a from-scratch index and an incremental one would disagree about it -- and
    /// `incremental == from-scratch` compares those renderings. The text is the one input that is
    /// the same in every run, on every machine, and for an open buffer as much as for a file read
    /// off disk.
    ///
    /// FNV-1a and not a `DefaultHasher`, whose output the standard library does not promise across
    /// releases: the name reaches the disk cache, so a rebuilt binary reading a cache it wrote
    /// before has to produce the same one.
    ///
    /// Two byte-identical files still collide, and the classes they hold are then identical in
    /// everything but which file to jump to. Computed once per file and only for a file that
    /// declares one, so a project with no `new class` in it pays nothing.
    pub(crate) fn file_mark(&self) -> u32 {
        *self.file_mark.get_or_init(|| {
            let mut hash = 0x811c_9dc5_u32;
            for byte in self.parsed.source().as_bytes() {
                hash = (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193);
            }
            hash
        })
    }

    /// Also lower statement bodies. For open buffers.
    pub(crate) fn with_bodies(mut self) -> Self {
        self.lower_bodies = true;
        self
    }

    pub(crate) fn run(mut self) -> FileHir {
        let statements = self.parsed.program().statements.as_slice();

        if self.lower_bodies {
            self.builder.mark_bodies_lowered();
            // A second pass over the bytes, for the one thing the tree cannot answer: a string being
            // typed is not closed yet, so it does not parse. Paid only for a buffer someone has
            // open, which is the only place anyone is typing.
            let lexed = phrost_syntax::strings_and_comments(self.parsed.source());
            self.builder.set_string_ranges(lexed.strings);
            self.builder.set_comment_ranges(lexed.comments);
        }
        self.statements(statements);

        // After the declarations, because a name written at file scope has to be recorded in the
        // same arena and this pass is what puts it there.
        if self.lower_bodies {
            let root = self.root_body(statements);
            self.builder.set_root_body(root);
            // Over every docblock in the file rather than beside each kind of declaration, because
            // `@see` is written above classes, methods, properties, functions and constants alike
            // and a rule installed at one of those is a rule missing at the other four. For an open
            // buffer only: what it produces is read by goto, hover and find-references, all of which
            // ask about a cursor, and paying for it across a whole checkout would buy nothing.
            self.doc_sees_in_file();
        }

        self.builder.finish()
    }

    pub(crate) fn statements(&mut self, statements: &[Statement<'p>]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    pub(crate) fn statement(&mut self, statement: &Statement<'p>) {
        match statement {
            Statement::Namespace(namespace) => self.namespace(namespace),
            Statement::Use(use_statement) => self.use_statement(use_statement),
            Statement::Class(class) => self.class(class),
            Statement::Interface(interface) => self.interface(interface),
            Statement::Trait(trait_declaration) => self.trait_declaration(trait_declaration),
            Statement::Enum(enum_declaration) => self.enum_declaration(enum_declaration),
            Statement::Function(function) => self.function(function),
            Statement::Constant(constant) => self.constant(constant),
            // `define('NAME', ...)`, which declares a constant the same way and is what WordPress
            // is written in -- see `defined_constants`.
            Statement::Expression(expression) => self.defined_constant(expression),
            // A bare `{ ... }` block can hold declarations, and they are as visible as top-level
            // ones.
            Statement::Block(block) => self.statements(block.statements.as_slice()),
            Statement::Declare(declare) => self.declare(declare),
            // Everything that can wrap a statement list -- `if`, `switch`, `try`, the loops. What
            // they hold is declared only when the code around it runs, which `conditional.rs`
            // records and ranking then reads.
            other => self.wrapped_declarations(other),
        }
    }

    // --- file-level directives ---

    /// `declare(strict_types=1)`, and whatever a block-bodied `declare` wraps.
    ///
    /// PHP requires the strict-types directive to be the file's first statement and to be written
    /// in the plain `declare(...);` form -- a block body is a fatal error. Neither is enforced
    /// here: reporting a malformed directive is a diagnostic's job, and lowering the file to
    /// something other than what it says would leave that finding with nothing to point at.
    ///
    /// The body is walked for the same reason the `Block` arm above exists. `declare(ticks=1) {
    /// class Foo {} }` declares `Foo` exactly as visibly as the top level does, and dropping the
    /// body would lose it.
    fn declare(&mut self, declare: &Declare<'p>) {
        for item in declare.items.iter() {
            if !item.name.value.eq_ignore_ascii_case(b"strict_types") {
                continue;
            }

            // Only a literal `1` enables it. `strict_types=0` is the default the rest of the
            // engine already assumes, and PHP rejects every value that is not one of those two.
            if let Expression::Literal(Literal::Integer(value)) = item.value
                && value.value == Some(1)
            {
                self.builder.mark_strict_types();
            }
        }

        match &declare.body {
            DeclareBody::Statement(statement) => self.statement(statement),
            DeclareBody::ColonDelimited(body) => self.statements(body.statements.as_slice()),
        }
    }

    // --- namespaces and imports ---

    fn namespace(&mut self, namespace: &Namespace<'p>) {
        let name = namespace
            .name
            .as_ref()
            .map(|identifier| self.intern_path(identifier));
        let name_range = namespace.name.as_ref().map_or_else(
            || self.within_the_text(namespace.namespace.span()),
            |identifier| self.within_the_text(identifier.span()),
        );

        // A `namespace App;` with no braces governs everything after it, so its range runs to the
        // end of the file rather than to the last statement mago managed to recover. Those are the
        // same thing in a file that parses -- and different in one being typed into, where the last
        // statement is broken and the cursor sits past it. Stopping at the last statement means a
        // name written there resolves in the global namespace instead of this one, which is every
        // completion at the end of a script.
        let full_range = match &namespace.body {
            NamespaceBody::Implicit(_) => TextRange::new(
                self.within_the_text(namespace.span()).start(),
                TextSize::try_from(self.source_len()).unwrap_or_default(),
            ),
            NamespaceBody::BraceDelimited(_) => self.within_the_text(namespace.span()),
        };

        let id = self.builder.add_namespace(NamespaceData {
            name,
            name_range,
            full_range,
        });

        // Declarations inside record this namespace. Restoring the previous one afterwards matters
        // for brace-delimited blocks, where a file can hold several namespaces side by side.
        let previous = self.current_namespace.replace(id);
        match &namespace.body {
            NamespaceBody::Implicit(body) => self.statements(body.statements.as_slice()),
            NamespaceBody::BraceDelimited(block) => self.statements(block.statements.as_slice()),
        }
        self.current_namespace = previous;
    }

    /// Lowers a `use` statement, flattening every group and typed form into one import per name.
    fn use_statement(&mut self, use_statement: &Use<'p>) {
        let full_range = self.within_the_text(use_statement.span());

        match &use_statement.items {
            UseItems::Sequence(sequence) => {
                for item in sequence.items.iter() {
                    self.import(item, ImportKind::ClassLike, None, full_range);
                }
            }
            UseItems::TypedSequence(sequence) => {
                let kind = import_kind(&sequence.r#type);
                for item in sequence.items.iter() {
                    self.import(item, kind, None, full_range);
                }
            }
            UseItems::TypedList(list) => {
                let kind = import_kind(&list.r#type);
                let prefix = self.intern_path(&list.namespace);
                for item in list.items.iter() {
                    self.import(item, kind, Some(&prefix), full_range);
                }
            }
            UseItems::MixedList(list) => {
                let prefix = self.intern_path(&list.namespace);
                for entry in list.items.iter() {
                    let kind = entry
                        .r#type
                        .as_ref()
                        .map_or(ImportKind::ClassLike, import_kind);
                    self.import(&entry.item, kind, Some(&prefix), full_range);
                }
            }
        }
    }

    fn import(
        &mut self,
        item: &UseItem<'p>,
        kind: ImportKind,
        prefix: Option<&Name>,
        full_range: TextRange,
    ) {
        let tail = self.written_path(&item.name);
        let path = match prefix {
            Some(prefix) => self.interner.name(&format!("{prefix}\\{tail}")),
            None => self.interner.name(&tail),
        };

        let alias = item
            .alias
            .as_ref()
            .map(|alias| self.interner.name(&identifier_text(alias.identifier.value)));
        let alias_range = item
            .alias
            .as_ref()
            .map(|alias| self.within_the_text(alias.identifier.span()));
        // Every lookup wants the locally bound name, and re-splitting the path each time is pure
        // waste, so it is computed once here.
        let local = alias.clone().unwrap_or_else(|| {
            let short = path.as_str().rsplit('\\').next().unwrap_or(path.as_str());
            self.interner.name(short)
        });

        self.builder.add_import(ImportData {
            path,
            local,
            alias,
            kind,
            namespace: self.current_namespace,
            path_range: self.within_the_text(item.name.span()),
            alias_range,
            full_range,
        });
    }

    // --- class-like declarations ---

    fn class(&mut self, class: &Class<'p>) {
        let mut modifiers = class_modifiers(&class.modifiers);
        modifiers.is_attribute = names_an_attribute(&class.attribute_lists, "Attribute");
        self.attribute_names(&class.attribute_lists);
        self.class_like(ClassLikeParts {
            is_anonymous: false,
            kind: ClassKind::Class,
            modifiers,
            name: identifier_text(class.name.value).into_owned(),
            name_range: self.within_the_text(class.name.span()),
            extends: class.extends.as_ref(),
            implements: class.implements.as_ref(),
            enum_backing: None,
            members: &class.members,
            attributes: &class.attribute_lists,
            declaration_start: declaration_start(
                &class.attribute_lists,
                Some(&class.modifiers),
                class.class.span(),
            ),
            full_range: self.within_the_text(class.span()),
            body_range: self.within_the_text(class.left_brace.join(class.right_brace)),
        });
    }

    fn interface(&mut self, interface: &Interface<'p>) {
        self.attribute_names(&interface.attribute_lists);
        self.class_like(ClassLikeParts {
            is_anonymous: false,
            kind: ClassKind::Interface,
            modifiers: ClassModifiers::default(),
            name: identifier_text(interface.name.value).into_owned(),
            name_range: self.within_the_text(interface.name.span()),
            extends: interface.extends.as_ref(),
            implements: None,
            enum_backing: None,
            members: &interface.members,
            attributes: &interface.attribute_lists,
            declaration_start: declaration_start(
                &interface.attribute_lists,
                None,
                interface.interface.span(),
            ),
            full_range: self.within_the_text(interface.span()),
            body_range: self.within_the_text(interface.left_brace.join(interface.right_brace)),
        });
    }

    fn trait_declaration(&mut self, declaration: &Trait<'p>) {
        self.attribute_names(&declaration.attribute_lists);
        self.class_like(ClassLikeParts {
            is_anonymous: false,
            kind: ClassKind::Trait,
            modifiers: ClassModifiers::default(),
            name: identifier_text(declaration.name.value).into_owned(),
            name_range: self.within_the_text(declaration.name.span()),
            extends: None,
            implements: None,
            enum_backing: None,
            members: &declaration.members,
            attributes: &declaration.attribute_lists,
            declaration_start: declaration_start(
                &declaration.attribute_lists,
                None,
                declaration.r#trait.span(),
            ),
            full_range: self.within_the_text(declaration.span()),
            body_range: self.within_the_text(declaration.left_brace.join(declaration.right_brace)),
        });
    }

    fn enum_declaration(&mut self, declaration: &Enum<'p>) {
        self.attribute_names(&declaration.attribute_lists);
        let enum_backing = declaration
            .backing_type_hint
            .as_ref()
            .map(|backing| self.hint(&backing.hint));

        self.class_like(ClassLikeParts {
            is_anonymous: false,
            kind: ClassKind::Enum,
            modifiers: ClassModifiers::default(),
            name: identifier_text(declaration.name.value).into_owned(),
            name_range: self.within_the_text(declaration.name.span()),
            extends: None,
            implements: declaration.implements.as_ref(),
            enum_backing,
            members: &declaration.members,
            attributes: &declaration.attribute_lists,
            declaration_start: declaration_start(
                &declaration.attribute_lists,
                None,
                declaration.r#enum.span(),
            ),
            full_range: self.within_the_text(declaration.span()),
            body_range: self.within_the_text(declaration.left_brace.join(declaration.right_brace)),
        });
    }

    /// The one place a class-like declaration is turned into HIR.
    ///
    /// The four PHP forms differ only in which parts they may have, so they are normalized into
    /// [`ClassLikeParts`] first. Four near-identical functions would be four places to forget the
    /// same fix.
    pub(crate) fn class_like(&mut self, parts: ClassLikeParts<'_, 'p>) {
        let name = self.interner.name(&parts.name);
        let extends = self.name_ref_list_from(parts.extends.map(|extends| &extends.types));
        let implements =
            self.name_ref_list_from(parts.implements.map(|implements| &implements.types));
        let docblock = self.docblock(parts.declaration_start);
        let is_ignored = self.doc_is_ignored(parts.declaration_start);
        let is_deprecated = self.is_deprecated(parts.attributes, parts.declaration_start);

        let templates = self.docblock_templates(parts.declaration_start);
        // Before anything the class's docblock writes a type in: `@extends Base<RowShape>` names an
        // alias the same docblock declares, and lowered without the table it read as a class nobody
        // declared.
        let aliases = std::rc::Rc::new(self.docblock_type_aliases(parts.declaration_start));
        let outer_aliases =
            std::mem::replace(&mut self.class_aliases, std::rc::Rc::clone(&aliases));
        let supertype_arguments =
            self.docblock_supertype_arguments(parts.declaration_start, &aliases);
        let mixins = self.docblock_mixins(parts.declaration_start, &aliases);
        let supertype_arguments = self.with_mixin_arguments(supertype_arguments, mixins);
        let attribute_literals = self.attribute_literals(parts.attributes, parts.declaration_start);
        let class = self.builder.add_class(ClassData {
            name,
            kind: parts.kind,
            is_anonymous: parts.is_anonymous,
            namespace: self.current_namespace,
            modifiers: ClassModifiers {
                is_deprecated,
                ..parts.modifiers
            },
            extends,
            implements,
            uses: IdSlice::EMPTY,
            trait_adaptations: IdSlice::EMPTY,
            enum_backing: parts.enum_backing,
            members: IdSlice::EMPTY,
            templates,
            supertype_arguments,
            mixins,
            attribute_literals,
            name_range: parts.name_range,
            full_range: parts.full_range,
            body_range: parts.body_range,
            docblock,
            is_conditional: self.conditional,
            is_ignored,
        });

        self.keep_attribute_lists(class, parts.attributes);
        self.class_members(class, parts.members, parts.declaration_start, &aliases);
        self.class_aliases = outer_aliases;
    }

    /// The string list a kept attribute writes on a class: `#[Fillable(['name', 'email'])]`, or the
    /// same written as separate arguments, which the attribute's own constructor accepts too.
    fn keep_attribute_lists(
        &mut self,
        class: phrost_hir::ClassId,
        lists: &Sequence<'_, AttributeList<'p>>,
    ) {
        for list in lists.iter() {
            for attribute in list.attributes.iter() {
                let written = self.written_path(&attribute.name);
                let Some(kept) = self.keyed_attributes.kept_list(&written) else {
                    continue;
                };
                let property = self.interner.name(kept);
                let Some(arguments) = attribute.argument_list.as_ref() else {
                    continue;
                };
                let mut entries = Vec::new();
                for argument in arguments.arguments.iter() {
                    let PartialArgument::Positional(positional) = argument else {
                        continue;
                    };
                    let values: Vec<&Expression<'p>> = match positional.value {
                        Expression::Array(array) => array
                            .elements
                            .iter()
                            .filter_map(|element| match element {
                                mago_syntax::cst::ArrayElement::Value(entry) => Some(entry.value),
                                _ => None,
                            })
                            .collect(),
                        value => vec![value],
                    };
                    for value in values {
                        let Expression::Literal(mago_syntax::cst::Literal::String(listed)) = value
                        else {
                            continue;
                        };
                        let Some(text) = listed.value else {
                            continue;
                        };
                        let range = self.within_the_text(listed.span());
                        let one = TextSize::from(1);
                        entries.push(phrost_hir::KeptEntryData {
                            name: self.interner.name(&String::from_utf8_lossy(text)),
                            range: TextRange::new(range.start() + one, range.end() - one),
                            value: None,
                        });
                    }
                }
                self.builder.add_kept_table(phrost_hir::KeptTableData {
                    class,
                    property,
                    from_attribute: true,
                    entries: entries.into_boxed_slice(),
                });
            }
        }
    }

    // --- functions and constants ---

    fn function(&mut self, function: &mago_syntax::cst::Function<'p>) {
        let name = self.interner.name(&identifier_text(function.name.value));
        self.attribute_names(&function.attribute_lists);
        let start = declaration_start(&function.attribute_lists, None, function.function.span());
        let params = self.params(&function.parameter_list, start);
        let return_type = self.return_type(function.return_type_hint.as_ref());
        let doc_return_type =
            self.doc_type_beside(return_type, start, crate::docblock::DocTag::Return);
        let throws = self.doc_throws(start);
        let contract = self.contract_beside(start, params, &function.parameter_list);
        let docblock = self.docblock(start);
        let is_ignored = self.doc_is_ignored(start);
        let is_no_discard =
            names_an_attribute(&function.attribute_lists, crate::members::NO_DISCARD);
        let is_deprecated = self.is_deprecated(&function.attribute_lists, start);

        let body = self.declared_body(&function.body, start);
        // What the body hands back, when neither the signature nor a docblock says. Last, so the
        // two that are written win without this being computed at all.
        //
        // A `yield` answers the same question differently and silences the rest -- see `yields`.
        let yielding = match doc_return_type {
            None => self.yielded_from_body(&function.body, return_type),
            Some(_) => crate::yields::Yielding::No,
        };
        let doc_return_type = match yielding {
            crate::yields::Yielding::Produces(yielded) => Some(yielded),
            crate::yields::Yielding::Unreadable => None,
            crate::yields::Yielding::No => match (return_type, doc_return_type) {
                (None, None) => self.returned_from_body(&function.body, &function.parameter_list),
                _ => doc_return_type,
            },
        };
        // And the other half: a chain that *describes* a type without naming one. Only where
        // nothing above found anything, so the cheap readings win and this walk is not taken.
        let returned_path = (return_type.is_none()
            && doc_return_type.is_none()
            && matches!(yielding, crate::yields::Yielding::No))
        .then(|| self.returned_path_from_body(&function.body, &function.parameter_list))
        .flatten()
        .map(Box::new);

        self.builder.add_function(FunctionData {
            is_no_discard,
            is_deprecated,
            returned_path,
            throws,
            name,
            namespace: self.current_namespace,
            params,
            return_type,
            doc_return_type,
            body,
            reads_own_arguments: self.reads_own_arguments(&function.body),
            returns_by_reference: function.ampersand.is_some(),
            contract,
            name_range: self.within_the_text(function.name.span()),
            full_range: self.within_the_text(function.span()),
            docblock,
            is_conditional: self.conditional,
            is_ignored,
        });

        // After the declaration, so that a function declared inside this one is recorded as
        // conditional whatever this one is: `outer` may be hoisted, but `inner` exists only once
        // `outer` has run.
        self.body_declarations(&function.body);
    }

    fn constant(&mut self, constant: &mago_syntax::cst::Constant<'p>) {
        let full_range = self.within_the_text(constant.span());
        self.attribute_names(&constant.attribute_lists);
        let constant_start =
            declaration_start(&constant.attribute_lists, None, constant.r#const.span());
        let docblock = self.docblock(constant_start);
        let is_ignored = self.doc_is_ignored(constant_start);
        let is_deprecated = self.is_deprecated(&constant.attribute_lists, constant_start);

        // `const A = 1, B = 2;` declares two constants; each gets its own entry so lookup never
        // has to know they shared a statement.
        for item in constant.items.iter() {
            let name = self.interner.name(&identifier_text(item.name.value));
            // The value first, which is what the constant is; a `@var` where the value names
            // nothing -- `const PHP_EOL = PHP_EOL;` is how the stubs write every constant the
            // engine fills in, and the tag is the only place its type can be said.
            let ty = self
                .constant_value_type(item.value)
                .or_else(|| self.doc_type(constant_start, crate::docblock::DocTag::Var));
            let value = match ty {
                Some(_) => None,
                None => self.constant_value_parts(item.value),
            };
            self.builder.add_constant(ConstantData {
                name,
                namespace: self.current_namespace,
                name_range: self.within_the_text(item.name.span()),
                full_range,
                docblock,
                is_conditional: self.conditional,
                is_ignored,
                is_deprecated,
                ty,
                value,
            });
        }
    }

    // --- shared helpers ---

    pub(crate) fn return_type(
        &mut self,
        hint: Option<&mago_syntax::cst::FunctionLikeReturnTypeHint<'p>>,
    ) -> Option<TypeRefId> {
        hint.map(|hint| self.hint(&hint.hint))
    }

    /// Interns a written class-like name and records it, returning a HIR name reference.
    pub(crate) fn name_ref(&mut self, identifier: &Identifier<'_>) -> NameRefId {
        self.name_ref_of(identifier, ImportKind::ClassLike)
    }

    /// Records each attribute's name as a name reference.
    ///
    /// An attribute names a class, so goto, hover, find-references and rename all have to reach it.
    /// Nothing else in lowering sees that name: an attribute's *arguments* are expressions and are
    /// lowered with the rest of them, but the name is an identifier in a position no expression walk
    /// visits.
    ///
    /// Called from every declaration that can carry attributes, which in PHP is nearly all of them
    /// -- including a parameter and a promoted property. `attributes.rs` in the analysis tests walks
    /// that list, so a position left out fails a test rather than going quiet.
    pub(crate) fn attribute_names(&mut self, lists: &Sequence<'_, AttributeList<'p>>) {
        for list in lists.iter() {
            for attribute in list.attributes.iter() {
                let reference = self.name_ref(&attribute.name);
                if self.lower_bodies {
                    self.attribute_refs.push((
                        self.within_the_text(attribute.name.span()).start(),
                        reference,
                    ));
                }
            }
        }
    }

    /// The name reference already recorded for an attribute whose name starts at `start`.
    ///
    /// A reverse scan because the declaration pass records them in roughly source order and the body
    /// pass asks in source order, so the answer is near the end; a file holds a handful of these and
    /// a map would cost more to build than the scan costs to walk.
    pub(crate) fn attribute_ref_at(&self, start: TextSize) -> Option<NameRefId> {
        self.attribute_refs
            .iter()
            .rev()
            .find(|(at, _)| *at == start)
            .map(|(_, reference)| *reference)
    }

    /// The string literals an attribute list writes.
    ///
    /// `#[ContentEntityType(id: 'node')]` is the only thing that says this class is what
    /// `getStorage('node')` returns, and no signature anywhere in the framework says it. So the
    /// literals are kept and the index asks a framework which of them mean something.
    ///
    /// Every *named* one, because there are few enough of them that keeping all is cheaper than
    /// deciding: roughly a thousand in a thirty-thousand-file project. Positional ones are not like
    /// that -- the same project writes 8 402 of them, of which 77 mean anything -- so those are kept
    /// only for the attributes [`KeyedAttributes`] names, and that list is handed down from
    /// above rather than known here.
    pub(crate) fn attribute_literals(
        &mut self,
        lists: &Sequence<'_, AttributeList<'p>>,
        declaration_start: TextSize,
    ) -> IdSlice<AttributeLiteralId> {
        let mut ids = Vec::new();
        for list in lists.iter() {
            for attribute in list.attributes.iter() {
                let written = self.written_path(&attribute.name);
                let name = self.interner.name(&written);
                let Some(arguments) = attribute.argument_list.as_ref() else {
                    continue;
                };
                // The cheap gate first: most attributes are not ones any framework asked about, and
                // testing that costs a handful of short-string comparisons against a list that is
                // empty for a project with no framework at all.
                let wanted = self.keyed_attributes.wants(&written);
                for (at, argument) in arguments.arguments.iter().enumerate() {
                    let (slot, value) = match argument {
                        PartialArgument::Named(named) => (
                            AttributeArgument::Named(
                                self.interner.name(&identifier_text(named.name.value)),
                            ),
                            named.value,
                        ),
                        // A spread writes no argument of its own -- `...$ids` fills however many
                        // the list has left -- so there is no position for a contract to name.
                        PartialArgument::Positional(positional)
                            if wanted && positional.ellipsis.is_none() =>
                        {
                            (
                                AttributeArgument::At(u16::try_from(at).unwrap_or(u16::MAX)),
                                positional.value,
                            )
                        }
                        _ => continue,
                    };
                    // An array argument a rule reads the keys of: one literal per entry, carrying
                    // the key it was written under. Gated twice over -- the attribute has to be one
                    // somebody asked about *and* the argument one whose entries they read -- because
                    // this is the one place lowering keeps a string per array element.
                    if wanted
                        && let AttributeArgument::Named(argument) = &slot
                        && self.keyed_attributes.wants_entries_of(argument.as_str())
                        && let Expression::Array(array) = value
                    {
                        for element in array.elements.iter() {
                            let mago_syntax::cst::ArrayElement::KeyValue(entry) = element else {
                                continue;
                            };
                            let (Some(key), Some(written)) =
                                (string_literal(entry.key), string_literal(entry.value))
                            else {
                                continue;
                            };
                            let data = AttributeLiteralData {
                                attribute: name.clone(),
                                argument: slot.clone(),
                                value: written,
                                key: Some(key),
                            };
                            ids.push(self.builder.add_attribute_literal(data));
                        }
                        continue;
                    }
                    let Some(value) = string_literal(value) else {
                        continue;
                    };
                    let data = AttributeLiteralData {
                        attribute: name.clone(),
                        argument: slot,
                        value,
                        key: None,
                    };
                    ids.push(self.builder.add_attribute_literal(data));
                }
            }
        }
        // The same declaration written the older way. Into the same list, because it is the same
        // fact: nothing above here learns that an attribute has two spellings.
        for data in self.annotation_literals(declaration_start) {
            ids.push(self.builder.add_attribute_literal(data));
        }
        self.builder.alloc_attribute_literal_list(&ids)
    }

    /// The same, for a name that belongs to one of PHP's other two symbol tables.
    pub(crate) fn name_ref_of(
        &mut self,
        identifier: &Identifier<'_>,
        kind: ImportKind,
    ) -> NameRefId {
        let (anchor, path) = self.written_name(identifier);
        self.add_name_ref(NameRefData {
            path,
            anchor,
            kind,
            range: self.within_the_text(identifier.span()),
            in_docblock: false,
            cross_reference: false,
        })
    }

    /// A written name as a path and the anchor it is resolved from, without recording a reference.
    ///
    /// For a name that has to be resolved later and is not a *use* of anything a reader should
    /// see twice -- a `Foo::class` carried with a returned chain is the same written name its own
    /// expression already references.
    pub(crate) fn written_name(&mut self, identifier: &Identifier<'_>) -> (NameAnchor, Name) {
        let written = self.written_path(identifier);
        let (anchor, path) = match identifier {
            Identifier::FullyQualified(_) => (NameAnchor::FullyQualified, written.as_str()),
            // PHP's `namespace\Foo` operator arrives as an ordinary qualified name, so the anchor
            // has to be recovered from the first segment. Missing it would make the def-map
            // consult imports for a name that explicitly asked to skip them.
            _ => match written.strip_prefix("namespace\\") {
                Some(rest) => (NameAnchor::CurrentNamespace, rest),
                None => (NameAnchor::Relative, written.as_str()),
            },
        };
        (anchor, self.interner.name(path))
    }

    /// Length of the source being lowered, for the file-scope body's range.
    ///
    /// The *parsed* source, which for a repaired tail is a few bytes longer than the file. That is
    /// the honest number here: the tree really does reach there, and a body stopping short of a
    /// statement it contains would break the invariant that every statement lies inside its body.
    /// The synthetic bytes are past every position a cursor can occupy, so nothing can be asked
    /// about them.
    pub(crate) fn source_len(&self) -> usize {
        self.parsed.source().len()
    }

    /// The source between a span's ends, when it is inside the file and is not empty.
    ///
    /// For the facts whose *text* is what a reader wants rather than their structure -- a
    /// parameter's default, which one action writes back into a file verbatim.
    pub(crate) fn written_between(&self, span: mago_span::Span) -> Option<Box<str>> {
        let range = phrost_syntax::text_range(span);
        let text = self
            .parsed
            .source()
            .get(usize::from(range.start())..usize::from(range.end()))?
            .trim();
        (!text.is_empty()).then(|| text.into())
    }

    /// The file's text, for the one reader that answers a question about a *body* on a file whose
    /// bodies are not lowered. See `own_arguments`.
    pub(crate) fn parsed_source(&self) -> &'p str {
        self.parsed.source()
    }

    /// Whether the file mentions `func_get_args`/`func_num_args` anywhere.
    ///
    /// The prefilter for [`Self::reads_own_arguments`], asked once per body and answering from one
    /// search of the file rather than one per declaration.
    pub(crate) fn file_may_read_own_arguments(&self) -> bool {
        *self
            .may_read_own_arguments
            .get_or_init(|| self.parsed.source().contains(crate::own_arguments::PREFIX))
    }

    /// Whether the file writes `yield` anywhere.
    ///
    /// The prefilter for [`Self::yielded_from_body`], asked once per body and answering from one
    /// search of the file rather than one walk per declaration.
    pub(crate) fn file_may_yield(&self) -> bool {
        *self
            .may_yield
            .get_or_init(|| self.parsed.source().contains(crate::yields::KEYWORD))
    }

    /// Allocates a name reference and records its written path.
    ///
    /// The one place that does: the path list is indexed by name-reference id, so adding one
    /// without recording its path silently shifts every later lookup.
    pub(crate) fn add_name_ref(&mut self, data: NameRefData) -> NameRefId {
        let path = data.path.clone();
        let id = self.builder.add_name_ref(data);
        debug_assert_eq!(self.name_ref_paths.len(), id_index(id));
        self.name_ref_paths.push(path);
        id
    }

    /// The docblocks written entirely inside `window`, in position order.
    ///
    /// Asked by region rather than by attachment, unlike [`Self::raw_docblock_before`]: the
    /// statement a docblock was written for may have been dropped by error recovery, and then
    /// there is nothing left to attach it to. Binary search on both ends, because this is asked
    /// per body and most bodies own no docblock at all.
    pub(crate) fn raw_docblocks_within(
        &self,
        window: TextRange,
    ) -> Vec<phrost_syntax::Docblock<'a>> {
        let blocks = self.parsed.docblocks();
        let first = blocks.partition_point(|block| block.range.start() < window.start());
        blocks[first..]
            .iter()
            .copied()
            .take_while(|block| block.range.end() <= window.end())
            .collect()
    }

    /// Every docblock in the file, in position order.
    ///
    /// For a reader whose tag is written above more than one kind of declaration, where asking per
    /// declaration means remembering every kind that can carry it. Unlike
    /// [`Self::raw_docblocks_within`] this needs no window, because the question is about the file.
    pub(crate) fn raw_docblocks(&self) -> Vec<phrost_syntax::Docblock<'a>> {
        self.parsed.docblocks().to_vec()
    }

    /// The docblock immediately above a declaration, as written.
    pub(crate) fn raw_docblock_before(
        &self,
        declaration_start: TextSize,
    ) -> Option<phrost_syntax::Docblock<'a>> {
        self.parsed.docblock_before(declaration_start)
    }

    /// The written path of an already-lowered name reference.
    pub(crate) fn hir_name_ref_path(&self, id: NameRefId) -> Name {
        self.name_ref_paths[id_index(id)].clone()
    }

    fn name_ref_list_from(
        &mut self,
        types: Option<&mago_syntax::cst::TokenSeparatedSequence<'p, Identifier<'p>>>,
    ) -> IdSlice<NameRefId> {
        let Some(types) = types else {
            return IdSlice::EMPTY;
        };

        let ids = types
            .iter()
            .map(|identifier| self.name_ref(identifier))
            .collect::<Vec<_>>();
        self.builder.alloc_name_ref_list(&ids)
    }

    pub(crate) fn docblock(
        &mut self,
        declaration_start: TextSize,
    ) -> Option<phrost_hir::DocblockId> {
        let docblock = self.parsed.docblock_before(declaration_start)?;
        Some(self.builder.add_docblock(DocblockData {
            text: docblock.text.into(),
            range: docblock.range,
        }))
    }

    /// The name as written, with any leading separator removed.
    fn written_path(&self, identifier: &Identifier<'_>) -> String {
        identifier_text(identifier.value())
            .trim_start_matches('\\')
            .to_owned()
    }

    fn intern_path(&mut self, identifier: &Identifier<'_>) -> Name {
        let path = self.written_path(identifier);
        self.interner.name(&path)
    }
}

/// The four class-like forms, normalized to the union of their parts.
pub(crate) struct ClassLikeParts<'a, 'p> {
    pub(crate) kind: ClassKind,
    pub(crate) modifiers: ClassModifiers,
    pub(crate) name: String,
    pub(crate) is_anonymous: bool,
    pub(crate) name_range: TextRange,
    pub(crate) extends: Option<&'a Extends<'p>>,
    pub(crate) implements: Option<&'a Implements<'p>>,
    pub(crate) enum_backing: Option<TypeRefId>,
    pub(crate) members: &'a Sequence<'p, ClassLikeMember<'p>>,
    /// The declaration's attributes, for the literals a framework may name -- see
    /// [`Lowerer::attribute_literals`].
    pub(crate) attributes: &'a Sequence<'p, AttributeList<'p>>,
    /// First byte of the whole declaration, attributes included -- what docblock attachment needs.
    pub(crate) declaration_start: TextSize,
    pub(crate) full_range: TextRange,
    /// The braces and what is between them -- see [`phrost_hir::ClassData::body_range`], which is
    /// where the difference from `full_range` matters.
    pub(crate) body_range: TextRange,
}

/// Where a declaration starts for the purpose of finding its docblock.
///
/// Not the keyword. Both attributes and modifiers can precede it -- `/** doc */ #[Attr] abstract
/// class X` -- and anything skipped ends up in the gap between the comment and the declaration,
/// which makes the docblock look like it belongs to something else and silently drops it.
pub(crate) fn declaration_start(
    attributes: &Sequence<'_, AttributeList<'_>>,
    modifiers: Option<&Sequence<'_, Modifier<'_>>>,
    keyword: Span,
) -> TextSize {
    let candidates = [
        attributes.first_span().map(|span| span.start.offset),
        modifiers
            .and_then(|modifiers| modifiers.first_span())
            .map(|span| span.start.offset),
        Some(keyword.start.offset),
    ];

    TextSize::from(
        candidates
            .into_iter()
            .flatten()
            .min()
            .expect("the keyword offset is always present"),
    )
}

pub(crate) fn class_modifiers(modifiers: &Sequence<'_, Modifier<'_>>) -> ClassModifiers {
    let mut result = ClassModifiers::default();
    for modifier in modifiers.iter() {
        match modifier {
            Modifier::Abstract(_) => result.is_abstract = true,
            Modifier::Final(_) => result.is_final = true,
            Modifier::Readonly(_) => result.is_readonly = true,
            _ => {}
        }
    }
    result
}

/// Whether a declaration carries `#[Attribute]`, which is what makes a class usable as one.
///
/// By the written name and not by resolving it: `Attribute` and `\Attribute` are the two spellings
/// anybody writes, and a class of that name in another namespace would at worst be ranked first at
/// a position where every class is already offered. Resolving instead would cost a def-map lookup
/// per attribute on every declaration in the project, for a bit read at one cursor position.
pub(crate) fn names_an_attribute(
    lists: &Sequence<'_, mago_syntax::cst::AttributeList<'_>>,
    word: &str,
) -> bool {
    lists.iter().any(|list| {
        list.attributes.iter().any(|attribute| {
            identifier_text(attribute.name.value())
                .trim_start_matches('\\')
                .eq_ignore_ascii_case(word)
        })
    })
}

fn import_kind(use_type: &mago_syntax::cst::UseType<'_>) -> ImportKind {
    if use_type.is_function() {
        ImportKind::Function
    } else {
        ImportKind::Constant
    }
}

/// Reads a name-ref id back as an index into the side table of written paths.
fn id_index(id: NameRefId) -> usize {
    lspkit_arena::ArenaId::index(id)
}

/// Visibility written on a member, defaulting to `public` as PHP does.
pub(crate) fn visibility_of(modifiers: &Sequence<'_, Modifier<'_>>) -> Visibility {
    for modifier in modifiers.iter() {
        match modifier {
            Modifier::Private(_) => return Visibility::Private,
            Modifier::Protected(_) => return Visibility::Protected,
            Modifier::Public(_) => return Visibility::Public,
            _ => {}
        }
    }
    Visibility::Public
}

/// PHP 8.4 asymmetric visibility: the `private(set)` half, when written.
pub(crate) fn set_visibility_of(modifiers: &Sequence<'_, Modifier<'_>>) -> Option<Visibility> {
    modifiers.iter().find_map(|modifier| match modifier {
        Modifier::PrivateSet(_) => Some(Visibility::Private),
        Modifier::ProtectedSet(_) => Some(Visibility::Protected),
        Modifier::PublicSet(_) => Some(Visibility::Public),
        _ => None,
    })
}

/// The text of a string literal expression, when that is what it is.
///
/// Lossy, as every string read out of a source file here is: a docblock or an attribute argument in
/// a non-UTF-8 encoding is worth reading approximately rather than dropping whole.
pub(crate) fn string_literal(expression: &Expression<'_>) -> Option<Box<str>> {
    let Expression::Literal(mago_syntax::cst::Literal::String(literal)) = expression else {
        return None;
    };
    Some(String::from_utf8_lossy(literal.value?).into())
}
