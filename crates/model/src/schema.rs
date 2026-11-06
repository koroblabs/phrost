//! What a migration writes about a table's columns, in words both sides of the framework seam read.
//!
//! Laravel's `$table->string('email')->nullable()` adds a column named `email` that holds a string
//! or nothing. Which builder method adds which column is the framework's vocabulary; finding the
//! calls in a body is the index's. Neither owns the other, which is this crate's bar: the index
//! hands over each call as written, and the framework says what it does to the table.

use crate::BuiltinTy;

/// One call on a table's builder, as written: `$table->string('email', 100)->nullable()`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BuilderCall {
    pub method: String,
    /// Each argument's string literals in order: one for a literal, every one for an array of them,
    /// none for anything else -- so a position is still a position when its value is not a string.
    pub arguments: Vec<Vec<String>>,
    /// The calls chained after it, each with its first argument where that is `true` or `false`:
    /// `nullable`, `nullable(false)`, `change`.
    pub then: Vec<(String, Option<bool>)>,
}

/// What one call does to a table's columns.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ColumnChange {
    /// A column added, or redefined where the call says it changes one that exists.
    Defined {
        name: String,
        /// What a row read back holds in it before anything casts it: `mixed` where the column's
        /// type is one PHP has no word for.
        holds: BuiltinTy,
        nullable: bool,
        /// A foreign key the database enforces: `->constrained()`, `->references(...)`. Such a column
        /// that cannot be null points at a row that exists, which is what makes the relation read
        /// through it never empty.
        constrained: bool,
    },
    /// `$table->foreign('account_id')->references('id')->on('accounts')`: a column defined
    /// elsewhere made a foreign key the database enforces.
    Constrained(String),
    Dropped(String),
    Renamed {
        from: String,
        to: String,
    },
}
