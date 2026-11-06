//! Owned, syntax-shaped IR for one PHP file. This is the boundary above the parser.
//!
//! A [`FileHir`] describes everything declared in one file — namespaces, imports, classes and
//! their members, functions, constants — with no lifetimes, no borrowed text, and no dependency
//! on how it was produced. Everything above this crate reads only this.
//!
//! That boundary is not stylistic. mago's tree is arena-bound (`Program<'arena>`), so a parsed
//! node physically cannot be stored in a long-lived index, and mago's node shape has already
//! changed once under this project. Lowering into an owned IR and dropping the arena is therefore
//! both forced and the thing that keeps a parser upgrade confined to one crate.
//!
//! # What this layer does and does not know
//!
//! It is *syntax-shaped*: `?Foo` stays a nullable wrapper around a written name, and nothing here
//! knows whether `Foo` exists, which file declares it, or what `self` refers to. Resolution is the
//! def-map's job, inheritance is the semantic layer's, and expression bodies are lowered
//! separately because they are far larger and needed far less often.
//!
//! What it *does* guarantee is that every declaration carries a name range — the goto target —
//! and that those ranges survive being cached and reloaded. The "no fallbacks" rule depends on it:
//! there is no fuzzy text scan to fall back on when a range is missing.

pub mod body;
mod docblock;
mod file;
mod ids;
mod import;
mod item;
mod name_ref;
pub mod render;
mod type_ref;

pub use self::{
    docblock::DocblockData,
    file::{Derived, FileHir, FileHirBuilder},
    ids::{
        AttributeLiteralId, BodyId, CatchId, ClassId, ConstantId, DocblockId, ExprId, FunctionId,
        HookId, ImportId, MatchArmId, MemberId, NameRefId, NamespaceId, ParamId, StmtId,
        SwitchCaseId, TemplateId, TraitAdaptationId, TypeRefId,
    },
    import::{ImportData, ImportKind, NamespaceData},
    item::{
        ANONYMOUS_CLASS_PREFIX, AttributeArgument, AttributeLiteralData, ClassData, ClassKind,
        ClassModifiers, ConditionTestData, ConditionalBranchData, ConstantData, ConstantTableData,
        ConstantValueData, ContractData, FunctionData, HookData, HookKind, Hooks, KeptCallableData,
        KeptClassData, KeptConstructionData, KeptEntryData, KeptTableData, KeptTextData,
        KeptValueData, MemberData, MemberKind, MemberModifiers, ParamData, ParamPromotion,
        ReturnBranchData, ReturnRootData, ReturnedKeyData, ReturnedPathData, ShapeLiteralData,
        TemplateData, TestCaseData, TraitAdaptationData, TraitAdaptationKind, display_class_name,
        is_anonymous_class_name,
    },
    name_ref::{NameAnchor, NameRefData},
    type_ref::{TypeRefData, refined_type_ref},
};
