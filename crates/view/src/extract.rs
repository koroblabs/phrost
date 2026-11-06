//! Lifting an expression into a variable of its own.
//!
//! Which expression, and whether lifting it is the same program, is
//! [`phrost_body::extractable_at`] -- it refuses a short-circuit, a branch and a closure, each
//! because hoisting would make the code run when it did not before. What this adds is the two things
//! that need the file rather than the body: a name nothing else is already using, and the
//! indentation the new line is written at.

use phrost_model::{TextRange, TextSize};

use crate::view::IndexView;

/// An expression to lift out, and everything writing the two lines needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Extraction {
    /// The expression, which the variable replaces.
    pub range: TextRange,
    /// Where the assignment goes: the first byte of the statement holding it.
    pub at: TextSize,
    /// What the statement is indented by, so the new line lines up with it.
    pub indent: String,
    /// Without the leading `$`, as names are stored everywhere else.
    pub name: String,
}

impl IndexView<'_> {
    /// The variable the range at `at` could become.
    pub fn extraction_at(&self, at: TextRange) -> Option<Extraction> {
        let open = self.current()?;
        let hir = open.hir();
        let found = phrost_body::extractable_at(hir, at)?;

        // Every name the scope already binds, so the new one does not shadow one of them -- which
        // would be a rename nobody asked for, and a silent one.
        let taken = phrost_body::locals_in_scope(hir, found.statement);
        let wanted = found
            .named_after
            .as_ref()
            .map(|name| stripped_of_a_verb(name))
            // A name PHP would accept as a variable. An anonymous class's is synthesised from its
            // offset -- `class@anonymous#4450` -- and offering `$class@anonymous#4450 = new class
            // {...};` writes a file that does not parse.
            .filter(|name| is_a_variable_name(name))
            .unwrap_or_else(|| "value".to_owned());

        Some(Extraction {
            range: found.range,
            at: found.statement,
            indent: self.indentation_at(found.statement),
            name: unused(&wanted, &taken),
        })
    }
}

/// Whether PHP would accept this as the name of a variable.
///
/// A letter or an underscore, then letters, digits and underscores. Asked because the suggestion
/// comes from what the expression *names*, and not everything with a name has one a person could
/// type: an anonymous class is called `class@anonymous#4450` here, after the offset that makes it
/// unique.
fn is_a_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// A method's name with the word that makes it a verb taken off.
///
/// `getTitle()` names a title, not a get. Only the prefixes that are the language's own convention
/// for an accessor, and only where something is left after them: `get()` is a name of its own.
fn stripped_of_a_verb(name: &str) -> String {
    for verb in [
        "get", "fetch", "find", "load", "read", "build", "make", "create",
    ] {
        if let Some(rest) = name.strip_prefix(verb)
            && rest.starts_with(|ch: char| ch.is_ascii_uppercase())
        {
            return lowered(rest);
        }
    }
    lowered(name)
}

/// The name with its first character lower-cased, which is how PHP writes a variable.
fn lowered(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// The name, or the first numbered form of it nothing in scope is already using.
///
/// Shadowing a local silently changes what every use of that name below means, which is a rename
/// nobody asked for -- and the one thing a refactor may not do is change the program.
fn unused(wanted: &str, taken: &[phrost_model::Name]) -> String {
    let held = |name: &str| taken.iter().any(|bound| bound.as_str() == name);
    if !held(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|nth| format!("{wanted}{nth}"))
        .find(|name| !held(name))
        .unwrap_or_else(|| wanted.to_owned())
}
