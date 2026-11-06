//! Inputs a class declares in a property's string, and the methods reading them by name.
//!
//! The framework seam's `DeclaredInputs`, restated below it for the reason the contracts are --
//! with its grammar carried down as the framework's own function, because the declaration is
//! written in a language only the framework knows. Nothing is stored per file: the reads sit beside
//! the declaration, so it is read from the file being analysed, where lowering kept it.

use phrost_model::DeclaredInput;

/// See the framework seam's `DeclaredInputs` for each field.
#[derive(Clone, Debug)]
pub struct DeclaredInputsRule {
    pub class: Box<str>,
    pub property: Box<str>,
    /// Each reading method, with what the declaration writes before a name it reads.
    pub readers: Vec<(Box<str>, Box<str>)>,
    pub parse: fn(&str) -> Vec<DeclaredInput>,
    /// What every declaration has beside its own, as a declaration writes it.
    pub always: Vec<Box<str>>,
    /// Methods that, declared on the class or called in it, mean its inputs come from elsewhere too.
    pub unless: Vec<Box<str>>,
}

impl DeclaredInputsRule {
    /// What the declaration writes before a name `method` reads, when this rule's methods include it.
    pub fn prefix_read_by(&self, method: &str) -> Option<&str> {
        self.readers
            .iter()
            .find(|(reader, _)| reader.eq_ignore_ascii_case(method))
            .map(|(_, prefix)| &**prefix)
    }
}
