use phrost_model::BuiltinTy;

use std::path::PathBuf;

/// A namespace-to-directory mapping a framework registers at runtime.
///
/// Deliberately expressed in this crate's own terms rather than as a `phrost-project` type: a
/// provider says what it knows, and the layer that owns autoload configuration decides what to do
/// with it. That keeps the arrow pointing one way and this crate's dependencies to almost nothing.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct AutoloadRule {
    /// PSR-4 prefix, with a trailing separator, as composer writes it: `Drupal\node\`.
    pub prefix: String,
    pub dir: PathBuf,
}

/// A method whose declared return type admits `null` that no run can produce.
///
/// The one thing a framework knows that its own signature does not say. Drupal's
/// `Connection::query()` is declared `?StatementInterface` and returns `NULL` only on the branch
/// where its exception handler has already thrown; every caller in a real project reads the result
/// straight away, so a checker that believes the signature reports the whole database layer as
/// unguarded. On one real project that was 118 of 379 null-safety findings, all of them noise.
///
/// Only ever *removes* a null. A framework may not use this to claim a type the declaration does not
/// mention -- that is a different fact with a different failure mode, and this one is safe precisely
/// because the worst it can do is miss a real null.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NonNullReturn {
    /// The class or interface whose declaration this is about. Anything below it in the hierarchy is
    /// covered too, because an override inherits the contract along with the signature.
    pub class: &'static str,
    /// The method name, compared the way PHP compares method names.
    pub method: &'static str,
    /// Whether the null is there at all, and if it is, in which state.
    pub null: NullFact,
}

/// Why a declared `null` is not reported, which is two different claims wearing one silence.
///
/// The distinction is the whole reason this is a field rather than a comment. A contract that says
/// *no run produces it* is a proof, and reporting the null would be a false positive; a contract
/// that says *a run could, and no project is in that state* is a judgement, and the null it hides is
/// real. Written as one thing, a reader of the list -- or of the output -- cannot tell which of the
/// seven entries they are looking at, and the second kind is something a project is entitled to see.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NullFact {
    /// The branch returning it cannot run. `Select::execute()` returns `NULL` when `preExecute()`
    /// is false, and `preExecute()` returns `TRUE` early when already prepared and otherwise falls
    /// through to returning the `TRUE` it just set.
    Unreachable,
    /// Real, in a state the framework does not hand the object out in -- so the prose finishes the
    /// sentence "this can be null only ...". Reported by `assumed-non-null`, which a project turns
    /// on when it wants to see what is being decided for it.
    OnlyWhen(&'static str),
    /// The same, where the state is one the *call* can be read for.
    ///
    /// `ContainerInterface::get($id)` returns null only when the second argument asks it to, and
    /// the default throws instead. A call that passes one argument is therefore not in that state,
    /// and saying "this can be null only when the call passes NULL_ON_INVALID_REFERENCE" on a call
    /// that plainly does not is a disclosure about nothing.
    ///
    /// Measured before it was built: **233 of 362** disclosures on the reference project were this
    /// contract, and every one of the 233 passed a single argument. Two thirds of the output was
    /// about a state no call in the project was in.
    OnlyWithArgument {
        /// Counted from zero, and the null needs an argument to *exist* at this position -- not to
        /// hold any particular value. Reading the value would mean resolving a class constant per
        /// call on a path a project turns on to read, and the count already separates every real
        /// case from every noisy one.
        at: u16,
        prose: &'static str,
    },
}

/// A method whose declared union is settled by an earlier call on the same receiver.
///
/// A query builder's shape, and every framework has one. Drupal's entity query declares
/// `int|array` on `execute()` because `count()` switches which it is; Laravel's builder does the
/// same thing with a different vocabulary. The union is honest about what the *method* can return
/// and useless about what *this call* returns, so a checker reading it reports the branch nobody
/// took -- 22 findings on one real project, every one of them a query nobody counted.
///
/// Only ever *narrows*, and only to a branch the declaration already lists. A framework may not use
/// this to claim a type the signature does not mention: the same restraint [`NonNullReturn`] has,
/// and for the same reason -- the worst this can do is answer a branch instead of a union, and the
/// worst that can do is miss a finding.
///
/// The switching call is looked for in the receiver's own chain, so `->count()->execute()` is read
/// and `$q->count(); $q->execute();` is not. That is the documented spelling in both frameworks;
/// the split one falls back to [`Self::otherwise`], which is the commoner answer anyway.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChainedReturn {
    /// The class or interface declaring the method. Anything below it is covered, because an
    /// override inherits the contract with the signature.
    pub class: &'static str,
    /// The method whose union is being settled.
    pub method: &'static str,
    /// The call that switches the answer, on the same receiver and earlier in the chain.
    pub after: &'static str,
    /// Which branch of the declared union survives when `after` was called.
    pub then: BuiltinTy,
    /// And when it was not. This is the answer a plain call gets, so it is the one that matters.
    pub otherwise: BuiltinTy,
}

/// The class-operand twin of [`ChainedReturn`]: a union of a collection and the thing it collects,
/// settled by an earlier call on the same receiver.
///
/// Laravel's model factory: `create()` declares `Collection<int, TModel>|TModel` because a
/// `count()` earlier in the chain -- or a count handed to `factory(3)` -- makes it many. Read as the
/// union, every `$user = User::factory()->create()` handed to a `User` parameter was a collection
/// that might arrive there. Selects, and only from the declared operands: the one at or below
/// [`Self::many`] where the chain counted, every other one where it did not, and nothing where a
/// count was handed over as a value nothing can read -- `factory($n)` may be either.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChainedOperand {
    /// The class declaring the methods. Anything below it is covered.
    pub class: &'static str,
    pub methods: &'static [&'static str],
    /// Calls earlier in the chain that make it many whatever they are given: `count`, `times`.
    pub after: &'static [&'static str],
    /// Calls earlier in the chain that make it many when given a count: `factory(3)`.
    pub counted_by: &'static [&'static str],
    /// The operand that is the many.
    pub many: &'static str,
}

/// A method whose declared union is settled by the call leaving an argument out.
///
/// The sibling of [`ChainedReturn`] for a union whose branches are *classes* rather than builtins,
/// and whose switch is the call's own shape rather than an earlier link of the chain.
/// `\Drupal::classResolver($class = NULL)` is declared
/// `ClassResolverInterface|object` because the one method is two: called bare it hands back the
/// resolver, and called with a class name it hands back an instance of that class. Both readings are
/// in the docblock -- "The class resolver **or if `$class` is provided**, a class instance" -- and
/// the union that states them says nothing about either call.
///
/// Only ever *selects*, and only from the branches the declaration itself lists. That restraint is
/// not a convention here: [`Self::then`] is checked against the operands of the method's own declared
/// return type at the moment it is used, and a branch that is not one of them does nothing. So the
/// worst a wrong entry can do is answer the wrong branch of a union the declaration wrote -- which is
/// what a reader of the declaration alone already gets -- and the worst *that* can do is miss a
/// finding. A framework may not use this to claim a type the signature does not mention; there is no
/// spelling of it that would.
///
/// Nothing is said about null. A nullable declaration is not a bare union, so the check above
/// declines and the declaration stands -- which is the safe direction, and the reason this carries no
/// switch for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DefaultedReturn {
    /// The class or interface declaring the method. Anything below it is covered, because an
    /// override inherits the contract with the signature.
    pub class: &'static str,
    /// The method whose union the call's shape settles.
    pub method: &'static str,
    /// The argument whose absence decides, counted from zero.
    ///
    /// A call writing this many arguments or fewer has left it out, so its default is what the
    /// method sees. Which is the whole fact: the branch below is the one the *default* selects.
    pub unless_given: u16,
    /// Which branch of the method's own declared union a call that leaves the argument out gets,
    /// fully qualified and without a leading separator.
    pub then: &'static str,
}

/// A method whose declared `object` says nothing, and whose argument names the class it returns.
///
/// `ClassResolverInterface::getInstanceFromDefinition($definition)` is declared `@return object` and
/// its body is `$definition::create($container)` or `new $definition()`, so a call writing
/// `Foo::class` gets a `Foo`. PHP's own way of stating that is a `@return T` bound to a
/// `class-string<T>` parameter, and Drupal writes no such tag -- but that is the only thing missing.
/// The claim is the same one, so this contributes it rather than describing a new one, and the class
/// that comes out is matched to the parameter, resolved against the caller's imports and required to
/// exist by exactly the code that serves the docblock.
///
/// # Why this is safe, when it cannot be safe the way the others are
///
/// [`DefaultedReturn`] and [`ChainedReturn`] are safe by a structural gate: the branch a provider
/// names has to be one of the operands the declaration already wrote, so a provider *selects* and
/// can never invent. `object` writes no branches, so there is nothing to select from and that gate
/// has nothing to check. Three things stand in its place.
///
/// **The provider names a position, never a class.** What comes back is whatever the *caller* wrote
/// at that argument, resolved in the caller's own file. So the answers this can produce are not
/// chosen here at all: they are the class names already written in the project's own source at that
/// call. A wrong entry in this list can read a name somebody wrote; it cannot produce one nobody
/// did. That is the crux, and it is a different kind of claim from the others -- they are bounded by
/// the declaration, and this one is bounded by the call.
///
/// **The declaration has to say `object` and nothing narrower.** Checked where the contract is used,
/// the same way [`DefaultedReturn`]'s branch is, and a method declaring anything else contributes
/// silence. Every class satisfies `object`, so narrowing inside it can never contradict what the
/// runtime enforces -- and the check is what makes that sentence true rather than a hope. It also
/// disarms the contract if the framework ever types the method, in the direction that says nothing.
///
/// **The class has to be one the index holds.** An argument naming a class the project does not
/// declare answers nothing, rather than a class whose every member would then be reported missing.
///
/// So everything a call can write that is not a class name is silence: a variable is not a literal
/// and is never resolved for this, a string naming nothing resolves to nothing, and an argument at
/// another position is not the one the contract named. A string that *does* name a declared class is
/// an answer, and a correct one -- `$definition::create()` cannot tell `Foo::class` from `'Foo'`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArgumentClassReturn {
    /// The class or interface declaring the method. Anything below it is covered, because an
    /// override inherits the contract with the signature.
    pub class: &'static str,
    /// The method whose return class the call names.
    pub method: &'static str,
    /// Which argument names it, counted from zero.
    ///
    /// Any position, unlike [`ArgumentBinding`]: this reads the call's own lowered arguments, where
    /// a catalog binding reads a single literal carried down from one step back.
    pub argument: u16,
}

/// A call whose string argument names the key of an element the enclosing class builds.
///
/// Drupal's forms: `buildForm()` writes `$form['email'] = ['#type' => 'email']`, and
/// `$form_state->getValue('email')` in the same class's `submitForm()` reads what was submitted for
/// that element -- by the element's own key, at whatever depth and in whichever of the class's
/// methods it was written. The string names nothing PHP declares, and the arrays the class builds
/// are the only statement of which keys exist.
///
/// What makes an array an *element* rather than any array is the framework's to say, so it is said
/// here: a key beginning with [`Self::property`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuiltKey {
    /// The class or interface declaring the reading call. Anything below it is covered.
    pub class: &'static str,
    /// The reading call, whose first argument is the key.
    pub method: &'static str,
    /// What a property key begins with -- Drupal's `#`, as in `#type` and `#title`. An array holding
    /// one is an element, and such a key is never itself the key of an element.
    pub property: &'static str,
    /// The property whose literal value says what an element is, shown beside its key: `#type`.
    pub described_by: &'static str,
    /// The property that files an element's value under a path of its own, whose first entry is
    /// then the key it is read by: Drupal's `#parents`.
    pub filed_under: &'static str,
}

/// A call on a test double whose string argument names a method of what the double stands in for.
///
/// A double configures its subject by name: PHPUnit's `$repo->method('find')`, or
/// `$repo->expects($this->once())->method('find')` one call further along. The double's own type says
/// what the subject is -- `createMock(Repo::class)` is a `MockObject&Repo` -- so the string is a
/// method of the very value the call is made on. Read that way, a rename of `Repo::find` reaches the
/// tests that configure it, and goto from a test lands on the method.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StandInMethod {
    /// What makes a value a double: a type one of its operands descends from.
    pub stand_in: &'static str,
    /// The configuring call.
    pub method: &'static str,
    /// Which argument names the method.
    pub at: u16,
    /// Calls between the double and the configuring one that hand back a builder for it.
    pub through: &'static [&'static str],
}
/// A method whose first argument names the class it returns.
///
/// `\Drupal::service('application.manager')` is declared `object`, and the id is the only thing in
/// the source that says what comes back. The binding says which catalog the id is a key into; the
/// catalog says what it names.
///
/// The *first* argument and no other, because that is the only one carried down to where this is
/// answered -- and every method that works this way puts the id first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArgumentBinding {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: &'static str,
    pub method: &'static str,
    /// Which argument is the key, counted from zero.
    ///
    /// Nearly always the first, because a method that looks a value up by a string puts the string
    /// first. `AccessResult::allowedIfHasPermission($account, $permission)` is the exception that
    /// makes this a field: it is how a permission check is written in every controller, and binding
    /// it without a position would bind the account.
    pub at: u16,
    /// What the method appends to the argument before looking the key up. Empty for none.
    ///
    /// Here because the method decides it, not the framework: Drupal's `invokeAll($hook)` runs
    /// `hook_$hook` while its `alter($type)` runs `hook_{$type}_alter`, so the argument of the
    /// second is a hook name with six characters missing. Bound without this, `alter('form')` was
    /// judged against a key called `form` -- and the suggestion for a typo of it would have been a
    /// hook name, which is not what a person can type at that position.
    ///
    /// [`DispatchedName`] has carried the same field since it existed, which is the whole reason
    /// this one is a field rather than an argument: the side that *writes* a name knew the
    /// transformation and the side that *reads* one did not.
    pub suffix: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// A class that declares, in a property's string default, the inputs a method reads by name.
///
/// Laravel's console command is the case: `protected $signature = 'mail:send {user} {--queue=}'`,
/// and `$this->argument('user')` declares `array|string|float|int|bool|null` because a signature
/// cannot say which input it is asked for. The declaration says what each one holds -- a required
/// argument a string, an option with no value a flag -- in a grammar only the framework knows, so
/// the provider reads it and hands down what it found.
///
/// Read from the file the class is declared in, which is where the reads are: a command's
/// `handle()` sits beside its signature.
#[derive(Clone, Copy, Debug)]
pub struct DeclaredInputs {
    /// The class whose subclasses declare inputs. Anything below it is covered.
    pub class: &'static str,
    /// The property whose string default declares them.
    pub property: &'static str,
    /// Each method reading one input by the name its first argument gives, with what the
    /// declaration writes before such a name: `option('queue')` reads the `--queue` a signature
    /// writes.
    pub readers: &'static [(&'static str, &'static str)],
    pub parse: InputDeclarations,
    /// What every declaration has beside its own, written as a declaration writes it: the
    /// application's `--help`, `--verbose`, `--env`. A read of one is a read of something declared.
    pub always: &'static [&'static str],
    /// Methods that, declared on the class or called in it, mean its inputs come from somewhere
    /// besides the property -- Symfony's `configure()`, `addOption()` -- so a read nothing declares
    /// is not judged there. Only a class extending [`Self::class`] directly is judged at all: a
    /// class between the two may add inputs of its own, as Laravel's `GeneratorCommand` adds
    /// `--test` in its constructor.
    pub unless: &'static [&'static str],
}

/// Every input a declaration writes, under the name it is written with, what it holds, and where in
/// the text it was given the name is.
pub type InputDeclarations = fn(&str) -> Vec<phrost_model::DeclaredInput>;

/// A table's columns, written as the calls a migration makes, and the classes whose rows they are.
///
/// Laravel's `Schema::create('books', function (Blueprint $table) { $table->string('name'); })` is
/// the only statement anywhere that a `Book` has a `name` holding a string: the model's `__get` reads
/// the row, and nothing in PHP declares it. Applied in the order the framework runs them, the
/// migrations are the table -- the reading Larastan makes, and the one completion on a model needs.
///
/// The calls are found below the seam, since only the index sees a body; what each builder call
/// does to the table is the framework's vocabulary, carried down as its own function for the reason
/// [`DeclaredInputs::parse`] is.
#[derive(Clone, Copy, Debug)]
pub struct WrittenSchema {
    /// The class whose static methods change a table: Laravel's `Schema` facade.
    pub facade: &'static str,
    /// Its methods that build a table, named by their first argument, from the callback their second
    /// is -- handed the builder.
    pub create: &'static [&'static str],
    /// Its methods that change one the same way.
    pub alter: &'static [&'static str],
    /// Its methods that drop the table their first argument names.
    pub drop: &'static [&'static str],
    /// Its methods that rename the table their first argument names to their second.
    pub rename: &'static [&'static str],
    /// The method of a migration the framework runs, whose calls are the schema's history -- and not
    /// `down()`, which undoes it.
    pub runs: &'static str,
    /// A directory a file has to be under for its calls to count: a test building a table of its own
    /// in `setUp()` is not the project's schema.
    pub directory: &'static str,
    /// What each call on the builder does to the table's columns.
    pub columns: BuilderCalls,
    /// The class whose subclasses are rows of a table.
    pub model: &'static str,
    /// The property whose string default names the table.
    pub table_property: &'static str,
    /// The table a class names when it declares none, from its short name.
    pub default_table: fn(&str) -> String,
}

/// What one call on a table's builder does to its columns.
pub type BuilderCalls =
    fn(&phrost_model::schema::BuilderCall) -> Vec<phrost_model::schema::ColumnChange>;

/// A static method handing back an instance of the class named after the one it was called on.
///
/// Laravel's `Contact::factory()` is `Database\Factories\ContactFactory::new()`: `HasFactory`
/// declares `@return TFactory`, a template nothing fills unless the model writes
/// `@use HasFactory<ContactFactory>`, and `Factory::resolveFactoryName()` finds the class by the
/// model's name. Read where nothing written fills the template, and where nothing the class
/// declares overrides the convention.
#[derive(Clone, Copy, Debug)]
pub struct NamedAfterCaller {
    /// The trait or class declaring the method.
    pub class: &'static str,
    pub method: &'static str,
    /// Members that, declared below [`Self::class`], name the class themselves: `newFactory()`, a
    /// static `$factory`.
    pub unless: &'static [&'static str],
    /// The directory, from the project's root, whose autoload prefix is the application's
    /// namespace -- and the namespace to assume where no rule maps one onto it.
    pub app_directory: &'static str,
    pub default_namespace: &'static str,
    /// The class the method hands back, from the class it was called on and the application's
    /// namespace.
    pub name: fn(&str, &str) -> String,
}

/// A template of a framework's class that a subclass fills by its own name, where it writes nothing.
///
/// Laravel's `Factory<TModel>`: a factory that writes neither `@extends Factory<User>` nor
/// `$model = User::class` makes `User` by name -- `Factory::modelName()` takes the factory's name
/// without `Database\Factories\` and `Factory`, and looks for it below the application's
/// `Models\` and then directly below the application. Read only where nothing up the chain wrote
/// the argument, and where no class below [`Self::class`] declares one of [`Self::unless`].
#[derive(Clone, Copy, Debug)]
pub struct NamedTemplateArgument {
    pub class: &'static str,
    /// The template, by name, that the convention fills.
    pub template: &'static str,
    /// A property whose `X::class` default names the argument outright, asked before the name:
    /// `protected $model = User::class;`.
    pub from_property: Option<&'static str>,
    /// Members that, declared below [`Self::class`], name the argument themselves: `$model`.
    pub unless: &'static [&'static str],
    /// See [`NamedAfterCaller`] for these two.
    pub app_directory: &'static str,
    pub default_namespace: &'static str,
    /// The classes the argument may be, in the order the framework tries them, from the subclass and
    /// the application's namespace.
    pub names: fn(&str, &str) -> Vec<String>,
}

/// A method whose declaration names a generic class bare, and the arguments the framework gives it
/// from the receiver's own templates.
///
/// Laravel before 12.x wrote `@return \Illuminate\Pagination\LengthAwarePaginator` over
/// `Eloquent\Builder::paginate()`, and later releases `LengthAwarePaginator<int, TModel>` -- the same
/// method, saying what it pages. A project on an older release holds the older declaration, and
/// every page it built was a page of nothing.
///
/// Only fills: the class the declaration names stands, and one already written with arguments is
/// left as written. The worst a wrong entry can do is name the wrong model for a page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FilledReturn {
    /// The class declaring the method. Anything below it, and anything handing calls to it through a
    /// `@mixin`, is covered.
    pub class: &'static str,
    pub method: &'static str,
    /// The arguments, in the order the returned class declares its templates.
    pub arguments: &'static [FilledArgument],
}

/// A static method that gives a class a method of the name it is handed, implemented by the closure
/// beside it: Laravel's `Collection::macro('sortByCollator', function (...) { ... })`, on any class
/// using `Macroable`.
///
/// Nothing declares such a method; the registration in a service provider's body is the only
/// statement of it. It answers on the class the call names and everything below it, and inside the
/// closure `$this` is the instance it was called on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MacroRegistrar {
    /// The trait (or class) whose users take registrations. A registration on a class that is not
    /// one of them registers nothing.
    pub provider: &'static str,
    /// The static method making one: `macro`.
    pub method: &'static str,
}

/// One argument of a [`FilledReturn`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilledArgument {
    /// A type PHP spells with a keyword: a page's `int` keys.
    Builtin(phrost_model::BuiltinTy),
    /// One of [`FilledReturn::class`]'s own templates, as the receiver supplies it.
    Template(&'static str),
}

/// A method whose return is the class one fixed key of a catalog names.
///
/// Laravel's `$request->user()` declares `mixed` and a guard's `user()` an `Authenticatable`, and
/// which model a user is lives in `config/auth.php`. [`ArgumentBinding`]'s shape with no argument:
/// the key is the provider's to state, and the project's configuration what it names. Nullable when
/// the method can hand back nothing -- no one signed in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyedReturn {
    /// The class or interface declaring the method. Anything below it is covered.
    pub class: &'static str,
    pub method: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
    pub key: &'static str,
    pub nullable: bool,
}

/// What a literal key a catalog does not list holds, for a catalog listing only the exceptions.
///
/// Laravel's translator hands back the line a key names, the array a group or a nested list is, and
/// the key itself where it names nothing -- so `trans('auth.failed')` is a string and `trans('auth')`
/// an array, and only the second needs filing. A key holding `outside` is not the catalog's to
/// answer: `trans('package::file.key')` names a package's files, which nothing here reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UnlistedKey {
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
    pub holds: phrost_model::BuiltinTy,
    pub outside: &'static str,
}

/// A free function whose argument names the class it returns: Laravel's `app('router')`.
///
/// The function's half of [`ArgumentBinding`], and a type of its own rather than a binding with no
/// class: a function has no hierarchy to cover, and a name that is a method on one class and a
/// function besides is two facts that must not answer for each other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FunctionBinding {
    /// Fully qualified, without the leading separator, compared the way PHP compares function names.
    pub function: &'static str,
    /// Which argument is the key, counted from zero.
    pub at: u16,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// A template language whose files are read as the PHP they hold.
///
/// A Blade template is HTML with PHP expressions in it, named `*.blade.php`, and every question a
/// reader asks of one is about those expressions. `to_php` turns the template into PHP **of the
/// same length with every newline in place** -- the expressions keep their bytes and everything
/// around them is overwritten -- so that an offset into the template is the same offset into the
/// PHP, and nothing above the seam translates a position. The second value is how many constructs
/// could not be written in place, for an instrument to count.
#[derive(Clone, Copy, Debug)]
pub struct TemplateDialect {
    /// The ending a template's file name has, compared as written: `.blade.php`.
    pub suffix: &'static str,
    pub to_php: fn(&str) -> (String, usize),
    /// Where the templates a call names by name live, from the project root: `resources/views`.
    pub directory: &'static str,
    /// What a template's name writes between directories: Laravel's `posts.show` is
    /// `posts/show.blade.php`.
    pub separator: &'static str,
    /// Variables every template has, whatever renders it, and the class each holds: Laravel shares
    /// `$errors` with every view.
    pub shared: &'static [(&'static str, &'static str)],
    /// The tags `to_php` writes as a call rendering another template.
    pub components: &'static [ComponentTag],
    /// The call `to_php` writes an include as: the template named by its first argument, handed the
    /// array of its second -- and every variable of the template including it, which a call
    /// rendering a template from code does not hand over.
    pub includes: Option<&'static str>,
    /// What in a template's structure the PHP made of it cannot carry -- a block opened and never
    /// closed, a closer that ends something else -- with where it is, in bytes of the template.
    pub problems: TemplateProblems,
}

/// How a project's Twig loader finds a template by the name a template writes.
///
/// Twig's filesystem loader reads `@node/node.html.twig` as that path under whichever directories
/// the `node` namespace was given, trying each in turn. Which directories those are is
/// configuration -- Drupal gives each extension its `templates/` *and* its own root, Symfony a
/// project's `templates/` and each bundle its views -- so the provider walks them and files every
/// template under the names the loader takes for it. Above the seam a name is a key, and nothing
/// translates one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TwigLoader {
    /// The catalog of places holding every template by the name a template is written to load it
    /// by -- the names worth offering. A name this provider chose.
    pub templates: &'static str,
    /// The catalog of every other name the loader finds a `.twig` file by: the same template
    /// reached through another of its namespace's directories, or a `.twig` file outside the
    /// directory templates are kept in. Answered, and not offered -- nobody needs Drupal's
    /// `@node/templates/node.html.twig` listed beside `@node/node.html.twig`.
    pub aliases: &'static str,
    /// The catalog whose keys are the namespaces whose directories were walked, so that a `.twig`
    /// name in one of them that neither catalog above holds is a template that is not there -- and
    /// the loader throws.
    pub namespaces: &'static str,
    /// The directories the walk did not enter, by name. A name passing through one names a file
    /// the walk could not have seen, and is not one a missing key can rule out.
    pub unwalked: &'static [&'static str],
    /// Variables every template has whatever renders it, each with its type: a class fully
    /// qualified, or a builtin by name. Drupal hands every template its `attributes`.
    pub globals: &'static [(&'static str, &'static str)],
}

/// An object a method builds that declares a named entry: Twig's `new TwigFilter('upper', ...)` in
/// an extension's `getFilters()`.
///
/// The construction is the only statement anywhere that the entry exists and what runs it, and it
/// is in a body -- Twig's own filters are in a dependency's -- so lowering keeps it from the methods
/// named here, in every file, and from no others.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConstructedEntry {
    /// The method whose body builds them.
    pub method: &'static str,
    /// The class constructed, fully qualified. Its first argument is the key.
    pub class: &'static str,
    /// The catalog the key is of. A name this provider chose.
    pub catalog: &'static str,
}

/// Which catalogs hold what a Twig template calls by name: its filters, functions and tests.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TwigCallables {
    pub filters: &'static str,
    pub functions: &'static str,
    pub tests: &'static str,
}

/// What a template's structure gets wrong, with where, in bytes of the template.
pub type TemplateProblems = fn(&str) -> Vec<(std::ops::Range<usize>, String)>;

/// A tag a template dialect writes as the call rendering the template it names.
///
/// Blade's `<x-alert :post="$post">` is `x(\alert, post: $post)`, rendering `components.alert` with
/// `$post`. The name is the first argument as written; the variables are named arguments.
#[derive(Clone, Copy, Debug)]
pub struct ComponentTag {
    /// The call the tag is written as.
    pub function: &'static str,
    /// What the name takes as a template's.
    pub prefix: &'static str,
    /// Variables every such template has besides: Laravel hands an anonymous component its
    /// `$attributes` and its `$slot`.
    pub shared: &'static [(&'static str, &'static str)],
}

/// A static call handing every template a variable, whatever renders it.
///
/// Laravel's `View::share('dates', $formatter)` in a service provider: `$dates` exists in every view
/// from then on, and that call is the only statement of what it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SharedTemplateData {
    /// The class the call is made on, fully qualified.
    pub class: &'static str,
    /// The method: its first argument is the variable's name -- or an array of them -- and its
    /// second the value.
    pub method: &'static str,
    /// A function that, called with nothing, hands back the instance the method is called on:
    /// Laravel's `view()` is the view factory, and `view()->share('locale', $locale)` in a
    /// middleware is the same statement as the facade's.
    pub helper: Option<&'static str>,
}

/// An array that renders the template it names under one key, handing it the values under keys
/// with a prefix.
///
/// Drupal's `['#theme' => 'pdf__certificate', '#data' => [...]]`: inside the hook's template `data`
/// is whatever the array held under `#data`, and nothing in the template says what that is. The
/// array is the declaration, the way a call rendering a template by name is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderArray {
    /// The key naming the template: `#theme`.
    pub key: &'static str,
    /// The catalog of places the name is a key of: which file is the template is that key's place.
    pub catalog: &'static str,
    /// What a key handing a variable over begins with: `#`.
    pub prefix: &'static str,
    /// Keys with the prefix that belong to the array and are no variable: `#cache`, `#attached`.
    pub own: &'static [&'static str],
    /// How a template's file name says the name it renders by, wherever the file is.
    ///
    /// Beside the catalog rather than instead of it: Drupal's registry names `item-list.html.twig`
    /// `item_list` in whichever directory a hook's `path` put it, and a catalog read from
    /// `templates/` directories cannot see one that is elsewhere -- 607 template reads on one
    /// project are in files a hook keeps under `src/`.
    pub file_name: Option<TemplateFileName>,
}

/// A template file's name read as the name a render array renders it by: `item-list.html.twig` is
/// `item_list`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TemplateFileName {
    /// What the file name ends in, taken off: `.html.twig`.
    pub suffix: &'static str,
    /// The character the file writes where the name has `named`: `-` for `_`.
    pub written: char,
    pub named: char,
}

/// A call that renders a template by name and hands it the variables an array holds.
///
/// Laravel's `view('posts.show', ['post' => $post])`: inside the template `$post` is whatever the
/// call passed, and nothing in the template says what that is. The call is the declaration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderedTemplate {
    /// Fully qualified, without the leading separator, compared the way PHP compares functions.
    pub function: &'static str,
    /// Which argument names the template, and which holds its variables, counted from zero.
    pub name_at: u16,
    pub data_at: u16,
}

/// A call naming the class a callee's bound closures run as, in place of the one their
/// `@param-closure-this` declares.
///
/// Pest binds every test closure to a test case it generates from the class
/// `uses(Tests\TestCase::class)` names, while `test()` declares only PHPUnit's `TestCase`. Written
/// bare, the call configures the file it is in; with `->in('Feature')` after it, every file below
/// those directories of the file it is in -- which is how `tests/Pest.php` configures a suite.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfiguredClosureThis {
    /// The class the closures' declarations name. A configured class descends from it.
    pub declared: &'static str,
    /// The functions beginning such a call: Pest's `uses()` and `pest()`.
    pub functions: &'static [&'static str],
    /// The methods along it whose arguments name classes as the function's do:
    /// `pest()->extend(TestCase::class)`.
    pub methods: &'static [&'static str],
    /// The method applying it to directories rather than to its own file.
    pub within: &'static str,
}

/// A class whose instance renders its own template: inside the template `$this` is that instance,
/// and each public property its class declares is a variable of the same name.
///
/// Livewire's component: `render()` returns `view('livewire.counter')` with no variables, and the
/// template reads `$count` and `$this->increment()`. One with no `render()` renders the template
/// its name spells under the namespace: `App\Livewire\Posts\CreatePost` is
/// `livewire.posts.create-post`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderingClass {
    /// The class every such class descends from, fully qualified. Its own properties are not
    /// handed over: only the ones a class below it declares.
    pub class: &'static str,
    /// Where such classes are declared, fully qualified, and the template name the first segment
    /// below it spells: `App\Livewire` and `livewire`.
    pub namespace: &'static str,
    pub templates: &'static str,
}

/// A name a framework's loader `class_alias`es to a class at start, which no file declares.
///
/// Laravel's `DB` for `Illuminate\Support\Facades\DB`: written `use DB;`, or bare in a file with
/// no namespace, and answered by the class it names as if that were declared under both. Owned,
/// because which names exist is the project's configuration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassAlias {
    /// Fully qualified, without the leading separator -- nearly always one segment.
    pub alias: String,
    /// Fully qualified, without the leading separator.
    pub class: String,
}

/// One key of one catalog, as configuration named it.
///
/// Owned rather than `&'static`, because unlike a binding this is read out of the project: the ids a
/// container holds depend on which modules are installed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogEntry {
    pub catalog: &'static str,
    pub key: String,
    /// What the key names, where the configuration says.
    ///
    /// Optional and not omitted, because "registered" and "names that" are two facts and only the
    /// second can be missing -- a service built by a factory, or one whose class is itself a
    /// parameter. Dropping such an entry made a declared service look undeclared, which a report
    /// then said out loud.
    pub names: Option<CatalogName>,
    /// The file declaring it, when one does -- read only to tell the project's own keys from its
    /// dependencies', which is the workspace's question and not the provider's.
    pub declared_in: Option<std::path::PathBuf>,
}

/// What a catalog key's value is, for the two kinds a configuration file can state.
///
/// A container states both: a service id names a class, and a parameter holds a value whose shape
/// its own YAML says -- `serializer.formats: [json, xml]` is an array, and
/// `ContainerInterface::getParameter()` declares `array|bool|string|int|float|\UnitEnum|null`
/// because it cannot say which. One type rather than two optional fields, because a key names one
/// thing and a pair of `Option`s would let it name two.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CatalogName {
    /// Fully qualified, without a leading separator.
    Class(String),
    /// The shape of the value the key holds.
    Shape(phrost_model::BuiltinTy),
}

/// A method that calls the callback it is given, and what it hands it.
///
/// PHP has no syntax for what a callback receives, so the only thing that can say is a
/// `@param callable(A): B` on the method -- and a framework's most-used callback-takers do not
/// write one. `Container::singleton($abstract, $concrete)` documents `\Closure|string|null` and
/// calls the closure with the container; `Schema::create($table, $callback)` documents `\Closure`
/// and calls it with a `Blueprint`. So the closure's parameter has no type at all, and a project
/// built around service providers and migrations is full of them.
///
/// The other source for a fact `ParamRow::callback_params_ty` already holds, and read through the
/// same view. Ranked below it: a method that *does* document its callback is describing that exact
/// version of itself, where this is a statement made from outside about a framework's source.
///
/// # Why the positions are a list with holes
///
/// Which parameter is the interesting one differs per method, and a hole is not the same as a short
/// list. `bind` calls `$concrete($container, $parameters)` -- the container is first. `extend` calls
/// `$closure($service, $container)` -- first is the service being decorated, whose class is
/// whatever was bound and is not knowable from here, and the container is *second*. So a position
/// this cannot name has to keep its place, or `extend`'s container would land on the service.
///
/// # Why naming a class the project may not have is safe
///
/// The reader requires the named class to be one the project declares, so an entry naming
/// `Illuminate\Foundation\Application` is simply inert in a project that has `illuminate/container`
/// without Laravel's foundation -- which is right, because there the callback really does receive a
/// bare container. That is also why the entries name the *concrete* application rather than the
/// container that lexically passes itself: in a Laravel project the object is an `Application`, and
/// naming the narrower class would report every `$app->environment()` in every service provider.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InvokedCallback {
    /// The class or interface declaring the method being called. Anything below it is covered.
    pub class: &'static str,
    /// The method that invokes the callback.
    pub method: &'static str,
    /// Which of the callee's *parameters* the callback fills, counted from zero.
    ///
    /// A parameter and not a written position, so a named argument works:
    /// `singleton(concrete: fn ($app) => ..., abstract: 'x')` writes the callback first and fills
    /// the second parameter, and it is the parameter this matches.
    ///
    /// A method that accepts the callback in more than one parameter needs one entry per
    /// parameter: `resolving($abstract, $callback)` is also written `resolving($callback)`, where
    /// the closure fills `$abstract`. The two are the same claim about different parameters rather
    /// than one claim to generalise.
    pub argument: usize,
    /// What the callback's own parameters receive, in written order. Fully qualified, without a
    /// leading separator.
    ///
    /// `None` for a position the framework does not name -- a string, an array, or a class only the
    /// binding knows. Positions past the end of the list say nothing, so a list may be shorter than
    /// the callback but never have a gap left out.
    pub parameters: &'static [Option<&'static str>],
}

/// A method whose return class was decided by an argument to the call that produced its receiver.
///
/// The shape [`ArgumentBinding`] cannot express, because the literal is one call earlier:
///
/// ```php
/// $storage = $this->entityTypeManager->getStorage('node');
/// $node = $storage->load($id);
/// ```
///
/// `load()` declares `?EntityInterface` and *cannot* say more, and neither can the receiver's class:
/// one storage class serves dozens of entity types, since `SqlContentEntityStorage` is the storage
/// for most of them. So there is nothing at the `load()` call to bind to. The id passed to
/// `getStorage` is the only thing in the source that names the entity, and it is written on the
/// previous link of the chain.
///
/// What the declaration still decides is the *shape*: `load()` admits null and keeps admitting it,
/// because an id that matches no row really does return null. Only the class is replaced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ForwardedArgument {
    /// The class or interface declaring the method being called. Anything below it is covered.
    pub class: &'static str,
    /// The method whose return class this decides.
    pub method: &'static str,
    /// The method that produced the receiver. Its first argument is the key.
    pub through: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
    /// Whether the catalog's class is what comes back, or what comes back holds.
    pub yields: Forwarded,
}

/// How many of the catalog's class a forwarded method gives back.
///
/// The distinction the rule has to carry because the declaration cannot. `load()` and
/// `loadMultiple()` both declare `EntityInterface` -- one bare and one in an array -- and both are
/// equally uninformative about *which* entity, so the substitution has to be told where in the shape
/// to land. Reading it off the declared type instead would look cleverer and be worse: a method with
/// no `@return` at all would then get whichever shape the absence of a declaration happens to imply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forwarded {
    /// One of it, keeping whatever the declaration said about null.
    Instance,
    /// An array of it. `loadMultiple()` and `loadByProperties()` return entities keyed by id.
    ManyOf,
}

/// An attribute argument that is a key in a catalog, with the class it is written on as the value.
///
/// The other half of [`ForwardedArgument`], and the half that has to come out of the project rather
/// than out of a list here: which entity types exist depends on which modules are installed.
/// `#[ContentEntityType(id: 'node')]` on `Drupal\node\Entity\Node` is what makes `'node'` mean that
/// class, and nothing in any signature says it.
///
/// The class is not named because it is not a choice: it is the declaration the attribute is written
/// on. That is what makes this safe to derive from a file -- there is nothing to get wrong except
/// which attribute to look at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttributeKey {
    /// The attribute's name, matched against the last segment of the one written.
    pub attribute: &'static str,
    /// Which argument holds the key.
    pub argument: AttributeArgument,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// A catalog key a declaration spells out of two of its own arguments.
///
/// [`AttributeKey`] covers the case where the argument *is* the key. This covers the case where the
/// keys are computed: Drupal generates one route per entry of an entity type's `links`, named
/// `entity.<id>.<link template>`, and none of those route names is written anywhere. They are 277 of
/// the reference project's route names, including `entity.node.canonical` -- 42 call sites in its own
/// code, and the commonest way there is to address an entity.
///
/// What makes deriving them safe rather than a guess is that the framework derives them the same way,
/// from the same two values, in code this rule quotes: `EntityBase::toUrl()` builds
/// `"entity.{$this->entityTypeId}." . str_replace(['-', 'drupal:'], ['_', ''], $rel)`. The rewrite
/// list here is that `str_replace`, and nothing about the shape is Drupal-specific -- a framework
/// that names things out of a declaration's own arguments states its own template and its own
/// rewrites.
///
/// The keys name **nothing**, which is the whole reason this is not an [`AttributeKey`]: a route is
/// not a class, and recording the entity class as what the key names would make
/// `Url::fromRoute('entity.node.canonical')` return a `Node` -- the same trap
/// [`AttributeKey`]'s catalog documents for `getStorage`. What comes back instead is the entry's
/// *value*, which for a link template is the path, and which is exactly the line a route from YAML
/// already shows beside its name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DerivedKey {
    /// The attribute's name, matched against the last segment of the one written.
    pub attribute: &'static str,
    /// The argument whose array entries each derive one key. Lowering keeps an array argument's
    /// entries only for the arguments named here.
    ///
    /// A name rather than an [`AttributeArgument`], so a positional one is not expressible: an
    /// attribute written with a bare leading literal has one argument, and a rule that needs two of
    /// them at once needs both named.
    pub keys_of: &'static str,
    /// The argument whose literal value every derived key is named after.
    pub named_after: &'static str,
    /// The spelling, with `{name}` and `{key}` filled in.
    pub spelling: &'static str,
    /// Rewrites applied to `{key}` in order, before it goes into the spelling.
    pub rewrite: &'static [(&'static str, &'static str)],
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// A catalog key carried in a *declaration's own name*.
///
/// A hook implementation is a plain function whose name is the contract:
/// `app_core_notifications_alter()` runs when `notifications_alter` is dispatched, because it is
/// named `<extension>_<hook>` and the kernel goes looking for exactly that. Nothing else in the
/// checkout says the hook exists -- which is why the catalog filled from the *dispatch* calls cannot
/// answer whether one is real: a typo at the call registers itself there.
///
/// So this fills a second catalog from the other side, and the two answer different questions about
/// one name. `prefixed_by` names a catalog whose keys are the prefixes -- for Drupal, the installed
/// extensions -- and `or_prefix` is a literal one, for a hook's own documentation
/// (`function hook_theme()` in an `*.api.php`).
///
/// The **longest** prefix wins, so `app_core_x` claims `x` rather than `core_x` claiming it for a
/// module called `app`. Measured on one real checkout: 462 names, and a deliberate typo of each of
/// the six the project dispatches suggests the right one while the four it does not hold suggest
/// nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DerivedName {
    /// A catalog whose keys are the prefixes a name may begin with.
    pub prefixed_by: &'static str,
    /// A literal prefix that counts too, including its separator. Empty for none.
    pub or_prefix: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// Which argument of an attribute holds its id.
///
/// Two arms because both spellings are in one framework's own source, and reading the wrong one
/// finds nothing. `#[ContentEntityType(id: 'node')]` names it; `#[FormElement('textfield')]` does
/// not, because the attribute class declares a single constructor parameter and the convention is to
/// write it bare. Since Drupal 10 that is the commoner of the two.
///
/// A positional one is not free the way a named one is: keeping every leading positional literal in
/// a project costs eight thousand strings to gain seventy-seven, so naming an attribute here is also
/// what tells lowering to keep its literals at all. Name only the attributes whose id is actually
/// read.
///
/// Naming one buys a second thing, whichever arm it is: lowering reads the *older* spelling of the
/// same declaration, a Doctrine-style `@ContentEntityType(id = "node")` in the docblock, for a name
/// this list holds and no other. So a rule written once covers a framework mid-migration, where a
/// real checkout carries both spellings for years -- and neither arm has to be written twice, since
/// `id = "x"` lands as [`Self::Named`] and the shorthand `@FormElement("x")` as [`Self::At`], the
/// same slots the attribute forms land in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttributeArgument {
    /// `id: 'node'`, matched by name the way PHP matches an argument name.
    Named(&'static str),
    /// `'textfield'`, counted from zero over the whole argument list.
    At(u16),
}

/// An array key whose value is a key in a catalog.
///
/// The shape [`ArgumentBinding`] cannot express, because there is no call. A framework's render and
/// configuration arrays are written as literals -- `['#type' => 'textfield', '#title' => $t]` -- and
/// `textfield` is a registered element id exactly the way a service id is registered, with nothing in
/// the source but the key beside it to say so. No receiver, no method, no argument list: only the
/// key.
///
/// The key is compared exactly, because PHP compares an array key exactly. `'#Type'` is a different
/// slot and reaches nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArrayKeyBinding {
    /// The key as it is written, including any sigil the convention puts on it: `#type`.
    pub key: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// A key whose value names the class that documents the array's *other* keys.
///
/// The second half of what [`ArrayKeyBinding`] starts. A binding says the value under `#type` is an
/// element id; this says the class that id names is also where the rest of the array is described.
/// `['#type' => 'textfield', '#maxlength' => 128]` -- nothing declares `#maxlength` anywhere, and the
/// only statement in the project that it exists is a bullet in `Textfield`'s own docblock.
///
/// A rule rather than a hardcoded reading of every array-key binding, because the two Drupal
/// contributes do not both behave this way: `'library' => ['a/b']` names a library and its siblings
/// are more libraries, not properties of one. Which binding governs its neighbours is the
/// framework's to state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DocumentedKeys {
    /// The key whose value chooses the class: `#type`.
    pub discriminator: &'static str,
    /// The catalog that value is a key of: `element`. The same catalog the [`ArrayKeyBinding`]
    /// names, and the two must agree or the discriminator resolves to nothing.
    pub catalog: &'static str,
    /// What every documented key begins with: `#`. The whole of the marker.
    ///
    /// Deliberately not a heading. Drupal's element classes write the list under a sentence rather
    /// than under a fixed word -- `Properties:` on `Textfield`, "Here is a list of properties that
    /// are used during the rendering and form processing of form elements" on `FormElementBase` --
    /// so there is no heading to anchor on and matching one would silently lose every inherited
    /// property. A docblock bullet whose name carries this prefix and ends in a colon is exact
    /// enough, and it is only ever read on a class a catalog already named.
    pub prefix: &'static str,
}

/// A `@mixin` a framework's classes behave as though they wrote, and do not.
///
/// Eloquent is the case: `Model::__callStatic` builds a `static` and `__call` forwards to
/// `newQuery()`, so every `User::where(...)` is an `Eloquent\Builder<User>` call -- and nothing on
/// `Model` says so, since the forwarding lives in a method body. A written `@mixin Builder<static>`
/// would state exactly this, and that is how it is applied: the class is read as if it carried
/// the tag, which reuses what a written one already does -- members reachable and never declared,
/// losing to every real one, and the argument binding the mixin's templates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImpliedMixin {
    /// The class whose unknown calls are forwarded. Anything below it is covered, as with a written
    /// `@mixin`, because a subclass inherits the forwarding with the rest of the declaration.
    pub class: &'static str,
    /// The class the calls reach, fully qualified and without a leading separator.
    pub mixin: &'static str,
    /// Whether the mixin is parameterised by the class the call was made on -- `Builder<static>`,
    /// which is what makes `User::where(...)->first()` a `User` rather than a `Model`.
    pub with_static: bool,
}

/// A method whose `return X::class;` says the class forwards its calls to an `X`.
///
/// Laravel's facade: `class Theme extends Facade { protected static function getFacadeAccessor() {
/// return ThemeService::class; } }` makes every `Theme::dispatch(...)` a call on a
/// `ThemeService`, through `__callStatic` -- which is what a `@mixin ThemeService` would say, and is
/// read as one. An application's own facades rarely carry `@method` tags, so this body is the only
/// statement of what they reach. A string accessor names a container binding instead and is not
/// read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MixinReturnedBy {
    /// The method, as it is written.
    pub method: &'static str,
}

/// A class whose `@method` tags a generator wrote from another class's methods.
///
/// Laravel's facades are documented by a script that reads each facade's `@see` classes and writes
/// a `@method static` per public method -- dropping every method template on the way, so
/// `Cache::remember('k', 60, fn () => $users)` is tagged as returning `mixed` where
/// `Repository::remember()` says `TCacheValue`. Where the source declares the method, its
/// declaration is what the tag summarised, and it is read instead.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodsTaggedFrom {
    /// The class carrying the tags, fully qualified.
    pub class: &'static str,
    /// The class they were written from.
    pub source: &'static str,
}

/// Methods a class declares out of a caller's reach and its `__call` hands straight back.
///
/// Eloquent's `increment()` and `decrement()` are protected, and `Model::__call` names them before
/// it forwards anything to the builder: `if (in_array($method, ['increment', ...])) return
/// $this->$method(...$parameters);`. So `$post->increment('views')` from a controller is refused by
/// PHP, sent to `__call`, and runs the declaration after all -- which is what it answers, what it
/// returns, and what an editor should offer. The list lives in a method body, so a framework says it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpenedByCall {
    /// The class whose `__call` reopens them. Anything below it is covered, since a subclass
    /// inherits the magic method with the rest.
    pub class: &'static str,
    pub methods: &'static [&'static str],
}

/// A property no declaration writes, answered by a class's `__get`.
///
/// The other direction of [`ArgumentBinding`]. A binding reads a *string* a call was given and says
/// what comes back; this reads the *name a property was spelled with* and says the same thing. Both
/// exist because the same fact is written two ways: `$node->get('field_amount')` and
/// `$node->field_amount` are the same read, and only the first has an argument to bind to.
///
/// A class with a `__get` answers to every name, so nothing may be *reported* about one -- and this
/// does not change that. It says what a name the framework recognises holds, and leaves every other
/// name exactly as dynamic as it was.
///
/// [`Self::holds`] is one class for every such property rather than something looked up per name,
/// because that is what these accessors are: `__get` on a Drupal entity hands back a field item list
/// whichever field was asked for, and the list is the same class for all of them. A framework whose
/// magic property varies by name has a different fact and would need a different contract.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MagicProperty {
    /// The class or interface whose `__get` answers. Anything below it is covered, because a
    /// subclass inherits the accessor with the rest of the declaration.
    pub class: &'static str,
    /// Which names this speaks for.
    pub name: MagicName,
    /// The class such a property holds, fully qualified and without a leading separator.
    pub holds: &'static str,
    /// Whether reading it can give null.
    ///
    /// Asked rather than assumed, because the two Drupal shapes disagree: an entity always has a
    /// list for a field it has, and the entity a reference field points at may be gone. Getting this
    /// wrong in the lenient direction is the one thing this contract can do that a missing answer
    /// could not -- see the null-safety rule in `CLAUDE.md`.
    pub nullable: bool,
}

/// A property no declaration writes, read as calling the method of the same name and then `through`
/// on what it hands back -- where that method returns a `returns`.
///
/// Eloquent's relations: `$post->author` is `$post->author()->getResults()`, because `__get` finds
/// the method, sees a relation, and loads it. The method is the project's own declaration, so it is
/// what goto lands on and what says which class comes back; the framework states only the rule that
/// connects the two spellings, which no declaration can.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodBackedProperty {
    /// The class or interface whose `__get` does this. Anything below it is covered.
    pub class: &'static str,
    /// What the method has to return for its name to be such a property.
    pub returns: &'static str,
    /// The method called on what it returns, whose result the property holds.
    pub through: &'static str,
    /// The suffix of the attribute that counts it, which a query's `withCount()` fills: Laravel's
    /// `$post->comments_count` beside `comments()`. Named after the method in snake case, and
    /// nothing until a query counted it.
    pub counted: Option<&'static str>,
    /// Relations that load what a key in the model's own row points at. See [`KeyedRelation`].
    pub keyed: &'static [KeyedRelation],
}

/// A relation that loads the row a key in the model's own row points at: Laravel's `BelongsTo`
/// reads `{method}_id`. Its result is empty only when the key can be: a column that cannot be null
/// and is a foreign key the database enforces always points at a row that exists, so
/// `$user->account` over `foreignIdFor(Account::class)->constrained()` is an `Account` and not
/// `?Account`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyedRelation {
    /// The relation class, fully qualified. Anything below it is covered.
    pub relation: &'static str,
    /// What the key column adds to the method's name in snake case: `_id`.
    pub suffix: &'static str,
}

/// Properties a class states in an array it writes, which `__get` answers from a row nobody
/// declared.
///
/// Eloquent's model is the case: `$casts = ['published_at' => 'datetime']` (or a `casts()` method
/// returning it) says `$post->published_at` is a date, and `$fillable = ['title']` that `title` is
/// an attribute at all. The columns themselves are in a database phrost never reads, so what a
/// class writes here is the whole of what anything can know -- and what it does not say, whether a
/// column is nullable, is said by [`Self::nullable`] for every one of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatedProperties {
    /// The class whose `__get` answers them. Anything below it is covered.
    pub class: &'static str,
    /// Properties and methods whose array maps a name to what it holds: `casts`.
    pub typed: &'static [&'static str],
    /// Properties whose array lists names and says nothing about them: `fillable`.
    pub listed: &'static [&'static str],
    /// Attributes on the class whose string list says the same: Laravel 12's `#[Fillable([...])]`.
    /// Fully qualified; matched by the last segment, as an attribute is written.
    pub listed_by_attribute: &'static [&'static str],
    /// What each spelling a typed array writes means. Matched whole first and then by what comes
    /// before a `:`, which is how `datetime:Y-m-d` and `decimal:2` are written.
    pub spellings: &'static [(&'static str, StatedType)],
    /// A prefix that wraps another spelling and changes nothing about the type: `encrypted:`.
    pub wrapping: &'static [&'static str],
    /// What a class written as the cast means, where it is not an enum -- an enum is itself.
    pub classes: &'static [(&'static str, StatedType)],
    /// A class implementing this interface is a cast of the project's own, and what its method of
    /// this name returns is what the property holds -- with whatever it says about null, since the
    /// framework hands such a cast the null rather than answering for it.
    pub custom: Option<(&'static str, &'static str)>,
    /// Names every such class has without stating them: `created_at`, `updated_at`.
    pub implicit: &'static [(&'static str, StatedType)],
    /// The key every such class has unless it says otherwise: Eloquent's incrementing `id`.
    pub key: Option<StatedKey>,
    /// Whether a read can give null. A column's nullability lives in the database, so only `true`
    /// is honest without a schema -- and the null-safety rule in `CLAUDE.md` says which way to err.
    pub nullable: bool,
}

/// The key a [`StatedProperties`] class has by default, and what saying otherwise looks like.
///
/// Eloquent casts `$primaryKey` to `$keyType` only while `$incrementing`, all three defaulted on
/// the model -- so a class, or a trait it takes (`HasUuids`), declaring any of them or the methods
/// reading them has a key this cannot name, and is left alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatedKey {
    pub name: &'static str,
    pub ty: StatedType,
    /// Members whose declaration below the rule's class changes the key.
    pub unless: &'static [&'static str],
}

/// What a [`StatedProperties`] spelling means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatedType {
    Builtin(phrost_model::BuiltinTy),
    Class(&'static str),
}

/// A property no declaration writes, read through the accessor method its name spells.
///
/// Eloquent's two spellings of an accessor: `getExcerptAttribute()` is read as `$post->excerpt`,
/// and so is `excerpt(): Attribute` -- `__get` studly-cases the name between `prefix` and `suffix`
/// for the first, and camel-cases it and looks for a method returning `attribute` for the second.
/// The method is the project's own declaration, so it is where goto lands and what says what comes
/// back; the framework states only how the two names correspond, which no declaration can.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AccessorProperty {
    /// The class or interface whose `__get` does this. Anything below it is covered.
    pub class: &'static str,
    /// Around the studly-cased name, for the method spelling: `get` and `Attribute`.
    pub prefix: &'static str,
    pub suffix: &'static str,
    /// What a method of the camel-cased name returns to be the other spelling.
    pub attribute: &'static str,
}

/// A method a class declares under a prefixed name and is called by without the prefix, with its
/// first parameter supplied by the framework -- and called on a builder for that class too.
///
/// Eloquent's local scopes: `scopePublished($query)` is called `Post::published()` and
/// `$query->published()`, the builder is passed in as `$query`, and what comes back is the builder.
/// Nothing declares `published`; the model's and the builder's `__call` find the prefixed method.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PrefixedMethod {
    /// The class or interface whose `__call` does this. Anything below it is covered.
    pub class: &'static str,
    /// What the declaration's name starts with, in front of the called name's first letter
    /// upper-cased.
    pub prefix: &'static str,
    /// The builder the call is made on, or hands back: `builder<class>`.
    pub builder: &'static str,
    /// An attribute marking a method that is called the same way *under its own name*: Laravel
    /// 12's `#[Scope] protected function popular($query)`, called `Post::popular()`. Called from
    /// a scope that cannot reach it, it goes to the magic method like the prefixed spelling.
    pub marker: Option<&'static str>,
}

/// A method no declaration names, answered by `__call` for every name that begins with a prefix --
/// and answered with the receiver, so a chain through one goes on.
///
/// Laravel's dynamic wheres: `whereEmail($email)` is `where('email', '=', $email)`, which the query
/// builder's `__call` reads off the name before handing back itself. What forwards to the builder
/// carries it along -- the Eloquent builder through its `@mixin`, a model through the builder it
/// creates -- and that forwarding is stated elsewhere, so only the builder is named here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FluentPrefix {
    /// The class whose `__call` does this. Anything below it is covered, and anything mixing it in.
    pub class: &'static str,
    /// What the name starts with, before an upper-case letter.
    pub prefix: &'static str,
}

/// A `__get` property whose type is looked up per name, in a table the project itself states.
///
/// [`MagicProperty`]'s own note says why this cannot be one: `holds` there is one class for every
/// name the accessor answers to, and here every name holds something different. Drupal's fields are
/// the case -- `$node->get('field_when')->value` is a `string` and `$node->get('field_count')->value`
/// is an `int`, and the only difference between the two reads is a string written two links back.
///
/// So the rule is a *lookback*, exactly as [`ForwardedArgument`] is: the receiver was produced by
/// `through(<key>)`, the key belongs to `catalog`, and the table filed under that key says what each
/// name holds. What fills the tables is [`PropertyTable`], which comes out of the project.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TabledAccess {
    /// The class or interface whose `__get` answers. Anything below it is covered.
    pub class: &'static str,
    /// The method that produced the receiver. Its first argument is the key.
    pub through: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
    /// The method that hands the whole table back at once, when the class has one.
    ///
    /// `FieldItemListInterface::getValue()` returns one entry per delta, each an array of the
    /// properties -- so what it answers is a list of the shape the table describes, and it is the
    /// same fact read the other way round. Only a [`PropertyTable::complete`] one may be read this
    /// way: a shape claims that the names it lists are the names there are.
    pub rows: Option<&'static str>,
}

/// What one *kind* of value is called, property by property.
///
/// Keyed by kind rather than by catalog key, because that is the grain the declaration has: Drupal
/// writes one `propertyDefinitions()` per field type and four hundred fields share thirty of them.
/// Which key is of which kind is [`CatalogKind`], and the two are joined at the read.
///
/// Owned and produced per project, because both halves come out of the checkout.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PropertyTable {
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
    /// The kind these are the properties of, in the framework's own vocabulary.
    pub kind: String,
    /// One entry per property, in the order the declaration writes them. `None` where the
    /// declaration names a type this cannot render in PHP's vocabulary -- the property is *there*,
    /// and saying nothing about what is in it is the honest answer.
    pub properties: Vec<(String, Option<phrost_model::BuiltinTy>)>,
    /// Whether these are all of them.
    ///
    /// A partial table still answers for a name it holds, and may never become an `array{...}`: a
    /// shape claims the keys it lists are the keys there are, and one that quietly omitted a key
    /// would state a type narrower than the truth -- which is how a checker reports working code.
    pub complete: bool,
}

/// A declaration that states, in a method's body, the properties of the kind it declares.
///
/// The other source for [`PropertyTable`], and the one a framework's *users* write. Drupal's field
/// types are the case:
///
/// ```php
/// #[FieldType(id: 'store_sales')]
/// class StoreSalesItem extends FieldItemBase {
///   public static function propertyDefinitions($d): array {
///     $properties['date'] = DataDefinition::create('string')->setLabel(...);
///     $properties['rate'] = DataDefinition::create('float');
///     return $properties;
///   }
/// }
/// ```
///
/// Read at ingest, where the body still is. Core's own kinds cannot come from here -- phrost lowers
/// bodies for project code and deliberately not for vendor, which is the memory rule -- so those
/// are the provider's own table and these are everything the project adds to it.
///
/// # All or nothing, per key
///
/// A statement this cannot read makes the table [`PropertyTable::complete`] `false` rather than
/// dropping it: the keys it did read are keys the declaration really writes, and a name it holds is
/// worth answering. What a partial table may not do is become an `array{...}` -- see that field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeclaredTable {
    /// The attribute marking the declaring class, matched on its last segment. Its literal is the
    /// kind, which is why it has to be one lowering already keeps -- see `KeyedAttributes`.
    pub attribute: &'static str,
    /// Which of its arguments holds the kind.
    pub argument: AttributeArgument,
    /// The method whose body holds the table.
    pub method: &'static str,
    /// The static methods whose first argument names what a key holds. Anything else on the right
    /// of the `=` leaves the table partial.
    pub builders: &'static [&'static str],
    /// What each name those builders take means in PHP. A name not here leaves the property in the
    /// table with nothing said about it, which is the honest answer and keeps the key.
    pub kinds: &'static [(&'static str, phrost_model::BuiltinTy)],
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// What kind of thing one catalog key is, in the framework's own vocabulary.
///
/// The other half of [`PropertyTable`]. `field_customer_type` is a `list_string`, which its own
/// `field.storage.*.yml` states and nothing in PHP does -- and `list_string` is what has a table.
///
/// Deliberately not a [`CatalogName::Class`]: a kind is not a class, and recording one as such
/// would make `$node->get('field_customer_type')` return the field-type plugin rather than the
/// list -- the trap [`AttributeKey`]'s own catalog documents for `getStorage`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogKind {
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
    /// The key.
    pub key: String,
    /// What kind of thing it is.
    pub kind: String,
}

/// Which property names a [`MagicProperty`] speaks for.
///
/// Two arms because the two facts are known at different times. `entity` is a name Drupal's own
/// source writes, so it is settled here; a field name is written in the project's configuration, so
/// the only thing settled here is *which catalog* to look it up in -- and the catalog is filled from
/// the checkout like every other one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagicName {
    /// One name, written out.
    Exactly(&'static str),
    /// Every key of a catalog. A name this provider chose; nothing below the seam knows what it
    /// means.
    KeyOf(&'static str),
}

/// One key of one catalog whose value is a place in a file rather than a class.
///
/// A route name, a permission id: declared in YAML, referred to from PHP by a string, and naming
/// nothing PHP has. There is no class to resolve such a key to -- only somewhere to go and something
/// to say about it -- which is why it cannot travel as a [`CatalogEntry`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlaceEntry {
    pub catalog: &'static str,
    pub key: String,
    /// The file the key is declared in.
    pub path: PathBuf,
    /// Zero-based, as the protocol counts them. See
    /// [`KeyedEntry`](crate::keys::KeyedEntry) for why a position and not an offset.
    pub line: u32,
    pub character: u32,
    /// How to describe it in a list: a route's path, a permission's title. Empty when the
    /// declaration says nothing worth showing, which is honest -- the key alone is still the answer.
    pub detail: String,
}

/// A docblock convention that names the declaration documenting this one.
///
/// `{@inheritdoc}` is PHP's own way of saying it and needs no contract. A framework can have a
/// second, and Drupal's is the whole reason this exists: `Implements hook_form_alter().` is the
/// entire docblock its standard asks of a hook implementation, because what the parameters mean is
/// documented on the hook and not on each of the hundreds of implementations. The sentence is not
/// decoration -- it names a declaration the index already holds, and following it is the difference
/// between a hover showing one line and one showing twenty.
///
/// Two facts in one rule rather than two, because they are the same fact read twice: what the prose
/// names, and -- where the framework dispatches by that name -- what the declaration under it has to
/// be called for the claim to be true.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DocumentedBy {
    /// The words the prose opens with when it is making the claim: `Implements`.
    ///
    /// Matched case-insensitively at the start of the prose and nowhere else. A sentence that
    /// mentions the word in passing is prose, and reading it as a claim would attribute another
    /// declaration's documentation to this one.
    pub lead: &'static str,
    /// What the named declaration is called before the part that varies: `hook_`.
    ///
    /// Doing double duty. After the lead it says where the name begins; anywhere else in the same
    /// docblock it marks a *reference* -- `@see hook_theme()` and a mention in prose are both names
    /// a reader wants to navigate to, and nothing else in a comment identifies one.
    pub names: &'static str,
    /// The catalog holding the names a declaration making the claim may begin with.
    ///
    /// Drupal dispatches a hook by *name*: `hook_form_alter` runs whatever `<extension>_form_alter`
    /// each installed extension declares. So a claim written over a function no installed extension
    /// could own is a claim nothing will honour, and the function silently never runs -- which is
    /// decidable from the two names and the catalog, with nothing guessed.
    ///
    /// The convention this describes joins the parts of a name with `_`, and reads an uppercase run
    /// in the reference as a hole standing for one or more of them: `hook_form_FORM_ID_alter` is
    /// implemented by `mymodule_form_views_exposed_form_alter`. That is not a list written from
    /// memory -- of the 333 hooks one real checkout declares, every placeholder is spelled that way
    /// and nothing else is spelled in uppercase at all.
    ///
    /// `None` for a convention that says nothing about the claiming declaration's name, and then
    /// nothing is checked.
    pub implementers: Option<&'static str>,
    /// The catalog holding the names the project dispatches itself, filled from [`DispatchedName`].
    ///
    /// The other half of the same question, and the reason it has to be here rather than looked up:
    /// a claim names a declaration, and a name a call dispatches *has* no declaration. Without this
    /// the reference is a dangling one -- nothing to navigate to and nothing that could say whether
    /// the name exists -- which is exactly the state that made the companion check unshippable.
    ///
    /// `None` for a convention whose framework dispatches nothing by a literal name.
    pub dispatched: Option<&'static str>,
}

/// A declaration whose docblock the framework shows to a person rather than merely storing.
///
/// Drupal's update system is the case and the whole reason this exists. The description beside a
/// pending update -- on `update.php`, and in what `drush updb` lists before it asks whether to go
/// ahead -- *is* that function's docblock, with the stars stripped and the lines joined:
/// `update_get_update_list()` reads one with `ReflectionFunction::getDocComment()` and
/// `UpdateRegistry::getPendingUpdateInformation()` reads the other. So a generated `@return string`
/// there is not a line a reader may skip. It is four words in the middle of a sentence somebody
/// reads while deciding whether to run a migration.
///
/// Two rules follow and they point opposite ways, which is why this is a rule of its own rather
/// than an arm of [`DocumentedBy`]: the docblock is **required**, because a declaration without one
/// gets an empty description, and it takes prose and **nothing else**. Of the 217 update hooks one
/// real checkout declares, not one carries a tag; the only four docblocks that do are in the
/// `*.api.php` files that document the hook itself and are never reflected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UserFacingDocblock {
    /// What the declaration's name carries before the part that varies: `_update_`.
    ///
    /// Matched at its **last** occurrence, and something has to come before it, because that is the
    /// framework's own reading: `^(?<module>.+)_update_(?<version>\d+)$` is greedy on the module,
    /// so `a_update_b_update_8001` is module `a_update_b` and update `8001`.
    pub names: &'static str,
    /// Whether what follows the name has to be a number.
    ///
    /// Load-bearing rather than decoration, and Drupal's own comment beside the pattern says why:
    /// "there are other possible functions which match `_update_`". `mymodule_update_status()` is an
    /// ordinary function whose docblock nobody publishes, and exempting it would take a
    /// documentation rule off a declaration the rule is right about.
    pub numbered: bool,
}

/// A method that dispatches an extension point by the name it is handed.
///
/// The shape no other contract expresses, and the thing that makes a project's own extension points
/// exist as far as the index is concerned. `$this->moduleHandler->invokeAll('notifications_alter')`
/// declares `hook_notifications_alter` as surely as a `*.api.php` file would, and rather less
/// visibly: there is no declaration anywhere, so the implementations scattered across the project
/// reference a name nothing holds. **The call site is the contract.**
///
/// # What is claimed, and what is not
///
/// Only a project's *own* code contributes, because only a project's own code has bodies in the
/// index -- and that is the right boundary rather than a limitation. A framework's own extension
/// points are documented in files the index already reads, so what is left is exactly the set
/// nobody documents.
///
/// Two gates, and the second is the one that matters. The name has to be written as a plain string
/// literal, and the **receiver has to be the class this names** -- not merely a method spelled the
/// same way. `alter` is a method any project declares for something of its own: one real checkout
/// calls it three times on a breadcrumb builder and a form-alter service, and a rule keyed on the
/// method name alone would index whatever those were passed as extension points. The same "sometimes
/// a key" mistake [`ArgumentBinding`] records, arriving from the other end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DispatchedName {
    /// The class or interface declaring the dispatching method. Anything below it is covered,
    /// because an override inherits the contract along with the signature.
    pub class: &'static str,
    /// The dispatching method.
    pub method: &'static str,
    /// Which argument holds the name, counted from zero.
    ///
    /// Any position, unlike [`ArgumentBinding`]: `invoke($module, $hook)` names the extension first
    /// and the hook second, and reading the first would claim a module name as an extension point.
    pub argument: u16,
    /// Whether that argument may also be a list of names rather than one.
    ///
    /// `alter(['field_widget_multivalue_form', 'field_widget_multivalue_' . $id . '_form'])` is the
    /// documented spelling for dispatching a general and a specific form of one point at once. Every
    /// element that is a plain literal is a name; one built from an expression is not, and is
    /// silence rather than a guess.
    pub several: bool,
    /// What the declaration the dispatched name implies is called before the part the call writes.
    ///
    /// The same prefix [`DocumentedBy::names`] carries, and it has to be stated again here because
    /// the two contracts are read in different places: what a docblock references is `hook_x`, and
    /// what the call writes is `x`. A provider spelling them differently would fill a catalog no
    /// reader can index into, which is what `the_dispatch_catalog_is_spelled_the_way_the_convention
    /// _reads_it` exists to refuse.
    pub names: &'static str,
    /// And after it.
    ///
    /// `alter('x')` runs `hook_x_alter` while `invokeAll('x')` runs `hook_x`, so the suffix is a
    /// property of the dispatching method rather than of the framework. Empty for the common case.
    pub suffix: &'static str,
    /// A name this provider chose. Nothing below the seam knows what it means.
    pub catalog: &'static str,
}

/// How a framework's docblock annotations are written as PHP attributes instead.
///
/// Doctrine's annotation syntax was every plugin system's way of putting metadata on a class before
/// PHP had attributes, and the frameworks that used it are midway through the move. Drupal ships
/// both spellings of every plugin type side by side -- `Drupal\rest\Annotation\RestResource` and
/// `Drupal\rest\Attribute\RestResource`, one segment apart -- so what a `@RestResource(...)` should
/// become is answerable from the project's own declarations rather than from a table of the hundred
/// annotations a framework has.
///
/// One segment and a short list, and the segment is what does the work. The list is for the
/// annotations written as an argument's *value*: `label = @Translation("Content")` becomes
/// `label: new TranslatableMarkup('Content')`, whose class is in no attribute namespace at all
/// because it is not an attribute -- it is the object the annotation always stood for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AnnotationRewrite {
    /// The namespace segment a framework's attribute classes sit in, matched against the
    /// second-to-last segment of a candidate's fully qualified name.
    pub attributes_in: &'static str,
    /// Annotations that stand for a plain object rather than for an attribute.
    pub values: &'static [AnnotationValueClass],
}

/// An annotation written as a value, and the class to construct in its place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AnnotationValueClass {
    /// The annotation's name as written, without the `@`.
    pub annotation: &'static str,
    /// Fully qualified, with no leading separator.
    pub constructs: &'static str,
}

/// A scalar in a configuration file that names something the index holds.
///
/// The half of a framework project nothing above PHP can see. Drupal declares a service's class, a
/// route's controller and a menu link's route in YAML; Symfony does the same in its own files with
/// its own keys. Measured on one real checkout: **1 414 such references in 72 of the project's own
/// YAML files**, every one naming something already in a catalog or the def-map, and not one of them
/// navigable -- against 499 untyped receivers left in all of its PHP.
///
/// Which key means what is the framework's to say and nothing else's, which is why this is a rule
/// and not a reader. The generic layer scans a line, asks which rule the position matches, and
/// answers from the table the rule names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct YamlReference {
    /// Matched against the *end* of the file's name: `routing.yml`, `services.yml`,
    /// `links.menu.yml`. Drupal names a file after the extension that owns it, so the suffix is
    /// what says which kind of file this is.
    pub in_files: &'static str,
    /// The key the value is written under, matched against the innermost key on the path to it:
    /// `_controller`, `route_name`, `class`. Empty matches any key, which is what a reference
    /// recognised by its own *spelling* needs -- see [`Self::prefix`].
    pub under: &'static str,
    /// A character the value must open with, and which is not part of the name: `@` for a Symfony
    /// container reference, which is how both frameworks spell one. Empty for a value that is the
    /// name as written.
    pub prefix: &'static str,
    /// A mark that may follow the prefix and says the referent may be missing: the `?` of
    /// `'@?id'`, which the container resolves to null when nothing registers `id`. Not part of the
    /// name, and a miss behind it is judged as an open catalog's is -- a near miss is worth a
    /// warning and an absence is what the mark is for. Empty for a rule with no such spelling.
    pub optional: &'static str,
    pub names: YamlTarget,
}

/// What a configuration value names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum YamlTarget {
    /// A key of a catalog this provider filled. The name is the provider's own word for it, and
    /// nothing below the seam knows what it means.
    CatalogKey(&'static str),
    /// A fully qualified class name, with or without a leading separator.
    Class,
    /// A class, or a class and one of its methods: `\Drupal\node\Controller\NodeController::view`.
    ///
    /// Its own kind rather than [`Self::Class`] with a rule about `::`, because where a reader lands
    /// differs: on the method when one is named, and on the class when none is.
    Callable,
    /// What a container's `factory` writes, which is three shapes and only two of them name a
    /// class.
    ///
    /// `Foo::create` is a class and a method, `\Foo` is a class, and a bare word is a **method**
    /// of the service written beside it -- `factory: ['@cloud.client_factory', 'authClient']`,
    /// where the `@id` is read by the container-id rule and this is the rest. Read as a class it
    /// resolved to nothing and the method it names was reached by nothing, which is how a live
    /// service reads as dead code.
    Factory,
    /// Several keys of one catalog in one scalar, separated by any of `separators`.
    ///
    /// Drupal writes `_permission: 'administer nodes+access content'` for *either* and
    /// `'a,b'` for *both*, and a reader on one of them wants that one.
    CatalogKeys {
        catalog: &'static str,
        separators: &'static str,
    },
}

/// What a framework's dependency-injection container builds, and the static calls that reach into
/// it from outside.
///
/// Two halves because the finding needs both: a static call that fetches a service is only worth
/// saying anything about in a class the container constructs -- there the dependency could be
/// handed in, and fetching it instead hides it from every reader and every test. Anywhere else a
/// static call is the only way to reach the container at all.
#[derive(Clone, Copy, Debug)]
pub struct ContainerRule {
    /// Interfaces whose implementers the container constructs, by their own factory.
    pub built_through: &'static [&'static str],
    /// Catalogs whose entries are classes the container constructs: a service definition.
    pub built_from: &'static [&'static str],
    /// The static calls that fetch something out of the container.
    pub reaches: &'static [ContainerReach],
}

/// One static call that reaches into the container.
#[derive(Clone, Copy, Debug)]
pub struct ContainerReach {
    /// The class the call is made on.
    pub class: &'static str,
    /// Whether a class descending from it counts too.
    pub descendants: bool,
    /// The methods that reach in, or empty when every static call on the class does.
    pub methods: &'static [&'static str],
    /// What to hand the class instead, for the message: "the service it asks for".
    pub instead: &'static str,
}
