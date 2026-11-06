//! What a docblock annotation is written as when the framework has moved on to attributes.
//!
//! One namespace segment and a short list of names, contributed the way everything else here is:
//! as strings, with nothing in this crate knowing whose they are. Where the segment comes from and
//! why it is enough is written at the seam that supplies it.
//!
//! Read by the code action that offers the conversion. Nothing on a request path asks, so this is a
//! table and not an index.

use super::ReturnContracts;

impl ReturnContracts {
    /// Records the namespace segment a framework's attribute classes sit in.
    pub fn rewrite_annotations_in(&mut self, segment: &str) {
        self.attributes_in = Some(segment.into());
    }

    /// Records an annotation that stands for a plain object, and the class to construct for it.
    pub fn construct_for_annotation(&mut self, annotation: &str, class: &str) {
        self.annotation_values
            .push((annotation.into(), class.into()));
    }

    /// The namespace segment an attribute class sits in, when some framework said.
    ///
    /// `None` for a project with no framework, and that is the whole gate: without it nothing knows
    /// what an annotation would become, so the conversion is never offered.
    pub fn attribute_namespace(&self) -> Option<&str> {
        self.attributes_in.as_deref()
    }

    /// The class a nested annotation constructs, for one written as an argument's value.
    ///
    /// Case-sensitively, because this is matched against a name written in a docblock and PHP class
    /// names are written the way their declaration spells them. A scan of four short strings, which
    /// is the same reasoning every neighbouring table records.
    pub fn class_for_annotation(&self, annotation: &str) -> Option<&str> {
        self.annotation_values
            .iter()
            .find(|(name, _)| &**name == annotation)
            .map(|(_, class)| &**class)
    }
}

impl ReturnContracts {
    /// Records that a file's mapping keys are class names, by the end of its name.
    pub fn class_keys_in_config(&mut self, in_files: &str) {
        self.class_keyed_files.push(in_files.into());
    }

    /// Whether a file of this name writes its class names as keys.
    pub fn class_keys_cover(&self, file_name: &str) -> bool {
        self.class_keyed_files
            .iter()
            .any(|suffix| file_name.ends_with(&**suffix))
    }

    /// Records what a scalar in one kind of configuration file names.
    pub fn reference_in_config(
        &mut self,
        in_files: &str,
        under: &str,
        prefix: &str,
        optional: &str,
        names: crate::configuration::ConfigTarget,
    ) {
        self.config_references
            .push(crate::configuration::ConfigReference {
                in_files: in_files.into(),
                under: under.into(),
                prefix: prefix.into(),
                optional: optional.into(),
                names,
            });
    }

    /// Every such rule, in the order the providers stated them.
    ///
    /// Order is the answer where two could match: a rule naming a key is stated before the one
    /// recognising a bare spelling, so `class: '@id'` -- which nobody writes, but which the scanner
    /// must still answer once -- reads as the key says.
    pub fn config_references(&self) -> &[crate::configuration::ConfigReference] {
        &self.config_references
    }

    /// Records how a framework's Twig loader names templates.
    pub fn add_twig_loader(&mut self, loader: crate::configuration::TwigLoader) {
        if !self.twig_loaders.contains(&loader) {
            self.twig_loaders.push(loader);
        }
    }

    /// Records that constructing `class` in a method named `method` declares a key of `catalog`.
    pub fn bind_constructed_entry(&mut self, method: &str, class: &str, catalog: &str) {
        let entry = (class.trim_start_matches('\\').into(), catalog.into());
        if !self.constructed_entries.contains(&entry) {
            self.constructed_entries.push(entry);
        }
        if !self
            .construction_methods
            .iter()
            .any(|known| &**known == method)
        {
            self.construction_methods.push(method.into());
        }
    }

    /// The methods whose constructions are kept, for lowering to read.
    pub fn construction_methods(&self) -> &[Box<str>] {
        &self.construction_methods
    }

    /// Records which catalogs hold a Twig template's filters, functions and tests.
    pub fn set_twig_callables(&mut self, filters: &str, functions: &str, tests: &str) {
        self.twig_callables = Some([filters.into(), functions.into(), tests.into()]);
    }

    /// The catalogs holding a Twig template's filters, functions and tests, in that order.
    pub fn twig_callables(&self) -> Option<&[Box<str>; 3]> {
        self.twig_callables.as_ref()
    }

    /// Every class whose construction declares a key, with the catalog: `(class, catalog)`.
    pub fn constructed_entries(&self) -> &[(Box<str>, Box<str>)] {
        &self.constructed_entries
    }

    /// Records that the templates whose names match `templates` see the variables of the template
    /// named `from`: a page including them by a name it computes.
    pub fn add_twig_context(&mut self, templates: &str, from: &str) {
        let context = (templates.into(), from.into());
        if !self.twig_contexts.contains(&context) {
            self.twig_contexts.push(context);
        }
    }

    /// Every such statement, as `(templates, from)`.
    pub fn twig_contexts(&self) -> &[(Box<str>, Box<str>)] {
        &self.twig_contexts
    }

    /// Every Twig loader a framework described. Empty for a project nothing renders Twig in.
    pub fn twig_loaders(&self) -> &[crate::configuration::TwigLoader] {
        &self.twig_loaders
    }
}
