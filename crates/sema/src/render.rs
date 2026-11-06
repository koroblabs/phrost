//! Canonical text rendering of a flattened member table.
//!
//! Same contract as the other renderers: sorted, positional, and free of anything that could
//! differ between two builds of the same input. Provenance is always shown, because the whole
//! point of a flattened table is *which* declaration won, and a snapshot that hid that would
//! pass while trait precedence was backwards.

use std::fmt::Write as _;

use phrost_ty::TypeTable;

use crate::table::MemberTable;

/// Renders a table as one line per member, plus any conflicts.
pub fn render_member_table(table: &MemberTable, types: &TypeTable) -> String {
    let mut out = String::new();

    for member in table.members() {
        let _ = write!(
            out,
            "{} {}",
            member.row.kind.as_str(),
            member.row.signature(types)
        );
        // An alias exposes the same declaration under a second name; showing both is the only way
        // to tell an alias from a rename.
        if member.name != member.row.name {
            let _ = write!(out, " (as {})", member.name);
        }
        let _ = write!(out, "  {}", member.visibility.as_str());
        for (flag, label) in [
            (member.row.modifiers.is_static(), "static"),
            (member.row.modifiers.is_abstract(), "abstract"),
            (member.row.modifiers.is_final(), "final"),
            (member.row.modifiers.is_readonly(), "readonly"),
        ] {
            if flag {
                let _ = write!(out, " {label}");
            }
        }
        let _ = writeln!(out, "  [{}]", member.provenance.label());
    }

    for conflict in table.conflicts() {
        let _ = writeln!(
            out,
            "!! {} {} from both {} and {}",
            conflict.kind.as_str(),
            conflict.member,
            conflict.first,
            conflict.second
        );
    }

    if table.is_incomplete() {
        // Said out loud, because a short table and a short-because-something-was-missing table
        // mean very different things to whoever reads the answer.
        let _ = writeln!(
            out,
            "?? incomplete: a parent, interface, or trait was missing or cyclic"
        );
    }

    if out.is_empty() {
        return "<no members>\n".to_owned();
    }

    out
}
