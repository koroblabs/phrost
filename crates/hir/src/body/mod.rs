//! Function-like bodies: statements, expressions, and where they are.
//!
//! Lowered only for open buffers, and stored in the same [`FileHir`](crate::FileHir) as the
//! declarations so that a body's type hints share the file's type-reference arena instead of
//! duplicating it. `FileHir::has_bodies` says whether anybody asked.

mod data;
mod render;

pub use self::{
    data::{
        ArgumentLiteral, ArithmeticOp, ArrayElementKind, AssignOp, BindKind, BodyData,
        CallableReceiver, CatchData, ClassTarget, ClosureKind, CompareOp, ExprData,
        INTERPOLATION_HOLE, LeaveKind, LiteralArgument, LiteralKind, LogicalOp, MatchArmData,
        NullTestKind, Passage, StmtData, SwitchCaseData, UnmodelledKind,
    },
    render::render_body,
};
