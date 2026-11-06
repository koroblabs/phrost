//! What a scalar in a configuration file names, as a framework stated it.
//!
//! Strings and a kind, like every other table a provider fills: nothing in this crate knows whose
//! keys these are or what a `route` is. What is stored is the statement "a value under this key, in
//! a file whose name ends this way, is a key of this table".
//!
//! Read by the feature that answers a request in a YAML buffer, and by nothing on the PHP path -- so
//! this is a short list scanned in order, not an index.

/// A class named in a configuration file, and where it is written.
///
/// The other half of what a framework's rules are for. A rule says a value under some key names a
/// class; this is that rule applied, once, to every configuration file the project owns -- so a
/// service's `class:` is a *reference* like any other. `references` lists it, a rename rewrites
/// it, and a class named only there is not dead code, none of which was reachable while a
/// configuration file could only be read by opening it.
///
/// A path rather than a `FileId`, because these files are not indexed and giving them one would
/// mean every reader of the file table having an opinion about a file with no declarations in it.
/// Derived at bootstrap and never cached: 430 sites on a real project's own code, 13 KB, 1.5 ms to
/// read again -- against a stored copy that `adopt` would have to remember to carry, which is the
/// shape §8 records for a body-derived fact that is only right on a cold start.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigSite {
    pub path: std::sync::Arc<std::path::Path>,
    /// Fully qualified, no leading separator.
    pub name: Box<str>,
    /// The name's own bytes in the file.
    pub range: phrost_model::TextRange,
    /// The method half of a `Class::method`, which a route writes for its controller.
    pub member: Option<(Box<str>, phrost_model::TextRange)>,
}

/// One rule a framework contributed, in this crate's own terms.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigReference {
    pub in_files: Box<str>,
    pub under: Box<str>,
    pub prefix: Box<str>,
    /// A mark after the prefix saying the referent may be missing. Empty when the rule has none.
    pub optional: Box<str>,
    pub names: ConfigTarget,
}

/// What a configuration value names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigTarget {
    CatalogKey(Box<str>),
    Class,
    Callable,
    /// A container `factory`: `Foo::create`, `\Foo`, or a bare method of the service beside it.
    Factory,
    /// Several keys of one catalog in one scalar. Each character of `separators` splits it.
    CatalogKeys {
        catalog: Box<str>,
        separators: Box<str>,
    },
}

impl ConfigReference {
    /// Whether this rule is about a file with this name.
    ///
    /// By suffix, because a framework names a configuration file after the extension that owns it:
    /// `app_core.routing.yml` is a routing file and so is `node.routing.yml`. Compared
    /// case-sensitively, as file names in these frameworks are.
    pub fn covers(&self, file_name: &str) -> bool {
        file_name.ends_with(&*self.in_files)
    }

    /// Whether a value written under `key` and opening with this rule's prefix is one it describes.
    ///
    /// An empty `under` matches any key, which is what a value recognised by its own spelling needs;
    /// the prefix is then the whole of the recognition. Both together is an `and`, so a rule may
    /// name a key *and* a spelling.
    pub fn matches(&self, key: &str, value: &str) -> bool {
        (self.under.is_empty() || &*self.under == key) && value.starts_with(&*self.prefix)
    }

    /// The value with this rule's prefix and optional mark taken off, which is the name itself.
    pub fn name_in<'a>(&self, value: &'a str) -> &'a str {
        let rest = value.strip_prefix(&*self.prefix).unwrap_or(value);
        match self.optional.is_empty() {
            true => rest,
            false => rest.strip_prefix(&*self.optional).unwrap_or(rest),
        }
    }

    /// Whether the value is written with this rule's optional mark, so that nothing registering
    /// the name it holds is a state the framework allows rather than a mistake.
    pub fn may_be_absent(&self, value: &str) -> bool {
        let rest = value.strip_prefix(&*self.prefix).unwrap_or(value);
        !self.optional.is_empty() && rest.starts_with(&*self.optional)
    }
}

/// How a project's Twig loader names its templates, as a framework stated it: the catalog holding
/// every template by the name a template is written with, the catalog of every other name the
/// loader takes, the catalog of namespaces walked, and the directories the walk did not enter.
///
/// Read by the feature answering a request in a template, and by nothing on the PHP path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TwigLoader {
    pub templates: Box<str>,
    pub aliases: Box<str>,
    pub namespaces: Box<str>,
    pub unwalked: Box<[Box<str>]>,
    /// Variables every template has, each with its type as the framework wrote it: a class fully
    /// qualified, or a builtin by name.
    pub globals: Box<[(Box<str>, Box<str>)]>,
}

impl TwigLoader {
    /// Whether a name's directories include one the walk did not enter, so that nothing the
    /// catalogs lack says anything about it.
    pub fn passes_unwalked(&self, name: &str) -> bool {
        let directories = name
            .rsplit_once('/')
            .map_or("", |(directories, _)| directories);
        directories
            .split('/')
            .any(|segment| self.unwalked.iter().any(|unwalked| **unwalked == *segment))
    }
}
