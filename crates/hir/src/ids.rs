//! Id families for the per-file IR.
//!
//! Every id is local to one [`FileHir`](crate::FileHir). Crossing files is the def-map's job and
//! needs a reference that also says which file — keeping that distinction in the type system is
//! what stops a stale id from resolving against the wrong file.

lspkit_arena::arena_id!(
    /// One `namespace` declaration. A file may hold several.
    pub struct NamespaceId;
);

lspkit_arena::arena_id!(
    /// One `use` import, after group imports are flattened.
    pub struct ImportId;
);

lspkit_arena::arena_id!(
    /// One class, interface, trait, or enum declaration.
    pub struct ClassId;
);

lspkit_arena::arena_id!(
    /// One free function declaration.
    pub struct FunctionId;
);

lspkit_arena::arena_id!(
    /// One namespace-level `const` or `define()`.
    pub struct ConstantId;
);

lspkit_arena::arena_id!(
    /// One method, property, class constant, or enum case.
    pub struct MemberId;
);

lspkit_arena::arena_id!(
    /// One parameter of a function or method.
    pub struct ParamId;
);

lspkit_arena::arena_id!(
    /// One `get` or `set` hook of a property.
    pub struct HookId;
);

lspkit_arena::arena_id!(
    /// One native type expression, as written.
    pub struct TypeRefId;
);

lspkit_arena::arena_id!(
    /// One class-like name as written, before resolution.
    pub struct NameRefId;
);

lspkit_arena::arena_id!(
    /// One docblock comment attached to a declaration.
    pub struct DocblockId;
);

lspkit_arena::arena_id!(
    /// One `insteadof` or `as` clause in a `use TraitName { ... }` block.
    pub struct TraitAdaptationId;
);

lspkit_arena::arena_id!(
    /// One `@template` parameter on a class-like declaration.
    pub struct TemplateId;
);

lspkit_arena::arena_id!(
    /// One named string-literal argument of an attribute on a class-like declaration.
    pub struct AttributeLiteralId;
);

lspkit_arena::arena_id!(
    /// One function-like body. Present only for files whose bodies were lowered.
    pub struct BodyId;
);

lspkit_arena::arena_id!(
    /// One statement inside a body.
    pub struct StmtId;
);

lspkit_arena::arena_id!(
    /// One expression inside a body.
    pub struct ExprId;
);

lspkit_arena::arena_id!(
    /// One `catch` clause.
    pub struct CatchId;
);

lspkit_arena::arena_id!(
    /// One `case`/`default` clause of a `switch`, after empty-bodied fallthrough labels are
    /// folded into the arm they fall through to.
    pub struct SwitchCaseId;
);

lspkit_arena::arena_id!(
    /// One arm of a `match` expression.
    pub struct MatchArmId;
);
