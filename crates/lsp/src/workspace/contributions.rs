//! Installing what one framework provider contributes.
//!
//! Its own file for the reason the project's own rules give: a new kind of contribution is a new
//! `for … in provider.x()` block, and if it lives beside the bootstrap sequence then every feature
//! that teaches a provider something edits the same two hundred lines as every other. This session
//! alone added four -- derived keys, derived names, an argument position, a documented-keys rule --
//! and each one touched that block.
//!
//! What stays in [`Workspace::set_root`](super::Workspace::set_root) is the *order*, which is
//! load-bearing and is documented there: the contributions come after composer and before the source
//! roots, and the project's own configuration comes last so that what a project says overrides what a
//! framework assumed.
//!
//! One method rather than one per kind, because the kinds share nothing but the provider they came
//! from -- and a hundred one-line methods is the same file with more ceremony.

use std::path::{Path, PathBuf};

use phrost_model::Interner;
use phrost_syntax_features::format::FormatStyle;

use super::Workspace;

impl Workspace {
    /// Everything one provider says about the project, installed in the order it has to be.
    ///
    /// `keyed` and `entered` are collected across every provider and installed once by the caller,
    /// because they have to be in place before the first file is lowered: a declaration lowered
    /// without them carries no literal, and nothing later goes back to look again.
    pub(super) fn install(
        &mut self,
        provider: &'static dyn phrost_framework::FrameworkProvider,
        root: &Path,
        interner: &mut Interner,
        keyed: &mut Vec<&'static str>,
        entered: &mut Vec<&'static str>,
        excluded: &mut Vec<PathBuf>,
    ) {
        self.frameworks.push(provider.id());
        // One reading of the project for every question below that is answered from its files.
        let project = provider.project(root);
        for rule in project.autoload_rules() {
            self.autoload.add_psr4(interner, &rule.prefix, [rule.dir]);
        }
        for extension in provider.file_extensions() {
            self.contributed_extensions.push((*extension).to_owned());
        }
        // Not indexed, unlike the above -- watched. What this provider just contributed was read out
        // of files with these extensions, so a change to one is a reason to ask it again.
        for extension in provider.configuration_extensions() {
            self.contributed_config_extensions
                .push((*extension).to_owned());
        }
        // The extension alone is not enough to find one: the files it names sit outside every
        // autoload rule and are usually gitignored, so discovery has to be pointed at them.
        self.framework_sources.extend(project.procedural_sources());
        excluded.extend(project.excluded_paths());
        // Indexed in full and not checked -- see `FrameworkProject::dependency_paths`.
        self.dependency_paths.extend(project.dependency_paths());
        // The last framework to name one wins, which is the same rule as everything else here.
        // Two frameworks with different standards in one project is a question nobody has, and a
        // deterministic answer beats refusing to format.
        if let Some(style) = provider.format_style() {
            // The same name serves two readers: the formatter, which reprints the file, and the
            // docblock generator, which has to lay a comment out the way this project writes one.
            if let Some(known) = FormatStyle::named(style) {
                self.format_style = known;
            }
            self.skeleton.set_house_style(Some(style));
        }
        // The last framework to name one wins, as with the format style above.
        if let Some(rewrite) = provider.annotation_rewrite() {
            self.skeleton
                .returns_mut()
                .rewrite_annotations_in(rewrite.attributes_in);
            for value in rewrite.values {
                self.skeleton
                    .returns_mut()
                    .construct_for_annotation(value.annotation, value.constructs);
            }
        }
        // What a framework finds by scanning, which nothing names and so nothing can reference.
        let discovered: Vec<Box<str>> = provider
            .discovered_namespaces()
            .iter()
            .map(|segment| (*segment).into())
            .collect();
        if !discovered.is_empty() {
            self.skeleton.set_discovered_namespaces(discovered);
        }
        let inherited: Vec<Box<str>> = provider
            .discovered_supertypes()
            .iter()
            .map(|name| (*name).into())
            .collect();
        if !inherited.is_empty() {
            self.skeleton.set_discovered_supertypes(inherited);
        }
        // The other thing a provider can say with a base class: that it runs the subclass and
        // catches what it throws. See `runs_and_catches` for why the two lists are apart.
        let run: Vec<Box<str>> = provider
            .runs_and_catches()
            .iter()
            .map(|name| (*name).into())
            .collect();
        self.skeleton.add_run_supertypes(run);
        // And the third: classes that answer names no declaration holds. See `dynamic_members`.
        self.skeleton.add_dynamic_supertypes(
            provider
                .dynamic_members()
                .iter()
                .map(|name| (*name).into())
                .collect::<Vec<Box<str>>>(),
        );
        // And the spelling that makes a string name a method of the class it sits in.
        self.skeleton.add_own_method_prefixes(
            provider
                .own_method_prefixes()
                .iter()
                .map(|prefix| (*prefix).into())
                .collect::<Vec<Box<str>>>(),
        );
        // And the calls on a test double whose string names a method of what it stands in for.
        self.skeleton
            .add_stand_in_methods(provider.stand_in_methods().iter().map(|rule| {
                phrost_index::StandInMethod {
                    stand_in: rule.stand_in.into(),
                    method: rule.method.into(),
                    at: rule.at,
                    through: rule.through.iter().map(|call| (*call).into()).collect(),
                }
            }));
        // And the calls whose string names the key of an element the enclosing class builds.
        self.skeleton
            .add_built_keys(
                provider
                    .built_keys()
                    .iter()
                    .map(|rule| phrost_index::BuiltKey {
                        class: rule.class.into(),
                        method: rule.method.into(),
                        property: rule.property.into(),
                        described_by: rule.described_by.into(),
                        filed_under: rule.filed_under.into(),
                    }),
            );
        // And the files where the key itself is the class, which no value-matching rule can say.
        for in_files in provider.yaml_class_keys() {
            self.skeleton.returns_mut().class_keys_in_config(in_files);
        }
        // Which key of which configuration file names what. Nothing is indexed by this -- the
        // catalogs it points at are filled elsewhere -- so it can be installed at any point.
        for reference in provider.yaml_references() {
            self.skeleton.returns_mut().reference_in_config(
                reference.in_files,
                reference.under,
                reference.prefix,
                reference.optional,
                match reference.names {
                    phrost_framework::YamlTarget::CatalogKey(catalog) => {
                        phrost_index::ConfigTarget::CatalogKey(catalog.into())
                    }
                    phrost_framework::YamlTarget::Class => phrost_index::ConfigTarget::Class,
                    phrost_framework::YamlTarget::Callable => phrost_index::ConfigTarget::Callable,
                    phrost_framework::YamlTarget::Factory => phrost_index::ConfigTarget::Factory,
                    phrost_framework::YamlTarget::CatalogKeys {
                        catalog,
                        separators,
                    } => phrost_index::ConfigTarget::CatalogKeys {
                        catalog: catalog.into(),
                        separators: separators.into(),
                    },
                },
            );
        }
        for contract in provider.non_null_returns() {
            self.skeleton.returns_mut().add_unproduced(
                contract.class,
                contract.method,
                phrost_model::BuiltinTy::Null,
                match &contract.null {
                    phrost_framework::NullFact::Unreachable => None,
                    phrost_framework::NullFact::OnlyWhen(state)
                    | phrost_framework::NullFact::OnlyWithArgument { prose: state, .. } => {
                        Some(state)
                    }
                },
                match &contract.null {
                    phrost_framework::NullFact::OnlyWithArgument { at, .. } => Some(*at),
                    _ => None,
                },
            );
        }
        for contract in provider.chained_returns() {
            self.skeleton.returns_mut().add_chained(
                contract.class,
                contract.method,
                contract.after,
                contract.then,
                contract.otherwise,
            );
        }
        for contract in provider.chained_operands() {
            for method in contract.methods {
                self.skeleton.returns_mut().add_chained_operand(
                    method,
                    phrost_index::ChainedOperandRule {
                        class: contract.class.into(),
                        after: contract.after.iter().map(|name| (*name).into()).collect(),
                        counted_by: contract
                            .counted_by
                            .iter()
                            .map(|name| (*name).into())
                            .collect(),
                        many: contract.many.into(),
                    },
                );
            }
        }
        for contract in provider.defaulted_returns() {
            self.skeleton.returns_mut().add_defaulted(
                contract.class,
                contract.method,
                contract.unless_given,
                contract.then,
            );
        }
        for contract in provider.argument_class_returns() {
            self.skeleton.returns_mut().add_argument_class(
                contract.class,
                contract.method,
                contract.argument,
            );
        }
        for invoked in provider.invoked_callbacks() {
            self.skeleton.returns_mut().add_invoked_callback(
                invoked.class,
                invoked.method,
                invoked.argument,
                invoked.parameters,
            );
        }
        for convention in provider.documented_elsewhere() {
            self.skeleton.returns_mut().add_documented_elsewhere(
                convention.lead,
                convention.names,
                convention.implementers,
                convention.dispatched,
            );
        }
        for published in provider.user_facing_docblocks() {
            self.skeleton
                .returns_mut()
                .add_user_facing_docblock(published.names, published.numbered);
        }
        if let Some(container) = provider.container() {
            let returns = self.skeleton.returns_mut();
            for interface in container.built_through {
                returns.add_container_interface(interface);
            }
            for catalog in container.built_from {
                returns.add_container_catalog(catalog);
            }
            for reach in container.reaches {
                returns.add_container_reach(
                    reach.class,
                    reach.descendants,
                    reach.methods,
                    reach.instead,
                );
            }
        }
        // Before any file is ingested, like the closed catalogs below and for the same reason:
        // a file lowered without the rules dispatches nothing, so re-ingesting it later is what
        // it would take to notice. Installed on the skeleton alone is enough here, unlike the
        // keyed attributes -- an open buffer's HIR is handed back to `ingest_lowered`, which
        // reads these from the same table a closed ingest does.
        for dispatch in provider.dispatched_names() {
            self.skeleton.returns_mut().add_dispatcher(
                dispatch.method,
                phrost_index::DispatchRule {
                    class: dispatch.class.into(),
                    argument: dispatch.argument,
                    several: dispatch.several,
                    names: dispatch.names.into(),
                    suffix: dispatch.suffix.into(),
                    catalog: dispatch.catalog.into(),
                },
            );
        }
        for binding in provider.argument_bindings() {
            self.skeleton.returns_mut().bind_argument(
                binding.class,
                binding.method,
                binding.at,
                binding.suffix,
                binding.catalog,
            );
        }
        for dialect in provider.template_dialects() {
            if let Some(function) = dialect.includes {
                self.skeleton
                    .returns_mut()
                    .bind_rendered_template(function, 0, 1, true);
            }
            for tag in dialect.components {
                self.skeleton
                    .returns_mut()
                    .bind_component_call(tag.function, tag.prefix);
            }
            self.skeleton
                .add_template_dialect(phrost_index::TemplateDialect {
                    suffix: dialect.suffix.into(),
                    to_php: dialect.to_php,
                    directory: dialect.directory.into(),
                    separator: dialect.separator.into(),
                    shared: dialect
                        .shared
                        .iter()
                        .map(|&(name, class)| (name.into(), class.into()))
                        .collect(),
                    components: dialect
                        .components
                        .iter()
                        .map(|tag| {
                            (
                                tag.prefix.into(),
                                tag.shared
                                    .iter()
                                    .map(|&(name, class)| (name.into(), class.into()))
                                    .collect(),
                            )
                        })
                        .collect(),
                    calls: dialect
                        .components
                        .iter()
                        .map(|tag| tag.function)
                        .chain(dialect.includes)
                        .map(Into::into)
                        .collect(),
                    problems: dialect.problems,
                });
        }
        for shared in provider.shared_template_data() {
            self.skeleton.returns_mut().bind_shared_template_data(
                shared.class,
                shared.method,
                shared.helper,
            );
        }
        for rendered in provider.rendered_templates() {
            self.skeleton.returns_mut().bind_rendered_template(
                rendered.function,
                rendered.name_at,
                rendered.data_at,
                false,
            );
        }
        for configured in provider.configured_closure_this() {
            self.skeleton
                .returns_mut()
                .bind_configured_this(phrost_index::ConfiguredThisRule {
                    declared: configured.declared.into(),
                    functions: configured
                        .functions
                        .iter()
                        .map(|&name| name.into())
                        .collect(),
                    methods: configured.methods.iter().map(|&name| name.into()).collect(),
                    within: configured.within.into(),
                });
        }
        for rendering in provider.rendering_classes() {
            self.skeleton.returns_mut().bind_rendering_class(
                rendering.class,
                rendering.namespace,
                rendering.templates,
            );
        }
        for unlisted in provider.unlisted_keys() {
            self.skeleton.returns_mut().bind_unlisted_key(
                unlisted.catalog,
                unlisted.holds,
                unlisted.outside,
            );
        }
        for binding in provider.function_bindings() {
            self.skeleton.returns_mut().bind_function_argument(
                binding.function,
                binding.at,
                binding.catalog,
            );
        }
        for prefixed in provider.prefixed_methods() {
            self.skeleton.returns_mut().bind_prefixed_method(
                prefixed.class,
                prefixed.prefix,
                prefixed.builder,
                prefixed.marker.is_some(),
            );
            // Keyed so lowering marks the methods carrying it -- see `KeyedAttributes::marks`.
            keyed.extend(prefixed.marker);
        }
        for backed in provider.method_backed_properties() {
            self.skeleton.returns_mut().bind_method_property(
                backed.class,
                backed.returns,
                backed.through,
                backed.counted,
                &backed
                    .keyed
                    .iter()
                    .map(|keyed| (keyed.relation, keyed.suffix))
                    .collect::<Vec<_>>(),
            );
        }
        for inputs in provider.declared_inputs() {
            self.skeleton
                .returns_mut()
                .bind_declared_inputs(phrost_index::DeclaredInputsRule {
                    class: inputs.class.into(),
                    property: inputs.property.into(),
                    readers: inputs
                        .readers
                        .iter()
                        .map(|(method, prefix)| ((*method).into(), (*prefix).into()))
                        .collect(),
                    parse: inputs.parse,
                    always: inputs.always.iter().map(|name| (*name).into()).collect(),
                    unless: inputs.unless.iter().map(|name| (*name).into()).collect(),
                });
        }
        for named in provider.named_template_arguments() {
            self.skeleton
                .returns_mut()
                .bind_named_argument(phrost_index::NamedArgumentRule {
                    class: named.class.into(),
                    template: named.template.into(),
                    from_property: named.from_property.map(Into::into),
                    unless: named.unless.iter().map(|name| (*name).into()).collect(),
                    app_directory: named.app_directory.into(),
                    default_namespace: named.default_namespace.into(),
                    names: named.names,
                });
        }
        for named in provider.named_after_caller() {
            self.skeleton
                .returns_mut()
                .bind_named_after(phrost_index::NamedAfterRule {
                    class: named.class.into(),
                    method: named.method.into(),
                    unless: named.unless.iter().map(|name| (*name).into()).collect(),
                    app_directory: named.app_directory.into(),
                    default_namespace: named.default_namespace.into(),
                    name: named.name,
                });
        }
        for schema in provider.written_schema() {
            let names = |names: &[&str]| names.iter().map(|name| (*name).into()).collect();
            self.skeleton.returns_mut().bind_written_schema(
                phrost_index::schema::WrittenSchemaRule {
                    facade: schema.facade.into(),
                    create: names(schema.create),
                    alter: names(schema.alter),
                    drop: names(schema.drop),
                    rename: names(schema.rename),
                    runs: schema.runs.into(),
                    directory: schema.directory.into(),
                    columns: schema.columns,
                    model: schema.model.into(),
                    table_property: schema.table_property.into(),
                    default_table: schema.default_table,
                },
            );
        }
        for stated in provider.stated_properties() {
            let kind = |held: &phrost_framework::StatedType| match held {
                phrost_framework::StatedType::Builtin(builtin) => {
                    phrost_index::StatedKind::Builtin(*builtin)
                }
                phrost_framework::StatedType::Class(class) => {
                    phrost_index::StatedKind::Class((*class).into())
                }
            };
            let pairs = |pairs: &[(&str, phrost_framework::StatedType)]| {
                pairs
                    .iter()
                    .map(|(name, held)| ((*name).into(), kind(held)))
                    .collect::<Vec<_>>()
            };
            let names = |names: &[&str]| names.iter().map(|name| (*name).into()).collect();
            self.skeleton.returns_mut().bind_stated_properties(
                phrost_index::StatedPropertiesRule {
                    class: stated.class.into(),
                    typed: names(stated.typed),
                    listed: names(stated.listed),
                    listed_by_attribute: names(stated.listed_by_attribute),
                    spellings: pairs(stated.spellings),
                    wrapping: names(stated.wrapping),
                    classes: pairs(stated.classes),
                    custom: stated
                        .custom
                        .map(|(interface, method)| (interface.into(), method.into())),
                    implicit: pairs(stated.implicit),
                    key: stated.key.map(|key| phrost_index::StatedKeyRule {
                        name: key.name.into(),
                        kind: kind(&key.ty),
                        unless: names(key.unless),
                    }),
                    nullable: stated.nullable,
                },
            );
        }
        for registrar in provider.macro_registrars() {
            self.skeleton
                .returns_mut()
                .bind_macro_registrar(registrar.provider, registrar.method);
        }
        for filled in provider.filled_returns() {
            self.skeleton.returns_mut().bind_filled_return(
                filled.method,
                phrost_index::FilledReturnRule {
                    class: filled.class.into(),
                    arguments: filled
                        .arguments
                        .iter()
                        .map(|argument| match argument {
                            phrost_framework::FilledArgument::Builtin(builtin) => {
                                phrost_index::FilledArgumentRule::Builtin(*builtin)
                            }
                            phrost_framework::FilledArgument::Template(name) => {
                                phrost_index::FilledArgumentRule::Template((*name).into())
                            }
                        })
                        .collect(),
                },
            );
        }
        for keyed in provider.keyed_returns() {
            self.skeleton.returns_mut().bind_keyed_return(
                keyed.method,
                phrost_index::KeyedReturnRule {
                    class: keyed.class.into(),
                    catalog: keyed.catalog.into(),
                    key: keyed.key.into(),
                    nullable: keyed.nullable,
                },
            );
        }
        for fluent in provider.fluent_prefixes() {
            self.skeleton
                .returns_mut()
                .bind_fluent_prefix(fluent.class, fluent.prefix);
        }
        for accessor in provider.accessor_properties() {
            self.skeleton.returns_mut().bind_accessor(
                accessor.class,
                accessor.prefix,
                accessor.suffix,
                accessor.attribute,
            );
        }
        for implied in provider.implied_mixins() {
            self.skeleton
                .add_implied_mixin(implied.class, implied.mixin, implied.with_static);
        }
        for mixin in provider.mixins_returned_by() {
            self.skeleton.returns_mut().add_mixin_method(mixin.method);
        }
        for entry in provider.constructed_entries() {
            self.skeleton.returns_mut().bind_constructed_entry(
                entry.method,
                entry.class,
                entry.catalog,
            );
        }
        if let Some(callables) = provider.twig_callables() {
            self.skeleton.returns_mut().set_twig_callables(
                callables.filters,
                callables.functions,
                callables.tests,
            );
        }
        for tagged in provider.methods_tagged_from() {
            self.skeleton.add_tag_source(tagged.class, tagged.source);
        }
        for opened in provider.opened_by_call() {
            self.skeleton
                .returns_mut()
                .open_methods(opened.class, opened.methods);
        }
        for magic in provider.magic_properties() {
            self.skeleton.returns_mut().bind_property(
                magic.class,
                match magic.name {
                    phrost_framework::MagicName::Exactly(name) => {
                        phrost_index::PropertyNames::Exactly(name.into())
                    }
                    phrost_framework::MagicName::KeyOf(catalog) => {
                        phrost_index::PropertyNames::KeyOf(catalog.into())
                    }
                },
                magic.holds,
                magic.nullable,
            );
        }
        for tabled in provider.tabled_accesses() {
            self.skeleton.returns_mut().bind_table(
                tabled.class,
                tabled.through,
                tabled.catalog,
                tabled.rows,
            );
        }
        for declared in provider.declared_tables() {
            // Into the keyed set as well, because the kind comes from the attribute's literal and
            // lowering keeps one only for a name in that set -- the half a rule added to one list
            // and not the other silently finds nothing.
            keyed.push(declared.attribute);
            self.skeleton
                .returns_mut()
                .add_declared_table(phrost_index::DeclaredTableRule {
                    attribute: declared.attribute.into(),
                    argument: match declared.argument {
                        phrost_framework::AttributeArgument::Named(name) => {
                            phrost_index::AttributeArgument::Named(interner.name(name))
                        }
                        phrost_framework::AttributeArgument::At(at) => {
                            phrost_index::AttributeArgument::At(at)
                        }
                    },
                    method: declared.method.into(),
                    builders: declared.builders.iter().map(|&it| it.into()).collect(),
                    kinds: declared
                        .kinds
                        .iter()
                        .map(|(name, holds)| ((*name).into(), *holds))
                        .collect(),
                    catalog: declared.catalog.into(),
                });
        }
        // Before any file is ingested, which is what makes the claims a file makes and the
        // claims it withdraws the same set.
        for catalog in provider.closed_catalogs() {
            self.skeleton.returns_mut().close_catalog(catalog);
        }
        for loader in provider.twig_loaders() {
            self.skeleton
                .returns_mut()
                .add_twig_loader(phrost_index::TwigLoader {
                    templates: loader.templates.into(),
                    aliases: loader.aliases.into(),
                    namespaces: loader.namespaces.into(),
                    unwalked: loader.unwalked.iter().map(|&name| name.into()).collect(),
                    globals: loader
                        .globals
                        .iter()
                        .map(|&(name, held)| (name.into(), held.into()))
                        .collect(),
                });
        }
        for array in provider.render_arrays() {
            self.skeleton
                .returns_mut()
                .bind_render_array(phrost_index::RenderArrayRule {
                    key: array.key.into(),
                    catalog: array.catalog.into(),
                    prefix: array.prefix.into(),
                    own: array.own.iter().map(|&own| own.into()).collect(),
                    file_name: array
                        .file_name
                        .map(|named| (named.suffix.into(), named.written, named.named)),
                });
        }
        // The array arguments a derived key reads the entries of. Into the same set the
        // attribute names go into, so a provider that adds a rule cannot leave lowering blind to
        // the argument it names -- see `phrost_index::KeyedAttributes`.
        for derived in provider.derived_keys() {
            keyed.push(derived.attribute);
            entered.push(derived.keys_of);
            self.skeleton.returns_mut().derive_keys_with(
                derived.attribute,
                derived.keys_of,
                derived.named_after,
                derived.spelling,
                derived.rewrite,
                derived.catalog,
            );
        }
        for derived in provider.derived_names() {
            self.skeleton.returns_mut().derive_names_with(
                derived.prefixed_by,
                derived.or_prefix,
                derived.catalog,
            );
        }
        for key in provider.attribute_keys() {
            match key.argument {
                phrost_framework::AttributeArgument::Named(argument) => {
                    self.skeleton.returns_mut().name_keys_with(
                        key.attribute,
                        argument,
                        key.catalog,
                    );
                }
                phrost_framework::AttributeArgument::At(position) => {
                    self.skeleton.returns_mut().position_keys_with(
                        key.attribute,
                        position,
                        key.catalog,
                    );
                }
            }
            // Every rule and not only the positional ones, because lowering goes out of its way
            // for a named attribute too: the older annotation spelling of the same declaration
            // is read only for a name in this list. Derived from the rules rather than written
            // twice, so a provider that adds a rule and forgets the list is not a state that
            // can exist.
            keyed.push(key.attribute);
        }
        for binding in provider.array_key_bindings() {
            self.skeleton
                .returns_mut()
                .bind_array_key(binding.key, binding.catalog);
        }
        // The second half of an array-key binding: which of them names a class that also
        // documents its neighbours. See `elements::DOCUMENTED_KEYS` for why not every binding
        // does.
        for rule in provider.documented_keys() {
            self.skeleton.returns_mut().document_keys_under(
                rule.discriminator,
                rule.catalog,
                rule.prefix,
            );
        }
        for forwarded in provider.forwarded_arguments() {
            self.skeleton.returns_mut().forward_argument(
                forwarded.class,
                forwarded.method,
                forwarded.through,
                forwarded.catalog,
                match forwarded.yields {
                    phrost_framework::Forwarded::Instance => phrost_index::Forwarded::Instance,
                    phrost_framework::Forwarded::ManyOf => phrost_index::Forwarded::ManyOf,
                },
            );
        }
        // Keys that name a place rather than a class -- a route, a permission. Installed before
        // the class-valued ones only because both go into the same table and neither collides:
        // no catalog holds both kinds.
        for entry in project.catalog_places() {
            if !self.is_a_dependency(&entry.path) {
                self.skeleton
                    .returns_mut()
                    .mark_own(entry.catalog, &entry.key);
            }
            self.skeleton.returns_mut().catalog_place(
                entry.catalog,
                &entry.key,
                phrost_index::Place {
                    path: std::sync::Arc::from(entry.path.as_path()),
                    line: entry.line,
                    character: entry.character,
                    detail: entry.detail.into(),
                },
            );
        }
        // In the order the provider gives them, because for a container that order is the
        // answer: a module redefining a core service id replaces it, and the last one read wins.
        for table in project.property_tables() {
            self.skeleton.returns_mut().add_property_table(
                table.catalog,
                &table.kind,
                table
                    .properties
                    .into_iter()
                    .map(|(name, holds)| (name.into_boxed_str(), holds))
                    .collect(),
                table.complete,
            );
        }
        for kind in project.catalog_kinds() {
            self.skeleton
                .returns_mut()
                .add_catalog_kind(kind.catalog, &kind.key, &kind.kind);
        }
        // After the tables and not per table: interning walks all of them, and the ids it makes
        // index into the type table as it stands now. `ProjectSkeleton::adopt` does it again when
        // it replaces that table.
        self.skeleton.intern_property_shapes();
        for alias in project.class_aliases() {
            self.skeleton.add_class_alias(&alias.alias, &alias.class);
        }
        for entry in project.catalog_entries() {
            // Which keys are the project's own is a question about paths, and the workspace is what
            // knows which of them its dependencies live in.
            if entry
                .declared_in
                .as_ref()
                .is_some_and(|path| !self.is_a_dependency(path))
            {
                self.skeleton
                    .returns_mut()
                    .mark_own(entry.catalog, &entry.key);
            }
            match &entry.names {
                Some(phrost_framework::CatalogName::Shape(shape)) => self
                    .skeleton
                    .returns_mut()
                    .catalog_shape(entry.catalog, &entry.key, *shape),
                names => self.skeleton.returns_mut().catalog_entry(
                    entry.catalog,
                    &entry.key,
                    match names {
                        Some(phrost_framework::CatalogName::Class(class)) => Some(class.as_str()),
                        _ => None,
                    },
                ),
            }
        }
    }
}
