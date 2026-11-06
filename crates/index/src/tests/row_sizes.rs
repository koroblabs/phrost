//! What one row of each retained table costs.
//!
//! A measurement rather than an assertion, printed so a change to a row's shape can be priced before
//! it is made. Only the tables the index *keeps* are here: a `FileHir` is dropped for a closed file,
//! so its rows do not scale with the project.
//!
//! # What it was written to answer, and the answer
//!
//! Giving `TypeId` a niche took the index from 92.6 MB to 88.9 MB, and every `*Id` in this workspace
//! is a bare integer newtype -- so the obvious next question is which of the others is worth the
//! same. Measured, **none of them**, and each for its own reason:
//!
//! - **The arena ids** (`ClassId`, `TypeRefId`, and the eighteen beside them, all from one macro in
//!   `lspkit-arena`) are retained only inside a `FileHir`, which exists for an *open* buffer. Twenty
//!   open files cost 2.5 MB in total, so a fraction of a fraction of that is the whole prize.
//! - **`TextSize` cannot have one.** Zero is a real position -- the start of a file -- so it is not a
//!   spare slot to reserve. An off-by-one would work for a table index and not for this: the type is
//!   arithmetic, added and compared and converted to byte offsets throughout.
//! - **And it would buy nothing even so.** `DeclaredSymbol`'s fields add up to 55 bytes in a 56-byte
//!   struct; taking four off `Option<TextRange>` leaves 51, which is still 56 once aligned. The one
//!   place an optional range is held per project-wide row has no room to give it up.
//! - **`Name` already has one**, being an `Arc<str>`: `Option<Name>` is 16 bytes like `Name`.
//! - **`FileId`** is held optionally only in a handful of scalars -- the view's current file, a
//!   lookup's return -- and never in a row there are many of.
//!
//! So `TypeId` was the one with weight, and this file is why nobody needs to re-derive that.

#[test]
fn print_the_size_of_every_retained_row() {
    use std::mem::size_of;

    let rows: &[(&str, usize)] = &[
        ("sema::MemberRow", size_of::<phrost_sema::MemberRow>()),
        ("sema::ParamRow", size_of::<phrost_sema::ParamRow>()),
        (
            "defmap::DeclaredSymbol",
            size_of::<phrost_defmap::DeclaredSymbol>(),
        ),
        (
            "defmap::NamespaceScope",
            size_of::<phrost_defmap::NamespaceScope>(),
        ),
        (
            "defmap::ImportBinding",
            size_of::<phrost_defmap::ImportBinding>(),
        ),
        (
            "defmap::ScopedAttributeLiteral",
            size_of::<phrost_defmap::ScopedAttributeLiteral>(),
        ),
        ("ty::TypeId", size_of::<phrost_ty::TypeId>()),
        ("Option<ty::TypeId>", size_of::<Option<phrost_ty::TypeId>>()),
        ("hir::ClassId", size_of::<phrost_hir::ClassId>()),
        (
            "Option<hir::ClassId>",
            size_of::<Option<phrost_hir::ClassId>>(),
        ),
        ("hir::TypeRefId", size_of::<phrost_hir::TypeRefId>()),
        (
            "Option<hir::TypeRefId>",
            size_of::<Option<phrost_hir::TypeRefId>>(),
        ),
        ("model::TextSize", size_of::<phrost_model::TextSize>()),
        ("model::TextRange", size_of::<phrost_model::TextRange>()),
        (
            "Option<model::TextRange>",
            size_of::<Option<phrost_model::TextRange>>(),
        ),
        ("model::Name", size_of::<phrost_model::Name>()),
        (
            "Option<model::Name>",
            size_of::<Option<phrost_model::Name>>(),
        ),
        ("model::FileId", size_of::<phrost_model::FileId>()),
        (
            "Option<model::FileId>",
            size_of::<Option<phrost_model::FileId>>(),
        ),
    ];
    for (name, size) in rows {
        println!("{name:34} {size:4} bytes");
    }
}
