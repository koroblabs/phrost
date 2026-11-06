//! What a framework's container builds, and which static calls reach into it from outside.
//!
//! A class the container constructs can be handed what it needs; a static call that fetches the
//! same thing from the container -- a service locator -- is then a dependency nobody can see or
//! replace. Which classes are built and which calls reach in are the framework's facts, contributed
//! as strings like everything else here.

use super::{ReturnContracts, catalogs::CatalogValue};

/// One static call that reaches into the container.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContainerReachRule {
    /// The class the call is made on.
    pub class: Box<str>,
    /// Whether a class descending from it counts: `File::load()` is the entity base's `load`.
    pub descendants: bool,
    /// The methods that reach in, or empty when every static call on the class does.
    pub methods: Vec<Box<str>>,
    /// What to hand the class instead, for the message.
    pub instead: Box<str>,
}

/// The container rules every framework present contributed.
#[derive(Debug, Default)]
pub(super) struct ContainerRules {
    /// Interfaces whose implementers the container constructs.
    built_through: Vec<Box<str>>,
    /// Catalogs whose entries are classes the container constructs.
    built_from: Vec<Box<str>>,
    reaches: Vec<ContainerReachRule>,
}

impl ReturnContracts {
    /// An interface whose implementers the container constructs.
    pub fn add_container_interface(&mut self, interface: &str) {
        let interface = Box::from(interface.trim_start_matches('\\'));
        if !self.container.built_through.contains(&interface) {
            self.container.built_through.push(interface);
        }
    }

    /// A catalog whose entries are classes the container constructs: a service id names one.
    pub fn add_container_catalog(&mut self, catalog: &str) {
        let catalog = Box::from(catalog);
        if !self.container.built_from.contains(&catalog) {
            self.container.built_from.push(catalog);
        }
    }

    /// A static call on `class` -- or on a class descending from it -- that reaches into the
    /// container: any of `methods`, or every one when there are none.
    pub fn add_container_reach(
        &mut self,
        class: &str,
        descendants: bool,
        methods: &[&str],
        instead: &str,
    ) {
        let rule = ContainerReachRule {
            class: class.trim_start_matches('\\').into(),
            descendants,
            methods: methods.iter().map(|&method| method.into()).collect(),
            instead: instead.into(),
        };
        if !self.container.reaches.contains(&rule) {
            self.container.reaches.push(rule);
        }
    }

    pub fn container_interfaces(&self) -> &[Box<str>] {
        &self.container.built_through
    }

    pub fn container_reaches(&self) -> &[ContainerReachRule] {
        &self.container.reaches
    }

    /// Whether a catalog that lists what the container constructs names this class.
    ///
    /// A scan of every entry, asked once per class a file declares, and only on a project whose
    /// framework contributed a reach rule at all.
    pub fn container_builds_by_catalog(&self, class: &str) -> bool {
        let class = class.trim_start_matches('\\');
        self.container.built_from.iter().any(|catalog| {
            self.catalogs.get(catalog).is_some_and(|keys| {
                keys.values().any(|value| {
                    matches!(value, CatalogValue::Class(named)
                        if named.trim_start_matches('\\').eq_ignore_ascii_case(class))
                })
            })
        })
    }
}
