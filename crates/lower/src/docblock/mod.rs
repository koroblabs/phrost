//! Types written in a docblock.
//!
//! `@var`, `@param`, and `@return` are where the type comes from on code that predates PHP's own
//! type declarations, which is most of a project of any age. Nothing here parses the docblock by
//! hand: mago's PHPDoc parser reads it, and this lowers the result into the same written-type
//! representation a native hint produces, so nothing above has two type languages to consume.
//!
//! # Only when the native declaration is silent
//!
//! A native hint always wins — it is what the runtime enforces, and a docblock disagreeing with one
//! is a mistake in the source rather than extra information. Parsing is skipped entirely when there
//! is a hint, which is also what keeps this affordable: on a modern project most declarations are
//! typed, so most docblocks are never read for types at all.
//!
//! # What does not survive
//!
//! The docblock type language says far more than a written PHP type can: `array<int, User>`,
//! `array{id: int}`, `int<0, max>`, `key-of<T>`. Where a shape has a written-PHP equivalent it is
//! used — `User[]` and `list<User>` are arrays, `?User` is nullable — and where it does not, the
//! type is recorded as unmodelled rather than approximated. Element types are the notable loss and
//! the reason a `foreach` variable still has no type; carrying them needs type arguments, which is
//! its own step.
//!
//! # What this file owns
//!
//! The two things every sibling here builds on: locating the docblock above a declaration and
//! reading a single tagged type out of it ([`DocTag`], [`tagged_type`]), plus the string/span
//! primitives ([`variable_name`], [`shift`], [`slice`]) a tag's type is read through. [`doc_throws`]
//! and [`doc_var_bindings`] live here too — they read a tag family directly rather than through
//! [`DocTag`], but are the same "locate, prefilter, parse, extract" shape as [`doc_type`]. The five
//! siblings ([`contract`], [`argument_binding`], [`assert`], [`template`], [`type_lowering`]) build
//! on this rather than duplicating it; none of them re-implement docblock lookup.

pub(crate) mod argument_binding;
mod assert;
mod callback;
mod constant_entry;
pub(crate) mod contract;
mod cross_reference;
mod hash_notation;
mod receiver_out;
pub(crate) mod template;
pub(crate) mod type_alias;
mod type_lowering;

use lspkit_arena::IdSlice;
use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, Tag, TagValue, r#type::Type, variable::Variable},
};
use mago_span::{HasSpan as _, Span};
use phrost_hir::{TypeRefData, TypeRefId};
use phrost_model::{BuiltinTy, TextRange};

use crate::lowerer::Lowerer;

use type_alias::TypeAliasTable;

/// Which tag a type is being looked for in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DocTag<'a> {
    /// `@var T`, for a property. Unnamed: a property's docblock describes only it.
    Var,
    /// `/** @var T $name */` above an assignment, matched by the variable it names.
    ///
    /// An unnamed `@var` above an assignment is accepted too: it is a common shorthand when the
    /// statement assigns one variable and there is nothing to disambiguate.
    LocalVar(&'a str),
    /// `@param T $name`, matched by the parameter's name.
    Param(&'a str),
    /// `@param-out T $name` -- what a by-reference parameter leaves in the caller's variable.
    ///
    /// A third type per parameter and not a refinement of the other two, because it is about a
    /// different moment: `@param` says what the callee accepts and this says what it wrote back.
    /// `preg_match(string $pattern, string $subject, array &$matches = null)` is the shape -- the
    /// signature's type is what may be passed in, and `@param-out array<int, string>` is what comes
    /// out. The bundled stubs write it 145 times.
    ParamOut(&'a str),
    /// `@param-closure-this T $name` -- what `$this` is inside a closure passed as that parameter.
    ParamClosureThis(&'a str),
    /// `@return T`.
    Return,
    /// `@assert-if-true T $this`, and its vendor-prefixed spellings.
    AssertIfTrue,
    /// `@assert-if-false T $this`.
    AssertIfFalse,
    /// `@phpstan-self-out T`, `@phpstan-this-out T`, `@psalm-this-out T` and the bare `@self-out` /
    /// `@this-out` -- what the receiver holds once the call has returned. mago folds every spelling
    /// into one tag value, so one variant reads them all.
    SelfOut,
}

impl Lowerer<'_, '_> {
    /// The docblock's type, beside whatever the declaration itself said.
    ///
    /// Read whether or not there is a native hint, because a docblock beside a hint says three
    /// different things and only one of them is noise: it supplies type arguments PHP has no syntax
    /// for (`@param Items<Special>`), it names a subclass of the hint (`@var \App\User` beside
    /// `UserInterface`), or it restates the hint. Which of the three it is takes a resolved type to
    /// tell, so the reading happens here and [`refined_type_ref`](phrost_hir::refined_type_ref) and
    /// the view decide.
    ///
    /// This was gated for a long time on the docblock containing a `<`, on the grounds that parsing
    /// one for every typed declaration in a project would not be free. Measuring it settled that:
    /// on a 30 596-file checkout the gate is worth 15 ms of a 2.6 s bootstrap, and what it cost was
    /// every subclass a docblock names.
    pub(crate) fn doc_type_beside(
        &mut self,
        native: Option<TypeRefId>,
        declaration_start: phrost_model::TextSize,
        tag: DocTag<'_>,
    ) -> Option<TypeRefId> {
        self.doc_type_beside_with_aliases(
            native,
            declaration_start,
            tag,
            &std::rc::Rc::clone(&self.class_aliases),
        )
    }

    /// [`Self::doc_type_beside`], with the enclosing class's `@phpstan-type` table in hand so a
    /// member's own `@return` / `@param` / `@var` can use it.
    ///
    /// Its own entry point rather than a change to [`Self::doc_type_beside`]'s signature: that one
    /// is also called for a free function, which has no class and so nothing to build the table
    /// from -- see [`crate::members`] for the one caller that has a real table to pass.
    pub(crate) fn doc_type_beside_with_aliases(
        &mut self,
        native: Option<TypeRefId>,
        declaration_start: phrost_model::TextSize,
        tag: DocTag<'_>,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        // Whether or not there is a native hint. Which of the three things a docblock beside one is
        // saying takes a resolved type to tell, so both are lowered and
        // [`refined_type_ref`](phrost_hir::refined_type_ref) and the view decide.
        let found = self.doc_type_with_aliases(declaration_start, tag, aliases)?;
        match tag {
            // What is written back is not bounded by what the hint accepts: `?int &$count` with
            // `@param-out int` hands back an int whatever it was given.
            DocTag::ParamOut(_) => Some(found),
            _ => {
                let found = self.iterating_what_the_hint_accepts(native, found);
                Some(self.keeping_the_null(native, found))
            }
        }
    }

    /// `@param Row[]` beside `iterable $rows`, read as the `iterable<Row>` it means.
    ///
    /// The brackets are how a docblock said "of rows" before generics, and the hint is what the
    /// runtime takes: a generator or a collection gets through. Taken as written the docblock won as
    /// an array and every collection handed in was reported, which PHPStan does not do -- it reads
    /// `Row[]` and `array<Row>` beside `iterable` exactly so, and an array with its key written
    /// (`array<int, Row>`, `list<Row>`) as the array it says. Both of ours with no key are the
    /// one-argument shape, and a list carries its `int`, so the line falls where theirs does.
    fn iterating_what_the_hint_accepts(
        &mut self,
        native: Option<TypeRefId>,
        doc: TypeRefId,
    ) -> TypeRefId {
        let Some(native) = native else {
            return doc;
        };
        let hint = match *self.builder.type_ref(native) {
            TypeRefData::Nullable(inner) => inner,
            _ => native,
        };
        if !matches!(
            self.builder.type_ref(hint),
            TypeRefData::Builtin(BuiltinTy::Iterable)
        ) {
            return doc;
        }
        self.iterable_of(hint, doc).unwrap_or(doc)
    }

    /// `doc` with each keyless array in it written over `iterable`, or `None` when it holds none.
    fn iterable_of(&mut self, iterable: TypeRefId, doc: TypeRefId) -> Option<TypeRefId> {
        match self.builder.type_ref(doc).clone() {
            TypeRefData::Generic {
                base,
                args,
                non_empty: false,
            } if matches!(
                self.builder.type_ref(base),
                TypeRefData::Builtin(BuiltinTy::Array)
            ) && self.builder.type_ref_list(args).len() == 1 =>
            {
                Some(self.builder.add_type_ref(TypeRefData::Generic {
                    base: iterable,
                    args,
                    non_empty: false,
                }))
            }
            TypeRefData::Nullable(inner) => {
                let inner = self.iterable_of(iterable, inner)?;
                Some(self.builder.add_type_ref(TypeRefData::Nullable(inner)))
            }
            TypeRefData::Union(operands) => {
                let parts = self.builder.type_ref_list(operands).to_vec();
                let mut changed = false;
                let parts = parts
                    .into_iter()
                    .map(|part| match self.iterable_of(iterable, part) {
                        Some(rewritten) => {
                            changed = true;
                            rewritten
                        }
                        None => part,
                    })
                    .collect::<Vec<_>>();
                if !changed {
                    return None;
                }
                let parts = self.builder.alloc_type_ref_list(&parts);
                Some(self.builder.add_type_ref(TypeRefData::Union(parts)))
            }
            _ => None,
        }
    }

    /// A docblock beside a hint that admits `null`, with the `null` put back if it left it out.
    ///
    /// `/** @var DOMNamedNodeMap<DOMAttr> */ public ?DOMNamedNodeMap $attributes;` says what the map
    /// holds and says nothing about whether there is one -- the runtime lets the property be `null`
    /// whatever a comment claims. Taken as written, the docblock either won and dropped the `null`,
    /// or lost to the hint and dropped the element type; both halves are the same claim, so here
    /// they become one type before anything reads either. Done at lowering because both readers of
    /// a declaration -- the written one and the interned one -- take the docblock from here.
    ///
    /// A union gains `null` as an operand rather than a `?` around it, which is the degenerate form
    /// every narrowing would have to see through.
    fn keeping_the_null(&mut self, native: Option<TypeRefId>, doc: TypeRefId) -> TypeRefId {
        // `mixed` admits `null` too and is left out on the hint's side: it is what an author writes
        // when PHP has no syntax for the type, and `@return T` beside `: mixed` is not `?T`.
        //
        // And only a docblock that says what the value *holds* -- arguments or a shape -- which is
        // the one kind that takes a hint's place. A bare name beside `?Parent` may be a template this
        // layer cannot see declared on the class (`@return TParent`), and a template's argument
        // settles its own nullability: Symfony's whole `->end()` chain is written that way.
        if !native.is_some_and(|native| self.admits_null(native, false))
            || self.admits_null(doc, true)
            || !self.says_what_it_holds(doc)
        {
            return doc;
        }
        match *self.builder.type_ref(doc) {
            TypeRefData::Union(operands) => {
                let mut parts = self.builder.type_ref_list(operands).to_vec();
                parts.push(
                    self.builder
                        .add_type_ref(TypeRefData::Builtin(BuiltinTy::Null)),
                );
                let parts = self.builder.alloc_type_ref_list(&parts);
                self.builder.add_type_ref(TypeRefData::Union(parts))
            }
            _ => self.builder.add_type_ref(TypeRefData::Nullable(doc)),
        }
    }

    /// Whether a written type has arguments or a shape, in one of its operands at least.
    fn says_what_it_holds(&self, id: TypeRefId) -> bool {
        match self.builder.type_ref(id) {
            TypeRefData::Generic { .. } | TypeRefData::Shape { .. } => true,
            TypeRefData::Nullable(inner) => self.says_what_it_holds(*inner),
            TypeRefData::Union(operands) => self
                .builder
                .type_ref_list(*operands)
                .iter()
                .any(|&operand| self.says_what_it_holds(operand)),
            _ => false,
        }
    }

    /// Whether a written type spells out that `null` gets through: `?T`, `T|null`, `null`, and
    /// `mixed` where `mixed` counts.
    fn admits_null(&self, id: TypeRefId, mixed: bool) -> bool {
        match self.builder.type_ref(id) {
            TypeRefData::Nullable(_) | TypeRefData::Builtin(BuiltinTy::Null) => true,
            TypeRefData::Builtin(BuiltinTy::Mixed) => mixed,
            TypeRefData::Union(operands) => self
                .builder
                .type_ref_list(*operands)
                .iter()
                .any(|&operand| self.admits_null(operand, mixed)),
            _ => false,
        }
    }

    /// The type a docblock declares for one thing, lowered like a native hint.
    ///
    /// `None` when there is no such tag, when its type says something this representation cannot
    /// hold or names one of the declaration's own templates, or when the docblock does not parse.
    /// All of them are the same answer to a caller: the declaration does not say. A tag written
    /// twice, prefixed and plain, is read in the order [`tagged_types_in`] gives.
    pub(crate) fn doc_type(
        &mut self,
        declaration_start: phrost_model::TextSize,
        tag: DocTag<'_>,
    ) -> Option<TypeRefId> {
        self.doc_type_with_aliases(
            declaration_start,
            tag,
            &std::rc::Rc::clone(&self.class_aliases),
        )
    }

    /// [`Self::doc_type`], with the enclosing class's `@phpstan-type` table in hand.
    pub(crate) fn doc_type_with_aliases(
        &mut self,
        declaration_start: phrost_model::TextSize,
        tag: DocTag<'_>,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        let docblock = self.raw_docblock_before(declaration_start)?;

        // A prefilter, because parsing is the expensive part and most docblocks carry no type at
        // all. `@` alone is enough: a docblock without one cannot hold a tag.
        if !docblock.text.contains('@') {
            return None;
        }

        // WordPress's `@return array { ... @type T $key ... }`, rewritten to the same length so
        // every span below is still a span of the file. See `hash_notation`.
        let hashed = match tag {
            DocTag::Return => hash_notation::Hashed::of(docblock.text),
            _ => None,
        };
        let text = hashed
            .as_ref()
            .map_or(docblock.text, |hashed| hashed.text.as_str());

        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        // A synthetic file id: the parser only uses it to stamp spans, and every span is rebased
        // onto the real file below.
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        // The docblock's text starts at its own offset in the file, but the parser was handed only
        // the comment, so its spans are relative to that.
        let base = docblock.range.start();
        let templates = match template::may_declare_a_template(text) {
            true => template::declared_templates(&document),
            false => Vec::new(),
        };
        // The first spelling that lowers. One naming a declaration's own template is refused, and
        // the refusal stands rather than handing over to the next spelling: a template nothing here
        // binds resolves to no class, so lowering it produces one the project does not declare --
        // `@param T $needle` on `in_array` would report every argument passed to it as taking a
        // class called `T` -- and the plain spelling beside one is narrower than what it says.
        // `wp_unslash()` writes `@return string|array` above a `@phpstan-return` that hands back
        // whatever it was given, and taken for a `$_POST` value the plain one was 314 findings on
        // WordPress about arrays nobody passes. What the templates *do* say is read from the
        // docblock by the contracts, which is where it can be tied to an argument.
        for found in tagged_types_in(&document, tag) {
            let Some(lowered) = self.lower_doc_type(found, base, text, aliases) else {
                continue;
            };
            if template::names_a_template(slice(text, found.span()), &templates) {
                // Except a conditional return, which is one of its branches whatever the call: with
                // each template standing for anything at all, the union is true of every call, where
                // refusing it left `Arr::wrap($value)` and `wp_unslash($value)` with no type on any
                // call their contract did not settle.
                //
                // A return only. A refused `@param-out` leaves the variable what it was, which for
                // `usort($rows, ...)` is the `Row[]` it went in as; erased, it came back `list<mixed>`.
                return match tag {
                    DocTag::Return => contract::as_conditional(found).and_then(|_| {
                        self.with_templates_erased(&document, found, base, text, aliases)
                    }),
                    _ => None,
                };
            }
            return Some(match &hashed {
                Some(hashed) => hashed.shape_of(self, &document, found, lowered, base, aliases),
                None => lowered,
            });
        }
        None
    }

    /// A docblock type lowered with each of the declaration's own templates standing for `mixed`,
    /// beside whatever aliases the class already gave.
    ///
    /// Not for its bound, though that is also true of every call. This answers where the contract
    /// could not -- a call whose argument it cannot read -- and the bound is then a claim about the
    /// call nobody made: `array_keys($reasons)` over keys nothing tracked read as `list<int|string>`
    /// off `K of array-key`, and was reported where `string[]` was wanted. `mixed` says what is known.
    fn with_templates_erased(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        found: &Type<'_>,
        base: phrost_model::TextSize,
        text: &str,
        aliases: &TypeAliasTable,
    ) -> Option<TypeRefId> {
        let mut standing = Vec::new();
        for element in document.elements.iter() {
            let Element::Tag(entry) = element else {
                continue;
            };
            let TagValue::Template(value) = &entry.value else {
                continue;
            };
            let mixed = self
                .builder
                .add_type_ref(TypeRefData::Builtin(BuiltinTy::Mixed));
            let name = self
                .interner
                .name(&String::from_utf8_lossy(value.name.value));
            standing.push((name, mixed));
        }
        self.lower_doc_type(found, base, text, &aliases.and_standing_for(standing))
    }

    /// The classes a declaration's `@throws` tags name, lowered.
    ///
    /// Lowering the type is what allocates its `NameRefData`, and for a long time that was the only
    /// reason to do it: goto wants the range, find-references wants the resolution, and the
    /// unused-import check wants to know the name was mentioned at all -- an import used only from
    /// a `@throws` otherwise reads as unused, which is a false positive of exactly the kind that
    /// teaches people to stop reading the list.
    ///
    /// Now the ids are kept too, because something finally asks what this doc-comment said would
    /// need a home: `doc-missing-throws` reads what a *callee* declares, and a caller cannot see
    /// the callee's docblock -- the index keeps ranges rather than text, so reading one is a disk
    /// read per call. See `ProjectSkeleton::declared_throws` for where they land, which is the
    /// sparse table beside the declarations this note asked for.
    ///
    /// Every tag rather than the first, unlike [`Self::doc_type`]: a method that documents three
    /// exceptions names three classes, and `find_map` would keep one.
    /// Whether the declaration's docblock marks it `@ignore`.
    ///
    /// phpDocumentor's "this is not part of the API", and the only thing in the source that can say
    /// so. WordPress's `wp-admin/includes/noop.php` declares nineteen of its commonest functions as
    /// empty stubs for one minimal load path -- `function __() {}` beside the real
    /// `__( $text, $domain = 'default' )` -- and eighteen of them carry the tag. Nothing else
    /// distinguishes the two: both are unconditional, both are user code, and the two files are
    /// never loaded together, which no declaration states.
    ///
    /// Read as a *tie-breaker* and never as a reason to skip a declaration: where a name has one
    /// declarer the tag changes nothing, so a project that marks a real function this way keeps it.
    ///
    /// Parsed rather than scanned, and that is not caution for its own sake -- §8 records a text
    /// scan reading Drupal's own documentation of a syntax as an instance of it, and a docblock is
    /// exactly where somebody demonstrates a tag. The substring prefilter keeps this off the parse
    /// path for every other declaration.
    ///
    /// Parsing alone is not the protection, though, and a test asserting it caught the claim: mago
    /// makes `@code` a *tag* and a line opening with `@` inside the block a tag of its own, so an
    /// `@ignore` written in an example arrives looking exactly like one written for real. The
    /// `@code`/`@endcode` span is tracked instead.
    pub(crate) fn doc_is_ignored(&mut self, declaration_start: phrost_model::TextSize) -> bool {
        self.doc_names_tag(declaration_start, "ignore")
    }

    /// Whether a declaration says it is deprecated, by either of the two spellings PHP has.
    ///
    /// `@deprecated` in a docblock and PHP 8.4's `#[\Deprecated]` say the same thing about the
    /// declaration, and a reader asking "is this deprecated" is not asking which of them said so --
    /// 797 docblocks in WordPress against seven attributes in Symfony is the whole distribution.
    /// Asked in one place for the reason §8.1 gives about three spellings of one thing: a rule
    /// written on one arm governs that arm and reads as covered, because that arm has tests.
    ///
    /// The attribute first, because it costs a walk over a list that is almost always empty and the
    /// docblock costs a substring scan of text.
    pub(crate) fn is_deprecated(
        &mut self,
        attributes: &mago_syntax::cst::sequence::Sequence<'_, mago_syntax::cst::AttributeList<'_>>,
        declaration_start: phrost_model::TextSize,
    ) -> bool {
        crate::lowerer::names_an_attribute(attributes, crate::members::DEPRECATED)
            || self.doc_names_tag(declaration_start, "deprecated")
    }

    /// Whether a docblock carries one tag, outside any `@code` example.
    ///
    /// One reader for both, because the care is the same and there is no version of this rule that
    /// should differ between tags: the substring prefilter that keeps it off the parse path, the
    /// parse that keeps a documented tag from reading as a written one, and the example span. Two
    /// copies of it is the shape §8.1 names, and the second copy is always the one missing a case.
    pub(crate) fn doc_names_tag(
        &mut self,
        declaration_start: phrost_model::TextSize,
        tag: &str,
    ) -> bool {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return false;
        };
        if !docblock.text.contains(&format!("@{tag}")) {
            return false;
        }

        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(docblock.text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );
        let mut in_example = false;
        for element in document.elements.iter() {
            let Element::Tag(entry) = element else {
                continue;
            };
            let name = entry.name.value;
            if name.eq_ignore_ascii_case(b"code") {
                in_example = true;
            } else if name.eq_ignore_ascii_case(b"endcode") {
                in_example = false;
            } else if !in_example && name.eq_ignore_ascii_case(tag.as_bytes()) {
                return true;
            }
        }
        false
    }

    pub(crate) fn doc_throws(
        &mut self,
        declaration_start: phrost_model::TextSize,
    ) -> IdSlice<TypeRefId> {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return IdSlice::EMPTY;
        };
        // The substring test is what keeps this off the parse path for the whole project. Most
        // docblocks carry no `@throws`, and for those the cost is one scan of text already in hand.
        if !docblock.text.contains("@throws") {
            return IdSlice::EMPTY;
        }

        let base = docblock.range.start();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(docblock.text.as_bytes());
        // A synthetic file id, as elsewhere here: the parser only stamps spans with it, and every
        // span is rebased onto the real file.
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let thrown: Vec<_> = document
            .elements
            .iter()
            .filter_map(|element| match element {
                Element::Tag(entry) => match &entry.value {
                    TagValue::Throws(value) => Some(value.r#type),
                    _ => None,
                },
                Element::Text(_) | Element::Code(_) => None,
            })
            .collect();

        let lowered: Vec<TypeRefId> = thrown
            .into_iter()
            .filter_map(|ty| {
                // `EMPTY`, not a real table: this is also called for a free function's `@throws`,
                // which has no class to have declared one, and nothing distinguishes that call from
                // a method's here -- see `doc_type_beside_with_aliases` for the entry point that
                // does.
                self.lower_doc_type(
                    ty,
                    base,
                    docblock.text,
                    &std::rc::Rc::clone(&self.class_aliases),
                )
            })
            .collect();
        self.builder.alloc_type_ref_list(&lowered)
    }

    /// Every `@var T $name` in the docblock above `declaration_start`, with the name it binds.
    ///
    /// Named ones only. A nameless `@var` has nothing to bind to on its own -- it is a note about
    /// whatever the statement below does -- and the assignment and the loop that consume one already
    /// read it themselves.
    pub(crate) fn doc_var_bindings(
        &mut self,
        declaration_start: phrost_model::TextSize,
    ) -> Vec<(phrost_model::Name, TypeRefId, phrost_model::TextRange)> {
        match self.raw_docblock_before(declaration_start) {
            Some(docblock) => self.var_bindings_in(docblock),
            None => Vec::new(),
        }
    }

    /// The `@global T $name` tags of a declaration's own docblock, as the bindings they mean.
    ///
    /// Only `@global`: a `@var` there is about the declaration, not about a local.
    pub(crate) fn doc_global_bindings(
        &mut self,
        declaration_start: phrost_model::TextSize,
    ) -> Vec<(phrost_model::Name, TypeRefId, phrost_model::TextRange)> {
        match self.raw_docblock_before(declaration_start) {
            Some(docblock)
                if docblock.text.contains("@global") && !docblock.text.contains("@var") =>
            {
                self.var_bindings_in(docblock)
            }
            _ => Vec::new(),
        }
    }

    /// Every `@var T $name` in the docblocks inside `window`.
    ///
    /// The recovery half of [`Self::doc_var_bindings`], which can only find a docblock by asking
    /// from the statement below it. mago drops the statement it cannot finish, and the statement
    /// someone is part-way through typing -- `$x->` with no member yet -- is exactly that one, so at
    /// the moment the annotation is needed most there is nothing left to ask from. Called with the
    /// span a body's own statements did not cover, which is what keeps a docblock a statement
    /// already claimed from being read a second time.
    pub(crate) fn orphaned_var_bindings(
        &mut self,
        window: TextRange,
    ) -> Vec<(phrost_model::Name, TypeRefId, TextRange)> {
        let mut bindings = Vec::new();
        for docblock in self.raw_docblocks_within(window) {
            bindings.extend(self.var_bindings_in(docblock));
        }
        bindings
    }

    /// Every `@var T $name` written in one docblock, with the name it binds.
    fn var_bindings_in(
        &mut self,
        docblock: phrost_syntax::Docblock<'_>,
    ) -> Vec<(phrost_model::Name, TypeRefId, phrost_model::TextRange)> {
        // The same prefilter as `doc_type`, and it matters more here: this is asked for every
        // statement in every open buffer, and almost none of them have a docblock with a tag.
        // Without the `@` for `var`, because `@psalm-var` does not contain `@var` -- and *with* it
        // for `global`, because nothing prefixes that one and the bare word is ordinary prose. That
        // difference is worth the two spellings: `check` over the reference project measured 1.31 s
        // with the loose test and 1.24 s with this one, on docblocks that merely say "global".
        if !docblock.text.contains("var") && !docblock.text.contains("@global") {
            return Vec::new();
        }

        let arena = LocalArena::new();
        let text = as_var_tags(docblock.text);
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let base = docblock.range.start();
        let named = document
            .elements
            .iter()
            .filter_map(|element| match element {
                Element::Tag(entry) => match &entry.value {
                    TagValue::Var(value) => {
                        Some((variable_name(value.variable.as_ref()?), value.r#type))
                    }
                    _ => None,
                },
                Element::Text(_) | Element::Code(_) => None,
            })
            .collect::<Vec<_>>();

        named
            .into_iter()
            .filter_map(|(name, written)| {
                let name = self.interner.name(name);
                // `EMPTY`: a body-level `@var` is read from `crates/lower/src/body.rs`, which this
                // crate's ownership split keeps untouched for this feature -- see the module doc on
                // `TypeAliasTable`.
                let ty = self.lower_doc_type(
                    written,
                    base,
                    &text,
                    &std::rc::Rc::clone(&self.class_aliases),
                )?;
                // The docblock's own range, because that is where the annotation is written: it has
                // to rank before the statement below it, and a position lookup has to be able to
                // land on it.
                Some((name, ty, docblock.range))
            })
            .collect()
    }
}

/// The docblock with every `@global` rewritten as the `@var` it means, keeping every byte in place.
///
/// `@global wpdb $wpdb` above `global $wpdb;` says exactly what `@var wpdb $wpdb` says, and it is
/// how WordPress types its globals: **599 of its files** write one, and `$wpdb`, `$post` and
/// `$comment` are the three commonest untyped receivers in that whole checkout. mago's docblock
/// parser has no `@global` -- the tag is phpDocumentor's and not one of the static-analysis
/// vocabularies it models -- so it arrives as text and nothing reads it.
///
/// Rewritten rather than parsed by hand, and padded to the same length rather than replaced, because
/// every range downstream is a byte offset into this text: a name reference in the type gets its
/// goto range from it, and `undefined-type` reports at it. `@global` is seven bytes and `@var` is
/// four, so the three the substitution gives back are spaces -- which the tag grammar already allows
/// between the tag and the type.
///
/// Only where a separator follows, so `@globally` is left alone.
fn as_var_tags(text: &str) -> std::borrow::Cow<'_, str> {
    const FROM: &str = "@global";
    const TO: &str = "@var   ";

    if !text.contains(FROM) {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = text.to_owned();
    let mut at = 0;
    while let Some(found) = out[at..].find(FROM) {
        let start = at + found;
        let after = start + FROM.len();
        // `@globally` is a different word; only a tag ends here. ASCII on both sides and the same
        // length, so no character boundary moves whatever the description after the tag holds.
        if out[after..]
            .chars()
            .next()
            .is_none_or(|next| next.is_whitespace())
        {
            out.replace_range(start..after, TO);
        }
        at = after;
    }
    std::borrow::Cow::Owned(out)
}

/// The type in a tag, when the tag is the one being looked for.
pub(crate) fn tagged_type<'arena>(
    tag: &Tag<'arena>,
    wanted: DocTag<'_>,
) -> Option<&'arena Type<'arena>> {
    match (&tag.value, wanted) {
        (TagValue::Var(value), DocTag::Var) => Some(value.r#type),
        (TagValue::Var(value), DocTag::LocalVar(name)) => match value.variable.as_ref() {
            Some(written) => (variable_name(written) == name).then_some(value.r#type),
            None => Some(value.r#type),
        },
        (TagValue::Return(value) | TagValue::RealReturn(value), DocTag::Return) => {
            Some(value.r#type)
        }
        (TagValue::AssertIfTrue(value), DocTag::AssertIfTrue)
        | (TagValue::AssertIfFalse(value), DocTag::AssertIfFalse) => {
            assert::asserted_about_this(value)
        }
        (TagValue::SelfOut(value), DocTag::SelfOut) => Some(value.r#type),
        (TagValue::Param(value), DocTag::Param(name)) => {
            // Matched by name, because `@param` lines are written in whatever order the author
            // liked and one of them may be missing.
            let written = value.parameter.as_ref()?;
            (variable_name(written) == name).then_some(value.r#type)
        }
        // Its own arm rather than an or-pattern beside the one above: mago gives the two tags
        // different value types, so there is nothing to share but the shape.
        (TagValue::ParamClosureThis(value), DocTag::ParamClosureThis(name)) => {
            (variable_name(&value.parameter) == name).then_some(value.r#type)
        }
        (TagValue::ParamOut(value), DocTag::ParamOut(name)) => {
            // The name is not optional on this tag: `@param-out` without one says nothing at all,
            // so mago requires it where `@param` does not.
            (variable_name(&value.parameter) == name).then_some(value.r#type)
        }
        _ => None,
    }
}

/// A `$name` written in a tag, without the `$`.
pub(crate) fn variable_name<'arena>(variable: &Variable<'arena>) -> &'arena str {
    std::str::from_utf8(variable.value)
        .unwrap_or_default()
        .trim_start_matches('$')
}

/// Moves a span from docblock-relative to file-relative.
pub(crate) fn shift(span: Span, base: phrost_model::TextSize) -> TextRange {
    let start = base + phrost_model::TextSize::from(span.start.offset);
    let end = base + phrost_model::TextSize::from(span.end.offset);
    TextRange::new(start, end)
}

/// The text a docblock-relative span covers.
pub(crate) fn slice(text: &str, span: Span) -> &str {
    text.get(span.start.offset as usize..span.end.offset as usize)
        .unwrap_or_default()
}

/// The type a tag states, preferring the tool-prefixed spelling over the plain one.
///
/// A declaration may write both, and Doctrine's `EntityRepository::find` writes exactly that:
///
/// ```text
/// @return object|null The entity instance or NULL if the entity can not be found.
/// @phpstan-return ?T
/// ```
///
/// The plain tag exists for a reader that cannot hold the precise one -- PHP has no syntax for `T`,
/// and `object|null` is the closest thing that is *true* -- so the prefixed tag is not a second
/// opinion, it is the statement. Taking whichever came first made `find()` an `object|null` while
/// `findOneBy()` beside it, which writes only the prefixed tag, answered the entity: the same class,
/// the same shape, four lines apart.
///
/// Every tag kind and not only `@return`, because every one of them has the twin: on one real vendor
/// tree `@phpstan-param` is **588** occurrences against `@phpstan-return`'s 381, and 524 of the whole
/// lot are in `doctrine/` alone.
///
/// mago folds the prefixed spellings into one tag value and records the prefix separately, which is
/// what makes this a comparison rather than a scan for names.
///
/// # What it costs, and why the cost is the point
///
/// `phrost coverage` goes *down*: 44 924 resolved member accesses to 44 918, untyped receivers 629 to
/// 637. Those six are the same sites as two `undefined-method` findings that disappeared, and they are
/// one shape -- Drupal core writes
///
/// ```text
/// @return \Drupal\Core\TypedData\TypedDataInterface|null
/// @phpstan-return ?T
/// ```
///
/// on `ListInterface::get()`, and most code holds a plain `FieldItemListInterface` with nothing to
/// bind `T` to. Before, the chain resolved to `TypedDataInterface` and the *next* call on it was
/// reported undefined, because that interface declares almost nothing. Now it resolves to nothing and
/// reports nothing. So the instrument is counting confidence rather than correctness here: six
/// confidently wrong answers became six honest silences.
///
/// Getting both would mean keeping the plain tag beside the precise one and falling back to it where
/// no argument binds the template -- a second type per member row, which is the largest slice of the
/// index heap. Priced, not built.
pub(crate) fn tagged_type_in<'d>(
    document: &'d mago_phpdoc_syntax::cst::Document<'d>,
    wanted: DocTag<'_>,
) -> Option<&'d Type<'d>> {
    tagged_types_in(document, wanted).next()
}

/// Every type a tag is written with, in the order [`tagged_type_in`] prefers them: the prefixed
/// spellings, then the plain ones, each in the order written.
///
/// For a reader that can tell when it could not hold one: what the plain tag is *for* is a reader
/// that cannot hold the precise one, so where a prefixed spelling lowers to nothing at all the plain
/// one is the statement made for us.
pub(crate) fn tagged_types_in<'d>(
    document: &'d mago_phpdoc_syntax::cst::Document<'d>,
    wanted: DocTag<'_>,
) -> impl Iterator<Item = &'d Type<'d>> {
    let tags = |prefixed: bool| {
        document
            .elements
            .iter()
            .filter_map(move |element| match element {
                Element::Tag(entry) if entry.vendor.is_some() == prefixed => {
                    tagged_type(entry, wanted)
                }
                _ => None,
            })
    };
    tags(true).chain(tags(false))
}
