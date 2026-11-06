//! What a member returns, where the declaration is wrong about it.
//!
//! Mostly project-level configuration: installed once, by whoever knows which frameworks are
//! present, and unchanged when a file changes. That is why it lives beside the skeleton's own tables
//! instead of inside a file's -- and why
//! [`ProjectSkeleton::adopt`](crate::ProjectSkeleton::adopt) exists to carry it across an index
//! reload.
//!
//! One table is not configuration. A catalog key can be *declared*, by an attribute on a class --
//! `#[ContentEntityType(id: 'node')]` is what makes `getStorage('node')` mean `Node` -- and that
//! comes and goes with the file. Those are kept apart, with the file that claimed each one, so
//! re-ingesting a file withdraws exactly what it had said.
//!
//! Nothing here names a framework. The facts arrive as strings from above.

use phrost_hir::AttributeArgument;
use phrost_model::{BuiltinTy, FileId, TextRange};
use rustc_hash::FxHashMap;

mod annotations;
mod catalogs;
mod container;
mod documentation;
mod language;

use self::catalogs::{CatalogValue, Claim};
pub use self::{
    catalogs::{NEAR_MISS_EDITS, NEAR_MISS_FLOOR, Place, edits_within},
    container::ContainerReachRule,
    documentation::{DocumentedByRule, DocumentedKey, DocumentedKeysRule, UserFacingDocblockRule},
};

/// Declarations whose stated sentinel never arrives.
///
/// Keyed by method name first because that is the lookup that fails: every member access in a
/// project asks, and all but a handful ask about a name nothing has a contract for. One hash of a
/// short string, and the hierarchy walk only happens for the names that matched.
#[derive(Debug, Default)]
pub struct ReturnContracts {
    /// Method name -> the classes that declare the contract for it.
    ///
    /// A list rather than one class, because two unrelated hierarchies can declare a contract for
    /// the same method name and the receiver decides which applies.
    unproduced: Vec<(Box<str>, Vec<Unproduced>)>,
    /// Method name -> methods whose first argument names the class they return.
    ///
    /// A `Vec` scanned rather than a map, and this one is measured: PHP compares method names without
    /// regard to case, so a hashed lookup has to fold the name first, and folding allocates on a path
    /// that every member access in a file walks. A framework contributes a handful of these, so the
    /// scan compares a handful of short strings and allocates nothing.
    from_argument: Vec<(Box<str>, Vec<ArgumentBinding>)>,
    /// Function name -> which argument names the class it returns, and in which catalog. The free
    /// function's half of [`Self::from_argument`], scanned for the same reason.
    from_function_argument: Vec<(Box<str>, FunctionArgument)>,
    /// Properties read through the method of the same name. A framework contributes one or two.
    method_properties: Vec<MethodProperty>,
    /// Properties read through the accessor their name spells. Same size, same reason.
    accessors: Vec<AccessorRule>,
    /// Methods `__call` answers for every name with a prefix. Same size, same reason.
    fluent_prefixes: Vec<FluentPrefixRule>,
    /// Catalog -> what a literal it does not list holds, and the text that puts a key outside it.
    unlisted: Vec<(Box<str>, phrost_model::BuiltinTy, Box<str>)>,
    /// Functions that render a template by name: which argument names it, which holds its
    /// variables, and whether the template also has every variable where the call is. A framework
    /// contributes one or two.
    rendered: Vec<(Box<str>, u16, u16, bool)>,
    /// Arrays that render the template they name -- Drupal's `#theme` -- and how they hand it
    /// variables.
    render_arrays: Vec<crate::renders::RenderArrayRule>,
    /// Calls handing every template a variable.
    shared_template_data: Vec<SharedTemplateRule>,
    /// Calls a template reader writes a component tag as: the function, and the prefix the name
    /// takes as a template's.
    component_calls: Vec<(Box<str>, Box<str>)>,
    /// Classes whose instance renders their own template: the class they descend from, the
    /// namespace they are declared under and the template name it spells.
    rendering_classes: Vec<(Box<str>, Box<str>, Box<str>)>,
    /// Calls naming the class a callee's bound closures run as. See `crate::ConfiguredThisRule`.
    configured_this: Vec<crate::ConfiguredThisRule>,
    /// Inputs a class declares in a property's string. See `crate::DeclaredInputsRule`.
    declared_inputs: Vec<crate::DeclaredInputsRule>,
    /// How a framework's migrations write a table -- see [`crate::schema`].
    written_schema: Vec<crate::schema::WrittenSchemaRule>,
    /// Static methods handing back the class named after their caller. See [`NamedAfterRule`].
    named_after: Vec<NamedAfterRule>,
    /// Templates a subclass fills by its own name. See [`NamedArgumentRule`].
    named_arguments: Vec<NamedArgumentRule>,
    /// Method name -> the fixed catalog key whose class it returns. See [`KeyedReturnRule`].
    keyed_returns: Vec<(Box<str>, Vec<KeyedReturnRule>)>,
    /// Bare generic returns a framework fills, by method name.
    filled_returns: Vec<(Box<str>, Vec<FilledReturnRule>)>,
    /// Static methods giving a class a method by name, and the trait whose users take them: see
    /// the seam's `MacroRegistrar`.
    macro_registrars: Vec<(Box<str>, Box<str>)>,
    /// Which names a class writes its stated properties under. See `crate::StatedPropertiesRule`.
    stated: Vec<crate::StatedPropertiesRule>,
    /// Methods called without the prefix they are declared under. Same size, same reason.
    prefixed_methods: Vec<PrefixedMethodRule>,
    /// Method name -> what it hands the callback it is given. Same shape and same reason as the
    /// one above; a framework contributes a handful.
    invokes_callback: Vec<(Box<str>, Vec<InvokedCallbackRow>)>,
    /// Method name -> the unions an earlier call on the same receiver settles. Same shape and same
    /// reason as the two above: a handful of short names, scanned rather than folded and hashed.
    chained: Vec<(Box<str>, Vec<ChainedBranch>)>,
    /// Method name -> the collection-or-element unions an earlier call settles. Same shape, same
    /// reason. See [`ChainedOperandRule`].
    chained_operands: Vec<(Box<str>, Vec<ChainedOperandRule>)>,
    /// Method name -> the unions the call's own shape settles, by leaving an argument out.
    ///
    /// Kept apart from [`Self::chained`] rather than folded into it: the branches of one are builtins
    /// and of the other are classes, and the two are asked at different moments -- the chain contract
    /// needs the steps before the call, this one needs only how many arguments the call wrote.
    defaulted: Vec<(Box<str>, Vec<DefaultBranch>)>,
    /// Method name -> methods whose return class one of the call's own arguments names.
    ///
    /// The claim a `@return T` bound to a `class-string<T>` parameter makes, for a declaration that
    /// does not make it. Kept apart from [`Self::from_argument`] because the two read different
    /// things: that one indexes a catalog with a string, and this one has nothing to look up -- the
    /// string *is* the class. Same shape and same reason as its neighbours: a handful of short
    /// names, scanned rather than folded and hashed.
    argument_class: Vec<(Box<str>, Vec<ArgumentClass>)>,
    /// How a docblock can say its documentation is written on another declaration.
    ///
    /// A framework's own coding standard, contributed as strings for the reason everything else here
    /// is: nothing in this crate names a framework.
    documented_elsewhere: Vec<DocumentedByRule>,
    /// Declarations whose docblock a framework shows to a person, by the shape of the name.
    ///
    /// A `Vec` scanned rather than a map, and a framework contributes two: the question is asked
    /// once per declaration by a rule a project has to switch on, against a handful of short
    /// infixes, and the commonest answer is that the list is empty.
    user_facing_docblocks: Vec<UserFacingDocblockRule>,
    /// What the framework's container builds and which static calls reach into it.
    container: container::ContainerRules,
    /// Method name -> the extension points a call to it declares by the name it is handed.
    ///
    /// Keyed by method name, and scanned rather than hashed, for the reason [`Self::from_argument`]
    /// records: a framework contributes a handful of these and nearly every call in a project asks
    /// about a name none of them has.
    dispatchers: Vec<(Box<str>, Vec<DispatchRule>)>,
    /// Catalog -> the name a call dispatched -> every call that dispatches it.
    ///
    /// Beside [`Self::declared`] rather than inside it, and the difference is what a key *names*: a
    /// declared key names a class the index holds, and this one names a place in a body. A dispatch
    /// has no declaration at all -- that is the whole reason it is worth indexing -- so there is
    /// nothing for [`Self::catalog_class`] to answer and nothing that could travel as a [`Claim`].
    ///
    /// Kept out of [`Self::catalog_holds`] on purpose. A dispatch is only a dispatch once its
    /// receiver has been checked against the rule's class, and this crate holds no hierarchy, so the
    /// membership question cannot be answered here. Whoever can walk the hierarchy asks for the
    /// calls and decides.
    dispatched: FxHashMap<Box<str>, FxHashMap<Box<str>, Vec<Dispatch>>>,
    /// Method name -> methods whose key was passed to the call that produced the receiver.
    ///
    /// Kept apart from [`Self::from_argument`] rather than folded into it with an optional field: the
    /// two are asked in different places -- one has a literal to hand, the other has to look back a
    /// step -- and a shared row would make every member access pay for the lookback.
    through_argument: Vec<(Box<str>, Vec<ForwardedBinding>)>,
    /// Method name -> the catalogs a literal first argument to it is a key of.
    ///
    /// The same rules as [`Self::through_argument`], keyed by the call that *writes* the key rather
    /// than by the one that reads it. Two indexes over one set of rules because the two questions
    /// arrive from opposite ends: resolving `$storage->load($id)` has the reading method in hand and
    /// looks back for the call that produced the receiver, while reporting a key that no catalog
    /// holds has the writing call in hand and never needs to know what was done with the result.
    /// Deriving either from the other at the point of use would mean scanning every rule there is,
    /// once per call site in the file.
    keyed_by_call: Vec<(Box<str>, Vec<ForwardedKey>)>,
    /// Catalog name -> what its keys name.
    ///
    /// A set of named tables rather than one table per concept, so nothing here has to know what a
    /// service or an entity type is. A framework picks the names and fills them; this stores them.
    catalogs: FxHashMap<Box<str>, FxHashMap<Box<str>, CatalogValue>>,
    /// The keys of each catalog the project's own files declare, as against its dependencies'.
    ///
    /// Read by nothing but the order a list of keys is offered in: 1 876 service ids on one real
    /// project, and the two hundred a person is writing against are theirs. A set beside the tables
    /// rather than a bit inside them, because it is as large as the project's own code and no
    /// larger, and every other reader of a key would have to step over the bit.
    own_keys: FxHashMap<Box<str>, rustc_hash::FxHashSet<Box<str>>>,
    /// The same, for keys a declaration claimed rather than configuration named.
    ///
    /// Claimants rather than an answer, because two declarations claiming one key have no answer
    /// between them -- see [`Self::catalog_class`].
    declared: FxHashMap<Box<str>, FxHashMap<Box<str>, Vec<Claim>>>,
    /// Properties a class answers through `__get`, and what reading one gives back.
    ///
    /// One flat list rather than a map keyed by name, because half of these are not keyed by a name
    /// at all -- a catalog rule speaks for every key the project registered -- and because a
    /// framework contributes a handful. A scan of a handful of short strings costs less than folding
    /// a name to hash it, which is the same measurement [`Self::from_argument`] records.
    magic_properties: Vec<PropertyBinding>,
    /// `@mixin` tags a framework's classes behave as though they wrote, by the class's folded name:
    /// the mixins as a written one would be stored, and the parameterised ones' types beside them
    /// for the generics reader -- the same two halves a written `@mixin Builder<static>` lands in.
    implied_mixins: FxHashMap<Box<str>, (Vec<phrost_sema::ClassRef>, Vec<phrost_ty::TypeId>)>,
    /// Methods a class's `__call` hands back to their own declarations, by the class's folded name.
    opened_methods: FxHashMap<Box<str>, Vec<Box<str>>>,
    /// Methods whose `return X::class;` makes `X` a mixin of the class declaring them.
    mixin_methods: Vec<Box<str>>,
    /// Classes each class's `@method` tags were generated from, by the class's folded name.
    tag_sources: FxHashMap<Box<str>, Vec<phrost_sema::ClassRef>>,
    /// The reads whose type is a table lookup, and the tables. Two lists because the rules are a
    /// handful compiled into the binary and the tables are hundreds read out of the checkout.
    tabled: Vec<TabledBinding>,
    tables: FxHashMap<Box<str>, FxHashMap<Box<str>, PropertyRows>>,
    /// What kind of thing each key is, which is how a table is found: the tables are keyed by
    /// kind, because four hundred keys on a real checkout share thirty of them.
    kinds: FxHashMap<Box<str>, FxHashMap<Box<str>, Box<str>>>,
    /// The declarations whose own body states a table. A handful, compiled into the binary.
    declared_tables: Vec<crate::tables::DeclaredTableRule>,
    /// Which attribute argument is a key in which catalog.
    attribute_keys: Vec<AttributeKeyRule>,
    derived_keys: Vec<DerivedKeyRule>,
    derived_names: Vec<DerivedNameRule>,
    /// Array key -> the catalogs the value written under it indexes.
    ///
    /// The shape a call cannot express, because there is no call: `['#type' => 'textfield']` names a
    /// registered element and nothing about it is a method, an argument or a receiver. Compared
    /// exactly and not folded, unlike every method name beside it -- PHP array keys are
    /// case-sensitive, and `'#Type'` is a different slot that means nothing.
    array_keys: Vec<(Box<str>, Vec<Box<str>>)>,
    /// Which array key names the class that documents the array's other keys. See
    /// [`DocumentedKeysRule`].
    documented_keys: Vec<DocumentedKeysRule>,
    /// The namespace segment a framework's attribute classes sit in, and the annotations that
    /// stand for a plain object rather than for an attribute.
    ///
    /// Read only by the offer to convert an annotation into an attribute, which is why they are a
    /// list of four short strings rather than a map: nothing on a request path asks.
    attributes_in: Option<Box<str>>,
    annotation_values: Vec<(Box<str>, Box<str>)>,
    /// What a scalar in a configuration file names. Read only by the feature that answers a request
    /// in one, so a list scanned in order rather than an index.
    config_references: Vec<crate::configuration::ConfigReference>,
    /// How the project's Twig loader names templates, one per framework rendering Twig.
    twig_loaders: Vec<crate::configuration::TwigLoader>,
    /// Templates whose variables are another's, as a project stated it: `(templates, from)`.
    twig_contexts: Vec<(Box<str>, Box<str>)>,
    /// Classes whose construction in a kept method declares a key, with the catalog it is of.
    constructed_entries: Vec<(Box<str>, Box<str>)>,
    /// The methods those constructions are read from.
    construction_methods: Vec<Box<str>>,
    /// The catalogs holding a Twig template's filters, functions and tests.
    twig_callables: Option<[Box<str>; 3]>,
    /// File-name suffixes whose mapping keys are class names -- see `yaml_class_keys`.
    class_keyed_files: Vec<Box<str>>,
    /// Catalogs that hold every key there is.
    ///
    /// The difference between "this id is not registered" and "this id is not one I can see", which
    /// is the difference between a report and silence. Only whoever fills a catalog knows which it
    /// is: Drupal's container is loaded from a list of files this reads in full, so an id missing
    /// from it does not exist -- while a route can be generated at runtime from an entity type's
    /// link templates and a permission from a callback, so a missing one may still be real.
    closed: Vec<Box<str>>,
}

/// A call handing every template a variable: `class::method($name, $value)`, and `helper()->method(
/// $name, $value)` where a function called with nothing hands back the instance.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SharedTemplateRule {
    pub class: Box<str>,
    pub method: Box<str>,
    pub helper: Option<Box<str>>,
}

/// A method that dispatches an extension point by the name it is handed.
///
/// The framework's own rule, restated in this crate's terms like every other one here. What makes
/// it different from every neighbour is the direction: the others read a name a call wrote and say
/// what it *means*, and this one says the name **exists** -- there is no declaration of it anywhere,
/// so the call is the only thing in the project that says so.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DispatchRule {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// Which argument holds the name, counted from zero.
    pub argument: u16,
    /// Whether that argument may be a list of names rather than one.
    pub several: bool,
    /// What the declaration the name implies is called before the part the call writes, and after
    /// it. `alter('x')` runs `hook_x_alter`; `invokeAll('x')` runs `hook_x`.
    pub names: Box<str>,
    pub suffix: Box<str>,
    pub catalog: Box<str>,
}

/// One call that dispatches one name.
///
/// Carries the receiver rather than a verdict about it, because the verdict needs a hierarchy and
/// this crate holds none. The gate is real and it is applied by whoever reads this -- see
/// [`ReturnContracts::dispatched`] for why it cannot be applied here, and [`Self::required`] for
/// what it is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Dispatch {
    pub file: FileId,
    /// The declaration name the call implies: the convention's prefix, what the call wrote, and the
    /// method's own suffix. `alter('x')` runs `hook_x_alter`.
    ///
    /// Interned, and the reason is that a reader hands it back as the name of the thing it found --
    /// which is a `Name` everywhere else in the project, because nothing above this layer should
    /// have to care that this one was pasted together rather than parsed.
    pub name: phrost_model::Name,
    /// The whole call, for a reader that wants to show what it passes.
    pub range: TextRange,
    /// Where the name itself is written: what a jump selects.
    ///
    /// A range and not the text, because nothing here keeps a closed file's text -- the same reason
    /// every declaration's `name_range` is a range. It survives compaction and a cache restore,
    /// which a snapshot of the source would not.
    pub name_range: TextRange,
    /// The dispatching method, as the call wrote it.
    pub method: Box<str>,
    /// The class the receiver has to be, or descend from, for this call to dispatch anything.
    pub required: Box<str>,
    /// What the call was written on, when the file itself settles it.
    ///
    /// `None` for a receiver this could not identify -- a bare local, a chain of two steps. Such a
    /// call is evidence that *something* dispatches the name and no evidence about *what*, and the
    /// two readers of this table want opposite things from that: one asks where a name comes from
    /// and must not answer, the other asks whether a name exists at all and must not report. Both
    /// are the safe direction of the same missing fact, which is why the row is kept rather than
    /// dropped.
    pub receiver: Option<DispatchReceiver>,
}

/// The class a dispatching call's receiver was read off, and the member read on it.
///
/// `$this->moduleHandler` is the enclosing class and `moduleHandler`; `\Drupal::moduleHandler()` is
/// `Drupal` and the same name.
///
/// A member of a named class rather than a resolved type, because resolving one at ingest would mean
/// asking the index about a file that may not be in it yet -- and then which answer a file got would
/// depend on the order files were discovered, which is the one thing the index is checked against.
/// Both halves are settled by the dispatching file alone.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DispatchReceiver {
    pub owner: Box<str>,
    pub member: Box<str>,
}

/// An attribute argument a framework says is a catalog key.
///
/// The class carrying the attribute is what the key names, so there is nothing else to record.
#[derive(Clone, PartialEq, Eq, Debug)]
struct AttributeKeyRule {
    /// The attribute's name. Matched against the last segment of the one written, which is how an
    /// attribute is written -- resolving it properly would mean running the def-map over every
    /// attribute in the project during ingest.
    attribute: Box<str>,
    argument: RuleArgument,
    catalog: Box<str>,
}

/// A catalog key a framework says a declaration spells out of two of its own arguments.
///
/// This crate's own spelling of `phrost_framework`'s rule, for the reason every contract here has
/// one: it arrives as strings from above and nothing in it may depend on how a file was parsed.
#[derive(Clone, PartialEq, Eq, Debug)]
struct DerivedKeyRule {
    attribute: Box<str>,
    /// The argument whose array entries each derive one key.
    keys_of: RuleArgument,
    /// The argument whose value every derived key is named after.
    named_after: RuleArgument,
    /// The spelling, with `{name}` and `{key}` filled in.
    spelling: Box<str>,
    /// Rewrites applied to `{key}`, in order.
    rewrite: Vec<(Box<str>, Box<str>)>,
    catalog: Box<str>,
}

impl DerivedKeyRule {
    /// The key this rule spells for one entity id and one entry key.
    fn spell(&self, name: &str, key: &str) -> Box<str> {
        let mut key = key.to_owned();
        for (from, to) in &self.rewrite {
            if key.contains(&**from) {
                key = key.replace(&**from, to);
            }
        }
        self.spelling
            .replace("{name}", name)
            .replace("{key}", &key)
            .into()
    }
}

/// A catalog key a framework says is carried in a declaration's own name.
#[derive(Clone, PartialEq, Eq, Debug)]
struct DerivedNameRule {
    /// The catalog whose keys are the prefixes a name may begin with.
    prefixed_by: Box<str>,
    /// A literal prefix that counts too, separator included. Empty for none.
    or_prefix: Box<str>,
    catalog: Box<str>,
}

/// Which argument of an attribute a rule speaks for.
///
/// This crate's own spelling of the same two cases the lowered literal carries, for the reason
/// everything else here has one: a contract arrives as strings from above and nothing in it may
/// depend on how a file was parsed.
#[derive(Clone, PartialEq, Eq, Debug)]
enum RuleArgument {
    /// `id:`, compared the way PHP compares an argument name.
    Named(Box<str>),
    /// Counted from zero, for an attribute whose id the call writes without naming it.
    At(u16),
}

impl RuleArgument {
    /// Whether a literal written at `slot` is the argument this rule names.
    fn matches(&self, slot: &AttributeArgument) -> bool {
        match (self, slot) {
            (Self::Named(wanted), AttributeArgument::Named(written)) => {
                wanted.eq_ignore_ascii_case(written.as_str())
            }
            (Self::At(wanted), AttributeArgument::At(written)) => wanted == written,
            _ => false,
        }
    }
}

/// A method whose first argument decides what it returns.
///
/// `\Drupal::service('application.manager')` returns whatever that id was registered as, and the
/// declaration can only say `mixed`. The binding says which table the argument indexes; the table
/// says what the answer is.
///
/// Which argument is the key is part of the rule, because it is the callee's business: nearly every
/// method that looks a value up by a string puts the string first, and
/// `AccessResult::allowedIfHasPermission($account, $permission)` does not. The body layer keeps every
/// literal a call wrote along with its position -- see `PathStep::MethodReturn` -- so this is a fact
/// What one method hands the callback it is given.
///
/// The stored form of the framework seam's `InvokedCallback`, keyed by method name the way every other
/// contract here is. `parameters` keeps its holes: a position the framework does not name is a
/// `None` that holds its place, because `extend`'s container is second and dropping the unnamed
/// service before it would move the container onto the service.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InvokedCallbackRow {
    /// Fully qualified, without a leading separator. Anything below it is covered.
    pub class: Box<str>,
    /// Which of the callee's parameters the callback fills. See
    /// the seam's `InvokedCallback::argument` for why it is a parameter and not a position.
    pub argument: usize,
    /// What the callback's parameters receive, in written order.
    pub parameters: Box<[Option<Box<str>>]>,
}

/// to state rather than one to work around.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgumentBinding {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// Which argument is the key, counted from zero.
    pub at: u16,
    /// What the method appends to the argument before looking the key up. Empty for none.
    ///
    /// The framework's own field, restated here for the reason `AttributeArgument` is: the store is
    /// below the seam. See `phrost_framework`'s `ArgumentBinding::suffix` for why the method rather
    /// than the framework decides it.
    pub suffix: Box<str>,
    /// Which catalog the argument is a key into.
    pub catalog: Box<str>,
}

/// A property read through the method of the same name. The seam's `MethodBackedProperty`, restated
/// below it for the reason [`FunctionArgument`] is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MethodProperty {
    pub class: Box<str>,
    pub returns: Box<str>,
    pub through: Box<str>,
    /// The suffix of the attribute counting it -- see the seam's `MethodBackedProperty`.
    pub counted: Option<Box<str>>,
    /// Relations read through a key in the model's row, and that key's suffix -- see the seam's
    /// `KeyedRelation`.
    pub keyed: Box<[(Box<str>, Box<str>)]>,
}

/// The framework seam's `NamedTemplateArgument`, restated below it -- see it for each field.
#[derive(Clone, Debug)]
pub struct NamedArgumentRule {
    pub class: Box<str>,
    pub template: Box<str>,
    pub from_property: Option<Box<str>>,
    pub unless: Vec<Box<str>>,
    pub app_directory: Box<str>,
    pub default_namespace: Box<str>,
    pub names: fn(&str, &str) -> Vec<String>,
}

/// The framework seam's `NamedAfterCaller`, restated below it -- see it for each field.
#[derive(Clone, Debug)]
pub struct NamedAfterRule {
    pub class: Box<str>,
    pub method: Box<str>,
    pub unless: Vec<Box<str>>,
    pub app_directory: Box<str>,
    pub default_namespace: Box<str>,
    pub name: fn(&str, &str) -> String,
}

/// A bare generic return the framework fills from the receiver. The seam's `FilledReturn`,
/// restated below it for the reason [`FunctionArgument`] is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilledReturnRule {
    pub class: Box<str>,
    pub arguments: Box<[FilledArgumentRule]>,
}

/// One argument of a [`FilledReturnRule`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FilledArgumentRule {
    Builtin(phrost_model::BuiltinTy),
    /// A template of [`FilledReturnRule::class`], by name.
    Template(Box<str>),
}

/// A method returning the class one fixed catalog key names. The seam's `KeyedReturn`, restated
/// below it for the reason [`FunctionArgument`] is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyedReturnRule {
    pub class: Box<str>,
    pub catalog: Box<str>,
    pub key: Box<str>,
    pub nullable: bool,
}

/// A property read through the accessor its name spells. The seam's `AccessorProperty`, restated
/// below it for the reason [`FunctionArgument`] is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AccessorRule {
    pub class: Box<str>,
    pub prefix: Box<str>,
    pub suffix: Box<str>,
    pub attribute: Box<str>,
}

/// A method `__call` answers for every name with a prefix, with the receiver. The seam's
/// `FluentPrefix`, restated below it for the reason [`FunctionArgument`] is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FluentPrefixRule {
    pub class: Box<str>,
    pub prefix: Box<str>,
}

impl FluentPrefixRule {
    /// Whether `name` is one of the names this answers: the prefix, then an upper-case letter.
    pub fn spells(&self, name: &str) -> bool {
        name.len() > self.prefix.len()
            && name.is_char_boundary(self.prefix.len())
            && name[..self.prefix.len()].eq_ignore_ascii_case(&self.prefix)
            && name[self.prefix.len()..]
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
    }
}

/// A method called without the prefix it is declared under. The seam's `PrefixedMethod`, restated
/// below it for the reason [`FunctionArgument`] is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrefixedMethodRule {
    pub class: Box<str>,
    pub prefix: Box<str>,
    pub builder: Box<str>,
    pub marked: bool,
}

/// Which argument of a free function is a key, and into which catalog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FunctionArgument {
    pub at: u16,
    pub catalog: Box<str>,
}

/// How many of the catalog's class a forwarded method gives back.
///
/// The framework's own `Forwarded`, restated here because this crate cannot see that one -- the same
/// arrangement `AttributeArgument` has, and for the same reason: the store is below the seam and the
/// mapping happens where both are visible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forwarded {
    /// One of it, keeping whatever the declaration said about null.
    Instance,
    /// An array of it.
    ManyOf,
}

/// A method whose return class was decided one call earlier.
///
/// `$storage->load($id)` after `getStorage('node')`: nothing at the `load` call names the entity,
/// and the receiver's class cannot either, because one storage class serves many entity types.
///
/// Only the class is taken from the catalog. Whether the result can be null, and whether there is
/// one of it, stay whatever the declaration said -- an id matching no row really does return null.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ForwardedBinding {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// The method that produced the receiver, whose first argument is the key.
    pub through: Box<str>,
    /// Which catalog that key is a key into.
    pub catalog: Box<str>,
    /// Whether the method gives back one of the catalog's class or an array of them.
    pub yields: Forwarded,
}

/// A call whose literal first argument is a key, read by a method one call later.
///
/// What [`ForwardedBinding`] says, read from the other end: the id in `getStorage('node')` is a key
/// whether or not anything is ever loaded from the storage it returns, so a report on it needs no
/// second call to look at.
///
/// [`Self::returns`] is what keeps that honest. `getStorage` is a name a project may declare for
/// something else entirely, and a name alone would make every one of them a catalog key -- the
/// "sometimes a key" mistake a closed catalog turns into an error on correct code. The call has to
/// hand back the class whose methods forward the key, which is a fact its own signature states.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ForwardedKey {
    /// The class or interface the call must return for its argument to be a key. Anything below it
    /// counts, because an override inherits the contract along with the signature.
    pub returns: Box<str>,
    /// Which catalog that argument is a key into.
    pub catalog: Box<str>,
}

/// A property a class answers through `__get`, which no declaration writes.
///
/// The property counterpart of [`ArgumentBinding`]: that one reads a string a call was given, this
/// one reads the name the property was spelled with. Both say what comes back, and both are needed
/// because the same read has two spellings.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PropertyBinding {
    /// The class or interface whose accessor answers. Anything below it is covered.
    pub class: Box<str>,
    /// Which property names this speaks for.
    pub name: PropertyNames,
    /// What reading one gives back, fully qualified.
    pub holds: Box<str>,
    /// Whether reading it can give null.
    pub nullable: bool,
}

/// A property whose type is looked up per name, in a table the project states.
///
/// The other half of [`PropertyBinding`], and the half it cannot be: that one holds one class for
/// every name its accessor answers to, and here every name holds something different. What settles
/// which table to read is a key written on an earlier link of the chain -- the same lookback
/// [`ForwardedKey`] uses.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TabledBinding {
    /// The class or interface whose accessor answers. Anything below it is covered.
    pub class: Box<str>,
    /// The method that produced the receiver. Its first argument is the key.
    pub through: Box<str>,
    /// Which catalog that key belongs to.
    pub catalog: Box<str>,
    /// The method handing back one entry per row, each holding every property. `None` for a class
    /// with no such method.
    pub rows: Option<Box<str>>,
}

/// One complete table, for the pass that interns the shape it describes.
///
/// A name because the tuple is three deep, and it is owned rather than borrowed because the caller
/// holds it across a mutable borrow of the type table -- see [`ReturnContracts::complete_tables`].
pub type CompleteTable = (Box<str>, Box<str>, Vec<(Box<str>, Option<BuiltinTy>)>);

/// What one catalog key's values are called and what each holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PropertyRows {
    /// One entry per property, in the order the declaration writes them. `None` where nothing can
    /// be said about what is in it -- the property is still *there*, which is the fact a shape
    /// needs.
    pub properties: Vec<(Box<str>, Option<BuiltinTy>)>,
    /// Whether these are all of them. Only a complete table may become an `array{...}`: a shape
    /// claims that the names it lists are the names there are.
    pub complete: bool,
}

/// Which property names a [`PropertyBinding`] speaks for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PropertyNames {
    /// One name, which the framework's own source writes.
    Exactly(Box<str>),
    /// Every key of a catalog, which the project's configuration fills.
    KeyOf(Box<str>),
}

/// One class's promise that a method never hands back a sentinel its declaration admits.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Unproduced {
    class: Box<str>,
    /// Which sentinel: `null` for a framework's non-null contract, `false` for a standard-library
    /// call whose failure branch a configuration decides.
    sentinel: BuiltinTy,
    /// The state the sentinel *is* produced in, for a contract that hides a real one.
    ///
    /// `None` where no run produces it at all, which is a proof and has nothing to disclose. Kept
    /// here rather than left in the provider because the message that discloses it is written three
    /// crates away, and a reason that lives only in a doc comment cannot be shown to anybody.
    only_when: Option<Box<str>>,
    /// The argument position whose presence the state needs, where the call can be read for it.
    ///
    /// `None` where the state is about the object rather than the call -- a file with no URI yet,
    /// a request stack nothing pushed to, a connection put in a silent error mode. Where it is
    /// `Some`, a call with no argument there is not in the state and has nothing to disclose.
    needs_argument: Option<u16>,
}

impl ReturnContracts {
    /// Records that `class::method` never returns the `sentinel` its type admits.
    ///
    /// `only_when` is the state a real sentinel would come from, and `None` says there is no such
    /// state -- see [`Unproduced::only_when`].
    pub fn add_unproduced(
        &mut self,
        class: &str,
        method: &str,
        sentinel: BuiltinTy,
        only_when: Option<&str>,
        needs_argument: Option<u16>,
    ) {
        let declarers = entry(&mut self.unproduced, method);
        let row = Unproduced {
            class: class.into(),
            sentinel,
            only_when: only_when.map(Box::from),
            needs_argument,
        };
        if !declarers.contains(&row) {
            declarers.push(row);
        }
    }

    /// The classes declaring that `method` never produces `sentinel`, empty when none do.
    ///
    /// The caller decides whether the receiver is one of them or below one -- the hierarchy is the
    /// view's to walk, and this crate has no opinion about conformance.
    ///
    /// An iterator rather than a slice: one list holds every sentinel so that a rule about them has
    /// one place to live, and filtering it must not allocate on a path every member access walks.
    pub fn unproduced_declarers(
        &self,
        method: &str,
        sentinel: BuiltinTy,
    ) -> impl Iterator<Item = &str> {
        find(&self.unproduced, method)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter(move |row| row.sentinel == sentinel)
            .map(|row| row.class.as_ref())
    }

    /// The same list, with the state each hides a real sentinel in.
    ///
    /// Separate from [`Self::unproduced_declarers`] rather than replacing it, because the two are
    /// asked on different paths: that one runs on every member access and wants no second field to
    /// carry, and this one runs only when a project asks to be told what is being decided for it.
    pub fn disclosed_declarers(
        &self,
        method: &str,
        sentinel: BuiltinTy,
    ) -> impl Iterator<Item = (&str, &str, Option<u16>)> {
        find(&self.unproduced, method)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter(move |row| row.sentinel == sentinel)
            .filter_map(|row| {
                Some((
                    row.class.as_ref(),
                    row.only_when.as_deref()?,
                    row.needs_argument,
                ))
            })
    }

    /// Records that `class::method`'s declared union is settled by an earlier `after` on the same
    /// receiver.
    pub fn add_chained(
        &mut self,
        class: &str,
        method: &str,
        after: &str,
        then: phrost_model::BuiltinTy,
        otherwise: phrost_model::BuiltinTy,
    ) {
        entry(&mut self.chained, method).push(ChainedBranch {
            class: class.into(),
            after: after.into(),
            then,
            otherwise,
        });
    }

    /// The chain contracts for `method`, empty when none.
    ///
    /// Keyed by method name so the commonest answer -- there is no contract -- costs one scan of a
    /// handful of short strings. Which class the receiver is, and whether the switching call was
    /// made, are the caller's: the hierarchy and the chain are the view's to walk and this crate
    /// has no opinion about either.
    pub fn chained_branches(&self, method: &str) -> &[ChainedBranch] {
        find(&self.chained, method).map_or(&[], Vec::as_slice)
    }

    /// Records that `class::method`'s union of a collection and its element is settled by an
    /// earlier call -- see [`ChainedOperandRule`].
    pub fn add_chained_operand(&mut self, method: &str, rule: ChainedOperandRule) {
        let rules = entry(&mut self.chained_operands, method);
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }

    /// The collection-or-element contracts for `method`, empty when none.
    pub fn chained_operands(&self, method: &str) -> &[ChainedOperandRule] {
        find(&self.chained_operands, method).map_or(&[], Vec::as_slice)
    }

    /// Records that `class::method`'s declared union collapses to `then` when the call writes no
    /// argument at `unless_given`.
    pub fn add_defaulted(&mut self, class: &str, method: &str, unless_given: u16, then: &str) {
        let branches = entry(&mut self.defaulted, method);
        let branch = DefaultBranch {
            class: class.into(),
            unless_given,
            then: then.into(),
        };
        if !branches.contains(&branch) {
            branches.push(branch);
        }
    }

    /// The default-branch contracts for `method`, empty when none.
    ///
    /// Keyed by method name for the reason [`Self::chained_branches`] is: the commonest answer is
    /// that there is no contract, and it costs one scan of a handful of short strings. Whether the
    /// receiver is one of the declaring classes, and whether the declaration really lists the branch,
    /// are both the caller's -- this crate holds no hierarchy and no types.
    pub fn defaulted_branches(&self, method: &str) -> &[DefaultBranch] {
        find(&self.defaulted, method).map_or(&[], Vec::as_slice)
    }

    /// Records that `class::method` returns the class its argument at `argument` names.
    pub fn add_argument_class(&mut self, class: &str, method: &str, argument: u16) {
        let rules = entry(&mut self.argument_class, method);
        let rule = ArgumentClass {
            class: class.into(),
            argument,
        };
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }

    /// The argument-names-the-class rules for `method`, empty when none.
    ///
    /// Keyed by method name for the reason [`Self::chained_branches`] is: the commonest answer is
    /// that there is no contract, and it costs one scan of a handful of short strings. Whether the
    /// receiver is one of the declaring classes, and whether the declaration is vague enough for
    /// this to be allowed to speak, are both the caller's -- this crate holds no hierarchy and no
    /// types.
    pub fn argument_class_returns(&self, method: &str) -> &[ArgumentClass] {
        find(&self.argument_class, method).map_or(&[], Vec::as_slice)
    }

    /// Records how a framework writes "what documents this is written on another declaration".
    ///
    /// `{@inheritdoc}` is PHP's own way of saying it and needs no contract. A framework can have a
    /// second: Drupal's coding standard writes `Implements hook_form_alter().` and nothing more,
    /// because what the parameters mean is documented on the hook rather than on each of the hundreds
    /// of implementations -- and a rule demanding `@param` there reports the standard as a defect.
    pub fn add_documented_elsewhere(
        &mut self,
        lead: &str,
        names: &str,
        implementers: Option<&str>,
        dispatched: Option<&str>,
    ) {
        let rule = DocumentedByRule {
            lead: lead.into(),
            names: names.into(),
            implementers: implementers.map(Into::into),
            dispatched: dispatched.map(Into::into),
        };
        if !self.documented_elsewhere.contains(&rule) {
            self.documented_elsewhere.push(rule);
        }
    }

    /// Records that a framework shows the docblock of a declaration named like this to a person.
    ///
    /// Drupal's update descriptions: what `update.php` prints beside a pending update is that
    /// function's whole docblock, so the tags a documentation rule would ask for become words in a
    /// sentence somebody reads. Required *and* closed to tags, which is why it is not a second
    /// spelling of `add_documented_elsewhere`.
    pub fn add_user_facing_docblock(&mut self, names: &str, numbered: bool) {
        let rule = UserFacingDocblockRule {
            names: names.into(),
            numbered,
        };
        if !self.user_facing_docblocks.contains(&rule) {
            self.user_facing_docblocks.push(rule);
        }
    }

    /// Whether some framework shows the docblock of a declaration called this to a person.
    ///
    /// Empty by default, so a project with no framework pays one emptiness check per declaration a
    /// documentation rule looks at.
    pub fn user_facing_docblock(&self, name: &str) -> bool {
        self.user_facing_docblocks
            .iter()
            .any(|rule| rule.covers(name))
    }

    /// Records that a call to `method` declares an extension point by one of its arguments.
    ///
    /// The rule arrives whole rather than as six parameters, and that is not tidiness: four of them
    /// are strings, two of those are a prefix and a suffix around the same name, and a call site
    /// that swapped them would compile and fill a catalog no reader can index into.
    pub fn add_dispatcher(&mut self, method: &str, rule: DispatchRule) {
        let rules = entry(&mut self.dispatchers, method);
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }

    /// The dispatch rules for `method`, empty when no framework named it.
    pub fn dispatch_rules(&self, method: &str) -> &[DispatchRule] {
        find(&self.dispatchers, method).map_or(&[], Vec::as_slice)
    }

    /// Whether any framework named a dispatching method at all.
    ///
    /// The gate that keeps ingest from walking a file's call sites when nothing would read one,
    /// which is every project with no framework detected -- and the reason indexing a project that
    /// dispatches nothing costs one emptiness check per file.
    pub fn names_any_dispatcher(&self) -> bool {
        !self.dispatchers.is_empty()
    }

    /// Records that a call declares `key` of `catalog`.
    pub fn claim_dispatch(&mut self, catalog: &str, key: &str, call: Dispatch) {
        let calls = self
            .dispatched
            .entry(catalog.into())
            .or_default()
            .entry(key.into())
            .or_default();
        if !calls.contains(&call) {
            calls.push(call);
        }
    }

    /// Withdraws every dispatch a file made of one key, and the key with it when nothing else
    /// dispatches it.
    pub fn withdraw_dispatch(&mut self, file: FileId, catalog: &str, key: &str) {
        let Some(keys) = self.dispatched.get_mut(catalog) else {
            return;
        };
        let Some(calls) = keys.get_mut(key) else {
            return;
        };
        calls.retain(|call| call.file != file);
        if calls.is_empty() {
            keys.remove(key);
        }
    }

    /// Every call that dispatches `key`, in the order they were claimed.
    ///
    /// Ungated: whether a call really is a dispatch depends on what its receiver turns out to be,
    /// and that is a hierarchy question this crate cannot answer. A caller that reports on the
    /// answer must apply [`Dispatch::required`] first.
    pub fn dispatches(&self, catalog: &str, key: &str) -> &[Dispatch] {
        self.dispatched
            .get(catalog)
            .and_then(|keys| keys.get(key))
            .map_or(&[], Vec::as_slice)
    }

    /// Every dispatched key and the calls claiming it, in a stable order.
    ///
    /// For the index dump, which is what `incremental == from-scratch` compares -- a dispatch a
    /// re-ingested file failed to withdraw is invisible without it.
    pub fn dispatched_keys(&self) -> Vec<(&str, &str, Vec<&Dispatch>)> {
        let mut rows = self
            .dispatched
            .iter()
            .flat_map(|(catalog, keys)| {
                keys.iter().map(move |(key, calls)| {
                    let mut calls = calls.iter().collect::<Vec<_>>();
                    calls.sort_by_key(|call| (call.file, call.range.start()));
                    (&**catalog, &**key, calls)
                })
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|(catalog, key, _)| (*catalog, *key));
        rows
    }

    /// Records that `class::method` returns whatever the argument at `at` names in `catalog`.
    ///
    /// `suffix` is what the method appends before looking the key up, empty for the usual case; it is
    /// a parameter of the one constructor rather than a second constructor beside it, because a
    /// second constructor is how one of the two comes to be missing a rule the other has.
    pub fn bind_argument(
        &mut self,
        class: &str,
        method: &str,
        at: u16,
        suffix: &str,
        catalog: &str,
    ) {
        let bindings = entry(&mut self.from_argument, method);
        let binding = ArgumentBinding {
            class: class.into(),
            at,
            suffix: suffix.into(),
            catalog: catalog.into(),
        };
        if !bindings.contains(&binding) {
            bindings.push(binding);
        }
    }

    /// The bindings for `method`, empty when it has none.
    pub fn argument_bindings(&self, method: &str) -> &[ArgumentBinding] {
        find(&self.from_argument, method).map_or(&[], Vec::as_slice)
    }

    /// Records that the free function `function` returns whatever its argument at `at` names in
    /// `catalog`.
    pub fn bind_function_argument(&mut self, function: &str, at: u16, catalog: &str) {
        let binding = FunctionArgument {
            at,
            catalog: catalog.into(),
        };
        match self
            .from_function_argument
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(function))
        {
            Some((_, existing)) => *existing = binding,
            None => self.from_function_argument.push((function.into(), binding)),
        }
    }

    /// The binding for the free function `function`, fully qualified.
    /// Records that `function` renders the template its argument `name_at` names, handing it the
    /// variables of the array at `data_at` -- and, where it `inherits`, every variable where the
    /// call is besides.
    pub fn bind_rendered_template(
        &mut self,
        function: &str,
        name_at: u16,
        data_at: u16,
        inherits: bool,
    ) {
        let function = function.trim_start_matches('\\');
        self.rendered
            .retain(|(known, ..)| !known.eq_ignore_ascii_case(function));
        self.rendered
            .push((function.into(), name_at, data_at, inherits));
    }

    /// Records that `class::method($name, $value)` hands every template the variable `$name` -- and
    /// so does `helper()->method(...)`, when a function called with nothing hands back the instance.
    pub fn bind_shared_template_data(&mut self, class: &str, method: &str, helper: Option<&str>) {
        let rule = SharedTemplateRule {
            class: class.trim_start_matches('\\').into(),
            method: method.into(),
            helper: helper.map(Box::from),
        };
        if !self.shared_template_data.contains(&rule) {
            self.shared_template_data.push(rule);
        }
    }

    /// Records that `function(name, key: value, ...)` renders the template `prefix` + `name` with
    /// those variables -- a component tag as a template reader writes it.
    pub fn bind_component_call(&mut self, function: &str, prefix: &str) {
        let rule = (function.into(), prefix.into());
        if !self.component_calls.contains(&rule) {
            self.component_calls.push(rule);
        }
    }

    /// Records that a class descending from `class` is the `$this` of the template it renders, and
    /// that one declared under `namespace` with no say renders the one its name spells below
    /// `templates`.
    pub fn bind_rendering_class(&mut self, class: &str, namespace: &str, templates: &str) {
        let rule = (
            class.trim_start_matches('\\').into(),
            namespace.trim_matches('\\').into(),
            templates.into(),
        );
        if !self.rendering_classes.contains(&rule) {
            self.rendering_classes.push(rule);
        }
    }

    /// Records a call naming the class a callee's bound closures run as.
    pub fn bind_configured_this(&mut self, rule: crate::ConfiguredThisRule) {
        if !self.configured_this.contains(&rule) {
            self.configured_this.push(rule);
        }
    }

    /// The calls naming the class a callee's bound closures run as.
    pub fn configured_this(&self) -> &[crate::ConfiguredThisRule] {
        &self.configured_this
    }

    /// Records a class declaring, in a property's string, the inputs a method reads by name.
    pub fn bind_declared_inputs(&mut self, rule: crate::DeclaredInputsRule) {
        self.declared_inputs.push(rule);
    }

    /// The classes declaring inputs in a property's string.
    pub fn declared_inputs(&self) -> &[crate::DeclaredInputsRule] {
        &self.declared_inputs
    }

    /// Records a static method handing back the class named after the one it was called on.
    pub fn bind_named_after(&mut self, rule: NamedAfterRule) {
        self.named_after.push(rule);
    }

    /// The static methods handing back a class named after their caller.
    pub fn named_after(&self) -> &[NamedAfterRule] {
        &self.named_after
    }

    /// Records a template a subclass fills by its own name where it writes nothing.
    pub fn bind_named_argument(&mut self, rule: NamedArgumentRule) {
        self.named_arguments.push(rule);
    }

    /// The templates subclasses fill by their own names.
    pub fn named_arguments(&self) -> &[NamedArgumentRule] {
        &self.named_arguments
    }

    /// Records how a framework's migrations write a table, and which classes are its rows.
    pub fn bind_written_schema(&mut self, rule: crate::schema::WrittenSchemaRule) {
        self.written_schema.push(rule);
    }

    /// How the project's migrations write its tables -- see [`crate::schema`].
    pub fn written_schema(&self) -> &[crate::schema::WrittenSchemaRule] {
        &self.written_schema
    }

    /// The classes whose instance renders their own template, as `(class, namespace, templates)`.
    pub fn rendering_classes(&self) -> &[(Box<str>, Box<str>, Box<str>)] {
        &self.rendering_classes
    }

    /// The calls a component tag is written as, as `(function, prefix)`.
    pub fn component_calls(&self) -> &[(Box<str>, Box<str>)] {
        &self.component_calls
    }

    /// The calls handing every template a variable.
    pub fn shared_template_data(&self) -> &[SharedTemplateRule] {
        &self.shared_template_data
    }

    /// Records an array that renders the template it names.
    pub fn bind_render_array(&mut self, rule: crate::renders::RenderArrayRule) {
        if !self.render_arrays.contains(&rule) {
            self.render_arrays.push(rule);
        }
    }

    /// Every array that renders the template it names, as the frameworks stated them.
    pub fn render_arrays(&self) -> &[crate::renders::RenderArrayRule] {
        &self.render_arrays
    }

    /// The functions that render a template, as `(function, name_at, data_at, inherits)`.
    pub fn rendered_templates(&self) -> &[(Box<str>, u16, u16, bool)] {
        &self.rendered
    }

    /// Records that a literal key `catalog` does not list holds `holds`, unless it contains
    /// `outside`.
    pub fn bind_unlisted_key(
        &mut self,
        catalog: &str,
        holds: phrost_model::BuiltinTy,
        outside: &str,
    ) {
        self.unlisted.retain(|(known, _, _)| **known != *catalog);
        self.unlisted.push((catalog.into(), holds, outside.into()));
    }

    /// What `key` holds in `catalog` where the catalog does not list it, when the catalog says.
    pub fn unlisted_shape(&self, catalog: &str, key: &str) -> Option<phrost_model::BuiltinTy> {
        self.unlisted
            .iter()
            .find(|(known, _, _)| **known == *catalog)
            .filter(|(_, _, outside)| outside.is_empty() || !key.contains(&**outside))
            .map(|&(_, holds, _)| holds)
    }

    pub fn function_argument_binding(&self, function: &str) -> Option<&FunctionArgument> {
        let function = function.strip_prefix('\\').unwrap_or(function);
        self.from_function_argument
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(function))
            .map(|(_, binding)| binding)
    }

    /// Records that `class::method` calls the callback at `argument` with `parameters`.
    ///
    /// One row per argument position, because a method taking its callback in two places is two
    /// claims -- see the framework seam's `InvokedCallback::argument`.
    pub fn add_invoked_callback(
        &mut self,
        class: &str,
        method: &str,
        argument: usize,
        parameters: &[Option<&'static str>],
    ) {
        let rows = entry(&mut self.invokes_callback, method);
        let row = InvokedCallbackRow {
            class: class.into(),
            argument,
            parameters: parameters
                .iter()
                .map(|named| named.map(Box::<str>::from))
                .collect(),
        };
        if !rows.contains(&row) {
            rows.push(row);
        }
    }

    /// What `method` hands the callbacks it is given, empty when it hands none.
    pub fn invoked_callbacks(&self, method: &str) -> &[InvokedCallbackRow] {
        find(&self.invokes_callback, method).map_or(&[], Vec::as_slice)
    }

    /// Records that `class::method` returns whatever `through`'s first argument named in `catalog`.
    pub fn forward_argument(
        &mut self,
        class: &str,
        method: &str,
        through: &str,
        catalog: &str,
        yields: Forwarded,
    ) {
        let bindings = entry(&mut self.through_argument, method);
        let binding = ForwardedBinding {
            class: class.into(),
            through: through.into(),
            catalog: catalog.into(),
            yields,
        };
        if !bindings.contains(&binding) {
            bindings.push(binding);
        }

        // The same rule, indexed by the call that writes the key. Built here rather than derived on
        // demand so that one rule cannot reach one index and not the other.
        let keys = entry(&mut self.keyed_by_call, through);
        let key = ForwardedKey {
            returns: class.into(),
            catalog: catalog.into(),
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    /// The forwarded bindings for `method`, empty when it has none.
    pub fn forwarded_bindings(&self, method: &str) -> &[ForwardedBinding] {
        find(&self.through_argument, method).map_or(&[], Vec::as_slice)
    }

    /// The catalogs a literal first argument to `method` is a key of, empty when it writes none.
    ///
    /// Several rules can name one call -- five methods forward what `getStorage` was asked for --
    /// and they collapse to the catalogs and the classes involved, because that is all a reader of
    /// the key needs. A duplicate here would be a second report on one literal.
    pub fn forwarded_keys(&self, method: &str) -> &[ForwardedKey] {
        find(&self.keyed_by_call, method).map_or(&[], Vec::as_slice)
    }

    /// Records that `class` answers a property through `__get`, and what one holds.
    pub fn bind_property(&mut self, class: &str, name: PropertyNames, holds: &str, nullable: bool) {
        let binding = PropertyBinding {
            class: class.into(),
            name,
            holds: holds.into(),
            nullable,
        };
        if !self.magic_properties.contains(&binding) {
            self.magic_properties.push(binding);
        }
    }

    /// Records that a property `class` does not declare reads as the method of the same name, then
    /// `through` on its result, where that method returns a `returns`.
    pub fn bind_method_property(
        &mut self,
        class: &str,
        returns: &str,
        through: &str,
        counted: Option<&str>,
        keyed: &[(&str, &str)],
    ) {
        let binding = MethodProperty {
            class: class.into(),
            returns: returns.into(),
            through: through.into(),
            counted: counted.map(Into::into),
            keyed: keyed
                .iter()
                .map(|&(relation, suffix)| (relation.into(), suffix.into()))
                .collect(),
        };
        if !self.method_properties.contains(&binding) {
            self.method_properties.push(binding);
        }
    }

    /// Records that a method declared `prefix` + `Name` on a class below `class` is called `name`,
    /// on that class and on a `builder` for it.
    /// `marked` when a method carrying the rule's marker attribute is called the same way under
    /// its own name -- lowering sets [`phrost_sema::MemberModifiers::is_marked`] on it.
    pub fn bind_prefixed_method(&mut self, class: &str, prefix: &str, builder: &str, marked: bool) {
        let rule = PrefixedMethodRule {
            class: class.into(),
            prefix: prefix.into(),
            builder: builder.into(),
            marked,
        };
        if !self.prefixed_methods.contains(&rule) {
            self.prefixed_methods.push(rule);
        }
    }

    /// Every prefixed-method rule, empty when no framework contributed one.
    pub fn prefixed_methods(&self) -> &[PrefixedMethodRule] {
        &self.prefixed_methods
    }

    /// Every method-backed property rule, empty when no framework contributed one.
    pub fn method_properties(&self) -> &[MethodProperty] {
        &self.method_properties
    }

    /// Records that a property on a class below `class` is read through the accessor its name
    /// spells: `prefix` + studly name + `suffix`, or the camel-cased name returning `attribute`.
    pub fn bind_accessor(&mut self, class: &str, prefix: &str, suffix: &str, attribute: &str) {
        let rule = AccessorRule {
            class: class.into(),
            prefix: prefix.into(),
            suffix: suffix.into(),
            attribute: attribute.into(),
        };
        if !self.accessors.contains(&rule) {
            self.accessors.push(rule);
        }
    }

    pub fn accessors(&self) -> &[AccessorRule] {
        &self.accessors
    }

    /// Records that a class's `__call` answers every name beginning `prefix`, with the receiver.
    pub fn bind_fluent_prefix(&mut self, class: &str, prefix: &str) {
        let rule = FluentPrefixRule {
            class: class.into(),
            prefix: prefix.into(),
        };
        if !self.fluent_prefixes.contains(&rule) {
            self.fluent_prefixes.push(rule);
        }
    }

    pub fn fluent_prefixes(&self) -> &[FluentPrefixRule] {
        &self.fluent_prefixes
    }

    /// Records that `method($name, $closure)` on a class using `provider` gives the class a method
    /// called `$name`.
    pub fn bind_macro_registrar(&mut self, provider: &str, method: &str) {
        let rule = (
            provider.trim_start_matches('\\').into(),
            Box::<str>::from(method),
        );
        if !self.macro_registrars.contains(&rule) {
            self.macro_registrars.push(rule);
        }
    }

    /// The static methods that register a macro, by name.
    pub fn macro_registrars(&self) -> impl Iterator<Item = &str> {
        self.macro_registrars.iter().map(|(_, method)| &**method)
    }

    /// The traits and classes whose users take macros.
    pub fn macro_providers(&self) -> impl Iterator<Item = &str> {
        self.macro_registrars
            .iter()
            .map(|(provider, _)| &**provider)
    }

    /// Records that `class::method`'s bare generic return holds the arguments `rule` states.
    pub fn bind_filled_return(&mut self, method: &str, rule: FilledReturnRule) {
        let rules = entry(&mut self.filled_returns, method);
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }

    /// The filled-return contracts for `method`, empty when none.
    pub fn filled_returns(&self, method: &str) -> &[FilledReturnRule] {
        find(&self.filled_returns, method).map_or(&[], Vec::as_slice)
    }

    /// Records that `class::method` returns the class `key` of `catalog` names.
    pub fn bind_keyed_return(&mut self, method: &str, rule: KeyedReturnRule) {
        let rules = entry(&mut self.keyed_returns, method);
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }

    /// The keyed-return contracts for `method`, empty when none.
    pub fn keyed_returns(&self, method: &str) -> &[KeyedReturnRule] {
        find(&self.keyed_returns, method).map_or(&[], Vec::as_slice)
    }

    /// Records the names a class writes the properties `__get` answers for it under.
    pub fn bind_stated_properties(&mut self, rule: crate::StatedPropertiesRule) {
        if !self.stated.contains(&rule) {
            self.stated.push(rule);
        }
    }

    pub fn stated_property_rules(&self) -> &[crate::StatedPropertiesRule] {
        &self.stated
    }

    /// Records that `class` behaves as though it wrote `@mixin` for `mixin`, with `argument` the type
    /// of the tag when it was parameterised.
    pub fn add_implied_mixin(
        &mut self,
        class: &str,
        mixin: phrost_sema::ClassRef,
        argument: Option<phrost_ty::TypeId>,
    ) {
        let (mixins, arguments) = self
            .implied_mixins
            .entry(class.to_ascii_lowercase().into())
            .or_default();
        if !mixins.contains(&mixin) {
            mixins.push(mixin);
            arguments.extend(argument);
        }
    }

    /// The `@mixin` tags a framework says `class` behaves as though it wrote, and their types.
    pub fn implied_mixins_of(
        &self,
        class: &str,
    ) -> (&[phrost_sema::ClassRef], &[phrost_ty::TypeId]) {
        if self.implied_mixins.is_empty() {
            return (&[], &[]);
        }
        self.implied_mixins
            .get(class.to_ascii_lowercase().as_str())
            .map_or((&[], &[]), |(mixins, arguments)| (mixins, arguments))
    }

    /// Records that `class`'s `__call` runs its own declaration of each of `methods`, so a scope PHP
    /// would refuse still reaches them.
    pub fn open_methods(&mut self, class: &str, methods: &[&str]) {
        let opened = self
            .opened_methods
            .entry(class.to_ascii_lowercase().into())
            .or_default();
        for method in methods {
            if !opened
                .iter()
                .any(|known| known.eq_ignore_ascii_case(method))
            {
                opened.push((*method).into());
            }
        }
    }

    /// The methods `class`'s `__call` reopens -- see [`Self::open_methods`].
    pub fn opened_methods_of(&self, class: &str) -> &[Box<str>] {
        if self.opened_methods.is_empty() {
            return &[];
        }
        self.opened_methods
            .get(class.to_ascii_lowercase().as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// Records that a method named `method` returning `X::class` makes `X` a mixin of its class.
    pub fn add_mixin_method(&mut self, method: &str) {
        if !self.mixin_methods.iter().any(|known| &**known == method) {
            self.mixin_methods.push(method.into());
        }
    }

    /// The methods [`Self::add_mixin_method`] named, for lowering to read.
    pub fn mixin_methods(&self) -> &[Box<str>] {
        &self.mixin_methods
    }

    /// Records that `class`'s `@method` tags were generated from `source`'s methods.
    pub fn add_tag_source(&mut self, class: &str, source: phrost_sema::ClassRef) {
        let sources = self
            .tag_sources
            .entry(class.to_ascii_lowercase().into())
            .or_default();
        if !sources.contains(&source) {
            sources.push(source);
        }
    }

    /// The classes `class`'s `@method` tags were generated from -- see [`Self::add_tag_source`].
    pub fn tag_sources_of(&self, class: &str) -> &[phrost_sema::ClassRef] {
        if self.tag_sources.is_empty() {
            return &[];
        }
        self.tag_sources
            .get(class.to_ascii_lowercase().as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// Every magic-property binding, empty when no framework contributed one.
    ///
    /// The whole list rather than a lookup, because a caller has to test the cheap half -- does the
    /// name match -- before the expensive half, which is whether the receiver descends from the
    /// class. Handing back rows to filter is what lets it choose that order.
    pub fn magic_properties(&self) -> &[PropertyBinding] {
        &self.magic_properties
    }

    /// Records that reading a property on `class` is a lookup in a table an earlier call named.
    pub fn bind_table(&mut self, class: &str, through: &str, catalog: &str, rows: Option<&str>) {
        let binding = TabledBinding {
            class: class.into(),
            through: through.into(),
            catalog: catalog.into(),
            rows: rows.map(Into::into),
        };
        if !self.tabled.contains(&binding) {
            self.tabled.push(binding);
        }
    }

    /// Every tabled-read rule, whole, for the reason [`Self::magic_properties`] gives.
    pub fn tabled_bindings(&self) -> &[TabledBinding] {
        &self.tabled
    }

    /// Records what one *kind* of value holds, name by name.
    pub fn add_property_table(
        &mut self,
        catalog: &str,
        kind: &str,
        properties: Vec<(Box<str>, Option<BuiltinTy>)>,
        complete: bool,
    ) {
        self.tables.entry(catalog.into()).or_default().insert(
            kind.into(),
            PropertyRows {
                properties,
                complete,
            },
        );
    }

    /// Records a declaration whose body states the properties of the kind it declares.
    pub fn add_declared_table(&mut self, rule: crate::tables::DeclaredTableRule) {
        if !self.declared_tables.contains(&rule) {
            self.declared_tables.push(rule);
        }
    }

    /// Every such rule, whole: a handful of them, scanned per declaration that carries a keyed
    /// attribute at all.
    pub fn declared_table_rules(&self) -> &[crate::tables::DeclaredTableRule] {
        &self.declared_tables
    }

    /// Takes a kind's table away again, for the file that declared it going or changing.
    pub fn withdraw_property_table(&mut self, catalog: &str, kind: &str) {
        if let Some(kinds) = self.tables.get_mut(catalog) {
            kinds.remove(kind);
        }
    }

    /// Records what kind of thing one key of `catalog` is.
    pub fn add_catalog_kind(&mut self, catalog: &str, key: &str, kind: &str) {
        self.kinds
            .entry(catalog.into())
            .or_default()
            .insert(key.into(), kind.into());
    }

    /// What kind of thing a key is, in the framework's own vocabulary.
    pub fn catalog_kind(&self, catalog: &str, key: &str) -> Option<&str> {
        self.kinds.get(catalog)?.get(key).map(|kind| &**kind)
    }

    /// Every complete table, for the pass that interns the shape each describes.
    ///
    /// Owned, because the caller has to hold the type table mutably while it reads these and a
    /// borrow of `self` would be the same borrow.
    pub fn complete_tables(&self) -> Vec<CompleteTable> {
        self.tables
            .iter()
            .flat_map(|(catalog, keys)| {
                keys.iter()
                    .filter(|(_, rows)| rows.complete)
                    .map(|(kind, rows)| (catalog.clone(), kind.clone(), rows.properties.clone()))
            })
            .collect()
    }

    /// What one key of `catalog` holds, or nothing where the project never said.
    pub fn property_table(&self, catalog: &str, key: &str) -> Option<&PropertyRows> {
        let kind = self.catalog_kind(catalog, key)?;
        self.tables.get(catalog)?.get(kind)
    }

    /// Records that an attribute's named argument is a key in `catalog`.
    pub fn name_keys_with(&mut self, attribute: &str, argument: &str, catalog: &str) {
        self.keys_with(attribute, RuleArgument::Named(argument.into()), catalog);
    }

    /// The same, for an attribute whose id the call writes without naming it.
    ///
    /// `#[FormElement('textfield')]`: the constructor names the parameter and the call does not, so
    /// there is nothing but the position to match. Kept apart from [`Self::name_keys_with`] rather
    /// than folded into it with an optional name, because the two are different facts and a rule
    /// that matched both would claim a key from any spelling.
    pub fn position_keys_with(&mut self, attribute: &str, position: u16, catalog: &str) {
        self.keys_with(attribute, RuleArgument::At(position), catalog);
    }

    fn keys_with(&mut self, attribute: &str, argument: RuleArgument, catalog: &str) {
        let rule = AttributeKeyRule {
            attribute: attribute.into(),
            argument,
            catalog: catalog.into(),
        };
        if !self.attribute_keys.contains(&rule) {
            self.attribute_keys.push(rule);
        }
    }

    /// Records that a declaration spells catalog keys out of two of its own arguments.
    pub fn derive_keys_with(
        &mut self,
        attribute: &str,
        keys_of: &str,
        named_after: &str,
        spelling: &str,
        rewrite: &[(&str, &str)],
        catalog: &str,
    ) {
        let rule = DerivedKeyRule {
            attribute: attribute.into(),
            keys_of: RuleArgument::Named(keys_of.into()),
            named_after: RuleArgument::Named(named_after.into()),
            spelling: spelling.into(),
            rewrite: rewrite
                .iter()
                .map(|(from, to)| ((*from).into(), (*to).into()))
                .collect(),
            catalog: catalog.into(),
        };
        if !self.derived_keys.contains(&rule) {
            self.derived_keys.push(rule);
        }
    }

    /// Records that a declaration's own name carries a catalog key.
    pub fn derive_names_with(&mut self, prefixed_by: &str, or_prefix: &str, catalog: &str) {
        let rule = DerivedNameRule {
            prefixed_by: prefixed_by.into(),
            or_prefix: or_prefix.into(),
            catalog: catalog.into(),
        };
        if !self.derived_names.contains(&rule) {
            self.derived_names.push(rule);
        }
    }

    /// Whether any framework reads a key out of a name at all, which gates the walk that would look.
    pub fn derives_any_name(&self) -> bool {
        !self.derived_names.is_empty()
    }

    /// The catalog key a declaration's name carries, as `(catalog, key)`.
    ///
    /// The **longest** prefix wins, so `app_core_notifications_alter` claims `notifications_alter`
    /// rather than a module called `app` claiming `core_notifications_alter`. Split at each separator
    /// and looked up, rather than tried against every prefix: a checkout has 763 installed extensions
    /// and a name has four underscores.
    pub fn name_key_of(&self, name: &str) -> Option<(&str, Box<str>)> {
        for rule in &self.derived_names {
            if let Some(rest) = name.strip_prefix(&*rule.or_prefix)
                && !rule.or_prefix.is_empty()
                && !rest.is_empty()
            {
                return Some((&rule.catalog, rest.into()));
            }
            // From the last separator back, so the first prefix that is a key is the longest one.
            let mut at = name.len();
            while let Some(cut) = name[..at].rfind('_') {
                at = cut;
                if self.catalog_holds(&rule.prefixed_by, &name[..cut]) {
                    let rest = &name[cut + 1..];
                    if !rest.is_empty() {
                        return Some((&rule.catalog, rest.into()));
                    }
                }
            }
        }
        None
    }

    /// Every derived key one declaration's literals spell, as `(catalog, key, detail)`.
    ///
    /// Given all the literals of one declaration, because a derived key needs two of them at once:
    /// the entry that supplies the key half and the argument that names it. A class declares one of
    /// these attributes, so "the literals of this declaration" is the occurrence.
    pub fn derived_keys_of<'a>(
        &'a self,
        attribute: &str,
        literals: &'a [(AttributeArgument, &'a str, Option<&'a str>)],
    ) -> Vec<(&'a str, Box<str>, &'a str)> {
        if self.derived_keys.is_empty() {
            return Vec::new();
        }
        let written = attribute.rsplit('\\').next().unwrap_or(attribute);
        let mut found = Vec::new();
        for rule in &self.derived_keys {
            if !rule.attribute.eq_ignore_ascii_case(written) {
                continue;
            }
            let Some(name) = literals.iter().find_map(|(argument, value, key)| {
                (key.is_none() && rule.named_after.matches(argument)).then_some(*value)
            }) else {
                continue;
            };
            for (argument, value, key) in literals {
                let Some(key) = key else { continue };
                if rule.keys_of.matches(argument) {
                    found.push((&*rule.catalog, rule.spell(name, key), *value));
                }
            }
        }
        found
    }

    /// Whether any framework derives keys at all, which gates the walk that would look.
    pub fn derives_any_key(&self) -> bool {
        !self.derived_keys.is_empty()
    }

    /// The catalog a written attribute's argument is a key in, if a framework said so.
    ///
    /// `attribute` is compared by its last segment: `#[ContentEntityType(...)]` and
    /// `#[\Drupal\Core\Entity\Attribute\ContentEntityType(...)]` are the same attribute written
    /// two ways, and both appear in real code.
    pub fn catalog_of_attribute(
        &self,
        attribute: &str,
        argument: &AttributeArgument,
    ) -> Option<&str> {
        let written = attribute.rsplit('\\').next().unwrap_or(attribute);
        self.attribute_keys
            .iter()
            .find(|rule| {
                rule.attribute.eq_ignore_ascii_case(written) && rule.argument.matches(argument)
            })
            .map(|rule| &*rule.catalog)
    }

    /// Whether any framework named an attribute argument at all.
    ///
    /// The gate that keeps ingest from walking a file's attribute literals when nothing would read
    /// them, which is every project with no framework detected.
    pub fn names_any_attribute_key(&self) -> bool {
        !self.attribute_keys.is_empty()
    }

    /// Records that the value written under `key` in an array literal indexes `catalog`.
    pub fn bind_array_key(&mut self, key: &str, catalog: &str) {
        let catalogs = match self.array_keys.iter().position(|(name, _)| &**name == key) {
            Some(at) => &mut self.array_keys[at].1,
            None => {
                self.array_keys.push((key.into(), Vec::new()));
                &mut self.array_keys.last_mut().expect("just pushed").1
            }
        };
        let catalog: Box<str> = catalog.into();
        if !catalogs.contains(&catalog) {
            catalogs.push(catalog);
        }
    }

    /// The catalogs the value written under `key` indexes, empty when no framework bound it.
    ///
    /// Exactly, not case-folded: an array key is compared byte for byte by PHP itself, so `'#Type'`
    /// reaches a slot nothing reads and must not be answered for.
    pub fn array_key_catalogs(&self, key: &str) -> &[Box<str>] {
        self.array_keys
            .iter()
            .find(|(name, _)| &**name == key)
            .map_or(&[], |(_, catalogs)| catalogs.as_slice())
    }

    /// Whether any framework bound an array key at all.
    ///
    /// The gate that keeps a request from walking a file's array literals when nothing would read
    /// one, which is every project with no framework detected.
    pub fn binds_any_array_key(&self) -> bool {
        !self.array_keys.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.unproduced.is_empty()
            && self.from_argument.is_empty()
            && self.from_function_argument.is_empty()
            && self.method_properties.is_empty()
            && self.prefixed_methods.is_empty()
            && self.through_argument.is_empty()
            && self.magic_properties.is_empty()
            && self.catalogs.is_empty()
            && self.declared.is_empty()
            && self.dispatched.is_empty()
    }
}

/// The list for `method`, added if it is not there yet.
fn entry<'a, T>(rows: &'a mut Vec<(Box<str>, Vec<T>)>, method: &str) -> &'a mut Vec<T> {
    let at = match rows
        .iter()
        .position(|(name, _)| name.eq_ignore_ascii_case(method))
    {
        Some(at) => at,
        None => {
            rows.push((method.into(), Vec::new()));
            rows.len() - 1
        }
    };
    &mut rows[at].1
}

/// The list for `method`, compared the way PHP compares a method name.
fn find<'a, T>(rows: &'a [(Box<str>, Vec<T>)], method: &str) -> Option<&'a Vec<T>> {
    rows.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(method))
        .map(|(_, values)| values)
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use super::*;

    #[test]
    fn a_contract_is_found_by_the_method_name_as_php_compares_it() {
        let mut contracts = ReturnContracts::default();
        contracts.add_unproduced("A\\Connection", "query", BuiltinTy::Null, None, None);

        assert_eq!(
            contracts
                .unproduced_declarers("QUERY", BuiltinTy::Null)
                .collect::<Vec<_>>(),
            ["A\\Connection"]
        );
        assert!(
            contracts
                .unproduced_declarers("execute", BuiltinTy::Null)
                .next()
                .is_none()
        );
    }

    #[test]
    fn a_bound_argument_names_a_class_through_its_catalog() {
        let mut contracts = ReturnContracts::default();
        contracts.bind_argument("A\\Container", "get", 0, "", "service");
        contracts.catalog_entry("service", "app.manager", Some("App\\Manager"));

        assert_eq!(
            contracts.argument_bindings("GET"),
            [ArgumentBinding {
                class: "A\\Container".into(),
                at: 0,
                suffix: "".into(),
                catalog: "service".into(),
            }]
        );
        assert_eq!(
            contracts.catalog_class("service", "app.manager"),
            Some("App\\Manager")
        );
        // Case-sensitively: a container does not answer for an id nobody registered.
        assert_eq!(contracts.catalog_class("service", "App.Manager"), None);
        assert_eq!(contracts.catalog_class("entity", "app.manager"), None);
    }

    #[test]
    fn a_forwarded_binding_is_found_by_the_method_and_names_the_call_before_it() {
        let mut contracts = ReturnContracts::default();
        contracts.forward_argument(
            "A\\Storage",
            "load",
            "getStorage",
            "entity",
            Forwarded::Instance,
        );
        contracts.catalog_entry("entity", "node", Some("App\\Node"));

        assert_eq!(
            contracts.forwarded_bindings("LOAD"),
            [ForwardedBinding {
                class: "A\\Storage".into(),
                through: "getStorage".into(),
                catalog: "entity".into(),
                yields: Forwarded::Instance,
            }]
        );
        // Not confused with the same-call kind, which is a different question at a different place.
        assert!(contracts.argument_bindings("load").is_empty());
        assert!(contracts.forwarded_bindings("create").is_empty());
    }

    #[test]
    fn a_key_can_name_a_place_instead_of_a_class() {
        let mut contracts = ReturnContracts::default();
        contracts.catalog_entry("service", "app.manager", Some("App\\Manager"));
        contracts.catalog_place(
            "route",
            "app.edit",
            Place {
                path: Arc::from(Path::new("/p/app.routing.yml")),
                line: 12,
                character: 0,
                detail: "/app/{id}/edit".into(),
            },
        );

        // A place names no class, so nothing that asks for one is affected: a call bound to the
        // route catalog keeps whatever type its declaration gave it.
        assert_eq!(contracts.catalog_class("route", "app.edit"), None);
        assert_eq!(
            contracts
                .catalog_placed_at("route", "app.edit")
                .map(|place| place.line),
            Some(12)
        );
        // And a class-valued key is not a place, so a jump asks the other question and gets nothing.
        assert!(
            contracts
                .catalog_placed_at("service", "app.manager")
                .is_none()
        );

        // One list and one description, whichever kind the key is: that is what a completion needs.
        assert_eq!(contracts.catalog_keys("route", "app.", 10), ["app.edit"]);
        assert_eq!(
            contracts.catalog_detail("route", "app.edit"),
            Some("/app/{id}/edit")
        );
        assert_eq!(
            contracts.catalog_detail("service", "app.manager"),
            Some("App\\Manager")
        );
        assert_eq!(contracts.catalog_sizes(), [("route", 1), ("service", 1)]);
    }

    #[test]
    fn a_later_definition_of_a_key_replaces_an_earlier_one() {
        let mut contracts = ReturnContracts::default();
        contracts.catalog_entry("service", "messenger", Some("Core\\Messenger"));
        contracts.catalog_entry("service", "messenger", Some("Mine\\Messenger"));

        assert_eq!(
            contracts.catalog_class("service", "messenger"),
            Some("Mine\\Messenger"),
            "a module that redefines a core id replaces it"
        );
        assert_eq!(contracts.catalog_sizes(), [("service", 1)]);
    }

    #[test]
    fn a_declared_key_names_the_class_that_claimed_it() {
        let mut contracts = ReturnContracts::default();
        contracts.name_keys_with("ContentEntityType", "id", "entity");
        let file = FileId::from_raw(3);

        let mut interner = phrost_model::Interner::new();
        let id = AttributeArgument::Named(interner.name("id"));
        assert_eq!(
            contracts.catalog_of_attribute("ContentEntityType", &id),
            Some("entity")
        );
        // Written fully qualified, which is legal and appears in real code.
        assert_eq!(
            contracts
                .catalog_of_attribute("Drupal\\Core\\Entity\\Attribute\\ContentEntityType", &id),
            Some("entity")
        );
        assert_eq!(
            contracts.catalog_of_attribute(
                "ContentEntityType",
                &AttributeArgument::Named(interner.name("label"))
            ),
            None
        );
        assert_eq!(contracts.catalog_of_attribute("Block", &id), None);
        // A rule on a *name* must not answer for a literal the call wrote without one, or an
        // attribute whose first argument is anything else would claim keys nobody declared.
        assert_eq!(
            contracts.catalog_of_attribute("ContentEntityType", &AttributeArgument::At(0)),
            None
        );

        contracts.claim_key(file, "entity", "node", "Drupal\\node\\Entity\\Node");
        assert_eq!(
            contracts.catalog_class("entity", "node"),
            Some("Drupal\\node\\Entity\\Node")
        );
        assert_eq!(contracts.catalog_keys("entity", "", 10), ["node"]);

        contracts.withdraw_key(file, "entity", "node");
        assert_eq!(contracts.catalog_class("entity", "node"), None);
        assert!(contracts.catalog_keys("entity", "", 10).is_empty());
    }

    #[test]
    fn an_array_key_is_bound_exactly_and_not_the_way_a_method_name_is() {
        // Every other table here folds case, because PHP compares a method name that way. An array
        // key is not a method name: PHP compares it byte for byte, so folding would answer for a
        // slot the runtime never reads.
        let mut contracts = ReturnContracts::default();
        assert!(!contracts.binds_any_array_key());

        contracts.bind_array_key("#type", "element");
        // Twice, because two providers naming the same key must not double the list.
        contracts.bind_array_key("#type", "element");

        assert!(contracts.binds_any_array_key());
        assert_eq!(contracts.array_key_catalogs("#type"), ["element".into()]);
        assert!(contracts.array_key_catalogs("#Type").is_empty());
        assert!(contracts.array_key_catalogs("type").is_empty());
        assert!(contracts.array_key_catalogs("#title").is_empty());
        // Not confused with a method binding, which reads a call's argument rather than a slot.
        assert!(contracts.argument_bindings("#type").is_empty());
    }

    #[test]
    fn a_key_two_declarations_claim_names_nothing() {
        // Naming one of them would mean ranking, and every ranking the index could do depends on
        // discovery order -- which is the invariant the whole project is checked against. Two
        // classes claiming one entity type id is fatal in the framework anyway, so this only arises
        // in code that does not run.
        let mut contracts = ReturnContracts::default();
        contracts.claim_key(FileId::from_raw(1), "entity", "node", "A\\Node");
        contracts.claim_key(FileId::from_raw(2), "entity", "node", "B\\Node");
        assert_eq!(contracts.catalog_class("entity", "node"), None);

        // The key is still offered, because it exists -- what is missing is the class, not the id.
        assert_eq!(contracts.catalog_keys("entity", "", 10), ["node"]);

        // And withdrawing one leaves the other answering.
        contracts.withdraw_key(FileId::from_raw(2), "entity", "node");
        assert_eq!(contracts.catalog_class("entity", "node"), Some("A\\Node"));
    }

    #[test]
    fn configuration_and_declarations_share_one_lookup() {
        let mut contracts = ReturnContracts::default();
        contracts.catalog_entry("service", "app.manager", Some("App\\Manager"));
        contracts.claim_key(FileId::from_raw(1), "entity", "node", "App\\Node");

        assert_eq!(
            contracts.catalog_class("service", "app.manager"),
            Some("App\\Manager")
        );
        assert_eq!(contracts.catalog_class("entity", "node"), Some("App\\Node"));
        assert_eq!(contracts.catalog_sizes(), [("entity", 1), ("service", 1)]);
        assert_eq!(
            contracts.declared_keys(),
            [("entity", "node", vec!["App\\Node"])]
        );
    }

    #[test]
    fn prose_is_matched_without_landing_inside_a_character() {
        // An em dash in a docblock panicked the checker: the prefix was compared by slicing the prose
        // to the prefix's length, and that length can fall in the middle of a character.
        let mut contracts = ReturnContracts::default();
        contracts.add_documented_elsewhere("Implements", "hook_", Some("extension"), Some("hook"));
        // Twice, because two providers detecting the same framework must not double the list.
        contracts.add_documented_elsewhere("Implements", "hook_", Some("extension"), Some("hook"));
        assert_eq!(contracts.documentation_conventions().len(), 1);

        assert!(contracts.documents_elsewhere("Implements hook_form_alter()."));
        assert!(contracts.documents_elsewhere("  implements HOOK_cron()."));
        assert!(!contracts.documents_elsewhere("Builds — the thing."));
        assert!(!contracts.documents_elsewhere("Impl"));
        // The word alone is prose, and the prefix alone names nothing.
        assert!(!contracts.documents_elsewhere("Implementsomething odd."));
        assert!(!contracts.documents_elsewhere("Implements hook_"));
        // A word between the lead and the name. Three docblocks on one real checkout write a
        // project's own hook this way, and coder's own sniff reads them as claims -- it anchors the
        // end of the line and not the word after the lead.
        assert!(contracts.documents_elsewhere("Implements custom hook_report_types_info()."));
        // A name mentioned in passing is still a claim, which is the cost of the line above and is
        // the same answer the sniff gives.
        assert!(contracts.documents_elsewhere("Implements the alter half of hook_form_alter()."));
        // Nothing that names no hook at all: `Implements callback_batch_operation().` and
        // `Implements the magic __toString() method.` are 54 and 22 docblocks of one checkout.
        assert!(!contracts.documents_elsewhere("Implements the magic __toString() method."));
        assert!(
            !contracts
                .documents_elsewhere("Implements Drupal\\Core\\Ajax\\CommandInterface:render().")
        );
        assert!(!ReturnContracts::default().documents_elsewhere("Implements hook_cron()."));
    }

    #[test]
    fn a_claim_gives_back_where_the_name_it_makes_is_written() {
        // The range is what makes the sentence navigable: nothing else in a comment says where a
        // declaration is named, so a claim that answered only yes or no could be read and not
        // followed.
        let mut contracts = ReturnContracts::default();
        contracts.add_documented_elsewhere("Implements", "hook_", Some("extension"), Some("hook"));
        let rule = &contracts.documentation_conventions()[0];

        let line = " * Implements hook_form_alter().";
        assert_eq!(rule.claim_in(line).map(|at| &line[at]), None);
        let line = "Implements  hook_form_alter().";
        assert_eq!(
            rule.claim_in(line).map(|at| &line[at]),
            Some("hook_form_alter")
        );
    }

    #[test]
    fn every_name_the_convention_marks_is_found_wherever_it_is_written() {
        // `@see hook_theme()` is the other half of the same fact, and a mention in prose is a third.
        // A word that merely contains the prefix is not one of them.
        let mut contracts = ReturnContracts::default();
        contracts.add_documented_elsewhere("Implements", "hook_", Some("extension"), Some("hook"));
        let rule = &contracts.documentation_conventions()[0];

        let text = "Implements hook_form_alter().\n@see hook_theme()\n$hook_info, my_hook_x, hook_";
        assert_eq!(
            rule.references_in(text)
                .into_iter()
                .map(|at| &text[at])
                .collect::<Vec<_>>(),
            ["hook_form_alter", "hook_theme"]
        );
    }

    #[test]
    fn a_default_branch_is_found_by_the_method_name_and_carries_the_position_that_decides() {
        let mut contracts = ReturnContracts::default();
        contracts.add_defaulted("A\\Container", "resolver", 0, "A\\ResolverInterface");
        // Twice, because two providers detecting the same framework must not double the list.
        contracts.add_defaulted("A\\Container", "resolver", 0, "A\\ResolverInterface");

        assert_eq!(
            contracts.defaulted_branches("RESOLVER"),
            [DefaultBranch {
                class: "A\\Container".into(),
                unless_given: 0,
                then: "A\\ResolverInterface".into(),
            }]
        );
        assert!(contracts.defaulted_branches("service").is_empty());
        // Not confused with the chain contract, which settles a union from a different fact.
        assert!(contracts.chained_branches("resolver").is_empty());
    }

    #[test]
    fn an_argument_class_rule_is_found_by_the_method_name_and_carries_the_position_that_names_it() {
        let mut contracts = ReturnContracts::default();
        contracts.add_argument_class("A\\ResolverInterface", "build", 0);
        // Twice, because two providers detecting the same framework must not double the list.
        contracts.add_argument_class("A\\ResolverInterface", "build", 0);

        assert_eq!(
            contracts.argument_class_returns("BUILD"),
            [ArgumentClass {
                class: "A\\ResolverInterface".into(),
                argument: 0,
            }]
        );
        assert!(contracts.argument_class_returns("resolver").is_empty());
        // Not confused with the catalog binding, which reads the same argument and looks it up
        // rather than taking it for the answer.
        assert!(contracts.argument_bindings("build").is_empty());
    }

    #[test]
    fn two_hierarchies_can_contract_the_same_method_name() {
        let mut contracts = ReturnContracts::default();
        contracts.add_unproduced("A\\Query", "execute", BuiltinTy::Null, None, None);
        contracts.add_unproduced("B\\Statement", "execute", BuiltinTy::Null, None, None);
        // Twice, because two providers detecting the same framework must not double the list.
        contracts.add_unproduced("A\\Query", "execute", BuiltinTy::Null, None, None);

        assert_eq!(
            contracts
                .unproduced_declarers("execute", BuiltinTy::Null)
                .count(),
            2
        );
    }
}

/// One method's union, and the branch a call that left an argument out gets.
///
/// [`Self::then`] names a class, and it is only ever *one of the operands the declaration already
/// wrote*: the reader checks that before using it, so this can select a branch and never invent one.
/// Nothing here enforces it -- this crate holds no types and no hierarchy -- which is why the rule is
/// written down at both ends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DefaultBranch {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// The argument whose absence decides, counted from zero. A call writing this many arguments or
    /// fewer has left it out.
    pub unless_given: u16,
    /// The branch that survives, fully qualified.
    pub then: Box<str>,
}

/// One method whose return class an argument to the call names.
///
/// Nothing here says which class: the answer is whatever the *caller* wrote, which is what makes the
/// rule unable to invent one. What it takes to be allowed to speak at all -- the declaration saying
/// nothing narrower than `object`, and the written name being a class the index holds -- is checked
/// by the reader, because this crate holds no types.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgumentClass {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// Which argument names the class, counted from zero.
    pub argument: u16,
}

/// A union of a collection and its element, and the earlier calls that settle it. The framework's
/// `ChainedOperand`, restated below the seam.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChainedOperandRule {
    /// The class declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// Calls earlier in the chain that make it many whatever they are given.
    pub after: Vec<Box<str>>,
    /// Calls earlier in the chain that make it many when they are given a count.
    pub counted_by: Vec<Box<str>>,
    /// The operand that is the many.
    pub many: Box<str>,
}

/// One method's union, and the earlier call on the same receiver that settles it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChainedBranch {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: Box<str>,
    /// The call that switches the answer, earlier in the same chain.
    pub after: Box<str>,
    pub then: phrost_model::BuiltinTy,
    pub otherwise: phrost_model::BuiltinTy,
}
