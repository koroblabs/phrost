use lspkit_arena::{Arena, IdSlice, IdVec};
use phrost_model::FileId;

use crate::{
    body::{BodyData, CatchData, ExprData, MatchArmData, StmtData, SwitchCaseData},
    docblock::DocblockData,
    ids::{
        AttributeLiteralId, BodyId, CatchId, ClassId, ConstantId, DocblockId, ExprId, FunctionId,
        HookId, ImportId, MatchArmId, MemberId, NameRefId, NamespaceId, ParamId, StmtId,
        SwitchCaseId, TemplateId, TraitAdaptationId, TypeRefId,
    },
    import::{ImportData, NamespaceData},
    item::{
        AttributeLiteralData, ClassData, ConstantData, FunctionData, HookData, MemberData,
        ParamData, TemplateData, TraitAdaptationData,
    },
    name_ref::NameRefData,
    type_ref::TypeRefData,
};

/// Everything declared in one PHP file, owned and free of lifetimes.
///
/// Built once per file revision and then read-only. Ids are local to this value: an id from
/// another file's HIR would silently index the wrong entity, which is why crossing files requires
/// the def-map's file-qualified references rather than a bare id.
#[derive(Debug)]
pub struct FileHir {
    file: FileId,
    namespaces: Arena<NamespaceId, NamespaceData>,
    imports: Arena<ImportId, ImportData>,
    classes: Arena<ClassId, ClassData>,
    functions: Arena<FunctionId, FunctionData>,
    constants: Arena<ConstantId, ConstantData>,
    members: Arena<MemberId, MemberData>,
    params: Arena<ParamId, ParamData>,
    hooks: Arena<HookId, HookData>,
    type_refs: Arena<TypeRefId, TypeRefData>,
    name_refs: Arena<NameRefId, NameRefData>,
    docblocks: Arena<DocblockId, DocblockData>,
    trait_adaptations: Arena<TraitAdaptationId, TraitAdaptationData>,
    templates: Arena<TemplateId, TemplateData>,
    attribute_literals: Arena<AttributeLiteralId, AttributeLiteralData>,
    kept_tables: Vec<crate::item::KeptTableData>,
    kept_texts: Vec<crate::item::KeptTextData>,
    kept_classes: Vec<crate::item::KeptClassData>,
    kept_constructions: Vec<crate::item::KeptConstructionData>,
    /// Bodies, present only when lowering was asked for them -- see [`Self::has_bodies`].
    bodies: Arena<BodyId, BodyData>,
    stmts: Arena<StmtId, StmtData>,
    exprs: Arena<ExprId, ExprData>,
    catches: Arena<CatchId, CatchData>,
    switch_cases: Arena<SwitchCaseId, SwitchCaseData>,
    match_arms: Arena<MatchArmId, MatchArmData>,
    stmt_lists: IdVec<StmtId>,
    expr_lists: IdVec<ExprId>,
    catch_lists: IdVec<CatchId>,
    switch_case_lists: IdVec<SwitchCaseId>,
    match_arm_lists: IdVec<MatchArmId>,
    /// Statements written at file scope, outside any declaration. A script, a `.module` file, or a
    /// bootstrap: PHP allows code there and it is where a lot of procedural code lives.
    root_body: Option<BodyId>,
    /// See [`FileHir::empty_body`].
    empty_body: BodyId,
    /// Which body each closure is written directly inside, built the first time a scope lookup
    /// asks and never again for this revision -- see [`Self::closures_directly_in`].
    closure_parents: std::sync::OnceLock<Box<[(BodyId, ExprId)]>>,
    /// Facts a layer above derives from the tree once per revision -- see [`Self::derived`].
    derived: [std::sync::OnceLock<Box<dyn std::any::Any + Send + Sync>>; Derived::COUNT],
    /// Whether bodies were lowered at all, so a body-less declaration can be told from a file
    /// nobody asked about. Without it, an interface method and an unlowered method look the same.
    bodies_lowered: bool,
    /// Whether the file opens with `declare(strict_types=1)`.
    ///
    /// A per-file fact rather than a per-callee one, because PHP applies it where a call is
    /// *written* and not where the function is declared: one file passing `"1"` to an `int`
    /// parameter is a coercion, and the identical call from a strict file is a `TypeError`. No
    /// judgement about whether an argument fits its parameter can be made without it.
    strict_types: bool,
    member_lists: IdVec<MemberId>,
    param_lists: IdVec<ParamId>,
    hook_lists: IdVec<HookId>,
    name_ref_lists: IdVec<NameRefId>,
    type_ref_lists: IdVec<TypeRefId>,
    adaptation_lists: IdVec<TraitAdaptationId>,
    template_lists: IdVec<TemplateId>,
    attribute_literal_lists: IdVec<AttributeLiteralId>,
    /// Ranges of the file's string literals, from the token stream.
    ///
    /// Populated only when bodies were lowered, because only an open buffer has anyone typing in it.
    /// From tokens rather than from the tree on purpose: a string being typed is not closed yet, so
    /// it does not parse, and the recovered tree has no literal for exactly the moment a completion
    /// list must not appear.
    string_ranges: Vec<phrost_model::TextRange>,
    /// Where the comments are, the same way and for open buffers only: what a reader of characters
    /// has to step over.
    comment_ranges: Vec<phrost_model::TextRange>,
}

impl FileHir {
    pub fn file(&self) -> FileId {
        self.file
    }

    // --- top-level declarations, in source order ---

    pub fn namespaces(&self) -> impl ExactSizeIterator<Item = (NamespaceId, &NamespaceData)> {
        self.namespaces.entries()
    }

    pub fn imports(&self) -> impl ExactSizeIterator<Item = (ImportId, &ImportData)> {
        self.imports.entries()
    }

    pub fn classes(&self) -> impl ExactSizeIterator<Item = (ClassId, &ClassData)> {
        self.classes.entries()
    }

    pub fn functions(&self) -> impl ExactSizeIterator<Item = (FunctionId, &FunctionData)> {
        self.functions.entries()
    }

    /// Every body in the file -- methods, functions, closures, hooks, and file scope -- in the
    /// order they were lowered.
    pub fn bodies(&self) -> impl ExactSizeIterator<Item = (BodyId, &BodyData)> {
        self.bodies.entries()
    }

    /// A fact a layer above this one derived from the tree, computed once for this revision.
    ///
    /// The tree is read-only once built, so anything computed from it is good for as long as it
    /// is -- and a request that resolves a thousand positions in a file should derive a per-body
    /// fact once, not a thousand times. This crate cannot name the types: they belong to the
    /// layer that knows the semantics. So a slot is a named place for one of them, filled on the
    /// first ask and read back typed; asking with a different type for the same slot is a
    /// programming error and says so.
    pub fn derived<T: std::any::Any + Send + Sync>(
        &self,
        slot: Derived,
        build: impl FnOnce(&Self) -> T,
    ) -> &T {
        self.derived[slot as usize]
            .get_or_init(|| Box::new(build(self)))
            .downcast_ref::<T>()
            .expect("one type per derived slot")
    }

    pub fn constants(&self) -> impl ExactSizeIterator<Item = (ConstantId, &ConstantData)> {
        self.constants.entries()
    }

    /// Every written class-like name in the file, in the order lowering met them.
    ///
    /// The whole set rather than a position index: references and rename want all of them, and a
    /// position lookup over a few dozen entries is not worth a second structure to keep in sync.
    pub fn name_refs(&self) -> impl ExactSizeIterator<Item = (NameRefId, &NameRefData)> {
        self.name_refs.entries()
    }

    /// Whether this file's bodies were lowered.
    ///
    /// `false` means nobody asked, not that there are no bodies. Every consumer of a body has to
    /// check: a method whose `body` is `None` is abstract when this is `true` and merely unread
    /// when it is `false`, and confusing the two is how a feature comes to report that a method
    /// does nothing.
    pub fn has_bodies(&self) -> bool {
        self.bodies_lowered
    }

    /// Whether the file declared `declare(strict_types=1)`.
    ///
    /// Always answerable: unlike a body, this is lowered in skeleton mode too, so a closed file
    /// gives the same answer as an open one.
    pub fn strict_types(&self) -> bool {
        self.strict_types
    }

    /// Whether the text just before `offset` is inside a string literal.
    ///
    /// The character before the cursor decides, not the one after it, because that is what someone
    /// typing is writing. In `"Total: $count\n"` the lexer hands back the text after the variable
    /// as a string part of its own, starting exactly where the variable ends -- so a cursor at the
    /// end of `$count` sat at the start of a string, and was offered nothing while it was naming a
    /// variable.
    ///
    /// Empty ranges mean nobody asked for bodies, not that the file has no strings -- the same
    /// distinction [`Self::has_bodies`] draws.
    pub fn inside_string(&self, offset: phrost_model::TextSize) -> bool {
        self.string_ranges
            .iter()
            .any(|range| range.start() < offset && offset <= range.end())
    }

    /// The string literal `offset` is inside, quotes included.
    ///
    /// Byte-scanned rather than read off the tree, which is the point: a string being typed is not
    /// closed yet and does not parse, and that is exactly when somebody wants to be offered
    /// something.
    pub fn string_at(&self, offset: phrost_model::TextSize) -> Option<phrost_model::TextRange> {
        self.string_ranges
            .iter()
            .copied()
            .find(|range| range.contains_inclusive(offset))
    }

    /// Every string literal in the file, delimiters included, in source order.
    ///
    /// The same ranges [`Self::string_at`] searches, for a reader that wants all of them rather
    /// than the one under a cursor: a class named by a string is found by comparing text, and
    /// there is no name reference to walk.
    pub fn string_literals(&self) -> impl Iterator<Item = phrost_model::TextRange> + '_ {
        self.string_ranges.iter().copied()
    }

    /// Every comment in the file, delimiters included, in source order -- for an open buffer, and
    /// empty for anything else.
    pub fn comment_ranges(&self) -> &[phrost_model::TextRange] {
        &self.comment_ranges
    }

    /// Statements at file scope, when there are any and bodies were lowered.
    pub fn root_body(&self) -> Option<BodyId> {
        self.root_body
    }

    /// One body with nothing in it, for a declaration that has none.
    ///
    /// An `interface`'s method and an `abstract` one bind parameters and hold no statements, which
    /// is a scope like any other from every reader's side -- so they are given this rather than
    /// left out, and the twenty places that walk a scope's statements find an empty list instead of
    /// each learning what a missing body means. What it is *not* is a claim that the declaration
    /// has a body: `MemberData::body` is still `None` there, which is what tells an abstract
    /// declaration from a concrete one.
    ///
    /// One per file, allocated when the file is built, because a file with no such declaration
    /// pays a single empty row for it.
    pub fn empty_body(&self) -> BodyId {
        self.empty_body
    }

    // --- lookup by id ---

    /// The closures written directly inside `body`, in source order: not inside a nested closure,
    /// whose own body holds those.
    ///
    /// A scope lookup asks this for the enclosing body of every position it resolves, and it used
    /// to answer by walking every statement of that body -- on a request that resolves a thousand
    /// positions in a file, that walk was 9 % of the whole. The answer is a property of the tree
    /// and not of the position, so it is derived once: each closure's directly enclosing body is
    /// the narrowest body whose range holds it, other than its own.
    pub fn closures_directly_in(&self, body: BodyId) -> impl Iterator<Item = ExprId> + '_ {
        let all = self.closure_parents.get_or_init(|| {
            let mut found = Vec::new();
            for (id, expr) in self.exprs.entries() {
                let ExprData::Closure {
                    body: own, range, ..
                } = expr
                else {
                    continue;
                };
                let parent = self
                    .bodies
                    .entries()
                    .filter(|(candidate, data)| {
                        *candidate != *own && data.range.contains_range(*range)
                    })
                    .min_by_key(|(_, data)| data.range.len())
                    .map(|(candidate, _)| candidate);
                if let Some(parent) = parent {
                    found.push((parent, id));
                }
            }
            // By parent, and stably, so each body's closures stay in the order they were
            // found and a lookup is a slice rather than a pass over every closure in the file.
            found.sort_by_key(|(parent, _)| *parent);
            found.into_boxed_slice()
        });
        let first = all.partition_point(|(parent, _)| *parent < body);
        let last = all.partition_point(|(parent, _)| *parent <= body);
        all[first..last].iter().map(|(_, closure)| *closure)
    }

    pub fn body(&self, id: BodyId) -> &BodyData {
        &self.bodies[id]
    }

    pub fn stmt(&self, id: StmtId) -> &StmtData {
        &self.stmts[id]
    }

    pub fn expr(&self, id: ExprId) -> &ExprData {
        &self.exprs[id]
    }

    /// Every expression the file wrote, in the order they were lowered.
    ///
    /// The arena rather than a walk, for a reader whose question is about an expression wherever it
    /// is written -- in a method, a closure, or a parameter default -- and that does not care which
    /// body it came from. A walk answers the second half as well and costs a recursion and a vector
    /// per body to do it; anything needing the enclosing scope should use one, and anything not
    /// needing it should not pay for one.
    pub fn expressions(&self) -> impl ExactSizeIterator<Item = (ExprId, &ExprData)> {
        self.exprs.entries()
    }

    pub fn catch(&self, id: CatchId) -> &CatchData {
        &self.catches[id]
    }

    pub fn switch_case(&self, id: SwitchCaseId) -> &SwitchCaseData {
        &self.switch_cases[id]
    }

    pub fn match_arm(&self, id: MatchArmId) -> &MatchArmData {
        &self.match_arms[id]
    }

    pub fn stmt_list(&self, slice: IdSlice<StmtId>) -> &[StmtId] {
        self.stmt_lists.slice(slice)
    }

    pub fn expr_list(&self, slice: IdSlice<ExprId>) -> &[ExprId] {
        self.expr_lists.slice(slice)
    }

    pub fn catch_list(&self, slice: IdSlice<CatchId>) -> &[CatchId] {
        self.catch_lists.slice(slice)
    }

    pub fn switch_case_list(&self, slice: IdSlice<SwitchCaseId>) -> &[SwitchCaseId] {
        self.switch_case_lists.slice(slice)
    }

    pub fn match_arm_list(&self, slice: IdSlice<MatchArmId>) -> &[MatchArmId] {
        self.match_arm_lists.slice(slice)
    }

    pub fn namespace(&self, id: NamespaceId) -> &NamespaceData {
        &self.namespaces[id]
    }

    pub fn class(&self, id: ClassId) -> &ClassData {
        &self.classes[id]
    }

    pub fn function(&self, id: FunctionId) -> &FunctionData {
        &self.functions[id]
    }

    pub fn constant(&self, id: ConstantId) -> &ConstantData {
        &self.constants[id]
    }

    pub fn member(&self, id: MemberId) -> &MemberData {
        &self.members[id]
    }

    pub fn param(&self, id: ParamId) -> &ParamData {
        &self.params[id]
    }

    pub fn hook(&self, id: HookId) -> &HookData {
        &self.hooks[id]
    }

    pub fn type_ref(&self, id: TypeRefId) -> &TypeRefData {
        &self.type_refs[id]
    }

    pub fn name_ref(&self, id: NameRefId) -> &NameRefData {
        &self.name_refs[id]
    }

    pub fn docblock(&self, id: DocblockId) -> &DocblockData {
        &self.docblocks[id]
    }

    /// Every docblock in the file, for a reader asking about a position rather than a declaration.
    pub fn docblocks(&self) -> impl ExactSizeIterator<Item = (DocblockId, &DocblockData)> {
        self.docblocks.entries()
    }

    pub fn template(&self, id: TemplateId) -> &TemplateData {
        &self.templates[id]
    }

    pub fn attribute_literal(&self, id: AttributeLiteralId) -> &AttributeLiteralData {
        &self.attribute_literals[id]
    }

    /// The property defaults lowering was asked to keep -- see [`crate::KeptTableData`].
    pub fn kept_tables(&self) -> &[crate::item::KeptTableData] {
        &self.kept_tables
    }

    /// The string property defaults lowering was asked to keep -- see [`crate::KeptTextData`].
    pub fn kept_texts(&self) -> &[crate::item::KeptTextData] {
        &self.kept_texts
    }

    /// The `X::class` property defaults lowering was asked to keep -- see [`crate::KeptClassData`].
    pub fn kept_classes(&self) -> &[crate::item::KeptClassData] {
        &self.kept_classes
    }

    /// The objects named methods build to declare entries -- see
    /// [`KeptConstructionData`](crate::item::KeptConstructionData).
    pub fn kept_constructions(&self) -> &[crate::item::KeptConstructionData] {
        &self.kept_constructions
    }

    pub fn trait_adaptation(&self, id: TraitAdaptationId) -> &TraitAdaptationData {
        &self.trait_adaptations[id]
    }

    // --- lists ---

    pub fn member_list(&self, slice: IdSlice<MemberId>) -> &[MemberId] {
        self.member_lists.slice(slice)
    }

    pub fn param_list(&self, slice: IdSlice<ParamId>) -> &[ParamId] {
        self.param_lists.slice(slice)
    }

    pub fn hook_list(&self, slice: IdSlice<HookId>) -> &[HookId] {
        self.hook_lists.slice(slice)
    }

    pub fn name_ref_list(&self, slice: IdSlice<NameRefId>) -> &[NameRefId] {
        self.name_ref_lists.slice(slice)
    }

    pub fn type_ref_list(&self, slice: IdSlice<TypeRefId>) -> &[TypeRefId] {
        self.type_ref_lists.slice(slice)
    }

    pub fn attribute_literal_list(
        &self,
        slice: IdSlice<AttributeLiteralId>,
    ) -> &[AttributeLiteralId] {
        self.attribute_literal_lists.slice(slice)
    }

    pub fn template_list(&self, slice: IdSlice<TemplateId>) -> &[TemplateId] {
        self.template_lists.slice(slice)
    }

    pub fn adaptation_list(&self, slice: IdSlice<TraitAdaptationId>) -> &[TraitAdaptationId] {
        self.adaptation_lists.slice(slice)
    }

    /// The innermost namespace containing `offset`.
    ///
    /// Needed because a file can declare several namespaces, and the same short name resolves
    /// differently in each. Returns `None` for the global namespace.
    pub fn namespace_at(&self, offset: phrost_model::TextSize) -> Option<NamespaceId> {
        self.namespaces
            .entries()
            .filter(|(_, data)| data.full_range.contains_inclusive(offset))
            .map(|(id, _)| id)
            .last()
    }

    /// Counts of each entity kind, for memory reporting and index statistics.
    pub fn stats(&self) -> FileHirStats {
        FileHirStats {
            namespaces: self.namespaces.len(),
            imports: self.imports.len(),
            classes: self.classes.len(),
            functions: self.functions.len(),
            constants: self.constants.len(),
            members: self.members.len(),
            params: self.params.len(),
            type_refs: self.type_refs.len(),
            docblocks: self.docblocks.len(),
            stmts: self.stmts.len(),
            exprs: self.exprs.len(),
        }
    }
}

/// The slots of [`FileHir::derived`]. One per fact somebody keeps; the count sizes the array.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Derived {
    /// What `phrost-body` knows about each body's writes, branches and guards.
    BodyFacts = 0,
    /// Every call expression in the file with its range, for "which call is this inside".
    CallExpressions = 1,
    /// Every member-access site, in the order the expression walk meets them.
    MemberSites = 2,
    /// Where the file writes `... ?? NULL`, by range.
    NullCoalesces = 3,
}

impl Derived {
    const COUNT: usize = 4;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileHirStats {
    pub namespaces: usize,
    pub imports: usize,
    pub classes: usize,
    pub functions: usize,
    pub constants: usize,
    pub members: usize,
    pub params: usize,
    pub type_refs: usize,
    pub docblocks: usize,
    /// Body nodes, and so zero unless bodies were lowered. What a change to the shape of an
    /// expression costs is read here: the body IR is the part of a file's HIR that grows with what
    /// the code *does* rather than with what it declares.
    pub stmts: usize,
    pub exprs: usize,
}

/// Construction-time counterpart to [`FileHir`].
///
/// Separate from the finished value so that the read side has no half-built states to defend
/// against: once `finish` returns, every id in the HIR resolves.
///
/// The `alloc_*_list` methods take a complete slice rather than appending one id at a time. That
/// is what makes a nested declaration — a class inside a function body — unable to interleave its
/// members into the enclosing list.
#[derive(Debug)]
pub struct FileHirBuilder {
    hir: FileHir,
}

impl FileHirBuilder {
    pub fn new(file: FileId) -> Self {
        Self {
            hir: FileHir {
                file,
                namespaces: Arena::new(),
                imports: Arena::new(),
                classes: Arena::new(),
                functions: Arena::new(),
                constants: Arena::new(),
                members: Arena::new(),
                params: Arena::new(),
                hooks: Arena::new(),
                type_refs: Arena::new(),
                name_refs: Arena::new(),
                docblocks: Arena::new(),
                trait_adaptations: Arena::new(),
                templates: Arena::new(),
                attribute_literals: Arena::new(),
                kept_tables: Vec::new(),
                kept_texts: Vec::new(),
                kept_classes: Vec::new(),
                kept_constructions: Vec::new(),
                bodies: Arena::new(),
                stmts: Arena::new(),
                exprs: Arena::new(),
                catches: Arena::new(),
                switch_cases: Arena::new(),
                match_arms: Arena::new(),
                stmt_lists: IdVec::new(),
                expr_lists: IdVec::new(),
                catch_lists: IdVec::new(),
                switch_case_lists: IdVec::new(),
                match_arm_lists: IdVec::new(),
                root_body: None,
                empty_body: <BodyId as lspkit_arena::ArenaId>::from_index(0),
                closure_parents: std::sync::OnceLock::new(),
                derived: std::array::from_fn(|_| std::sync::OnceLock::new()),
                bodies_lowered: false,
                strict_types: false,
                member_lists: IdVec::new(),
                param_lists: IdVec::new(),
                hook_lists: IdVec::new(),
                name_ref_lists: IdVec::new(),
                type_ref_lists: IdVec::new(),
                adaptation_lists: IdVec::new(),
                template_lists: IdVec::new(),
                attribute_literal_lists: IdVec::new(),
                string_ranges: Vec::new(),
                comment_ranges: Vec::new(),
            },
        }
    }

    pub fn add_namespace(&mut self, data: NamespaceData) -> NamespaceId {
        self.hir.namespaces.alloc(data)
    }

    pub fn add_import(&mut self, data: ImportData) -> ImportId {
        self.hir.imports.alloc(data)
    }

    pub fn add_class(&mut self, data: ClassData) -> ClassId {
        self.hir.classes.alloc(data)
    }

    pub fn add_function(&mut self, data: FunctionData) -> FunctionId {
        self.hir.functions.alloc(data)
    }

    pub fn add_constant(&mut self, data: ConstantData) -> ConstantId {
        self.hir.constants.alloc(data)
    }

    pub fn add_member(&mut self, data: MemberData) -> MemberId {
        self.hir.members.alloc(data)
    }

    pub fn add_param(&mut self, data: ParamData) -> ParamId {
        self.hir.params.alloc(data)
    }

    pub fn add_hook(&mut self, data: HookData) -> HookId {
        self.hir.hooks.alloc(data)
    }

    /// Records where the file's strings are, from the token stream.
    pub fn set_string_ranges(&mut self, ranges: Vec<phrost_model::TextRange>) {
        self.hir.string_ranges = ranges;
    }

    /// Records where the file's comments are, from the same token stream.
    pub fn set_comment_ranges(&mut self, ranges: Vec<phrost_model::TextRange>) {
        self.hir.comment_ranges = ranges;
    }

    pub fn add_type_ref(&mut self, data: TypeRefData) -> TypeRefId {
        self.hir.type_refs.alloc(data)
    }

    /// Reads back a type reference already added, for a decision that depends on its shape.
    pub fn type_ref(&self, id: TypeRefId) -> &TypeRefData {
        self.hir.type_ref(id)
    }

    /// The same, for the operands of a composite one.
    pub fn type_ref_list(&self, ids: lspkit_arena::IdSlice<TypeRefId>) -> &[TypeRefId] {
        self.hir.type_ref_list(ids)
    }

    /// Reads back a class-like already added, for a member whose meaning depends on its owner.
    pub fn class(&self, id: ClassId) -> &ClassData {
        self.hir.class(id)
    }

    /// Reads back a name reference already added, for a decision about how it was spelled.
    pub fn name_ref(&self, id: NameRefId) -> &NameRefData {
        self.hir.name_ref(id)
    }

    /// The statements a body already added holds, for a lowering that puts more in front of them.
    pub fn body_statements(&self, body: BodyId) -> &[StmtId] {
        self.hir.stmt_list(self.hir.body(body).statements)
    }

    /// Replaces a body's statement list -- see [`Self::body_statements`].
    pub fn set_body_statements(&mut self, body: BodyId, statements: IdSlice<StmtId>) {
        self.hir.bodies[body].statements = statements;
    }

    pub fn add_name_ref(&mut self, data: NameRefData) -> NameRefId {
        self.hir.name_refs.alloc(data)
    }

    pub fn add_docblock(&mut self, data: DocblockData) -> DocblockId {
        self.hir.docblocks.alloc(data)
    }

    pub fn add_template(&mut self, data: TemplateData) -> TemplateId {
        self.hir.templates.alloc(data)
    }

    pub fn add_attribute_literal(&mut self, data: AttributeLiteralData) -> AttributeLiteralId {
        self.hir.attribute_literals.alloc(data)
    }

    pub fn add_kept_table(&mut self, data: crate::item::KeptTableData) {
        self.hir.kept_tables.push(data);
    }

    pub fn add_kept_text(&mut self, data: crate::item::KeptTextData) {
        self.hir.kept_texts.push(data);
    }

    pub fn add_kept_class(&mut self, data: crate::item::KeptClassData) {
        self.hir.kept_classes.push(data);
    }

    pub fn add_kept_construction(&mut self, data: crate::item::KeptConstructionData) {
        self.hir.kept_constructions.push(data);
    }

    pub fn add_trait_adaptation(&mut self, data: TraitAdaptationData) -> TraitAdaptationId {
        self.hir.trait_adaptations.alloc(data)
    }

    pub fn add_body(&mut self, data: BodyData) -> BodyId {
        self.hir.bodies.alloc(data)
    }

    pub fn add_stmt(&mut self, data: StmtData) -> StmtId {
        self.hir.stmts.alloc(data)
    }

    pub fn add_expr(&mut self, data: ExprData) -> ExprId {
        self.hir.exprs.alloc(data)
    }

    pub fn add_catch(&mut self, data: CatchData) -> CatchId {
        self.hir.catches.alloc(data)
    }

    pub fn add_switch_case(&mut self, data: SwitchCaseData) -> SwitchCaseId {
        self.hir.switch_cases.alloc(data)
    }

    pub fn add_match_arm(&mut self, data: MatchArmData) -> MatchArmId {
        self.hir.match_arms.alloc(data)
    }

    pub fn alloc_stmt_list(&mut self, ids: &[StmtId]) -> IdSlice<StmtId> {
        self.hir.stmt_lists.push_slice(ids)
    }

    pub fn alloc_expr_list(&mut self, ids: &[ExprId]) -> IdSlice<ExprId> {
        self.hir.expr_lists.push_slice(ids)
    }

    pub fn alloc_catch_list(&mut self, ids: &[CatchId]) -> IdSlice<CatchId> {
        self.hir.catch_lists.push_slice(ids)
    }

    pub fn alloc_switch_case_list(&mut self, ids: &[SwitchCaseId]) -> IdSlice<SwitchCaseId> {
        self.hir.switch_case_lists.push_slice(ids)
    }

    pub fn alloc_match_arm_list(&mut self, ids: &[MatchArmId]) -> IdSlice<MatchArmId> {
        self.hir.match_arm_lists.push_slice(ids)
    }

    /// Reads back an expression already allocated in this builder.
    ///
    /// Needed because a `/** @var T $x */` is found from the statement's position, which is only
    /// known after the assignment inside it has been lowered.
    pub fn expr(&self, id: ExprId) -> &ExprData {
        &self.hir.exprs[id]
    }

    pub fn expr_mut(&mut self, id: ExprId) -> &mut ExprData {
        &mut self.hir.exprs[id]
    }

    /// Records that this file's bodies were lowered, even if it happens to contain none.
    pub fn mark_bodies_lowered(&mut self) {
        self.hir.bodies_lowered = true;
    }

    /// Records the file's `declare(strict_types=1)`.
    pub fn mark_strict_types(&mut self) {
        self.hir.strict_types = true;
    }

    pub fn set_root_body(&mut self, body: BodyId) {
        self.hir.root_body = Some(body);
    }

    pub fn alloc_member_list(&mut self, ids: &[MemberId]) -> IdSlice<MemberId> {
        self.hir.member_lists.push_slice(ids)
    }

    pub fn alloc_param_list(&mut self, ids: &[ParamId]) -> IdSlice<ParamId> {
        self.hir.param_lists.push_slice(ids)
    }

    pub fn alloc_hook_list(&mut self, ids: &[HookId]) -> IdSlice<HookId> {
        self.hir.hook_lists.push_slice(ids)
    }

    pub fn alloc_name_ref_list(&mut self, ids: &[NameRefId]) -> IdSlice<NameRefId> {
        self.hir.name_ref_lists.push_slice(ids)
    }

    pub fn alloc_type_ref_list(&mut self, ids: &[TypeRefId]) -> IdSlice<TypeRefId> {
        self.hir.type_ref_lists.push_slice(ids)
    }

    pub fn alloc_attribute_literal_list(
        &mut self,
        ids: &[AttributeLiteralId],
    ) -> IdSlice<AttributeLiteralId> {
        self.hir.attribute_literal_lists.push_slice(ids)
    }

    pub fn alloc_template_list(&mut self, ids: &[TemplateId]) -> IdSlice<TemplateId> {
        self.hir.template_lists.push_slice(ids)
    }

    pub fn alloc_adaptation_list(
        &mut self,
        ids: &[TraitAdaptationId],
    ) -> IdSlice<TraitAdaptationId> {
        self.hir.adaptation_lists.push_slice(ids)
    }

    /// Reads back the parameters of a list already allocated in this builder.
    ///
    /// Needed because constructor property promotion turns parameters into properties: the
    /// promoted members can only be built after the parameters exist.
    pub fn params_in(&self, slice: IdSlice<ParamId>) -> impl Iterator<Item = &ParamData> {
        self.hir
            .param_list(slice)
            .iter()
            .map(move |&id| self.hir.param(id))
    }

    /// A class's members are attached after the class exists, because a member records its owner.
    /// Supertype arguments written inside the body -- `/** @use Trait<T> */` above a trait use --
    /// added to the ones the class's own docblock wrote.
    /// Adds to a class's `@mixin`s one its code states -- see the lowering of a method body.
    pub fn add_class_mixins(&mut self, class: ClassId, extra: &[TypeRefId]) {
        let mut all = self
            .hir
            .type_ref_list(self.hir.classes[class].mixins)
            .to_vec();
        all.extend_from_slice(extra);
        self.hir.classes[class].mixins = self.hir.type_ref_lists.push_slice(&all);
    }

    /// Reads back a statement already allocated in this builder.
    pub fn stmt(&self, id: crate::StmtId) -> &crate::body::StmtData {
        self.hir.stmt(id)
    }

    pub fn add_class_supertype_arguments(&mut self, class: ClassId, extra: &[TypeRefId]) {
        let mut all = self
            .hir
            .type_ref_list(self.hir.classes[class].supertype_arguments)
            .to_vec();
        all.extend_from_slice(extra);
        self.hir.classes[class].supertype_arguments = self.hir.type_ref_lists.push_slice(&all);
    }

    pub fn set_class_members(&mut self, class: ClassId, members: IdSlice<MemberId>) {
        self.hir.classes[class].members = members;
    }

    /// Trait uses are attached after the class exists, for the same reason as members: an
    /// adaptation is only meaningful once there is a class to adapt.
    pub fn set_class_traits(
        &mut self,
        class: ClassId,
        uses: IdSlice<NameRefId>,
        adaptations: IdSlice<TraitAdaptationId>,
    ) {
        self.hir.classes[class].uses = uses;
        self.hir.classes[class].trait_adaptations = adaptations;
    }

    /// Releases the capacity the builder over-allocated, then freezes the result.
    ///
    /// Shrinking matters here: this value is retained for every file in the project, and a `Vec`
    /// grown by doubling can hold nearly twice the bytes it needs.
    pub fn finish(mut self) -> FileHir {
        // Before the shrinking, because it is one more row: see [`FileHir::empty_body`].
        self.hir.empty_body = self.hir.bodies.alloc(BodyData {
            statements: IdSlice::EMPTY,
            range: phrost_model::TextRange::empty(phrost_model::TextSize::from(0)),
            is_generator: false,
        });
        self.hir.namespaces.shrink_to_fit();
        self.hir.imports.shrink_to_fit();
        self.hir.classes.shrink_to_fit();
        self.hir.functions.shrink_to_fit();
        self.hir.constants.shrink_to_fit();
        self.hir.members.shrink_to_fit();
        self.hir.params.shrink_to_fit();
        self.hir.hooks.shrink_to_fit();
        self.hir.type_refs.shrink_to_fit();
        self.hir.name_refs.shrink_to_fit();
        self.hir.docblocks.shrink_to_fit();
        self.hir.trait_adaptations.shrink_to_fit();
        self.hir.bodies.shrink_to_fit();
        self.hir.stmts.shrink_to_fit();
        self.hir.exprs.shrink_to_fit();
        self.hir.catches.shrink_to_fit();
        self.hir.switch_cases.shrink_to_fit();
        self.hir.match_arms.shrink_to_fit();
        self.hir.member_lists.shrink_to_fit();
        self.hir.param_lists.shrink_to_fit();
        self.hir.hook_lists.shrink_to_fit();
        self.hir.name_ref_lists.shrink_to_fit();
        self.hir.type_ref_lists.shrink_to_fit();
        self.hir.adaptation_lists.shrink_to_fit();
        self.hir
    }
}

#[cfg(test)]
mod tests {
    use super::FileHir;

    #[test]
    fn the_hir_owns_everything_it_describes() {
        // The mechanical proof of the parser boundary. mago's tree is `Program<'arena>`, so if a
        // borrow from it ever leaked into the HIR this stops compiling -- which is far better than
        // discovering it when the index tries to retain a file.
        fn assert_owned<T: 'static>() {}
        assert_owned::<FileHir>();
    }
}
