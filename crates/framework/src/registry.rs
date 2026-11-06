use std::path::{Path, PathBuf};

use crate::{
    providers,
    rule::{
        AnnotationRewrite, ArgumentBinding, ArgumentClassReturn, ArrayKeyBinding, AttributeKey,
        AutoloadRule, CatalogEntry, CatalogKind, ChainedReturn, ContainerRule, DeclaredTable,
        DefaultedReturn, DerivedKey, DerivedName, DispatchedName, DocumentedBy, DocumentedKeys,
        ForwardedArgument, FunctionBinding, InvokedCallback, MagicProperty, MethodBackedProperty,
        NonNullReturn, PlaceEntry, PrefixedMethod, PropertyTable, TabledAccess, UserFacingDocblock,
        YamlReference,
    },
};

/// What one framework contributes.
///
/// Every method but [`Self::id`] and [`Self::detect`] has a default, so adding a capability to the
/// seam does not touch any existing provider — and adding a provider does not touch the generic
/// layer. That is the whole point: a new framework is a new directory and one line in [`PROVIDERS`].
pub trait FrameworkProvider: Sync {
    /// Stable identifier, for logs and configuration. Not a user-facing name.
    fn id(&self) -> &'static str;

    /// Whether this framework is present in the project.
    ///
    /// Probes the filesystem rather than reading configuration, because a framework's presence is
    /// not always something composer records — and because a wrong answer here is cheap: a provider
    /// that declares nothing costs nothing.
    fn detect(&self, root: &Path) -> bool;

    /// What this framework knows about one project, read from its files.
    ///
    /// One object per root rather than one method per question, because the questions share their
    /// evidence: which modules are installed decides the namespaces to register, the files to index
    /// by hand and the catalogs to fill, and all of it is read off the same directory tree. Asked
    /// separately, each answer walked that tree again -- three full walks of a Drupal docroot before
    /// discovery took its own, 0.4 s of every start on a real checkout. The project is made once by
    /// whoever installs the contributions and answers every question from one look.
    ///
    /// The default knows nothing, so a provider that reads no files says nothing here.
    fn project(&self, root: &Path) -> Box<dyn FrameworkProject> {
        let _ = root;
        Box::new(())
    }

    /// File extensions this framework puts PHP in, beyond the conventional ones.
    ///
    /// Drupal's `.module` and `.install` files are PHP and hold hook implementations, so a project
    /// where they are skipped is missing a large part of its own code.
    fn file_extensions(&self) -> &'static [&'static str] {
        &[]
    }

    /// File extensions this framework reads its *own* answers out of, beyond PHP.
    ///
    /// The counterpart of [`Self::file_extensions`]: that one says which files hold code, this one
    /// says which files decide what [`Self::project`] contributes. Drupal reads `.yml` -- a service
    /// id, a route, a module's namespace and a field's kind are all declared there -- so editing one
    /// changes what the index should hold, and until the change reaches the index the answer for
    /// every id declared in it is the one from before the edit.
    ///
    /// Extensions rather than names, because the names are not a list anything could keep: a
    /// provider reads `*.services.yml`, `*.info.yml`, `*.permissions.yml` and more from four
    /// different modules of its own, and a second list of them beside those reads is a list that
    /// goes stale the first time somebody teaches it a fifth file. The cost of the coarser answer is
    /// a re-read of the project when a `.yml` that decides nothing is written, which is a warm start
    /// -- and a project whose framework says nothing here pays nothing at all.
    fn configuration_extensions(&self) -> &'static [&'static str] {
        &[]
    }

    /// Methods whose declared `null` the framework never actually returns.
    ///
    /// A `&'static` slice rather than something built per project, because these are facts about a
    /// framework's own source and not about the checkout: nothing here may vary with what is
    /// installed, or the same code would be checked differently on two machines.
    fn non_null_returns(&self) -> &'static [NonNullReturn] {
        &[]
    }

    /// Methods that call the callback they are given, and what they hand it.
    ///
    /// `&'static`, like [`Self::non_null_returns`] and for the same reason: which argument a method
    /// invokes and what it passes are facts about a framework's own source, not about a checkout.
    /// See [`InvokedCallback`]; a framework answering nothing costs nothing.
    fn invoked_callbacks(&self) -> &'static [InvokedCallback] {
        &[]
    }

    /// Methods whose declared union is settled by an earlier call on the same receiver.
    ///
    /// A query builder's shape. See [`ChainedReturn`]; a framework answering nothing costs nothing.
    fn chained_returns(&self) -> &'static [ChainedReturn] {
        &[]
    }

    /// Methods whose declared union of a collection and its element is settled by an earlier call.
    /// See [`ChainedOperand`](crate::rule::ChainedOperand).
    fn chained_operands(&self) -> &'static [crate::rule::ChainedOperand] {
        &[]
    }

    /// Methods whose declared union is settled by the call leaving an argument out.
    ///
    /// The class-valued sibling of [`Self::chained_returns`]. See [`DefaultedReturn`] for why it can
    /// only select a branch the declaration already names; a framework answering nothing costs
    /// nothing.
    fn defaulted_returns(&self) -> &'static [DefaultedReturn] {
        &[]
    }

    /// Methods declared `object` whose argument names the class they return.
    ///
    /// The fact a `@return T` bound to a `class-string<T>` parameter would have stated, for a
    /// framework that states it in prose instead. See [`ArgumentClassReturn`] for why it is safe
    /// without the gate the two above have; a framework answering nothing costs nothing.
    fn argument_class_returns(&self) -> &'static [ArgumentClassReturn] {
        &[]
    }

    /// How a docblock says that what documents it is written on another declaration.
    ///
    /// `{@inheritdoc}` is PHP's own way of saying it and needs no framework. Drupal has a second:
    /// `Implements hook_form_alter().` is the whole docblock its standard asks for, because what the
    /// parameters mean is documented on the hook and not on each of the hundreds of implementations.
    ///
    /// See [`DocumentedBy`] for what one carries. A framework answering nothing costs nothing.
    fn documented_elsewhere(&self) -> &'static [DocumentedBy] {
        &[]
    }

    /// Declarations whose docblock the framework shows to a person, so it takes prose alone.
    ///
    /// Drupal's update descriptions are the case: the whole docblock, tags included, is what
    /// somebody reads on `update.php` while deciding whether to run the update. The docblock is
    /// therefore required *and* closed to tags, which is the opposite pair from
    /// [`Self::documented_elsewhere`] -- there the prose is the cross-reference and the tags live
    /// on the declaration it names.
    ///
    /// See [`UserFacingDocblock`]. A framework answering nothing costs nothing.
    fn user_facing_docblocks(&self) -> &'static [UserFacingDocblock] {
        &[]
    }

    /// What the framework's container builds and which static calls reach into it, when it has a
    /// container a class can be handed things by. See [`ContainerRule`].
    fn container(&self) -> Option<&'static ContainerRule> {
        None
    }

    /// Methods that dispatch an extension point by the name they are handed.
    ///
    /// `&'static`, like the contracts above and for the same reason: which methods dispatch is a
    /// fact about the framework's own source. *Which names* are dispatched is not -- that comes out
    /// of the project's own calls, which is the whole point. See [`DispatchedName`].
    fn dispatched_names(&self) -> &'static [DispatchedName] {
        &[]
    }

    /// Methods whose first argument names the class they return.
    ///
    /// `&'static`, like [`Self::non_null_returns`] and for the same reason: which methods work this
    /// way is a fact about the framework's own source. What the argument *means* is not, which is why
    /// the values come separately, from [`Self::catalog_entries`].
    fn argument_bindings(&self) -> &'static [ArgumentBinding] {
        &[]
    }

    /// Free functions whose argument names the class they return. See [`FunctionBinding`].
    fn function_bindings(&self) -> &'static [FunctionBinding] {
        &[]
    }

    /// Methods whose return class an argument to the *previous* call in the chain decides.
    ///
    /// `&'static` for the same reason as the two above. The difference from
    /// [`Self::argument_bindings`] is only where the key is written -- see
    /// [`ForwardedArgument`](crate::rule::ForwardedArgument) for why some APIs leave it there.
    fn forwarded_arguments(&self) -> &'static [ForwardedArgument] {
        &[]
    }

    /// Properties no declaration writes, answered by a class's `__get`.
    ///
    /// `&'static` for the reason the contracts above are: which classes have a magic accessor, and
    /// what it hands back, is a fact about the framework's own source. *Which names* it answers for
    /// may still come out of the project -- see [`MagicName`](crate::rule::MagicName).
    fn magic_properties(&self) -> &'static [MagicProperty] {
        &[]
    }

    /// Methods called without the prefix they are declared under. See [`PrefixedMethod`].
    fn prefixed_methods(&self) -> &'static [PrefixedMethod] {
        &[]
    }

    /// Calls handing every template a variable. See
    /// [`SharedTemplateData`](crate::rule::SharedTemplateData).
    fn shared_template_data(&self) -> &'static [crate::rule::SharedTemplateData] {
        &[]
    }

    /// Calls that render a template and hand it variables. See
    /// [`RenderedTemplate`](crate::rule::RenderedTemplate).
    fn rendered_templates(&self) -> &'static [crate::rule::RenderedTemplate] {
        &[]
    }

    /// Arrays that render the template they name. See [`RenderArray`](crate::rule::RenderArray).
    fn render_arrays(&self) -> &'static [crate::rule::RenderArray] {
        &[]
    }

    /// Calls naming the class a callee's bound closures run as. See
    /// [`ConfiguredClosureThis`](crate::rule::ConfiguredClosureThis).
    fn configured_closure_this(&self) -> &'static [crate::rule::ConfiguredClosureThis] {
        &[]
    }

    /// Classes whose instance renders their own template. See
    /// [`RenderingClass`](crate::rule::RenderingClass).
    fn rendering_classes(&self) -> &'static [crate::rule::RenderingClass] {
        &[]
    }

    /// Template languages whose files are read as the PHP they hold. See
    /// [`TemplateDialect`](crate::rule::TemplateDialect).
    fn template_dialects(&self) -> &'static [crate::rule::TemplateDialect] {
        &[]
    }

    /// How the project's Twig loader is configured, when this framework renders Twig. See
    /// [`TwigLoader`](crate::rule::TwigLoader).
    fn twig_loaders(&self) -> &'static [crate::rule::TwigLoader] {
        &[]
    }

    /// What a literal a catalog does not list holds. See
    /// [`UnlistedKey`](crate::rule::UnlistedKey).
    fn unlisted_keys(&self) -> &'static [crate::rule::UnlistedKey] {
        &[]
    }

    /// Methods `__call` answers for every name with a prefix, with the receiver. See
    /// [`FluentPrefix`](crate::rule::FluentPrefix).
    fn fluent_prefixes(&self) -> &'static [crate::rule::FluentPrefix] {
        &[]
    }

    /// Properties read through the method of the same name. See [`MethodBackedProperty`].
    fn method_backed_properties(&self) -> &'static [MethodBackedProperty] {
        &[]
    }

    /// Static methods that give a class a method by name. See
    /// [`MacroRegistrar`](crate::rule::MacroRegistrar).
    fn macro_registrars(&self) -> &'static [crate::rule::MacroRegistrar] {
        &[]
    }

    /// Methods whose bare generic return the framework fills from the receiver. See
    /// [`FilledReturn`](crate::rule::FilledReturn).
    fn filled_returns(&self) -> &'static [crate::rule::FilledReturn] {
        &[]
    }

    /// Methods whose return is the class a fixed catalog key names. See
    /// [`KeyedReturn`](crate::rule::KeyedReturn).
    fn keyed_returns(&self) -> &'static [crate::rule::KeyedReturn] {
        &[]
    }

    /// Properties read through the accessor their name spells. See
    /// [`AccessorProperty`](crate::rule::AccessorProperty).
    fn accessor_properties(&self) -> &'static [crate::rule::AccessorProperty] {
        &[]
    }

    /// Inputs a class declares in a property's string and a method reads by name. See
    /// [`DeclaredInputs`](crate::rule::DeclaredInputs).
    fn declared_inputs(&self) -> &'static [crate::rule::DeclaredInputs] {
        &[]
    }

    /// Static methods handing back the class named after the one they were called on. See
    /// [`NamedAfterCaller`](crate::rule::NamedAfterCaller).
    fn named_after_caller(&self) -> &'static [crate::rule::NamedAfterCaller] {
        &[]
    }

    /// Templates a subclass fills by its own name. See
    /// [`NamedTemplateArgument`](crate::rule::NamedTemplateArgument).
    fn named_template_arguments(&self) -> &'static [crate::rule::NamedTemplateArgument] {
        &[]
    }

    /// A table's columns as migrations write them, and the classes whose rows they are. See
    /// [`WrittenSchema`](crate::rule::WrittenSchema).
    fn written_schema(&self) -> &'static [crate::rule::WrittenSchema] {
        &[]
    }

    /// Properties a class states in an array it writes. See
    /// [`StatedProperties`](crate::rule::StatedProperties).
    fn stated_properties(&self) -> &'static [crate::rule::StatedProperties] {
        &[]
    }

    /// `@mixin` tags a framework's classes behave as though they wrote -- see
    /// [`ImpliedMixin`](crate::rule::ImpliedMixin).
    fn implied_mixins(&self) -> &'static [crate::rule::ImpliedMixin] {
        &[]
    }

    /// Objects a named method builds to declare an entry. See
    /// [`ConstructedEntry`](crate::rule::ConstructedEntry).
    fn constructed_entries(&self) -> &'static [crate::rule::ConstructedEntry] {
        &[]
    }

    /// The catalogs holding a Twig template's filters, functions and tests, when this provider
    /// fills them. See [`TwigCallables`](crate::rule::TwigCallables).
    fn twig_callables(&self) -> Option<crate::rule::TwigCallables> {
        None
    }

    /// Methods whose `return X::class;` makes `X` a mixin of the class -- see
    /// [`MixinReturnedBy`](crate::rule::MixinReturnedBy).
    fn mixins_returned_by(&self) -> &'static [crate::rule::MixinReturnedBy] {
        &[]
    }

    /// Classes whose `@method` tags were generated from another's methods -- see
    /// [`MethodsTaggedFrom`](crate::rule::MethodsTaggedFrom).
    fn methods_tagged_from(&self) -> &'static [crate::rule::MethodsTaggedFrom] {
        &[]
    }

    /// Methods a class's `__call` hands back to their own declarations -- see
    /// [`OpenedByCall`](crate::rule::OpenedByCall).
    fn opened_by_call(&self) -> &'static [crate::rule::OpenedByCall] {
        &[]
    }

    /// Reads whose type is looked up per name, in a table the project states.
    ///
    /// See [`TabledAccess`](crate::rule::TabledAccess) for why a `__get` that answers differently
    /// for every name cannot be a [`MagicProperty`](crate::rule::MagicProperty).
    fn tabled_accesses(&self) -> &'static [TabledAccess] {
        &[]
    }

    /// Declarations whose body states the properties of the kind they declare.
    ///
    /// See [`DeclaredTable`](crate::rule::DeclaredTable). The attribute it names must also be one
    /// [`Self::attribute_keys`] or this rule puts in the keyed set, or lowering keeps no literal
    /// for it and nothing is found.
    fn declared_tables(&self) -> &'static [DeclaredTable] {
        &[]
    }

    /// Attribute arguments that are catalog keys, with the annotated class as the value.
    ///
    /// `&'static` like the contracts above, and for the same reason: *which* attribute carries an id
    /// is a fact about the framework. Which ids exist is not, and that comes out of the project --
    /// the index reads it off the declarations it has already parsed.
    fn attribute_keys(&self) -> &'static [AttributeKey] {
        &[]
    }

    /// Catalog keys a declaration spells out of two of its own arguments.
    ///
    /// The same split as [`Self::attribute_keys`] one step further along: *how* a framework spells a
    /// generated name is a fact about the framework, and which names it generates comes out of the
    /// project's declarations. See [`DerivedKey`].
    fn derived_keys(&self) -> &'static [DerivedKey] {
        &[]
    }

    /// Catalog keys carried in a declaration's own name. See [`DerivedName`].
    fn derived_names(&self) -> &'static [DerivedName] {
        &[]
    }

    /// How this framework's docblock annotations are written as attributes instead.
    ///
    /// `None` for a framework that never used Doctrine's syntax, which is most of them now. See
    /// [`AnnotationRewrite`] for why this is one segment and a short list rather than a table of
    /// every annotation there is.
    fn annotation_rewrite(&self) -> Option<AnnotationRewrite> {
        None
    }

    /// Scalars in this framework's configuration files that name something the index holds.
    ///
    /// `&'static` for the reason every table above is: which key of which file means what is a fact
    /// about the framework. *Which* ids exist is not, and comes out of the catalogs the project
    /// filled. See [`YamlReference`].
    /// Namespace segments whose classes a framework *finds* rather than being told about.
    ///
    /// Drupal's plugin managers scan `src/Plugin/<Type>/` of every extension and instantiate what
    /// they find, so `Drupal\\node\\Plugin\\Field\\FieldWidget\\Foo` is reached without any
    /// file naming it. That is not a gap in the reference index -- there is genuinely nothing to
    /// point at -- so it is stated as reachability instead, and it is what keeps a dead-code
    /// report from calling 232 live classes of one real project dead.
    ///
    /// A segment of the namespace rather than a path, because PSR-4 makes the two the same thing
    /// and the namespace is what the index already holds.
    fn discovered_namespaces(&self) -> &'static [&'static str] {
        &[]
    }

    /// Supertypes whose subclasses a framework finds rather than is told about.
    ///
    /// The other half of discovery, and the precise half. A namespace says *where* a framework
    /// looks; this says *what* it instantiates when it gets there -- Drupal builds every
    /// `ServiceProviderBase` it finds in a module's `src/` root, and Drush runs every
    /// `DrushCommands`. Neither is named anywhere, and a report of what nothing uses lists both
    /// until it is told.
    ///
    /// Read from the ancestor down rather than from each candidate up, because that is the
    /// direction the index already answers: one scan per supertype, not one walk per class.
    fn discovered_supertypes(&self) -> &'static [&'static str] {
        &[]
    }

    /// Supertypes whose subclasses this framework *runs*, catching whatever they throw.
    ///
    /// Apart from [`Self::discovered_supertypes`] because the two say different things about one
    /// hierarchy. That list is about a class being *built* where no `new` is written, and spares it
    /// from `unused-declaration`. This one is about its exceptions having **no caller to be told
    /// to**: a test method's `@throws` would describe the test runner's control flow, which nobody
    /// documents and nobody could act on.
    ///
    /// Deliberately not a list of exception classes. Which exceptions a project treats as part of a
    /// contract is a policy that project has to choose -- PHPStan calls it `checkedExceptions` and
    /// PhpStorm ships a default -- and a framework naming the base class it *runs* is a fact
    /// instead.
    fn runs_and_catches(&self) -> &'static [&'static str] {
        &[]
    }

    /// Supertypes whose subclasses answer names no declaration holds.
    ///
    /// A third thing a provider can say with a base class, and the third list on one hierarchy
    /// climb. `__call` already excuses a class from `undefined-method` -- PHP's own way of saying
    /// "this answers anything" -- and it is a *class* mechanism: a mocking library hands back a
    /// generated object behind an **interface**, and an interface declaring `__call` would be a
    /// claim about every implementer.
    ///
    /// So `Mockery\MockInterface` declares almost nothing and proxies everything, and PHPUnit
    /// splits its fluent builder across two interfaces the concrete class implements together --
    /// `method()` returns the one without `with()` on it. Typing a mock is what makes that visible,
    /// and it is the library's shape rather than the caller's mistake.
    fn dynamic_members(&self) -> &'static [&'static str] {
        &[]
    }

    /// Prefixes that make a string literal name a method of the class it is written in.
    ///
    /// Drupal's form API writes a callback as `'::districtCallback'`, and the builder resolves the
    /// leading `::` against `$form_state->getFormObject()` -- which is the class the array was
    /// written in. `'#submit' => ['::save']` and `'#ajax' => ['callback' => '::refresh']` are the
    /// same convention, and it is the commoner spelling in real code: the array form
    /// `[$this, 'save']` resolves already and this one answered nothing at all.
    ///
    /// A prefix rather than a rule per key, because what the framework settles is the *spelling*:
    /// any string starting with it names a method of the enclosing class, wherever it is written.
    /// Nothing else in PHP begins a string that way, so the match cannot claim an ordinary one.
    fn own_method_prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    /// Calls on a test double whose string argument names a method of what it stands in for.
    ///
    /// The other way a string names a method: not by its spelling but by the call it is passed to,
    /// on a value whose type says what it stands in for. See [`crate::rule::StandInMethod`].
    fn stand_in_methods(&self) -> &'static [crate::rule::StandInMethod] {
        &[]
    }

    /// Calls whose string argument names a key a sibling method writes into an array it builds.
    ///
    /// See [`crate::rule::BuiltKey`].
    fn built_keys(&self) -> &'static [crate::rule::BuiltKey] {
        &[]
    }

    /// Files whose mapping *keys* are class names, matched by the end of the file's name.
    ///
    /// A container written for autowiring gives a service no id of its own -- the key *is* the
    /// class:
    ///
    /// ```yaml
    /// services:
    ///   Drupal\\app\\Action\\Resend:
    ///     arguments: ['@logger']
    /// ```
    ///
    /// A rule in [`Self::yaml_references`] cannot say this: those match a value written under a
    /// key, and here there is no value. 48 services on one real project are written this way, and
    /// while nothing read them the class each one names had no reference, was left behind by a
    /// rename, and was reported as reached by nothing.
    ///
    /// A key that holds no separator is an ordinary id and is left alone -- `app.provider` names
    /// a service, `Drupal\\app\\Resend` names a class, and the `\\` is the whole of the
    /// difference.
    fn yaml_class_keys(&self) -> &'static [&'static str] {
        &[]
    }

    fn yaml_references(&self) -> &'static [YamlReference] {
        &[]
    }

    /// Array keys whose value is a key in a catalog.
    ///
    /// `&'static` like the contracts above: which key a framework's array convention puts an id
    /// under is a fact about the framework. Which ids exist is not, and comes from
    /// [`Self::attribute_keys`] read off the project's own declarations.
    fn array_key_bindings(&self) -> &'static [ArrayKeyBinding] {
        &[]
    }

    /// Which array key names the class that documents the array's other keys.
    ///
    /// Where a `#`-prefixed property comes from at all: nothing in a project declares `#maxlength`,
    /// and the only statement that it exists is a bullet in the docblock of the class `#type`
    /// chose. See [`DocumentedKeys`] for why this is a rule of its own rather than a reading of
    /// every [`ArrayKeyBinding`].
    fn documented_keys(&self) -> &'static [DocumentedKeys] {
        &[]
    }

    /// Catalogs whose key set is complete: a key missing from one does not exist.
    ///
    /// Only a provider can say. Drupal's container is built from a list of files that can be read in
    /// full, so an unregistered service id is a mistake; its routes are not, because
    /// `entity.node.canonical` is generated at runtime from an entity type's link templates and is
    /// declared in no file at all. Measured on one real project: 372 of 372 service ids are in the
    /// catalog, against 82 of 101 route names.
    ///
    /// Naming a catalog here is what lets something report on it, so a wrong entry here reports
    /// correct code.
    fn closed_catalogs(&self) -> &'static [&'static str] {
        &[]
    }

    /// The name of the house style this framework's code is written in.
    ///
    /// A name rather than a set of settings, because the settings are several dozen decisions the
    /// formatter has already made and restating them here would be a copy that drifts. Drupal's
    /// standard is two-space indent, single quotes and no trailing close tag; the formatter knows it
    /// under a name, and this says which name.
    ///
    /// `None` for a framework with no standard of its own, which leaves whatever the project or the
    /// default says.
    fn format_style(&self) -> Option<&'static str> {
        None
    }
}

/// One framework's reading of one project: everything a provider learns from the files.
///
/// Every method is defaulted to nothing, so a framework says only what it knows. The trait is
/// separate from [`FrameworkProvider`] because its answers come from a walk of the project, and the
/// object holding the walk is what lets four questions cost one look.
pub trait FrameworkProject {
    /// Namespace mappings the framework registers at runtime, which composer does not know about.
    fn autoload_rules(&self) -> Vec<AutoloadRule> {
        Vec::new()
    }

    /// Files holding PHP that no autoload rule can name.
    ///
    /// An autoloader keys a file by the class inside it, so a file that declares only functions and
    /// constants has nothing to look up -- and a framework that ships such files loads them by hand
    /// instead. Composer's map then mentions none of them, which leaves discovery finding them only
    /// where they happen to be tracked by git. Drupal's `t()` is in a gitignored `core/includes`,
    /// and every hook implementation is in a `.module` beside an `*.info.yml` that composer has
    /// never heard of.
    ///
    /// Read out of the project like [`Self::catalog_entries`], and for the same reason: which of
    /// them exist depends on what is installed. Named file by file rather than by directory, because
    /// the directory holding one also holds test fixtures and templates that are not worth indexing.
    fn procedural_sources(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Catalog keys whose value is a place in a file rather than a class.
    ///
    /// Read out of the project like [`Self::catalog_entries`], and for the same reason: which routes
    /// and permissions exist depends on what is installed.
    fn catalog_places(&self) -> Vec<PlaceEntry> {
        Vec::new()
    }

    /// Paths that are PHP by extension and not code by nature: a directory the framework writes
    /// generated classes into, a settings file holding credentials.
    ///
    /// Neither indexed nor checked, whether or not the project happens to gitignore them -- a
    /// compiled template cache and a settings file are what they are in every checkout. A directory
    /// excludes everything under it; a file excludes itself.
    fn excluded_paths(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Paths this project reads and does not own: the framework's own code, and the extensions a
    /// package manager put there.
    ///
    /// Apart from [`Self::excluded_paths`] because the two say opposite things about *indexing*.
    /// An excluded path is not indexed at all. One of these is indexed in full -- goto, hover and
    /// completion into it are most of what a person does all day -- and is not **checked**: a
    /// finding in it is about code they cannot change, and it arrives in the one place they cannot
    /// dismiss it, the file they opened to read.
    ///
    /// `vendor/` needs no entry; a package manager's own directory is already the whole of what
    /// [`SourceOrigin`](phrost_model::SourceOrigin) decides. What this is for is a framework that
    /// keeps its code somewhere else: Drupal registers `Drupal\Core\` at `<docroot>/core`, which
    /// composer installs and gitignores and which is not under `vendor/` at all -- 11 256 files on
    /// one real checkout, every one of them checked the moment it was opened.
    fn dependency_paths(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// What the keys of this framework's catalogs name, read out of the project.
    ///
    /// Read from the files the framework itself loads and no others. A container this provider can
    /// see all of gives deterministic answers; one assembled partly from files it does not parse
    /// would answer for some ids and not others, with nothing to tell a reader which.
    fn catalog_entries(&self) -> Vec<CatalogEntry> {
        Vec::new()
    }

    /// Global names a framework's loader gives a class at start. See
    /// [`ClassAlias`](crate::rule::ClassAlias).
    fn class_aliases(&self) -> Vec<crate::rule::ClassAlias> {
        Vec::new()
    }

    /// What each key of a catalog holds, name by name.
    ///
    /// Read out of the checkout rather than listed in the provider, because both halves are the
    /// project's: which keys exist is its configuration, and what each holds is a declaration
    /// somewhere in its code. See [`PropertyTable`](crate::rule::PropertyTable).
    fn property_tables(&self) -> Vec<PropertyTable> {
        Vec::new()
    }

    /// What kind of thing each key of a catalog is, for the tables above to be found by.
    ///
    /// See [`CatalogKind`](crate::rule::CatalogKind) for why a kind is not a class.
    fn catalog_kinds(&self) -> Vec<CatalogKind> {
        Vec::new()
    }
}

/// The project of a framework that reads no files: every answer is the default.
impl FrameworkProject for () {}

/// Every known framework.
///
/// The one place a framework is named. If a second place ever needs the list, it should call
/// [`detect`] instead — a `match` on framework identity in the generic layer is how the previous
/// attempt ended up with framework branches in three unrelated features.
static PROVIDERS: &[&dyn FrameworkProvider] = &[
    &providers::drupal::Drupal,
    &providers::laravel::Laravel,
    &providers::mockery::Mockery,
    &providers::pest::Pest,
    &providers::phpunit::PhpUnit,
    &providers::symfony::Symfony,
    &providers::twig::Twig,
];

/// The frameworks present in a project.
pub fn detect(root: &Path) -> Vec<&'static dyn FrameworkProvider> {
    PROVIDERS
        .iter()
        .copied()
        .filter(|provider| provider.detect(root))
        .collect()
}

/// The providers a project asked for by id, and the ids that name none.
///
/// Detection probes the filesystem, so it answers no for a layout nobody anticipated -- and a
/// project that knows better says so in its configuration. Added to what was detected rather than
/// replacing it; see `ProjectConfig::framework`.
///
/// The unmatched ids come back rather than being dropped, because the whole failure this guards
/// against is a misspelling turning a framework's knowledge off while looking like the feature not
/// working. Compared without regard to case, since an id is a word a person types.
pub fn named(ids: &[String]) -> (Vec<&'static dyn FrameworkProvider>, Vec<String>) {
    let mut found = Vec::new();
    let mut unknown = Vec::new();
    for id in ids {
        let wanted = id.trim();
        match PROVIDERS
            .iter()
            .copied()
            .find(|provider| provider.id().eq_ignore_ascii_case(wanted))
        {
            Some(provider) => found.push(provider),
            None => unknown.push(wanted.to_owned()),
        }
    }
    (found, unknown)
}

/// Every id a project may name, for a message that lists them.
pub fn known_ids() -> Vec<&'static str> {
    PROVIDERS.iter().map(|provider| provider.id()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every contract names a class the way the index stores one.
    ///
    /// The failure this exists for is silent: a leading `\`, a single-quoted `\C` that never
    /// reached the compiler as a separator, or a short name -- none of them match anything, so the
    /// contract simply does nothing and the only symptom is findings that were supposed to go away.
    /// Nothing else in the project would notice.
    ///
    /// Over every kind of contract, from one list. When this checked only the non-null returns, the
    /// service bindings went unchecked for as long as they existed -- which is what a per-kind test
    /// buys you: the kind added next is the one nothing looks at.
    #[test]
    fn every_contract_names_a_class_the_index_could_hold() {
        for provider in PROVIDERS {
            let mut named: Vec<(&str, &str, &str)> = Vec::new();
            for contract in provider.non_null_returns() {
                named.push(("non-null return", contract.class, contract.method));
            }
            for binding in provider.argument_bindings() {
                named.push(("argument binding", binding.class, binding.method));
            }
            for defaulted in provider.defaulted_returns() {
                named.push(("defaulted return", defaulted.class, defaulted.method));
                // The branch is a class the index has to hold too: a misspelling of it matches no
                // operand of the declared union, so the contract silently never fires.
                named.push(("defaulted return", defaulted.then, defaulted.method));
            }
            for contract in provider.argument_class_returns() {
                named.push(("argument class return", contract.class, contract.method));
            }
            for invoked in provider.invoked_callbacks() {
                named.push(("invoked callback", invoked.class, invoked.method));
                // The classes it hands the callback are names the index has to hold too, and a
                // misspelling of one is silent in the worst way: the reader requires the class to
                // be declared, so a typo makes the entry do nothing at all.
                for named_class in invoked.parameters.iter().flatten() {
                    named.push(("invoked callback", named_class, invoked.method));
                }
            }
            for forwarded in provider.forwarded_arguments() {
                named.push(("forwarded argument", forwarded.class, forwarded.method));
                // The method that produced the receiver is a bare name too, and a wrong one here
                // fails to match every chain rather than the wrong ones.
                named.push(("forwarded argument", forwarded.class, forwarded.through));
            }
            for dispatch in provider.dispatched_names() {
                named.push(("dispatched name", dispatch.class, dispatch.method));
            }
            for magic in provider.magic_properties() {
                // Both ends are classes the index has to hold: the receiver the accessor is on, and
                // what reading one gives back. A wrong spelling of either answers nothing, silently.
                let name = match magic.name {
                    crate::rule::MagicName::Exactly(name) => name,
                    // A catalog is not a member name, and the shape check below is about names. The
                    // catalog itself is checked by `every_catalog_has_both_a_filler_and_a_reader`.
                    crate::rule::MagicName::KeyOf(_) => "read",
                };
                named.push(("magic property", magic.class, name));
                named.push(("magic property", magic.holds, name));
            }

            assert!(
                !named.is_empty()
                    || provider
                        .project(Path::new("/nowhere"))
                        .catalog_entries()
                        .is_empty(),
                "{}: fills a catalog nothing reads from",
                provider.id()
            );

            for (kind, class, method) in named {
                assert!(
                    !class.starts_with('\\'),
                    "{}: {kind} {class} is written with a leading separator, which no stored name has",
                    provider.id()
                );
                // Not "contains a separator": Drupal's own `\Drupal` lives at the root, which is the
                // whole reason the composer classmap is loaded in full. A short name is only wrong
                // when the class is not really there, and that is the index's question, not this
                // one. Generalising this test over every contract kind is what surfaced it -- the
                // rule as first written would have rejected a name that is correct.
                assert!(
                    !class.is_empty() && !class.ends_with('\\'),
                    "{}: {kind} {class} is not a name the index stores",
                    provider.id()
                );
                assert!(
                    !method.is_empty() && !method.contains('\\'),
                    "{}: {kind} {method} is not a bare method name",
                    provider.id()
                );
            }
        }
    }

    /// A catalog that nothing indexes into is a table built for no reader, and a binding into a
    /// catalog nothing fills answers nothing. Either alone is a wiring mistake with no symptom.
    #[test]
    fn every_catalog_has_both_a_filler_and_a_reader() {
        for provider in PROVIDERS {
            let read: Vec<&str> =
                provider
                    .argument_bindings()
                    .iter()
                    .map(|binding| binding.catalog)
                    .chain(
                        provider
                            .forwarded_arguments()
                            .iter()
                            .map(|forwarded| forwarded.catalog),
                    )
                    .chain(
                        provider
                            .array_key_bindings()
                            .iter()
                            .map(|binding| binding.catalog),
                    )
                    .chain(provider.magic_properties().iter().filter_map(
                        |magic| match magic.name {
                            crate::rule::MagicName::KeyOf(catalog) => Some(catalog),
                            crate::rule::MagicName::Exactly(_) => None,
                        },
                    ))
                    .collect();
            for catalog in &read {
                assert!(
                    !catalog.is_empty(),
                    "{}: a binding names no catalog",
                    provider.id()
                );
            }

            // An array key is compared exactly, so a stray space or a missing sigil is a binding
            // that matches nothing -- with no error and no symptom but a feature that never fires.
            for binding in provider.array_key_bindings() {
                assert_eq!(
                    binding.key.trim(),
                    binding.key,
                    "{}: array key `{}` is not written the way an array writes one",
                    provider.id(),
                    binding.key
                );
                assert!(
                    !binding.key.is_empty(),
                    "{}: an array key binding names no key",
                    provider.id()
                );
            }
        }
    }

    /// A dispatch catalog is filled by calls and read through a docblock convention, and the two
    /// spell the dispatched declaration's name independently.
    ///
    /// Two halves that must agree by string and nothing else checks that they do: a convention
    /// reading `hook_x` against a catalog filled with `x` finds nothing at all, which reads as the
    /// feature never having been built rather than as a typo. Filling a catalog no convention names
    /// is the same failure with the halves swapped.
    #[test]
    fn the_dispatch_catalog_is_spelled_the_way_the_convention_reads_it() {
        for provider in PROVIDERS {
            for dispatch in provider.dispatched_names() {
                let reader = provider
                    .documented_elsewhere()
                    .iter()
                    .find(|rule| rule.dispatched == Some(dispatch.catalog));
                let reader = reader.unwrap_or_else(|| {
                    panic!(
                        "{}: nothing reads the `{}` catalog the {}::{} rule fills",
                        provider.id(),
                        dispatch.catalog,
                        dispatch.class,
                        dispatch.method
                    )
                });
                assert_eq!(
                    reader.names,
                    dispatch.names,
                    "{}: the convention reads a name beginning `{}` and {}::{} claims one beginning \
                     `{}`",
                    provider.id(),
                    reader.names,
                    dispatch.class,
                    dispatch.method,
                    dispatch.names
                );
            }

            for rule in provider.documented_elsewhere() {
                let Some(catalog) = rule.dispatched else {
                    continue;
                };
                assert!(
                    provider
                        .dispatched_names()
                        .iter()
                        .any(|dispatch| dispatch.catalog == catalog),
                    "{}: the convention reads the `{catalog}` catalog and nothing fills it",
                    provider.id()
                );
            }
        }
    }
}
