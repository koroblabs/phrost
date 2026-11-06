//! Drupal.
//!
//! One module per thing Drupal knows that no generic layer can, because they have nothing to do with
//! each other and a single file would be edited by every capability added:
//!
//! - [`layout`] -- where the installation is, since composer lets a project choose;
//! - [`extensions`] -- the namespaces the kernel registers at runtime, from `*.info.yml`;
//! - [`database`] -- where the database layer's own signatures are wrong;
//! - [`elements`] -- which render element a `#type` key is asking for;
//! - [`entities`] -- which entity type a storage handler was asked for;
//! - [`fields`] -- which fields an entity has, from the configuration that declares them, and what
//!   reading one gives back when it is written as a property rather than as a call;
//! - [`hooks`] -- the one docblock line a hook implementation carries, and what it binds;
//! - [`places`] -- the route names, permission ids and library names declared in YAML;
//! - [`procedural`] -- the PHP core requires by hand, which no autoload rule can name;
//! - [`services`] -- which container ids exist, and how code asks for one.
//!
//! What is left here is the seam itself: one line per capability, so that adding the next one adds a
//! file rather than editing this.

mod annotations;
mod config;
mod configuration;
mod container;
mod database;
mod elements;
mod entities;
mod extensions;
mod field_types;
mod fields;
mod forms;
mod hooks;
mod layout;
mod places;
mod procedural;
mod services;
mod sites;
mod survey;
mod templates;
mod themes;

use std::path::{Path, PathBuf};

use crate::{
    registry::{FrameworkProject, FrameworkProvider},
    rule::{
        ArgumentBinding, ArrayKeyBinding, AttributeKey, AutoloadRule, CatalogEntry, DerivedKey,
        DerivedName, ForwardedArgument, NonNullReturn, PlaceEntry,
    },
};

/// Every literal-argument binding this provider contributes, from all of its modules.
///
/// One slice because the seam takes one, built here rather than in either module so that neither has
/// to know the other exists.
///
/// Written out by index because a `const` slice cannot be concatenated. That spelling fails silently
/// in one direction -- a module gaining a binding this list does not name simply does not contribute
/// it, and nothing else in the project would notice -- which is what
/// [`every_module_binding_reaches_the_seam`](tests::every_module_binding_reaches_the_seam) is for.
static BINDINGS: &[ArgumentBinding] = &[
    services::BINDINGS[0],
    services::BINDINGS[1],
    services::BINDINGS[2],
    places::BINDINGS[0],
    places::BINDINGS[1],
    places::BINDINGS[2],
    places::BINDINGS[3],
    places::BINDINGS[4],
    places::BINDINGS[5],
    hooks::BINDINGS[0],
    hooks::BINDINGS[1],
    hooks::BINDINGS[2],
    hooks::BINDINGS[3],
    fields::BINDINGS[0],
    fields::BINDINGS[1],
    fields::BINDINGS[2],
    config::BINDINGS[0],
    config::BINDINGS[1],
    config::BINDINGS[2],
    config::BINDINGS[3],
    config::BINDINGS[4],
];

/// Every declaration whose null this framework's runtime cannot produce, from all of its modules.
///
/// Written out by index for the reason [`BINDINGS`] is, and guarded by the same test. This one fails
/// silently in the direction that costs attention rather than correctness: a module stating a
/// guarantee that never reaches the seam leaves the reports it was meant to remove exactly where
/// they were, which reads as the rule not working.
static NON_NULL_RETURNS: &[NonNullReturn] = &[
    database::NON_NULL_RETURNS[0],
    database::NON_NULL_RETURNS[1],
    entities::NON_NULL_RETURNS[0],
    entities::NON_NULL_RETURNS[1],
    entities::NON_NULL_RETURNS[2],
    services::NON_NULL_RETURNS[0],
    services::NON_NULL_RETURNS[1],
];

/// Every attribute that declares a catalog key, from all of this provider's modules.
///
/// Written out by index for the reason [`BINDINGS`] is, and guarded by the same test: a module that
/// grows one and is not added here contributes nothing, silently, and the only symptom is a catalog
/// that stays empty.
static ATTRIBUTE_KEYS: &[AttributeKey] = &[
    entities::ATTRIBUTE_KEYS[0],
    entities::ATTRIBUTE_KEYS[1],
    elements::ATTRIBUTE_KEYS[0],
    elements::ATTRIBUTE_KEYS[1],
];

/// Every derived key, from all of this provider's modules. Written out by index like [`BINDINGS`].
static DERIVED_KEYS: &[DerivedKey] = &[entities::DERIVED_KEYS[0], entities::DERIVED_KEYS[1]];

/// Every key carried in a declaration's own name. Same rule as [`BINDINGS`].
static DERIVED_NAMES: &[DerivedName] = &[hooks::DERIVED_NAMES[0]];

/// Every array key that names a catalog entry, from all of this provider's modules.
///
/// Written out by index for the reason [`BINDINGS`] is, and guarded by the same test: a module that
/// grows one and is not added here contributes nothing, silently, and the only symptom is a key that
/// stays unrecognised.
static ARRAY_KEY_BINDINGS: &[ArrayKeyBinding] = &[
    elements::ARRAY_KEYS[0],
    places::ARRAY_KEYS[0],
    themes::ARRAY_KEYS[0],
];

#[derive(Debug)]
pub struct Drupal;

impl FrameworkProvider for Drupal {
    fn id(&self) -> &'static str {
        "drupal"
    }

    fn detect(&self, root: &Path) -> bool {
        layout::docroot_of(root).is_some()
    }

    /// `'::districtCallback'` in `'#ajax' => ['callback' => ...]`, `'#submit' => ['::save']`, and
    /// `'#validate'`. The form builder resolves the leading `::` against the form object, which is
    /// the class the array is written in -- the same target `[$this, 'save']` already reaches.
    fn own_method_prefixes(&self) -> &'static [&'static str] {
        &["::"]
    }

    fn project(&self, root: &Path) -> Box<dyn FrameworkProject> {
        match layout::docroot_of(root) {
            Some(docroot) => Box::new(DrupalProject::read(root, docroot)),
            None => Box::new(()),
        }
    }

    /// Drupal keeps procedural PHP — hooks, most of all — in files with these extensions.
    ///
    /// A project where they are skipped is missing its own hook implementations, which is a large
    /// part of what anyone working on a Drupal site reads.
    fn file_extensions(&self) -> &'static [&'static str] {
        procedural::EXTENSIONS
    }

    /// Everything this provider reads is YAML: `*.info.yml` for the namespaces, `*.services.yml` for
    /// the container, `*.routing.yml` and `*.permissions.yml` for the places, `field.storage.*.yml`
    /// for the field kinds.
    fn configuration_extensions(&self) -> &'static [&'static str] {
        &["yml"]
    }

    /// Every one of those files, since being a suffix discovery understands is not enough to be
    /// found: see [`procedural`] for the two things that hide them from a walk of the project.
    fn non_null_returns(&self) -> &'static [NonNullReturn] {
        NON_NULL_RETURNS
    }

    fn chained_returns(&self) -> &'static [crate::rule::ChainedReturn] {
        database::CHAINED_RETURNS
    }

    fn defaulted_returns(&self) -> &'static [crate::rule::DefaultedReturn] {
        services::DEFAULTED_RETURNS
    }

    fn argument_class_returns(&self) -> &'static [crate::rule::ArgumentClassReturn] {
        services::ARGUMENT_CLASS_RETURNS
    }

    /// `Implements hook_form_alter().` and nothing else is the whole docblock Drupal's standard asks
    /// of a hook implementation: what the parameters mean is documented on the hook, in core, and
    /// not on each of the hundreds of implementations. Measured on one project, a documentation rule
    /// that did not know this reported the standard as a defect 310 times. See [`hooks`] for what
    /// else follows from reading the sentence as the cross-reference it is.
    fn documented_elsewhere(&self) -> &'static [crate::rule::DocumentedBy] {
        hooks::DOCUMENTED_BY
    }

    /// An update hook's docblock is the description Drupal prints beside the pending update, so it
    /// is required and takes prose alone -- see [`hooks`] for where each of the two is read.
    fn user_facing_docblocks(&self) -> &'static [crate::rule::UserFacingDocblock] {
        hooks::USER_FACING_DOCBLOCKS
    }

    /// A controller, form, plugin or service is built by the container and can be handed what it
    /// fetches statically instead -- see [`container`].
    fn container(&self) -> Option<&'static crate::rule::ContainerRule> {
        Some(&container::CONTAINER)
    }

    /// The four module-handler methods that run a hook by name, which is how a Drupal project
    /// declares a hook of its own -- see [`hooks`] for why there is nothing else that would say one
    /// exists.
    fn dispatched_names(&self) -> &'static [crate::rule::DispatchedName] {
        hooks::DISPATCHED_NAMES
    }

    /// Two-space indent, single quotes, no trailing close tag. Drupal's own standard, which the
    /// formatter ships as a preset -- so this is one name rather than thirty settings.
    fn format_style(&self) -> Option<&'static str> {
        Some("drupal")
    }

    /// Both kinds at once, because the seam takes one list and what a catalog holds is not this
    /// method's business.
    fn argument_bindings(&self) -> &'static [ArgumentBinding] {
        BINDINGS
    }

    fn forwarded_arguments(&self) -> &'static [ForwardedArgument] {
        entities::FORWARDED
    }

    fn attribute_keys(&self) -> &'static [AttributeKey] {
        ATTRIBUTE_KEYS
    }

    /// See [`annotations`]: every plugin type's annotation class has an attribute class one
    /// namespace segment along, which is what makes converting one findable rather than tabulated.
    fn annotation_rewrite(&self) -> Option<crate::rule::AnnotationRewrite> {
        Some(annotations::REWRITE)
    }

    /// The two Drupal finds by walking rather than by being told.
    ///
    /// `src/Plugin/` of every extension is where the plugin managers look. `Drupal\Tests\<module>\`
    /// is where the test runner does -- a test class is named by nothing and instantiated by
    /// PHPUnit, which is the same statement about reachability and not a special case for tests:
    /// 142 of one real project's 203 unreferenced classes are its own tests.
    fn discovered_namespaces(&self) -> &'static [&'static str] {
        &["Plugin", "Tests"]
    }

    /// The two a Drupal checkout builds by finding them: a module's service provider, and the
    /// Drush commands beside it. One of each on the reference project, and both were reported as
    /// used by nothing.
    fn discovered_supertypes(&self) -> &'static [&'static str] {
        &[
            "Drupal\\Core\\DependencyInjection\\ServiceProviderBase",
            "Drush\\Commands\\DrushCommands",
        ]
    }

    /// A container written for autowiring keys its services by class -- 48 of them on one real
    /// project, and every one a reference nothing could see.
    fn yaml_class_keys(&self) -> &'static [&'static str] {
        &["services.yml"]
    }

    /// See [`configuration`]: 1 414 references in one real project's own YAML, every one naming
    /// something already indexed, and which key means which table is this provider's to say.
    fn yaml_references(&self) -> &'static [crate::rule::YamlReference] {
        configuration::REFERENCES
    }

    fn array_key_bindings(&self) -> &'static [ArrayKeyBinding] {
        ARRAY_KEY_BINDINGS
    }

    /// See [`entities::DERIVED_KEYS`]: an entity type's link templates each generate a route, and
    /// none of those route names is written anywhere for the YAML reader to find.
    fn derived_keys(&self) -> &'static [DerivedKey] {
        DERIVED_KEYS
    }

    /// See [`hooks::DERIVED_NAMES`]: a hook implementation's name is the contract, and nothing else
    /// in a checkout says the hook exists.
    fn derived_names(&self) -> &'static [DerivedName] {
        DERIVED_NAMES
    }

    /// See [`elements::DOCUMENTED_KEYS`]: `#type` chooses the class, and that class's docblock is
    /// the only place the rest of the array's keys are written down.
    fn documented_keys(&self) -> &'static [crate::rule::DocumentedKeys] {
        elements::DOCUMENTED_KEYS
    }

    fn magic_properties(&self) -> &'static [crate::rule::MagicProperty] {
        fields::MAGIC
    }

    fn tabled_accesses(&self) -> &'static [crate::rule::TabledAccess] {
        fields::TABLED
    }

    fn declared_tables(&self) -> &'static [crate::rule::DeclaredTable] {
        fields::DECLARED
    }

    /// The container, library and entity type ids -- see [`places`] for why the library catalog
    /// earns this and the routes and permissions read the same way do not, and see
    /// [`FrameworkProvider::closed_catalogs`] for what it costs to be wrong about it.
    ///
    /// The entity catalog was absent for two independent reasons, both measured on a real checkout
    /// and both now removed: its key set was 107 of 111, because it read only the attribute spelling
    /// of an entity declaration and not the annotation one contrib still ships, and nothing could
    /// report it in any case. [`entities::CATALOG`] carries both measurements. The element catalog
    /// stays out for a reason neither of those was -- a `#type` key is not always Drupal's, see
    /// [`elements`] -- and the `a_closed_catalog_has_a_reader_that_can_report_it` test below is what
    /// keeps the second half from being skipped by adding a word to this list.
    fn closed_catalogs(&self) -> &'static [&'static str] {
        &[services::CATALOG, places::LIBRARIES, entities::CATALOG]
    }

    fn twig_loaders(&self) -> &'static [crate::rule::TwigLoader] {
        templates::TWIG_LOADERS
    }

    fn render_arrays(&self) -> &'static [crate::rule::RenderArray] {
        themes::RENDER_ARRAYS
    }

    fn built_keys(&self) -> &'static [crate::rule::BuiltKey] {
        forms::BUILT_KEYS
    }
}

/// One Drupal site, read once.
///
/// Holds the one walk of the docroot every answer below is read from. Made by [`Drupal::project`]
/// for a root that has a docroot, and dropped once the contributions are installed.
#[derive(Debug)]
struct DrupalProject {
    root: PathBuf,
    docroot: PathBuf,
    survey: survey::Survey,
}

impl DrupalProject {
    fn read(root: &Path, docroot: PathBuf) -> Self {
        Self {
            root: root.to_path_buf(),
            survey: survey::Survey::of(&docroot),
            docroot,
        }
    }
}

impl FrameworkProject for DrupalProject {
    fn autoload_rules(&self) -> Vec<AutoloadRule> {
        extensions::autoload_rules(&self.docroot, &self.survey.descriptors)
    }

    fn procedural_sources(&self) -> Vec<PathBuf> {
        self.survey.procedural.clone()
    }

    fn excluded_paths(&self) -> Vec<PathBuf> {
        sites::excluded_paths(&self.docroot)
    }

    /// Core, and every extension a package manager installed.
    ///
    /// `contrib` by name because that is where composer's installer paths put them and where every
    /// Drupal project keeps them -- `custom` beside it is the project's own and stays checked.
    fn dependency_paths(&self) -> Vec<PathBuf> {
        let mut found = vec![self.docroot.join("core")];
        for kind in ["modules", "themes", "profiles"] {
            found.push(self.docroot.join(kind).join("contrib"));
        }
        found
    }

    fn catalog_places(&self) -> Vec<PlaceEntry> {
        let descriptors = &self.survey.descriptors;
        let mut found = places::catalog_places(&self.docroot, descriptors);
        // The project root and not the docroot: a Drupal site keeps its exported configuration
        // beside the web root, not inside it.
        found.extend(fields::catalog_places(&self.root, descriptors));
        // Core's own directories and every extension's, which is what the route reader walks too --
        // `item_list`, `table` and `container` are declared by core and written in every other
        // render array.
        //
        // Core *last*, which is the opposite order from the route reader above, and for a reason
        // rather than by accident: a later entry wins, a module redefining a core route really does
        // replace it, and a theme shipping its own `table.html.twig` does not redefine `table` -- it
        // overrides how the active theme draws it. Pointing a reader at whichever theme happens to be
        // installed would answer a question about rendering with a claim about declaration.
        //
        // The templates are the ones the docroot walk found, read twice: for the hooks the files
        // implement and for the names Twig loads them by.
        let extensions = templates::Extension::all(descriptors);
        let files = templates::template_files(&self.docroot, &extensions, &self.survey.templates);
        found.extend(themes::catalog_places(&extensions, &files));
        found.extend(templates::catalog_places(
            &self.docroot,
            &extensions,
            &files,
            &self.survey.templates,
        ));
        found.extend(config::catalog_places(
            &self.root,
            &places::core_directories(&self.docroot),
            descriptors,
        ));
        found
    }

    fn catalog_entries(&self) -> Vec<CatalogEntry> {
        let descriptors = &self.survey.descriptors;
        let mut found = services::catalog_entries(&self.docroot, descriptors);
        found.extend(hooks::catalog_entries(descriptors));
        found
    }

    fn property_tables(&self) -> Vec<crate::rule::PropertyTable> {
        // The project root, for the reason `catalog_places` gives: a Drupal site keeps its exported
        // configuration beside the web root rather than inside it.
        fields::property_tables(&self.root, &self.survey.descriptors)
    }

    fn catalog_kinds(&self) -> Vec<crate::rule::CatalogKind> {
        fields::catalog_kinds(&self.root, &self.survey.descriptors)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ARRAY_KEY_BINDINGS, ATTRIBUTE_KEYS, BINDINGS, NON_NULL_RETURNS, config, database, elements,
        entities, fields, hooks, places, services, themes,
    };

    /// Every binding a module declares is one the seam actually sees.
    ///
    /// [`BINDINGS`] names each entry by index, so a module that grows one and does not extend that
    /// list contributes nothing new -- no error, no warning, and the only symptom is a feature that
    /// quietly does not work on the call it was written for. Counting is enough to catch it: an index
    /// written twice or left out both change the total.
    #[test]
    fn every_module_binding_reaches_the_seam() {
        let declared = services::BINDINGS.len()
            + places::BINDINGS.len()
            + hooks::BINDINGS.len()
            + fields::BINDINGS.len()
            + config::BINDINGS.len();
        assert_eq!(
            BINDINGS.len(),
            declared,
            "a module's binding list changed and `BINDINGS` was not extended to match -- add the \
             missing `<module>::BINDINGS[n]` line"
        );

        for binding in services::BINDINGS
            .iter()
            .chain(places::BINDINGS)
            .chain(hooks::BINDINGS)
            .chain(fields::BINDINGS)
            .chain(config::BINDINGS)
        {
            assert!(
                BINDINGS.contains(binding),
                "{}::{} is declared and never reaches the seam",
                binding.class,
                binding.method
            );
        }
    }

    /// The same, for the declarations whose null the runtime cannot produce.
    ///
    /// A module stating one and not extending [`NON_NULL_RETURNS`] contributes nothing, and the only
    /// symptom is the reports it was written to remove still being there.
    #[test]
    fn every_module_non_null_return_reaches_the_seam() {
        let declared = database::NON_NULL_RETURNS.len()
            + entities::NON_NULL_RETURNS.len()
            + services::NON_NULL_RETURNS.len();
        assert_eq!(
            NON_NULL_RETURNS.len(),
            declared,
            "a module's non-null list changed and `NON_NULL_RETURNS` was not extended to match -- \
             add the missing `<module>::NON_NULL_RETURNS[n]` line"
        );

        for contract in database::NON_NULL_RETURNS
            .iter()
            .chain(entities::NON_NULL_RETURNS)
            .chain(services::NON_NULL_RETURNS)
        {
            assert!(
                NON_NULL_RETURNS.contains(contract),
                "{}::{} is declared and never reaches the seam",
                contract.class,
                contract.method
            );
        }
    }

    /// The same, for the attributes that declare a catalog key.
    ///
    /// A module gaining one and not extending [`ATTRIBUTE_KEYS`] leaves its catalog empty, and an
    /// empty catalog reads exactly like a feature nobody built.
    #[test]
    fn every_module_attribute_key_reaches_the_seam() {
        let declared = entities::ATTRIBUTE_KEYS.len() + elements::ATTRIBUTE_KEYS.len();
        assert_eq!(
            ATTRIBUTE_KEYS.len(),
            declared,
            "a module's attribute-key list changed and `ATTRIBUTE_KEYS` was not extended to match \
             -- add the missing `<module>::ATTRIBUTE_KEYS[n]` line"
        );

        for key in entities::ATTRIBUTE_KEYS
            .iter()
            .chain(elements::ATTRIBUTE_KEYS)
        {
            assert!(
                ATTRIBUTE_KEYS.contains(key),
                "{} is declared and never reaches the seam",
                key.attribute
            );
        }
    }

    /// The same, for the array keys that name a catalog entry.
    ///
    /// A module gaining one and not extending [`ARRAY_KEY_BINDINGS`] leaves its catalog unreachable
    /// from an array literal, and the only symptom is a key that goto and hover never recognise.
    #[test]
    fn every_module_array_key_reaches_the_seam() {
        let declared =
            elements::ARRAY_KEYS.len() + places::ARRAY_KEYS.len() + themes::ARRAY_KEYS.len();
        assert_eq!(
            ARRAY_KEY_BINDINGS.len(),
            declared,
            "a module's array-key list changed and `ARRAY_KEY_BINDINGS` was not extended to match \
             -- add the missing `<module>::ARRAY_KEYS[n]` line"
        );

        for binding in elements::ARRAY_KEYS
            .iter()
            .chain(places::ARRAY_KEYS)
            .chain(themes::ARRAY_KEYS)
        {
            assert!(
                ARRAY_KEY_BINDINGS.contains(binding),
                "{} is declared and never reaches the seam",
                binding.key
            );
        }
    }

    /// Whether anything this provider contributes can report a key `catalog` does not hold.
    ///
    /// The three shapes the whole-file projection of catalog uses walks, and nothing else: the
    /// argument a call was given, the value under an array key, and -- since the entity catalog
    /// needed it -- the argument to the call named by a
    /// [`ForwardedArgument`](crate::rule::ForwardedArgument), where the id is written one call before
    /// the method that consumes it. A catalog reached some other way, or filled and never read at
    /// all, has no reader here.
    fn has_a_reader(catalog: &str) -> bool {
        use crate::registry::FrameworkProvider;

        let provider = super::Drupal;
        provider
            .argument_bindings()
            .iter()
            .any(|binding| binding.catalog == catalog)
            || provider
                .array_key_bindings()
                .iter()
                .any(|binding| binding.catalog == catalog)
            || provider
                .forwarded_arguments()
                .iter()
                .any(|binding| binding.catalog == catalog)
    }

    /// Closing a catalog only means something if something can report a key it does not hold.
    ///
    /// This is a silent trap in the expensive direction: adding a word to [`Drupal::closed_catalogs`]
    /// reads as switching a diagnostic on, compiles, passes every other test, and changes not one
    /// finding on a real project. Measured, on the entity catalog before it had a reader: closing it
    /// moved nothing. The honest way to close a catalog is to give it one first.
    #[test]
    fn a_closed_catalog_has_a_reader_that_can_report_it() {
        use crate::registry::FrameworkProvider;

        for catalog in super::Drupal.closed_catalogs() {
            assert!(
                has_a_reader(catalog),
                "the `{catalog}` catalog is closed and nothing can report a key it does not hold -- \
                 a closed catalog is read through an argument binding, an array key, or a forwarded \
                 argument, so this sets a flag with no reader. Give it one of those before closing \
                 it."
            );
        }
    }

    /// And the guard still bites, which the test above cannot show on its own.
    ///
    /// Every catalog named today has a reader, so that test passes whether or not the predicate is
    /// still capable of failing -- a widening that accidentally answered `true` for everything would
    /// look exactly like a green run. A name no rule mentions is the case it has to refuse.
    #[test]
    fn a_catalog_nothing_reads_has_no_reader() {
        assert!(!has_a_reader("nothing_names_this"));
    }
}
