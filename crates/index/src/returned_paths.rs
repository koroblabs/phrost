//! The member chain a declaration's body hands back, for the callers that cannot read the body.
//!
//! The sibling of [`ThrownRow`](crate::ThrownRow) and stored for the same reason one layer over: a
//! closed file keeps no statements, so a *description* of what its body returns is the only thing a
//! caller can be given. `SqlBase::select()` in Drupal core declares no return type and carries no
//! `@return`, and its whole body is `return $this->getDatabase()->select($table, $alias, $options);`
//! -- which names no type and describes one exactly.
//!
//! Sparse, and it has to be: core declares **4 181** methods with neither a return type nor a
//! `@return`, and 1 235 of them are a chain this can hold. The rest hand back an array literal or a
//! local, which says `array` or nothing and neither is worth a row.
//!
//! Resolved on demand rather than at ingest, because that is the half ingest cannot do: the class
//! the chain walks into is declared in another file, which may not be indexed yet and whose answer
//! must not depend on the order files arrived in.

use phrost_model::Name;

/// One declaration's returned chain, with its root resolved against the file that wrote it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReturnedRow {
    /// The declaring class, for a method. `None` for a free function.
    pub owner: Option<Name>,
    /// The method or function name, as the lookup asks for it.
    pub name: Name,
    /// The class the chain starts from, resolved at ingest. `None` for `$this`, `static::` and
    /// `self::`, which all mean the class the declaration is in -- and which the *caller* knows
    /// better than the declaration does, because a subclass calling an inherited wrapper starts
    /// from itself.
    pub root: Option<Name>,
    /// Member names in written order. Never empty: a root with no steps names a type outright, and
    /// that is already stored as one.
    pub steps: Box<[Name]>,
    /// The first step's first argument, when it is one a contract reads.
    ///
    /// The whole of what `return \Drupal::service('utilities.manager');` says: the chain names
    /// `Drupal::service`, which describes every service there is, and the string is what picks one.
    /// Small, and the table is sparse to begin with.
    pub key: Option<ReturnedKey>,
}

/// The first argument of a returned chain's first call, in the two spellings a contract reads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReturnedKey {
    /// A string literal, which a catalog contract reads as a key.
    Text(Name),
    /// `Foo::class`, resolved to the class at ingest, which a `class-string<T>` contract reads.
    Class(Name),
}

impl lspkit_memsize::MemSize for ReturnedRow {
    /// The step list's buffer. Every `Name` is a pointer into the interner, which counts its own.
    fn heap_size(&self) -> usize {
        lspkit_memsize::flat(&self.steps)
    }
}
