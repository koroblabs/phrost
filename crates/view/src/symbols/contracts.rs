//! What a declaration promises beyond what its signature can say.
//!
//! A method whose return type is `object`, or `array`, or `?Node`, and whose docblock says which
//! class, which element, or which branch of its own union a given call gets. Split from the member
//! lookup beside it because the two grow for different reasons: a new kind of *declaration* is an arm
//! in the lookup, and a new thing a docblock can promise is a contract and a reader for it.
//!
//! Everything here needs the *call* -- what it wrote, how many arguments, what the receiver was
//! written with -- which is what [`CallFacts`](super::CallFacts) carries and what separates these
//! from anything the declaration answers alone.

use phrost_model::{Absence, BuiltinTy, Name};
use phrost_sema::{MemberKind, MemberProvenance};

use phrost_hir::FileHir;
use phrost_index::Forwarded;

use super::call::{argument_for, only_expression};
use super::{CallFacts, INHERIT_DEPTH, arguments_from};
use crate::references::SentinelReach;
use crate::resolve::HIERARCHY_DEPTH;
use crate::{
    resolve::{Held, RelativeScope},
    view::IndexView,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct Shape {
    absent: Option<Absence>,
    many: bool,
}

impl Shape {
    fn of(absent: Option<Absence>, many: bool) -> Self {
        Self { absent, many }
    }

    /// The class, wrapped as the tag wrote it: the container first, then the sentinel over it.
    ///
    /// That order and not the other: `@return ?T[]` is "an array, or nothing", never "an array of
    /// things that may be nothing".
    fn around(self, held: Held) -> Held {
        let held = match self.many {
            true => Held::ListOf(Box::new(held)),
            false => held,
        };
        beside_the_sentinel(held, self.absent)
    }
}

/// A value with the sentinel its declaration also admitted put back beside it.
///
/// The one place a resolved [`Absence`] becomes a type, so the rule lives once: a `null` wraps and a
/// `false` stands as an operand. [`Held::one_of`] is what draws that line -- it lifts a null out into
/// a [`Held::Nullable`] and leaves a `false` in the union -- which is the same split the interned
/// union constructor makes one layer down, for the same reason. `false` is a value PHP hands back and
/// `null` is the absence of one, and a reader guards them differently.
fn beside_the_sentinel(held: Held, absent: Option<Absence>) -> Held {
    let Some(absent) = absent else {
        return held;
    };
    let mut operands = vec![held];
    if absent.admits_false() {
        operands.push(Held::Builtin(BuiltinTy::False));
    }
    if absent.admits_null() {
        operands.push(Held::Builtin(BuiltinTy::Null));
    }
    IndexView::one_of(operands)
}

/// What a catalog key turned out to name, for the two kinds of catalog a container has.
enum Named {
    Class(String),
    Shape(phrost_model::BuiltinTy),
}

/// Which branch a conditional settled on, or every branch a call may take where nothing did.
///
/// Several is not a failure to answer: the call is one of them, and read for this call each says
/// more than the declaration's own union can -- a template there stands for anything, and here for
/// whatever the argument is.
#[derive(Clone)]
enum Settled {
    One(phrost_sema::ReturnBranch),
    /// Any of these, which is what a call nothing settles gets: each branch read for this call.
    Any(Vec<phrost_sema::ReturnBranch>),
}

impl Settled {
    /// One branch, or the several a call may take -- the same one twice being one.
    fn of(mut branches: Vec<phrost_sema::ReturnBranch>) -> Self {
        branches.dedup();
        match branches.as_slice() {
            [one] => Self::One(one.clone()),
            _ => Self::Any(branches),
        }
    }
}

impl IndexView<'_> {
    /// The declared type with a framework's non-null contract applied, when one is.
    ///
    /// Applied on the way *out* rather than at ingest, so the declaration keeps saying what it says:
    /// hover on `Connection::query()` still shows the `?StatementInterface` that is written there,
    /// and only the value the call produces is treated as the framework's contract describes it.
    ///
    /// Three gates in order of what they cost, because this runs for every member access in a file:
    /// the kind, then one array index for the nullability, then a hash of the name. The hierarchy is
    /// only walked for a nullable method whose name something has a contract for.
    pub(super) fn contracted(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        ty: phrost_ty::TypeId,
    ) -> phrost_ty::TypeId {
        // The twin that answers "what did a contract decide" reads the declaration as written, so
        // that the ordinary checks speak where a contract had silenced them.
        if self.contracts == crate::view::Contracts::Lifted {
            return ty;
        }
        if kind != MemberKind::Method {
            return ty;
        }
        let Some((sentinel, without)) = self.sentinel_taken_off(ty) else {
            return ty;
        };

        let contracted = self
            .skeleton
            .returns()
            .unproduced_declarers(name, sentinel)
            .any(|declarer| {
                class_fqn.eq_ignore_ascii_case(declarer)
                    || self.descends_from(class_fqn, declarer, HIERARCHY_DEPTH)
            });

        match contracted {
            true => {
                self.contract_applied.set(true);
                without
            }
            false => ty,
        }
    }

    /// The sentinel a type admits and the id of what is left once it is taken off.
    ///
    /// Two shapes, because the type table stores the two sentinels differently: a null is lifted
    /// into a [`phrost_ty::TypeRef::Nullable`] wrapper, and a `false` stays an operand of a union.
    ///
    /// A union of exactly two, and that is a limit rather than a choice: what is left has to *have*
    /// an id, and for two operands the survivor already does. Taking one out of three would mean
    /// interning a new union, and this side of the index is read-only. `PDOStatement|false` and
    /// `int|false` -- every shape this contract is written for -- are two.
    fn sentinel_taken_off(&self, ty: phrost_ty::TypeId) -> Option<(BuiltinTy, phrost_ty::TypeId)> {
        match *self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => Some((BuiltinTy::Null, inner)),
            phrost_ty::TypeRef::Union(operands) => {
                let types = self.skeleton.types();
                let [left, right] = types.operands(operands) else {
                    return None;
                };
                let sentinel = |id| match types.get(id) {
                    phrost_ty::TypeRef::Builtin(builtin @ (BuiltinTy::Null | BuiltinTy::False)) => {
                        Some(*builtin)
                    }
                    _ => None,
                };
                match (sentinel(*left), sentinel(*right)) {
                    (Some(found), None) => Some((found, *right)),
                    (None, Some(found)) => Some((found, *left)),
                    // Two sentinels and nothing else, or neither: no contract can be about it.
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The branch of a method's declared union that a call leaving an argument out settles.
    ///
    /// The class-valued sibling of the chain contract, for the shape where what decides is the call
    /// itself rather than an earlier link of the chain. `\Drupal::classResolver()`
    /// declares `ClassResolverInterface|object` because the one method is two -- bare it hands back
    /// the resolver, with a class name it hands back an instance of that class -- and the union that
    /// covers both says nothing about either.
    ///
    /// # Why this cannot claim a type the declaration does not mention
    ///
    /// [`Self::declares_return_operand`] is the whole of it. The branch a framework names is looked
    /// up among the operands of the method's *own* declared return type, and a name that is not one
    /// of them answers nothing. So this is a selector over what the declaration already wrote, and a
    /// wrong entry in a provider's list misses an answer rather than inventing one -- which is the
    /// property every other contract at that seam has and the reason they are safe.
    ///
    /// It also means the contract disarms itself when the framework's signature changes under it:
    /// a `classResolver()` that one day declares a single class stops matching, silently, in the
    /// direction that says nothing rather than the one that says something false.
    ///
    /// Gated on the method name first -- a scan of the handful of short strings a framework
    /// contributes -- so every other call on every other receiver pays that and nothing more. Then
    /// the arity, then the hierarchy climb, then the operand walk, which is the order their cost
    /// runs in.
    ///
    /// `arity` is a closure for that ordering and no other reason: reading it off an expression is a
    /// lookup into the file's HIR, and every method call on every path through here would pay it for
    /// a table that is empty in every project with no framework. It answers `None` for a reader that
    /// could not tell how many arguments were written, and then nothing fires -- an unknown call
    /// shape is not a call that wrote none.
    pub(crate) fn returned_by_default(
        &self,
        class_fqn: &str,
        method: &str,
        arity: impl FnOnce() -> Option<u16>,
    ) -> Option<Held> {
        let branches = self.skeleton.returns().defaulted_branches(method);
        if branches.is_empty() {
            return None;
        }
        let arity = arity()?;
        let branch = branches.iter().find(|branch| {
            arity <= branch.unless_given
                && (class_fqn.eq_ignore_ascii_case(&branch.class)
                    || self.descends_from(class_fqn, &branch.class, HIERARCHY_DEPTH))
        })?;
        if !self.declares_return_operand(class_fqn, method, &branch.then) {
            return None;
        }
        // Through the index rather than straight to the name, for the reason
        // [`Self::returned_from_argument`] gives: a `Held` for a class with no declaration would
        // report every member of it as missing.
        Some(Held::Class(self.class_location(&branch.then)?.fqn))
    }

    /// Whether a method's declared return type is a union that lists `named` among its operands.
    ///
    /// The check that keeps [`Self::returned_by_default`] a selector. A union and nothing else: a
    /// single declared class is already the answer and needs no contract, and a nullable or an
    /// intersection is a shape whose branches are not alternatives a call picks between -- declining
    /// there leaves the declaration standing, which is the safe direction.
    fn declares_return_operand(&self, class_fqn: &str, method: &str, named: &str) -> bool {
        let table = self.members(class_fqn);
        let Some(member) = table.find(MemberKind::Method, method) else {
            return false;
        };
        let Some(declared) = self.declared_ty(&member.row) else {
            return false;
        };
        let phrost_ty::TypeRef::Union(operands) = *self.skeleton.types().get(declared) else {
            return false;
        };
        self.skeleton
            .types()
            .operands(operands)
            .iter()
            .any(|&operand| {
                self.class_of_stored(operand, None)
                    .is_some_and(|class| class.as_str().eq_ignore_ascii_case(named))
            })
    }

    /// What a call's first argument says it returns, by whichever contract a framework gave the
    /// method: a catalog the argument is a key of, or an input the receiver's class declares.
    pub(crate) fn returned_from_argument(
        &self,
        class_fqn: &str,
        method: &str,
        argument: Option<&str>,
    ) -> Option<Held> {
        self.returned_from_catalog_argument(class_fqn, method, argument)
            .or_else(|| self.returned_from_declared_input(class_fqn, method, argument?))
    }

    /// What an input the receiver's class declares holds: Laravel's `$this->argument('user')` is a
    /// string when the command's `$signature` writes `{user}`, where the method declares every
    /// scalar there is.
    ///
    /// Read from the file being analysed, which is where the declaration and the reads both are: a
    /// command's `handle()` sits beside its signature. A class declared in another file is not asked
    /// about -- the declaration's answer stands -- which is a missing answer and not a wrong one.
    /// The method name is the gate, then the class in this file, then the declaration's text, and the
    /// hierarchy last, as for the catalog.
    fn returned_from_declared_input(
        &self,
        class_fqn: &str,
        method: &str,
        argument: &str,
    ) -> Option<Held> {
        let contracts = self.skeleton.returns();
        let mut rules = contracts
            .declared_inputs()
            .iter()
            .filter_map(|rule| Some((rule, rule.prefix_read_by(method)?)))
            .peekable();
        rules.peek()?;
        let open = self.current()?;
        let hir = open.hir();
        let class = hir.classes().find_map(|(id, _)| {
            open.scope()
                .symbol_of(phrost_defmap::DeclarationSource::Class(id))
                .filter(|symbol| symbol.fqn.as_str().eq_ignore_ascii_case(class_fqn))
                .map(|_| id)
        })?;
        rules.find_map(|(rule, prefix)| {
            let declared = hir
                .kept_texts()
                .iter()
                .find(|kept| kept.class == class && kept.property.as_str() == &*rule.property)?;
            let wanted = format!("{prefix}{argument}");
            let shape = (rule.parse)(&declared.text)
                .into_iter()
                .find(|input| input.name == wanted)?
                .shape;
            (class_fqn.eq_ignore_ascii_case(&rule.class)
                || self.descends_from(class_fqn, &rule.class, HIERARCHY_DEPTH))
            .then(|| Self::held_of_shape(shape))
        })
    }

    /// A [`phrost_model::ValueShape`] as the value it describes.
    fn held_of_shape(shape: phrost_model::ValueShape) -> Held {
        let one = Held::Builtin(shape.builtin);
        let one = match shape.nullable {
            true => Self::nullable(one),
            false => one,
        };
        match shape.list {
            true => Held::ListOf(Box::new(one)),
            false => one,
        }
    }

    /// The class a call's first argument names, when a framework bound that method to a catalog.
    ///
    /// `\Drupal::service('application.manager')` declares `mixed` and returns whatever the container
    /// was told to build; the id is the only thing that says which. So the argument is looked up in
    /// the catalog the binding names, and what comes back is a class like any other.
    ///
    /// `None` means the override does not apply -- there is no binding for this method, the argument
    /// was not a literal, or the catalog has no such key -- and the caller then asks what the
    /// declaration says. That is not a retry after a failure: a binding that does not match is a
    /// question that was never asked, and `getStorage()` still declares a useful type for an id
    /// nothing registered.
    ///
    /// Three gates, in the order their cost was measured. The literal has to be there at all; then
    /// the method has to be bound, which is a scan of a handful of short names; then the literal has
    /// to *be* a key, which is one hash.
    ///
    /// The hierarchy is checked last, and that ordering is the whole of why this is affordable.
    /// `get` is among the commonest method names in a Drupal project -- config, form state, and every
    /// field list have one -- so asking "does this receiver descend from the container interface"
    /// first meant a sixteen-level climb on `$config->get('foo')`, and diagnostics on a large file
    /// grew by a third. Asking whether `foo` is a registered service id first settles almost every
    /// one of them with a failed hash lookup.
    fn returned_from_catalog_argument(
        &self,
        class_fqn: &str,
        method: &str,
        argument: Option<&str>,
    ) -> Option<Held> {
        let argument = argument?;
        let contracts = self.skeleton.returns();

        let named = contracts
            .argument_bindings(method)
            .iter()
            .filter_map(|binding| {
                let applies = || {
                    class_fqn.eq_ignore_ascii_case(&binding.class)
                        || self.descends_from(class_fqn, &binding.class, HIERARCHY_DEPTH)
                };
                // A key names a class or holds a value, and a catalog is one kind or the other: a
                // service id names the class the container builds, a container *parameter* holds a
                // value whose shape its YAML says -- and `getParameter()` can only declare
                // `array|bool|string|int|float|\UnitEnum|null`, because a signature cannot say
                // which parameter it is being asked about.
                if let Some(class) = contracts.catalog_class(&binding.catalog, argument) {
                    return applies().then(|| Named::Class(class.to_owned()));
                }
                let shape = contracts
                    .catalog_shape_of(&binding.catalog, argument)
                    .or_else(|| contracts.unlisted_shape(&binding.catalog, argument))?;
                applies().then_some(Named::Shape(shape))
            })
            .next()?;

        match named {
            // Through the catalog rather than straight to a name, because a catalog may name a class
            // the index does not have -- a service pointing at a class its module deleted -- and a
            // `Held` for a class with no declaration would report every member of it as missing.
            Named::Class(class) => Some(Held::Class(self.class_location(&class)?.fqn)),
            Named::Shape(shape) => Some(Held::Builtin(shape)),
        }
    }

    /// The class a fixed catalog key names, for a method a framework bound to one: Laravel's
    /// `$request->user()` and a guard's `user()`, which `config/auth.php` decides.
    ///
    /// The method name is the gate, then the key -- which a project without the configuration never
    /// filed -- and the hierarchy last, as for [`Self::returned_from_argument`].
    pub(crate) fn returned_by_key(&self, class_fqn: &str, method: &str) -> Option<Held> {
        let contracts = self.skeleton.returns();
        let (class, nullable) = contracts.keyed_returns(method).iter().find_map(|rule| {
            let class = contracts.catalog_class(&rule.catalog, &rule.key)?;
            (class_fqn.eq_ignore_ascii_case(&rule.class)
                || self.descends_from(class_fqn, &rule.class, HIERARCHY_DEPTH))
            .then_some((class, rule.nullable))
        })?;
        let held = Held::Class(self.class_location(class)?.fqn);
        Some(match nullable {
            true => Self::nullable(held),
            false => held,
        })
    }

    /// The class a static method hands back by the name of the class it was called on: Laravel's
    /// `Contact::factory()`, which is the factory `Factory::resolveFactoryName()` finds -- see the
    /// seam's `NamedAfterCaller`.
    ///
    /// Only where nothing else says. A template the class's `@use` filled is what the declaration
    /// answers with, and a member below the rule's class that names the class itself (`newFactory()`)
    /// is what the runtime asks first. The method name is the gate, then the hierarchy, then the
    /// name -- and the class has to be one the index holds, or the declaration stands.
    pub(crate) fn returned_named_after(&self, class_fqn: &str, method: &str) -> Option<Held> {
        let contracts = self.skeleton.returns();
        let rule = contracts
            .named_after()
            .iter()
            .find(|rule| rule.method.eq_ignore_ascii_case(method))?;
        if !self.descends_from(class_fqn, &rule.class, HIERARCHY_DEPTH) {
            return None;
        }
        let members = self.members(class_fqn);
        let overridden = rule.unless.iter().any(|name| {
            [MemberKind::Method, MemberKind::Property]
                .into_iter()
                .filter_map(|kind| members.find(kind, name))
                .any(|member| {
                    !matches!(&member.provenance, MemberProvenance::Trait(from)
                        if from.as_str().eq_ignore_ascii_case(&rule.class))
                })
        });
        if overridden || self.writes_arguments_for(class_fqn, &rule.class) {
            return None;
        }
        let namespace = self
            .skeleton
            .namespace_of_directory(&rule.app_directory)
            .unwrap_or(&rule.default_namespace);
        let named = (rule.name)(self.class_location(class_fqn)?.fqn.as_str(), namespace);
        Some(Held::Class(self.class_location(&named)?.fqn))
    }

    /// Whether `class_fqn` or a class above it wrote arguments for `ancestor` in a supertype tag.
    fn writes_arguments_for(&self, class_fqn: &str, ancestor: &str) -> bool {
        let mut class = class_fqn.to_owned();
        for _ in 0..HIERARCHY_DEPTH {
            if !self.written_arguments_for(&class, ancestor).is_empty() {
                return true;
            }
            let Some(parent) = self
                .class_header(&class)
                .and_then(|header| header.extends.first().cloned())
            else {
                return false;
            };
            class = parent.as_str().to_owned();
        }
        false
    }

    /// The class a free function's argument names, when a framework bound the function to a
    /// catalog: Laravel's `app('router')`, which declares `mixed` for every alias it is handed.
    ///
    /// The function's half of [`Self::returned_from_argument`], and gated the same way: a binding for
    /// the name first, which almost no function has, then the literal, then the key.
    pub(crate) fn returned_from_function_argument(
        &self,
        function: &str,
        arguments: &[phrost_hir::body::LiteralArgument],
    ) -> Option<Held> {
        self.named_by_function_argument(function, |at| {
            arguments
                .iter()
                .find(|argument| argument.position == at)
                .map(|argument| argument.value.as_str())
        })
    }

    /// The same, for a call read off the text of a chain being typed: `app('router')->`.
    pub(crate) fn returned_from_function_text(
        &self,
        function: &str,
        arguments: &[Box<str>],
    ) -> Option<Held> {
        self.named_by_function_argument(function, |at| unquoted(arguments.get(usize::from(at))?))
    }

    /// What a free function's contract answers for a call read as text: the class one argument
    /// names as `Foo::class`, or the branch a left-out argument's default settles.
    ///
    /// The completion chain's half of what [`Self::returned_by_function_contract`] does for a call
    /// expression. `app(Signup::class)->` and `app()->` are statements still being typed, so there
    /// is no call node, and the contract went unread: the root fell to the declared return -- the
    /// union of every branch, `mixed` among them -- and nothing was offered where goto, reading the
    /// finished statement, went straight to the method.
    ///
    /// Only what text settles: a class constant names a class and is not null, a missing argument
    /// is its default, and the word `null` is null. Anything else -- a variable where the class
    /// goes -- leaves the declaration standing.
    pub(crate) fn returned_from_written_call_contract(
        &self,
        offset: phrost_model::TextSize,
        function: &str,
        arguments: &[Box<str>],
    ) -> Option<Held> {
        let named = |position: u16| {
            let written = arguments.get(usize::from(position))?;
            let written = written.strip_suffix("::class").unwrap_or(written);
            self.written_class(offset, written)
        };
        let holds = |argument: u16, test: &phrost_sema::ConditionTest, by_default: Option<bool>| {
            let Some(written) = arguments.get(usize::from(argument)) else {
                return by_default;
            };
            let names_a_class = written.ends_with("::class") && named(argument).is_some();
            match test {
                phrost_sema::ConditionTest::ClassString(_) => names_a_class.then_some(true),
                phrost_sema::ConditionTest::Null if names_a_class => Some(false),
                phrost_sema::ConditionTest::Null => {
                    written.eq_ignore_ascii_case("null").then_some(true)
                }
                _ => None,
            }
        };
        let branch = |branch: &phrost_sema::ReturnBranch| match branch {
            phrost_sema::ReturnBranch::ArgumentClass(position) => {
                Some(Held::Class(named(*position)?))
            }
            phrost_sema::ReturnBranch::Ty(ty) if self.names_something_real(*ty) => {
                Some(Held::Stored {
                    ty: *ty,
                    scope: None,
                })
            }
            _ => None,
        };
        match self.function_contract(function)? {
            phrost_sema::Contract::ReturnsArgumentClass {
                parameter,
                absent,
                many,
                ..
            } => Some(Shape::of(*absent, *many).around(Held::Class(named(*parameter)?))),
            phrost_sema::Contract::ConditionalReturn {
                argument,
                test,
                then,
                otherwise,
                by_default,
            } => branch(match holds(*argument, test, *by_default)? {
                true => then,
                false => otherwise,
            }),
            phrost_sema::Contract::TestChain { cases, otherwise } => {
                for case in cases {
                    if holds(case.argument, &case.test, case.by_default)? != case.negated {
                        return branch(&case.then);
                    }
                }
                branch(otherwise)
            }
            _ => None,
        }
    }

    fn named_by_function_argument<'k>(
        &self,
        function: &str,
        key_at: impl FnOnce(u16) -> Option<&'k str>,
    ) -> Option<Held> {
        let contracts = self.skeleton.returns();
        let binding = contracts.function_argument_binding(function)?;
        let key = key_at(binding.at)?;
        // A key names a class or holds a value, as for the method's half -- and a catalog listing
        // only the keys that hold something else says what every other literal holds.
        if let Some(class) = contracts.catalog_class(&binding.catalog, key) {
            // Through the index, for the reason the method's half gives: a catalog may name a class
            // nothing declares.
            return Some(Held::Class(self.class_location(class)?.fqn));
        }
        contracts
            .catalog_shape_of(&binding.catalog, key)
            .or_else(|| contracts.unlisted_shape(&binding.catalog, key))
            .map(Held::Builtin)
    }

    /// What a factory returns for a call the text was read from.
    ///
    /// The same two claims [`Self::returned_class_named_by_argument`] and
    /// [`Self::returned_by_method_contract`] read, from strings rather than from lowered arguments: a
    /// chain has no expression behind it, so the position is all there is to match the declaration's
    /// against. Both from one lookup, because a member carries one contract.
    ///
    /// Only what a *string* settles. A slot bound to an argument's type has nothing to read here --
    /// the statement being typed does not parse -- and the class's own template bound stands in for
    /// it, exactly as it does for an argument the call left out.
    pub(crate) fn returned_class_written(
        &self,
        offset: phrost_model::TextSize,
        class_fqn: &str,
        method: &str,
        arguments: &[Option<Box<str>>],
    ) -> Option<Held> {
        self.returned_class_naming(class_fqn, method, |position| {
            let written = arguments.get(usize::from(position))?.as_deref()?;
            self.written_class(offset, written)
        })
    }

    /// The same, for the first argument of a chain a body returned, already resolved to a class at
    /// ingest -- see `ReturnedKey::Class`. The class has to be one the index holds, for the reason
    /// [`Self::returned_from_argument`] gives.
    pub(crate) fn returned_class_for(
        &self,
        class_fqn: &str,
        method: &str,
        named: &str,
    ) -> Option<Held> {
        self.returned_class_naming(class_fqn, method, |position| {
            (position == 0).then_some(())?;
            Some(self.class_location(named)?.fqn)
        })
    }

    /// What a factory returns when `name_at` says which class each argument position names.
    fn returned_class_naming(
        &self,
        class_fqn: &str,
        method: &str,
        name_at: impl Fn(u16) -> Option<Name>,
    ) -> Option<Held> {
        let named = |position: u16| Some(Held::Class(name_at(position)?));
        let table = self.members(class_fqn);
        let entry = table.find(MemberKind::Method, method)?;
        // One class only: a factory naming the class it builds has no alternatives to choose
        // between, and reading the first of several would be narrower than the declaration.
        if let Some(phrost_sema::Contract::ReturnsApplied {
            class,
            arguments: slots,
            many,
            alternatives,
        }) = entry.row.contract.as_deref()
            && alternatives.is_empty()
        {
            let filled = slots
                .iter()
                .map(|slot| match slot {
                    phrost_sema::ReturnBranch::ArgumentClass(position) => named(*position),
                    _ => None,
                })
                .collect();
            return self
                .applied_instance(*class, filled)
                .map(|held| Shape::of(None, *many).around(held));
        }
        let (position, shape, beside) = self.argument_naming_the_class(class_fqn, method, entry)?;
        Some(shape.around(self.beside_the_declared_return(entry, beside, named(position)?)))
    }

    /// The class a factory returns, when the call named it.
    ///
    /// `$container->get(Repo::class)` and `$this->createMock(Repo::class)`: the declaration says
    /// `@return T` and only the call says what `T` is. The name is resolved against the imports of
    /// the file that wrote it, which is where the argument was written.
    ///
    /// Through the index rather than straight to the name, for the reason
    /// [`Self::returned_from_argument`] gives: a `Held` for a class with no declaration would report
    /// every member of it as missing.
    pub(crate) fn returned_class_named_by_argument(
        &self,
        offset: phrost_model::TextSize,
        class_fqn: &str,
        method: &str,
        arguments: &[phrost_hir::body::LiteralArgument],
    ) -> Option<Held> {
        let table = self.members(class_fqn);
        let entry = table.find(MemberKind::Method, method)?;
        let (parameter, shape, beside) =
            self.argument_naming_the_class(class_fqn, method, entry)?;
        // The position the declaration named, and only that one -- unless the parameter is variadic,
        // where every argument from there on fills it and the result is one of them. A literal at
        // any other position is some other argument, and reading it would name the wrong class
        // rather than none.
        let named = arguments_from(arguments, &entry.row.params, parameter)
            .into_iter()
            // `f(null)` writes a literal that is not a name. PHP reserves the three keywords, so
            // nothing can declare a class one of them would resolve to -- but a contract that says
            // "this argument names a class" should say so, rather than rely on the lookup failing.
            .filter(|argument| argument.kind == phrost_hir::body::ArgumentLiteral::Text)
            .filter_map(|argument| self.written_class(offset, argument.value.as_str()))
            .map(Held::Class)
            .collect::<Vec<_>>();
        // The null the tag wrote goes back on the outside. `class-string<T>` says which class and
        // nothing about whether one was found, so `@return ?T` -- `NodeFinder::findFirstInstanceOf`
        // -- is that class with the declaration's own null over it.
        let named = (!named.is_empty()).then(|| Self::one_of(named))?;
        Some(shape.around(self.beside_the_declared_return(entry, beside, named)))
    }

    /// Which argument names the class a call returns, when something says one does.
    ///
    /// Two sources of a single claim. A declaration can state it itself -- `@return T` bound to a
    /// `class-string<T>` parameter -- and for library code that writes docblocks that is the whole
    /// of it. A framework can state it for a declaration that does not: Drupal's class resolver
    /// takes a class name, hands back an instance of it, declares `object`, and says the rest only
    /// in prose. Same claim, so the same reader answers it and the same code decides what is silence.
    ///
    /// The contributed half carries a gate the declared half does not need, because the two are
    /// trusted for different reasons: a `@return T` is the author of the method speaking about their
    /// own method, and a contributed rule is a list in this repository speaking about somebody
    /// else's. See [`Self::declares_only_an_object`].
    ///
    /// Gated on the method name first -- a scan of the handful of short strings a framework
    /// contributes, empty in every project with no framework -- then the hierarchy, then the
    /// declaration. That is the order their cost runs in.
    fn argument_naming_the_class(
        &self,
        class_fqn: &str,
        method: &str,
        entry: &phrost_sema::ResolvedMember,
    ) -> Option<(u16, Shape, bool)> {
        if let Some(phrost_sema::Contract::ReturnsArgumentClass {
            parameter,
            absent,
            many,
            beside,
        }) = entry.row.contract.as_deref()
        {
            return Some((*parameter, Shape::of(*absent, *many), *beside));
        }

        let contributed = self.skeleton.returns().argument_class_returns(method);
        if contributed.is_empty() {
            return None;
        }
        let rule = contributed.iter().find(|rule| {
            class_fqn.eq_ignore_ascii_case(&rule.class)
                || self.descends_from(class_fqn, &rule.class, HIERARCHY_DEPTH)
        })?;
        // A contributed rule states a position and never a null: it describes a factory whose
        // declaration says `object`, and there is no tag beside it to have written one.
        // A contributed rule states a position and nothing else: the declaration it describes says
        // `object`, so there is no second half to compose with.
        self.declares_only_an_object(class_fqn, method, entry)
            .then_some((rule.argument, Shape::default(), false))
    }

    /// The factory's answer beside what the declaration itself returns, where the two are unrelated.
    ///
    /// `createMock(string $type): MockObject` with `@return MockObject&T` is the shape, and both
    /// halves are true at once: PHP enforces the hint, so the value *is* a `MockObject`, and the tag
    /// says it is also whatever the call named. A test then calls the mocked class's methods through
    /// it and PHPUnit's own on the same value -- 521 `undefined-method` on one framework's tests for
    /// reading only one of them.
    ///
    /// Only where neither class is the other's: a hint that is a supertype of what the call named
    /// says nothing the named class does not, and a hint that is a *subtype* is the better answer
    /// the reader already has. Unrelated is the case an intersection was written for.
    fn beside_the_declared_return(
        &self,
        entry: &phrost_sema::ResolvedMember,
        beside: bool,
        named: Held,
    ) -> Held {
        // Only where the tag wrote an intersection. Asked of every factory it is two hierarchy walks
        // per call for an answer almost no declaration wants -- 15 ms of a 180 ms sweep -- and it
        // would compose a hint the author never wrote beside the template.
        if !beside {
            return named;
        }
        let Held::Class(found) = &named else {
            return named;
        };
        let Some(declared) = entry
            .row
            .ty
            .and_then(|ty| self.class_of_stored(ty, None))
            .filter(|declared| {
                !declared.as_str().eq_ignore_ascii_case(found.as_str())
                    && !self.descends_from(found.as_str(), declared.as_str(), HIERARCHY_DEPTH)
                    && !self.descends_from(declared.as_str(), found.as_str(), HIERARCHY_DEPTH)
            })
        else {
            return named;
        };
        Self::all_of(vec![Held::Class(declared), named])
    }

    /// Whether a method's declared return type is the bare `object` builtin.
    ///
    /// What stands in for [`Self::declares_return_operand`] where there is nothing to select from.
    /// A contract that picks a branch is safe because the branch is one the declaration wrote; a
    /// contract that says which *class* an `object` is cannot be safe that way, because `object`
    /// writes no branches and every class satisfies it. So the check runs the other direction: the
    /// declaration has to be saying nothing at all. Anything narrower -- a class, a union, a
    /// nullable -- is a declaration with an opinion, and a contributed rule does not get to
    /// contradict one.
    ///
    /// That makes the narrowing sound rather than merely useful: it happens strictly inside the
    /// declared type, so it can never disagree with what the runtime enforces. And it disarms the
    /// contract by itself if the framework ever types the method, silently and in the direction that
    /// says nothing.
    ///
    /// Climbing when the found row declares nothing, because that is what reading the declaration
    /// means here: `ClassResolver::getInstanceFromDefinition` is `{@inheritdoc}` over the
    /// interface's `@return object`, and the interface is where the type is written.
    fn declares_only_an_object(
        &self,
        class_fqn: &str,
        method: &str,
        entry: &phrost_sema::ResolvedMember,
    ) -> bool {
        let Some(declared) = self.declared_ty(&entry.row).or_else(|| {
            self.inherited_ty(class_fqn, MemberKind::Method, method, INHERIT_DEPTH)
                .map(|(ty, _)| ty)
        }) else {
            return false;
        };
        // `?object` too, which is the same statement with a null on it -- and a contributed rule
        // answers without that null on purpose. Symfony's container declares
        // `get(string $id, int $invalidBehavior = self::EXCEPTION_ON_INVALID_REFERENCE): ?object`,
        // so the null is what the *second* argument opts into and 295 of the 296 `get(X::class)`
        // calls on one real checkout do not write it. Keeping it would put a
        // `possibly-null-receiver` on all 295 to be right about the one -- which is the trade the
        // other way round from the one §5 protects, since the noise is on calls that cannot be
        // null rather than a null being hidden.
        matches!(
            self.skeleton.types().get(declared),
            phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Object)
        ) || matches!(
            self.skeleton.types().get(declared),
            phrost_ty::TypeRef::Nullable(inner)
                if matches!(
                    self.skeleton.types().get(*inner),
                    phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Object)
                )
        )
    }

    /// What a method's contract gives back for this call.
    ///
    /// Two shapes from one lookup, because a member carries one contract and flattening its class
    /// twice would be paid by every method call on every path this walks. A conditional return is a
    /// branch the call settles; a `@return Bar<T>` is a class the declaration wrote out with
    /// arguments only the call can fill.
    ///
    /// `$mapper->map(Foo::class)`: the declaration says the result is a `T` when the argument names a
    /// class and `mixed` when it does not, and only the call says which. The scope travels with the
    /// branch for the reason every other stored type carries one -- a branch may be written `static`.
    pub(crate) fn returned_by_method_contract(
        &self,
        class_fqn: &str,
        method: &str,
        called_on: Option<Name>,
        at: &CallFacts<'_>,
    ) -> Option<Held> {
        let table = self.members(class_fqn);
        let entry = table.find(MemberKind::Method, method)?;

        // Forwarded to another class's instance, whose templates the call's receiver is then what
        // binds -- see `member_held_on`, which does the same for a declared return.
        if entry.forwarded
            && let Some((mixin, arguments)) =
                self.implied_forward(class_fqn, called_on.as_ref(), MemberKind::Method, method)
        {
            let at = CallFacts {
                receiver: &arguments,
                ..*at
            };
            return self.returned_by_method_contract(mixin.as_str(), method, None, &at);
        }

        // `Closure::fromCallable($f)` is `$f` made into a `Closure`, so invoking it does what
        // invoking `$f` does -- a pair, a first-class callable, a name of a runtime function. PHP's
        // own, and no docblock can say it: the stub's `@return Closure` names the class and not the
        // call it wraps.
        if class_fqn
            .trim_start_matches('\\')
            .eq_ignore_ascii_case("Closure")
            && method.eq_ignore_ascii_case("fromCallable")
        {
            let wrapped = at.written_argument(&entry.row.params, 0)?;
            let returns = self.callback_held(at, wrapped)?;
            return Some(Held::Callable(Box::new(crate::resolve::Callable {
                returns,
                literal: None,
            })));
        }

        let contract = entry.row.contract.as_deref()?;

        // Who wrote the declaration decides what `self` means, exactly as it does for a declared
        // return type.
        let declaring = match &entry.provenance {
            MemberProvenance::Own | MemberProvenance::Trait(_) => None,
            MemberProvenance::Inherited(fqn) | MemberProvenance::Interface(fqn) => {
                Some(fqn.clone())
            }
        };
        // The class the table belongs to rather than the one the call was made on, for the reason
        // `member_held_on` gives.
        let found_on = self.members_owner(class_fqn)?;
        let called = called_on.unwrap_or_else(|| found_on.clone());
        let scope = RelativeScope {
            declaring: declaring.unwrap_or(found_on),
            called,
        };

        // A class with arguments is not a branch: nothing is being chosen between, and the arguments
        // come from the call -- or from the receiver, for a slot naming one of the class's own
        // templates. Under the call's scope, because the class may be the receiver's own:
        // `Collection::map()` is `static<TKey, TMapValue>`, where `static` is whatever collection
        // it was called on and `TKey` what that one was written with. Read without one, `static`
        // named nothing and every `->map(...)` ended the chain it was in.
        if let phrost_sema::Contract::ReturnsApplied {
            class,
            arguments,
            many,
            alternatives,
        } = contract
        {
            let filled = arguments
                .iter()
                .map(|branch| {
                    self.held_of_branch(at, branch.clone(), &entry.row.params, Some(scope.clone()))
                })
                .collect();
            return self
                .applied_either(at, *class, alternatives, filled, Some(&scope))
                .map(|held| Shape::of(None, *many).around(held));
        }

        // The entry of a constant's table at the key the call passed -- `self::TABLE[T]` means the
        // declaring class's table, as `self` on any return does.
        if let phrost_sema::Contract::ConstantEntry { table, parameter } = contract {
            return self.constant_entry_held(
                at,
                &entry.row.params,
                table,
                *parameter,
                Some(&scope),
            );
        }

        // A union whose operands are bound by different things: one by an argument's type and one by
        // what the receiver was written with. Every operand has to answer -- a union with an
        // unresolved operand dropped is narrower than the truth, and a type narrower than the truth
        // reports correct code.
        if let phrost_sema::Contract::ReturnsOneOf(operands) = contract {
            let mut held = Vec::with_capacity(operands.len());
            for operand in operands.iter().cloned() {
                held.push(self.held_of_branch(
                    at,
                    operand,
                    &entry.row.params,
                    Some(scope.clone()),
                )?);
            }
            return Some(Self::one_of(held));
        }

        let branch = self.settled_branch(at, contract, &entry.row.params)?;
        self.held_of_settled(at, branch, &entry.row.params, Some(scope))
    }

    /// The same, for a free function: `response(null)` against
    /// `@return ($content is null ? ResponseFactory : HttpResponse)`, or `array_pop($items)` against
    /// `@return ($array is non-empty-array ? V : V|null)`.
    pub(crate) fn returned_by_function_contract(
        &self,
        fqn: &str,
        at: &CallFacts<'_>,
    ) -> Option<Held> {
        let contract = self.function_contract(fqn)?;
        // The signature, and only once the contract says there is something to read: matching a name
        // to a parameter needs the list, and reading it for every call would be a clone per call for
        // the nearly-every function that promises nothing.
        let params = self.function_params(fqn)?;

        // Every kind of contract, which this used to be half of: only a *conditional* was read, so a
        // free function's `@template T` + `@param class-string<T>` + `@return T` -- the factory shape
        // -- answered whatever the hint said. That matters more than it sounds for a framework whose
        // `.module`, `.inc` and `.install` files are free functions by definition.
        match contract {
            // A class with arguments is not a branch: nothing is being chosen between, and the
            // arguments come from the call.
            phrost_sema::Contract::ReturnsApplied {
                class,
                arguments,
                many,
                alternatives,
            } => {
                let filled = arguments
                    .iter()
                    .map(|branch| self.held_of_branch(at, branch.clone(), &params, None))
                    .collect();
                self.applied_either(at, *class, alternatives, filled, None)
                    .map(|held| Shape::of(None, *many).around(held))
            }
            // The null the tag wrote goes back on: `class-string<T>` says which class and nothing
            // about whether one was found, so `@return ?T` is that class with the declaration's own
            // null over it.
            phrost_sema::Contract::ReturnsArgumentClass {
                parameter,
                absent,
                many,
                // A free function has no member row to read a hint off, and an intersection on one
                // is not a shape anybody writes.
                beside: _,
            } => self
                .class_named_at(at, &params, *parameter)
                .map(Held::Class)
                .map(|held| Shape::of(*absent, *many).around(held)),
            // The entry of a constant's table at the key the call passed.
            phrost_sema::Contract::ConstantEntry { table, parameter } => {
                self.constant_entry_held(at, &params, table, *parameter, None)
            }
            _ => {
                let branch = self.settled_branch(at, contract, &params)?;
                // A free function has no enclosing class, so a relative type cannot appear in a
                // branch.
                self.held_of_settled(at, branch, &params, None)
            }
        }
    }

    /// What a class with arguments -- or any of the alternatives written beside it -- is for this
    /// call, filled with what the call's arguments resolved to.
    ///
    /// An alternative whose arguments break one of its templates' bounds is one the call cannot
    /// make, and is dropped: Eloquent's `map()` returns `static<TKey, TMapValue>` only when the
    /// callback made models -- `TModel of Model` -- and a base collection otherwise, so a callback
    /// building arrays leaves `Support\Collection<int, array{...}>` and nothing else. Kept, the
    /// union read every `->get()->map(fn () => [...])` as a collection of models too, and a
    /// `prepend([...])` on it as an argument the model half refuses. Only where something is left:
    /// a lone class whose bound is broken is still what the declaration says.
    fn applied_either(
        &self,
        at: &CallFacts<'_>,
        class: phrost_ty::TypeId,
        alternatives: &[phrost_ty::TypeId],
        filled: Vec<Option<Held>>,
        scope: Option<&RelativeScope>,
    ) -> Option<Held> {
        let mut classes = Vec::with_capacity(1 + alternatives.len());
        for &candidate in std::iter::once(&class).chain(alternatives) {
            classes.push(
                self.class_location(self.class_of_stored(candidate, scope)?.as_str())?
                    .fqn,
            );
        }
        if classes.len() > 1 {
            let kept = classes
                .iter()
                .filter(|class| !self.breaks_a_bound(at.hir, class.as_str(), &filled))
                .cloned()
                .collect::<Vec<_>>();
            if !kept.is_empty() {
                classes = kept;
            }
        }
        let mut held = Vec::with_capacity(classes.len());
        for class in classes {
            held.push(self.applied_instance_of(class, filled.clone())?);
        }
        Some(Self::one_of(held))
    }

    /// Whether an argument cannot be what a template of `class` is bounded by: a bound naming a
    /// class, and an argument that is certainly no object. Anything less certain -- `mixed`, a
    /// union, a callable that may be an object -- breaks nothing.
    fn breaks_a_bound(
        &self,
        hir: &phrost_hir::FileHir,
        class: &str,
        filled: &[Option<Held>],
    ) -> bool {
        let Some(templates) = self.class_templates(class) else {
            return false;
        };
        filled.iter().zip(templates).any(|(argument, template)| {
            let (Some(argument), Some(bound)) = (argument, template.bound) else {
                return false;
            };
            self.class_of_stored(bound, None).is_some()
                && matches!(
                    self.kind_of_held(hir, argument),
                    Some(
                        BuiltinTy::Int
                            | BuiltinTy::Float
                            | BuiltinTy::String
                            | BuiltinTy::Bool
                            | BuiltinTy::True
                            | BuiltinTy::False
                            | BuiltinTy::Array
                            | BuiltinTy::Null
                    )
                )
        })
    }

    /// What a settled -- or unsettled -- conditional answers with.
    fn held_of_settled(
        &self,
        at: &CallFacts<'_>,
        settled: Settled,
        params: &[phrost_sema::ParamRow],
        scope: Option<RelativeScope>,
    ) -> Option<Held> {
        match settled {
            Settled::One(branch) => self.held_of_branch(at, branch, params, scope),
            Settled::Any(branches) => {
                let mut held = Vec::with_capacity(branches.len());
                for branch in branches {
                    let one = self.held_of_branch(at, branch, params, scope.clone())?;
                    // `mixed` admits every other branch already, the absence included -- and a
                    // stored one is not a spelling the nullable constructor can see through:
                    // `?mixed` was the answer for a promise's `wait($unwrap)`.
                    if self.kind_of_held(at.hir, &one) == Some(BuiltinTy::Mixed) {
                        return Some(one);
                    }
                    held.push(one);
                }
                // A null in any branch belongs to the union, for `one_of`'s reason: an interned
                // `?string` is a spelling that constructor cannot see into, `?string|?string[]` is
                // the degenerate form no guard narrows, and a branch that is only the absence is the
                // nullable spelling of the rest -- `?Row`, never `Row|null` beside it.
                let nullable = held
                    .iter()
                    .any(|one| self.null_reach_of(at.hir, one) != crate::SentinelReach::Impossible);
                // A branch that is itself a union joins as its operands, so a type two branches
                // both name is named once: `class_implements()` is `A` for an object and `A|false`
                // for anything else, and joined whole the two read `A|A|false`.
                let rest = held
                    .into_iter()
                    .filter_map(|one| self.without_null(one))
                    .flat_map(|one| match self.union_operands(at.offset, at.hir, &one) {
                        Some(operands) => {
                            operands.into_iter().map(|operand| operand.held).collect()
                        }
                        None => vec![one],
                    })
                    .collect::<Vec<_>>();
                if rest.is_empty() {
                    return Some(Held::Builtin(BuiltinTy::Null));
                }
                let joined = Self::one_of(rest);
                Some(match nullable {
                    true => Self::nullable(joined),
                    false => joined,
                })
            }
        }
    }

    /// Which branch of a conditional return type answers for this call -- or, where the call does not
    /// settle it, every branch it may take, each to be read for this call.
    fn settled_branch(
        &self,
        at: &CallFacts<'_>,
        contract: &phrost_sema::Contract,
        params: &[phrost_sema::ParamRow],
    ) -> Option<Settled> {
        if let phrost_sema::Contract::LiteralCases {
            argument,
            cases,
            otherwise,
        } = contract
        {
            let value = self.string_passed_or_defaulted(at, params, *argument)?;
            return Some(Settled::One(
                cases
                    .iter()
                    .find(|(literal, _)| literal.as_str() == value)
                    .map_or(otherwise.clone(), |(_, branch)| branch.clone()),
            ));
        }
        // A chain: the first link whose test picks it gives its branch, and every link passing on
        // gives the last. A link this cannot settle settles nothing -- what it would have picked is
        // unknown, and so is everything after it -- unless what it would have picked is only the
        // absence. Then the call is either that or whatever the rest of the chain settles, which is
        // the rest's answer with a null beside it: `$wpdb->get_results($wpdb->prepare(...))` is
        // `null` for a query nothing can read and `list<stdClass>|null` for any other, and both
        // together are what the declaration says about a query it cannot see.
        //
        // Not every branch the way a lone conditional is, and measured: `wp_unslash()` is a chain
        // whose last link hands its argument back, and read for the call it passed a superglobal's
        // `string|array` on -- 129 findings on WordPress, the ones its declaration's `mixed` was
        // chosen to keep out.
        if let phrost_sema::Contract::TestChain { cases, otherwise } = contract {
            return self
                .chain_settled(at, params, cases, otherwise, None)
                .map(Settled::of);
        }
        let phrost_sema::Contract::ConditionalReturn {
            argument,
            test,
            then,
            otherwise,
            by_default,
        } = contract
        else {
            // Every branch agreed at lowering, so there is nothing left for the call to settle.
            if let phrost_sema::Contract::AgreedReturn(branch) = contract {
                return Some(Settled::One(branch.clone()));
            }
            return None;
        };
        let holds = self.test_holds(at, params, *argument, test, *by_default, None);
        let Some(holds) = holds else {
            // Nothing settled it, and the call is still one of the two branches -- each read *for
            // this call*, which is what the declaration's own union cannot do: `tap($repository,
            // $maybe)` is a `HigherOrderTapProxy` or the `Repository` it was handed, where the
            // declaration says `TValue`, a template it can only stand in for with `mixed`. A branch
            // the call cannot fill leaves the declaration to answer, as before.
            //
            // Both, and not the else branch alone: that read holds for `reset`, whose else is
            // `V|false` and already admits everything, and it is wrong for `array_first`, whose
            // else is a bare `null` -- answering it said the array is empty on every call with no
            // guard above it. A branch that is only the absence goes second, so the pair reads as
            // the other one made nullable.
            return Some(Settled::of(vec![then.clone(), otherwise.clone()]));
        };
        Some(Settled::One(match holds {
            true => then.clone(),
            false => otherwise.clone(),
        }))
    }

    /// The branches a chain picks for this call: the first link whose test holds, the last branch
    /// when every link passes on, and the absence beside either where a link that would only have
    /// given the absence could not be settled.
    ///
    /// A link the call does not settle is asked again of each part of its argument -- see
    /// [`Self::chain_split`] -- and anything still unsettled after that settles nothing, for the
    /// reason `settled_branch` gives.
    fn chain_settled(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        cases: &[phrost_sema::TestCase],
        otherwise: &phrost_sema::ReturnBranch,
        narrowed: Option<(u16, &Held)>,
    ) -> Option<Vec<phrost_sema::ReturnBranch>> {
        let mut absent = Vec::new();
        for (link, case) in cases.iter().enumerate() {
            match self.test_holds(
                at,
                params,
                case.argument,
                &case.test,
                case.by_default,
                narrowed,
            ) {
                Some(holds) if holds != case.negated => {
                    return Some([case.then.clone()].into_iter().chain(absent).collect());
                }
                Some(_) => continue,
                None if self.is_the_absence(&case.then) => absent.push(case.then.clone()),
                None if narrowed.is_none() => {
                    let mut picked =
                        self.chain_split(at, params, &cases[link..], otherwise, case.argument)?;
                    picked.extend(absent);
                    return Some(picked);
                }
                None => return None,
            }
        }
        Some([otherwise.clone()].into_iter().chain(absent).collect())
    }

    /// A chain judged twice for an argument that may be null: once for its null and once for the
    /// rest.
    ///
    /// Eloquent's `find()` is `($key is Model ? TModel|null : ($key is (Arrayable|array) ? static :
    /// TModel|null))`, and a `?User` passed there is a `User` or a null -- which the first test
    /// answers yes for one and no for the other, so it settles nothing whole. But each part settles
    /// every link: the `User` is a model, and the null is neither a model nor a list of keys. The
    /// second test sees only what the first left, which is the null; judged against the whole
    /// argument instead it took the `User` for an `Arrayable` and put the collection in the answer.
    /// PHPStan 2.2.15 had exactly that bug (phpstan-src#6586).
    ///
    /// The null and the rest only, not every operand of a union. Split by operand, `wp_unslash()`
    /// handed a superglobal's `string|array` back as exactly that, and measured: 133 WordPress
    /// findings, the ones `settled_branch` records the declaration's `mixed` keeping out. A null is
    /// the one part a parameter is so often written to take beside the value it is about.
    ///
    /// One level: a part that still settles nothing settles nothing for the call.
    fn chain_split(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        cases: &[phrost_sema::TestCase],
        otherwise: &phrost_sema::ReturnBranch,
        argument: u16,
    ) -> Option<Vec<phrost_sema::ReturnBranch>> {
        let held = self.argument_held(at, params, argument)?;
        if !self.takes_a_null_off(&held) {
            return None;
        }
        let parts = [self.without_null(held)?, Held::Builtin(BuiltinTy::Null)];
        let mut picked: Vec<phrost_sema::ReturnBranch> = Vec::new();
        for part in &parts {
            for branch in
                self.chain_settled(at, params, cases, otherwise, Some((argument, part)))?
            {
                if !picked.contains(&branch) {
                    picked.push(branch);
                }
            }
        }
        Some(picked)
    }

    /// Whether a branch is nothing but `null` -- the one branch whose union with another says only
    /// that the other may be absent.
    fn is_the_absence(&self, branch: &phrost_sema::ReturnBranch) -> bool {
        matches!(
            branch,
            phrost_sema::ReturnBranch::Ty(ty)
                if matches!(
                    self.skeleton.types().get(*ty),
                    phrost_ty::TypeRef::Builtin(BuiltinTy::Null)
                )
        )
    }

    /// Whether one condition test holds for a call: `Some` when the call settles it either way,
    /// `None` when nothing it wrote does. Shared by a conditional and by every link of a chain, so a
    /// test reads the same wherever it is written.
    fn test_holds(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        argument: u16,
        test: &phrost_sema::ConditionTest,
        by_default: Option<bool>,
        narrowed: Option<(u16, &Held)>,
    ) -> Option<bool> {
        // What the argument is: resolved off the call, or -- inside a chain judging one part of it at
        // a time -- the part being judged. See [`Self::chain_split`].
        let held_of = || match narrowed {
            Some((position, held)) if position == argument => Some(held.clone()),
            _ => self.argument_held(at, params, argument),
        };
        // How many arguments the call wrote says which sort of nothing an absent one was: a count
        // short of the position is a parameter left out, and the declaration's own default answers
        // for it. Anything else is an argument this cannot read, which settles nothing.
        let defaulted = || match at.arity() {
            Some(count) if count <= argument => by_default,
            _ => None,
        };
        // A string test reads the string the way a chain of them does -- the constant a call wrote
        // stands for its value, and so does the one a default names. `wpdb::get_results($query,
        // $output = OBJECT)` tests `$output is 'OBJECT'` in a chain, and nearly every call leaves
        // it out; reading only a literal default, the link settled nothing for all of them.
        if let phrost_sema::ConditionTest::Literal(wanted) = test
            && let Some(value) = self.string_passed_or_defaulted(at, params, argument)
        {
            return Some(value == wanted.as_str());
        }
        match test {
            // What sort of value the argument is takes *resolving* it rather than reading what was
            // written: `preg_replace($p, $r, $value)` decides on `$value`'s type, and a variable is
            // what nearly every real call passes there.
            phrost_sema::ConditionTest::OfKind(kind) => {
                match held_of() {
                    Some(held) => match self.kind_of_held(at.hir, &held) {
                        // `mixed` is the *absence* of a kind, not a kind that fails the test.
                        // Comparing it settles the else branch on a value nothing is known about --
                        // confidently, and usually wrongly. `array<string, mixed>` read out of a
                        // config array is how a real call reaches here.
                        Some(phrost_model::BuiltinTy::Mixed) => None,
                        found => found.map(|found| found == *kind),
                    },
                    // Nothing to resolve, which for a reader that read the call out of text is not
                    // the same as nothing known: what it can read at all is a string -- a quoted
                    // literal or the class a `::class` names, both strings at runtime. Mockery's
                    // `expects('name')` is the shape, and `($something is string ? Expectation :
                    // HigherOrderMessage)` is what decides every `->with()` after it.
                    None => match at.written_string(argument) {
                        Some(_) => Some(*kind == phrost_model::BuiltinTy::String),
                        None => defaulted(),
                    },
                }
            }
            // A kind, or an instance of one class: `Arrayable|array`. Settled the way the kind is,
            // with an object asked whether its class is the one named -- and a scalar answering no
            // to both, which is the call nearly every `find($id)` makes.
            phrost_sema::ConditionTest::OfKindOrInstance { kind, class } => match held_of() {
                Some(held) => self.is_kind_or_instance(at, &held, *kind, *class),
                None => match at.written_string(argument) {
                    Some(_) => Some(*kind == phrost_model::BuiltinTy::String),
                    None => defaulted(),
                },
            },
            // Whether the argument holds anything, which no declared type says: an empty array is
            // falsy in PHP, so what settles it is whether a guard above the call proved the
            // argument truthy. `reset($rows)` under `if (empty($rows)) { return; }` cannot be the
            // `false` its signature admits.
            // Whether the argument holds nothing. For a variadic parameter the call answers it
            // both ways: anything written at or after the position fills it, and nothing written
            // leaves it empty -- unless what was written is a spread, which may itself be empty.
            // For any other parameter only a written literal settles it, or the default.
            phrost_sema::ConditionTest::Empty => match params.get(usize::from(argument)) {
                Some(param) if param.is_variadic => match at.arguments() {
                    Some(written) => {
                        let from = written.get(usize::from(argument)..).unwrap_or_default();
                        let spread = from.iter().any(|&written| {
                            matches!(
                                at.hir.expr(written),
                                phrost_hir::body::ExprData::Argument { spread: true, .. }
                            )
                        });
                        (!spread).then_some(from.is_empty())
                    }
                    None => at.arity().map(|count| count <= argument),
                },
                _ => match at.written_argument(params, argument) {
                    Some(written) => match at.hir.expr(written) {
                        phrost_hir::body::ExprData::ArrayLiteral { elements, .. } => {
                            match at.hir.expr_list(*elements) {
                                [] => Some(true),
                                _ => holds_an_element(at.hir, written).then_some(false),
                            }
                        }
                        _ => None,
                    },
                    None => defaulted(),
                },
            },
            phrost_sema::ConditionTest::NonEmpty => match at.written_argument(params, argument) {
                // The *then* branch only. Nothing proving the value truthy is not evidence that it
                // is empty, and settling the else branch on that is confidently wrong -- the same
                // trap the `mixed` arm above documents, arriving from the other side. An argument
                // left out is different: nothing was passed, and `by_default` answers for it.
                Some(written) => (phrost_body::value_path_of(at.hir, written)
                    .is_some_and(|path| self.proven_truthy(at.offset, at.hir, &path))
                    || held_of().is_some_and(|held| self.declared_non_empty(at.hir, &held)))
                .then_some(true),
                None => defaulted(),
            },
            // `null` written answers it, and so does a value whose type has no null in it: `tap($value,
            // function ($v) { ... })` passes a closure, and `($callback is null ? HigherOrderTapProxy
            // : TValue)` read only the keyword, so every tap with a callback settled nothing. A
            // value that admits null, or one nothing is known about, still settles nothing.
            phrost_sema::ConditionTest::Null => {
                if let Some(literal) = argument_for(at.literals, params, argument) {
                    return self.literal_satisfies(at.offset, literal, test);
                }
                match held_of() {
                    Some(held) => match self.kind_of_held(at.hir, &held)? {
                        BuiltinTy::Null => Some(true),
                        BuiltinTy::Mixed => None,
                        _ if self.takes_a_null_off(&held) => None,
                        _ => Some(false),
                    },
                    None => match at.written_string(argument) {
                        Some(_) => Some(false),
                        None => defaulted(),
                    },
                }
            }
            // A literal the call wrote answers by its text, and anything else by its form -- see
            // `string_test`.
            phrost_sema::ConditionTest::NonEmptyString
            | phrost_sema::ConditionTest::NonFalsyString => {
                let falsy_too = matches!(test, phrost_sema::ConditionTest::NonFalsyString);
                if let Some(literal) = argument_for(at.literals, params, argument) {
                    return self.literal_satisfies(at.offset, literal, test);
                }
                match at.written_argument(params, argument) {
                    Some(written) => self.string_test_holds(at, written, falsy_too),
                    None => match at.written_string(argument) {
                        Some(written) => Some(string_holds(written, falsy_too)),
                        None => defaulted(),
                    },
                }
            }
            // A shape is read off the array the call wrote, or the one its variable was assigned.
            phrost_sema::ConditionTest::Shape {
                key,
                accepts,
                sealed,
            } => match at.written_argument(params, argument) {
                Some(written) => self.shape_test_holds(at, written, key, accepts, *sealed),
                None => defaulted(),
            },
            phrost_sema::ConditionTest::Scalar => match held_of() {
                Some(held) => self.is_scalar(at, &held),
                None => match argument_for(at.literals, params, argument) {
                    Some(literal) => self.literal_satisfies(at.offset, literal, test),
                    None => match at.written_string(argument) {
                        Some(_) => Some(true),
                        None => defaulted(),
                    },
                },
            },
            // An instance of a class, which only resolving the argument can say -- and a literal or a
            // string the scanner read is no object at all.
            phrost_sema::ConditionTest::Instance(class) => match held_of() {
                Some(held) => self.is_instance(at, &held, *class),
                None => match argument_for(at.literals, params, argument)
                    .map(|_| ())
                    .or_else(|| at.written_string(argument).map(|_| ()))
                {
                    Some(()) => Some(false),
                    None => defaulted(),
                },
            },
            _ => match argument_for(at.literals, params, argument) {
                Some(literal) => self.literal_satisfies(at.offset, literal, test),
                // The same string the `OfKind` arm reads, for the two tests a string can settle on
                // its own. Every other test needs a kind the scanner did not keep.
                None => match at.written_string(argument) {
                    Some(written) => match test {
                        phrost_sema::ConditionTest::Literal(wanted) => {
                            Some(written == wanted.as_str())
                        }
                        phrost_sema::ConditionTest::ClassString(bound) => {
                            self.names_a_class_of(at.offset, written, *bound)
                        }
                        _ => None,
                    },
                    None => defaulted(),
                },
            },
        }
    }

    /// Whether the value is known truthy here: a guard above this position proved it, or what was
    /// written into it says so by its form.
    ///
    /// Two searches, because a local's claims are ranked by `local_binding` and everything reached
    /// through a path is answered by the guard walk -- the same split every other reader of a
    /// narrowing makes. What both answer with is a `NotNull` that is *not* strict: `$x !== null`
    /// leaves a `0` and an `[]` behind it and only truthiness rules them out, which is the whole
    /// difference this asks about.
    ///
    /// The form is the other half, and the one no guard states: an array literal holding something
    /// is never empty. Laravel's `upsert` guards `empty($values)` and then, in a branch,
    /// `$values = [$values]` -- truthy on both paths, and asked only of the guard the branch's write
    /// withdrew every claim, so `array_first($values)` read as possibly null. Each write is asked
    /// where it was written and the path that skipped them where they had not yet happened, which is
    /// what each of them means.
    fn proven_truthy(
        &self,
        offset: phrost_model::TextSize,
        hir: &FileHir,
        path: &phrost_body::ValuePath,
    ) -> bool {
        if let phrost_body::PathRoot::ArrayLiteral(literal) = path.root
            && path.steps.is_empty()
        {
            return holds_an_element(hir, literal);
        }
        let never = |hir: &FileHir, expr| self.call_never_returns(offset, hir, expr);
        let asserts = self.asserting();
        let (narrowed, written_at) = match &path.root {
            phrost_body::PathRoot::Local(name) if path.steps.is_empty() => {
                match phrost_body::local_binding(hir, offset, name.as_str(), &never, &asserts) {
                    Some(bound) => (bound.value, bound.written_at),
                    None => return false,
                }
            }
            _ => match phrost_body::narrowed_at(hir, offset, path, &never, &asserts) {
                Some(narrowed) => (narrowed, None),
                None => return false,
            },
        };
        // A literal is only as non-empty as it was written while nothing has had it since:
        // `array_pop($stack)` takes it by reference and empties it, and a binding is not withdrawn by
        // a call. Which parameters are by-reference is the callee's to say, so any call passing the
        // value after the write refuses the claim -- the one being settled starts at `offset` and is
        // not one of them.
        let untouched_since = |written: phrost_model::TextSize| {
            phrost_body::passed_to_any_call(hir, offset, path)
                .iter()
                .all(|passed| passed.at < written || passed.at >= offset)
        };
        let written_non_empty =
            |value: &phrost_body::ValuePath, at: phrost_model::TextSize| match value.root {
                phrost_body::PathRoot::ArrayLiteral(literal) if value.steps.is_empty() => {
                    holds_an_element(hir, literal) && untouched_since(at)
                }
                _ => self.proven_truthy(at, hir, value),
            };
        match &narrowed.root {
            phrost_body::PathRoot::NotNull { refused, .. } => refused.proves_truth(),
            phrost_body::PathRoot::ArrayLiteral(_) => {
                written_at.is_some_and(|at| written_non_empty(&narrowed, at))
            }
            phrost_body::PathRoot::MaybeWritten {
                writes,
                read,
                before,
                otherwise,
            } => {
                let skipped = match otherwise {
                    phrost_body::Otherwise::Impossible => true,
                    phrost_body::Otherwise::NotNull { refused } if refused.proves_truth() => true,
                    _ => self.proven_truthy(*before, hir, read),
                };
                skipped
                    && writes
                        .iter()
                        .all(|write| written_non_empty(&write.value, write.at))
            }
            _ => false,
        }
    }

    /// Whether a declared type rules the empty array out: a `non-empty-array` or `non-empty-list`,
    /// or a shape with a key it does not mark optional. `array{1: int, 2: int}` cannot be `[]`, and
    /// psl's own static-analysis fixture hands one to `first()` and reads the result as an `int`.
    ///
    /// Declared, and not a [`Held::Record`] built from a literal, although a literal is never empty
    /// as written: whether it is still holding something is a question about what has had it since,
    /// which [`Self::proven_truthy`] asks and a type cannot.
    fn declared_non_empty(&self, hir: &FileHir, held: &Held) -> bool {
        match held {
            Held::Written(id) => match hir.type_ref(*id) {
                phrost_hir::TypeRefData::Generic { non_empty, .. } => *non_empty,
                phrost_hir::TypeRefData::Shape { fields, .. } => {
                    hir.type_ref_list(*fields).iter().any(|field| {
                        matches!(
                            hir.type_ref(*field),
                            phrost_hir::TypeRefData::ShapeField {
                                optional: false,
                                ..
                            }
                        )
                    })
                }
                _ => false,
            },
            Held::Stored { ty, .. } => {
                let types = self.skeleton.types();
                matches!(
                    types.get(*ty),
                    phrost_ty::TypeRef::Generic {
                        non_empty: true,
                        ..
                    }
                ) || types
                    .shape_keys(*ty)
                    .iter()
                    .any(|(_, _, optional)| !optional)
            }
            Held::OneOf(operands) => {
                !operands.is_empty()
                    && operands
                        .iter()
                        .all(|operand| self.declared_non_empty(hir, operand))
            }
            _ => false,
        }
    }

    /// The string the call passes at `position`, or the parameter's default when it passes nothing
    /// there -- a quoted literal either way, or a constant whose declaration holds one.
    ///
    /// `get_post($id)` passes no `$output`, and its default is the constant `OBJECT`, declared by
    /// `define('OBJECT', 'OBJECT')` in a file of its own. `None` for anything else, which is a call
    /// that settles no case and leaves the declaration standing.
    pub(super) fn string_passed_or_defaulted(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        position: u16,
    ) -> Option<String> {
        if let Some(literal) = super::arguments_from(at.literals, params, position).first() {
            return match literal.kind {
                phrost_hir::body::ArgumentLiteral::Constant => {
                    self.constant_string(at.offset, literal.value.as_str())
                }
                _ if literal.wrote_its_value() => Some(literal.value.as_str().to_owned()),
                _ => None,
            };
        }
        // Written but not a literal: a variable says nothing this can compare.
        if at.arity()? > position {
            return None;
        }
        let default = params
            .get(usize::from(position))?
            .default
            .as_deref()?
            .trim();
        match unquoted(default) {
            Some(text) => Some(text.to_owned()),
            None => self.constant_string(at.offset, default),
        }
    }

    /// The branch of a function's string-test chain that a call written as *text* picks -- the
    /// completion chain's reader, which has the arguments as characters and no call node.
    ///
    /// The same three sources as [`Self::string_passed_or_defaulted`]: a quoted literal, a constant,
    /// or the parameter's default when the call wrote nothing there. Only a branch that is a written
    /// type, since the others need the call.
    pub(crate) fn literal_case_for_text(
        &self,
        offset: phrost_model::TextSize,
        fqn: &str,
        arguments: &[Box<str>],
    ) -> Option<Held> {
        let phrost_sema::Contract::LiteralCases {
            argument,
            cases,
            otherwise,
        } = self.function_contract(fqn)?
        else {
            return None;
        };
        let params = self.function_params(fqn)?;
        let written = match arguments.get(usize::from(*argument)) {
            Some(text) => text.trim().to_owned(),
            None => params
                .get(usize::from(*argument))?
                .default
                .as_deref()?
                .trim()
                .to_owned(),
        };
        let value = match unquoted(&written) {
            Some(text) => text.to_owned(),
            None => self.constant_string(offset, &written)?,
        };
        let branch = cases
            .iter()
            .find(|(literal, _)| literal.as_str() == value)
            .map_or(otherwise.clone(), |(_, branch)| branch.clone());
        match branch {
            phrost_sema::ReturnBranch::Ty(ty) => Some(Held::Stored { ty, scope: None }),
            _ => None,
        }
    }

    /// The string a global constant holds, from either way of declaring one: `const X = 'v';` or
    /// `define('X', 'v');`.
    fn constant_string(&self, offset: phrost_model::TextSize, name: &str) -> Option<String> {
        let (anchor, path) = phrost_hir::NameAnchor::of(name);
        let declared = self.resolve_name(offset, path, anchor, phrost_hir::ImportKind::Constant)?;
        let text = self.source_between(
            declared.file,
            phrost_model::TextRange::new(declared.name_range.end(), declared.full_range.end()),
        )?;
        let text = text.trim_start();
        // After the name: `= 'v';` for `const`, and `', 'v' )` for `define` -- whose name is the
        // quoted first argument, so what follows it is its closing quote and a comma.
        let value = match text.strip_prefix('=') {
            Some(rest) => rest.trim().trim_end_matches(';').trim(),
            None => text
                .trim_start_matches(['\'', '"'])
                .trim_start()
                .strip_prefix(',')?
                .trim()
                .trim_end_matches(';')
                .trim_end()
                .trim_end_matches(')')
                .trim(),
        };
        unquoted(value).map(str::to_owned)
    }

    /// The number a global constant holds, read off its own declaration.
    ///
    /// Not stored anywhere: a constant's value is text in whichever file declares it, and the index
    /// keeps a range rather than a value -- the same bargain hover already makes to show `case
    /// SHIPPED = 'shipped'`. Read once, for the one call whose conditional return type turns on a
    /// number, which is `pathinfo` and its handful of neighbours.
    ///
    /// Only the plain form. A constant written as an expression -- `PATHINFO_ALL & ~PATHINFO_DIRNAME`
    /// -- is arithmetic nothing here evaluates, and answering `None` for it leaves the declaration's
    /// own union standing, which is what it said before.
    fn constant_number(&self, offset: phrost_model::TextSize, name: &str) -> Option<i64> {
        // The anchor from the name rather than assumed relative, which is the pair `NameAnchor::of`
        // hands back: a call writes `\JSON_THROW_ON_ERROR` as often as `JSON_THROW_ON_ERROR`, and
        // reading the first as a relative name with a separator still on it resolves nothing. The
        // fourth reader to have taken half of that pair.
        let (anchor, path) = phrost_hir::NameAnchor::of(name);
        let declared = self.resolve_name(offset, path, anchor, phrost_hir::ImportKind::Constant)?;
        let text = self.source_between(
            declared.file,
            phrost_model::TextRange::new(declared.name_range.end(), declared.full_range.end()),
        )?;
        text.trim_start()
            .strip_prefix('=')?
            .trim()
            .trim_end_matches(';')
            .trim()
            .parse()
            .ok()
    }

    /// Whether the literal a call wrote satisfies a test.
    fn literal_satisfies(
        &self,
        offset: phrost_model::TextSize,
        literal: &phrost_hir::body::LiteralArgument,
        test: &phrost_sema::ConditionTest,
    ) -> Option<bool> {
        let keyword = literal.kind == phrost_hir::body::ArgumentLiteral::Keyword;
        Some(match test {
            phrost_sema::ConditionTest::Null => keyword && literal.value.as_str() == "null",
            phrost_sema::ConditionTest::True => keyword && literal.value.as_str() == "true",
            phrost_sema::ConditionTest::False => keyword && literal.value.as_str() == "false",
            phrost_sema::ConditionTest::Literal(wanted) => {
                literal.wrote_its_value() && literal.value == *wanted
            }
            // The number a flag stands for, which the call writes as the constant that names it --
            // `pathinfo($path, PATHINFO_EXTENSION)`, never `pathinfo($path, 4)`. So the digits are
            // compared where they were written and the constant is looked up where it is declared.
            phrost_sema::ConditionTest::Number(wanted) => match literal.kind {
                phrost_hir::body::ArgumentLiteral::Number => {
                    literal.value.as_str().parse::<i64>().ok() == Some(*wanted)
                }
                phrost_hir::body::ArgumentLiteral::Constant => {
                    self.constant_number(offset, literal.value.as_str()) == Some(*wanted)
                }
                _ => false,
            },
            // A string and `Foo::class` share one slot, because both are strings at runtime -- so
            // the only thing that tells them apart is whether the project declares a class by that
            // name.
            phrost_sema::ConditionTest::ClassString(bound) => {
                if !literal.wrote_its_value() {
                    return Some(false);
                }
                return self.names_a_class_of(offset, literal.value.as_str(), *bound);
            }
            // What a string holds, read off the text the call wrote. An interpolated string is a
            // skeleton with a hole where each expression was, and whatever fills a hole, the text
            // around it is still there: `"SELECT * FROM {$table}"` is not falsy whatever `$table`
            // holds. A keyword or a number is no string at all, and a constant is whatever its
            // declaration says, which is not read here.
            phrost_sema::ConditionTest::NonEmptyString
            | phrost_sema::ConditionTest::NonFalsyString => {
                let falsy_too = matches!(test, phrost_sema::ConditionTest::NonFalsyString);
                match literal.kind {
                    phrost_hir::body::ArgumentLiteral::Text => {
                        string_holds(literal.value.as_str(), falsy_too)
                    }
                    phrost_hir::body::ArgumentLiteral::Interpolated => {
                        let written = literal
                            .value
                            .as_str()
                            .chars()
                            .filter(|&found| found != phrost_hir::body::INTERPOLATION_HOLE)
                            .collect::<String>();
                        // Text around a hole proves the string holds something; it proves the string
                        // is not `'0'` only where it could not be that one character by itself.
                        match string_holds(&written, falsy_too) {
                            true => true,
                            false => return None,
                        }
                    }
                    phrost_hir::body::ArgumentLiteral::Keyword
                    | phrost_hir::body::ArgumentLiteral::Number => false,
                    phrost_hir::body::ArgumentLiteral::Constant => return None,
                }
            }
            // A literal is a string, a number or a keyword, and none of them is an object.
            phrost_sema::ConditionTest::Instance(_) => false,
            // And every one of them is a scalar but `null`; a constant is whatever it was declared.
            phrost_sema::ConditionTest::Scalar => match literal.kind {
                phrost_hir::body::ArgumentLiteral::Keyword => literal.value.as_str() != "null",
                phrost_hir::body::ArgumentLiteral::Constant => return None,
                _ => true,
            },
            // Settled from the argument's type or from a guard above the call, not from what was
            // written -- see `settled_branch`.
            phrost_sema::ConditionTest::OfKind(_)
            | phrost_sema::ConditionTest::OfKindOrInstance { .. }
            | phrost_sema::ConditionTest::NonEmpty
            | phrost_sema::ConditionTest::Empty
            | phrost_sema::ConditionTest::Shape { .. } => {
                return None;
            }
        })
    }

    /// Whether a value is of `kind` or an instance of `class`, when that can be told.
    ///
    /// `None` for anything the value's type does not settle: `mixed`, a union, and an object whose
    /// class is not known -- each could be either.
    fn is_kind_or_instance(
        &self,
        at: &CallFacts<'_>,
        held: &Held,
        kind: phrost_model::BuiltinTy,
        class: phrost_ty::TypeId,
    ) -> Option<bool> {
        let found = self.kind_of_held(at.hir, held)?;
        if found == kind {
            return Some(true);
        }
        match found {
            phrost_model::BuiltinTy::Mixed => None,
            phrost_model::BuiltinTy::Object => {
                let types = self.skeleton.types();
                let named = match types.get(class) {
                    phrost_ty::TypeRef::Generic { base, .. } => types.get(*base),
                    other => other,
                };
                let phrost_ty::TypeRef::Named(wanted) = named else {
                    return None;
                };
                let held_class = self.class_of_held(at.offset, at.hir, held)?;
                Some(
                    held_class.as_str().eq_ignore_ascii_case(wanted.as_str())
                        || self.descends_from(held_class.as_str(), wanted.as_str(), INHERIT_DEPTH),
                )
            }
            // Every other kind is a scalar, which is neither.
            _ => Some(false),
        }
    }

    /// Whether a value is an instance of `class`, when that can be told.
    ///
    /// Asked of every operand of a union, with the null asked apart: `?\Closure` given a closure or
    /// nothing is either, and only an answer every operand agrees on settles the test. An operand
    /// is judged the way a declaration is -- the same class or one below it is an instance, an
    /// unrelated class with nothing below it to bridge them is not, and a parent or an interface
    /// may be either, so it says nothing.
    fn is_instance(
        &self,
        at: &CallFacts<'_>,
        held: &Held,
        class: phrost_ty::TypeId,
    ) -> Option<bool> {
        let types = self.skeleton.types();
        let named = match types.get(class) {
            phrost_ty::TypeRef::Generic { base, .. } => types.get(*base),
            other => other,
        };
        let phrost_ty::TypeRef::Named(wanted) = named else {
            return None;
        };
        let operand = |kind: Option<phrost_model::BuiltinTy>, named: Option<Name>| match kind? {
            phrost_model::BuiltinTy::Mixed => None,
            phrost_model::BuiltinTy::Object => match self.relate_classes(&named?, wanted) {
                phrost_ty::compat::Fit::Accepted => Some(true),
                phrost_ty::compat::Fit::Rejected => Some(false),
                _ => None,
            },
            // A closure is held as what invoking it gives back, with `Closure` for its class -- and
            // a `callable` that is not one may still be a string or an array, so only a yes counts.
            phrost_model::BuiltinTy::Callable => (self.relate_classes(&named?, wanted)
                == phrost_ty::compat::Fit::Accepted)
                .then_some(true),
            // Every other kind is a scalar, an array or a null, which is no instance of anything.
            _ => Some(false),
        };
        let mut answers = match self.union_operands(at.offset, at.hir, held) {
            Some(operands) => operands
                .into_iter()
                .map(|one| operand(one.kind, one.class))
                .collect::<Vec<_>>(),
            None => vec![operand(
                self.kind_of_held(at.hir, held),
                self.class_of_held(at.offset, at.hir, held),
            )],
        };
        if self.null_reach_of(at.hir, held) != crate::SentinelReach::Impossible {
            answers.push(Some(false));
        }
        let first = answers.first().copied().flatten()?;
        answers
            .iter()
            .all(|answer| *answer == Some(first))
            .then_some(first)
    }

    /// Whether a string names a class, and one of `bound` or below it where the test was written
    /// over one.
    ///
    /// The class itself and not an instance of it, so there is no subclass to leave room for: a
    /// named class either descends from the bound or does not. A bound the project does not declare
    /// is no bound -- it is a template, or a class nothing here can see, and either way the test is
    /// what it was before it carried one.
    fn names_a_class_of(
        &self,
        offset: phrost_model::TextSize,
        written: &str,
        bound: Option<phrost_ty::TypeId>,
    ) -> Option<bool> {
        let Some(class) = self.written_class(offset, written) else {
            return Some(false);
        };
        let types = self.skeleton.types();
        let bound = bound.and_then(|bound| match types.get(bound) {
            phrost_ty::TypeRef::Named(name) if self.class_header(name.as_str()).is_some() => {
                Some(name.clone())
            }
            _ => None,
        });
        let Some(bound) = bound else {
            return Some(true);
        };
        Some(
            class.as_str().eq_ignore_ascii_case(bound.as_str())
                || self.descends_from(class.as_str(), bound.as_str(), INHERIT_DEPTH),
        )
    }

    /// Whether a value is a scalar, when that can be told: every operand of a union asked, the null
    /// apart, and only an answer they all agree on settles it. A `callable` may be a string or an
    /// object, so it says nothing.
    fn is_scalar(&self, at: &CallFacts<'_>, held: &Held) -> Option<bool> {
        use phrost_model::BuiltinTy as Kind;
        let operand = |kind: Option<Kind>| match kind? {
            Kind::Int | Kind::Float | Kind::String | Kind::Bool | Kind::True | Kind::False => {
                Some(true)
            }
            Kind::Mixed | Kind::Callable | Kind::Iterable => None,
            _ => Some(false),
        };
        let mut answers = match self.union_operands(at.offset, at.hir, held) {
            Some(operands) => operands
                .into_iter()
                .map(|one| operand(one.kind))
                .collect::<Vec<_>>(),
            None => vec![operand(self.kind_of_held(at.hir, held))],
        };
        if self.null_reach_of(at.hir, held) != crate::SentinelReach::Impossible {
            answers.push(Some(false));
        }
        let first = answers.first().copied().flatten()?;
        answers
            .iter()
            .all(|answer| *answer == Some(first))
            .then_some(first)
    }

    /// What one branch of a conditional return type holds.
    /// What the receiver supplied for a branch that names one of the declaring class's templates.
    ///
    /// `None` when the branch names no template, or when nothing -- the receiver's arguments, an
    /// `@extends` between the two, the template's bound -- says what it is; both mean the same to the
    /// caller: the branch stands as written.
    ///
    /// A receiver written without arguments still asks the other two. It returned early here, so
    /// `$country->posts()->find(1)` on a relation built in an untyped body read `?TRelatedModel`
    /// while `->first()` beside it, a plain `@return`, fell to the bound.
    ///
    /// The declaring class comes from the scope the branch was resolved in, which is the only thing
    /// here that knows *whose* templates to look for: a `@return T` on an inherited method is the
    /// interface's `T`, not the class's.
    fn supplied_for_branch(
        &self,
        at: &CallFacts<'_>,
        scope: &Option<RelativeScope>,
        ty: phrost_ty::TypeId,
    ) -> Option<Held> {
        let scope = scope.as_ref()?;
        let called = scope.called.as_str();
        let owner = scope.declaring.as_str();
        // An intersection is one claim per operand, and a `Held` can carry that where the type table
        // cannot: rebuilding an interned type needs it open for writing, and `Held::AllOf` needs
        // nothing. `@return MockObject&T` on PHPUnit's mock builder is the shape -- the value really
        // is both, and reading only the half that resolves loses whichever one the call is about.
        if let phrost_ty::TypeRef::Intersection(parts) = self.skeleton.types().get(ty) {
            let parts = self.skeleton.types().operands(*parts).to_vec();
            let mut substituted = false;
            let mut held = Vec::with_capacity(parts.len());
            for part in parts {
                match self.supplied_for_branch(at, &Some(scope.clone()), part) {
                    Some(part) => {
                        substituted = true;
                        held.push(part);
                    }
                    // A real class stands as the declaration wrote it.
                    None => held.push(self.canonical_held(part)),
                }
            }
            // Nothing was a template, so the intersection is what it always was and the caller's own
            // reading of it is the better one.
            return substituted.then(|| Self::all_of(held));
        }
        // The ladder a plain `@return` goes through, so a branch and a declaration can never
        // disagree about one template -- `?T` under a null included.
        let supplied = match self.declared_through(called, owner, ty, at.receiver, scope) {
            super::Declared::Held(held) => held,
            super::Declared::Type {
                ty: found,
                nullable,
            } if found != ty || nullable => {
                let held = self.canonical_held(found);
                match nullable {
                    true => Self::nullable(held),
                    false => held,
                }
            }
            super::Declared::Type { .. } | super::Declared::Unbound => return None,
        };
        // Both paths, because the receiver's own argument arrives however the property that declared
        // it was written -- `@var InputBag<string>` is an interned `string` and the `''` beside it at
        // the call is a `Held::Builtin`, and two spellings of one value do not fold.
        Some(match supplied {
            // Canonical where that loses nothing. A relative word keeps the scope it was resolved
            // in: `Model::all()` is a `Collection<int, static>` meaning the class it was called on,
            // and folded without its scope `->first()` answered a `static` of nothing.
            Held::Stored { ty, scope } => match self.skeleton.types().get(ty) {
                phrost_ty::TypeRef::Relative(_) => Held::Stored { ty, scope },
                _ => self.canonical_held(ty),
            },
            Held::Written(id) => match at.hir.type_ref(id) {
                phrost_hir::TypeRefData::Builtin(builtin) => Held::Builtin(*builtin),
                _ => Held::Written(id),
            },
            held => held,
        })
    }

    /// An interned type as the [`Held`] a reader would have built for it directly.
    ///
    /// A `Held::Stored` of `string` and a `Held::Builtin(String)` are the same value, and until one of
    /// them is chosen they compare unequal -- so `string|string` survives the union constructor's
    /// repeat fold and reaches a tooltip. The same three-spellings rule this codebase keeps meeting,
    /// applied where a new `Held` is built rather than at every reader that compares two.
    fn canonical_held(&self, ty: phrost_ty::TypeId) -> Held {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Builtin(builtin) => Held::Builtin(*builtin),
            phrost_ty::TypeRef::Named(_) => match self.class_of_stored(ty, None) {
                Some(class) => Held::Class(class),
                None => Held::Stored { ty, scope: None },
            },
            _ => Held::Stored { ty, scope: None },
        }
    }

    pub(super) fn held_of_branch(
        &self,
        at: &CallFacts<'_>,
        branch: phrost_sema::ReturnBranch,
        params: &[phrost_sema::ParamRow],
        scope: Option<RelativeScope>,
    ) -> Option<Held> {
        match branch {
            // A written type, and one more thing it can be: a template the *declaring class*
            // supplies. `@return TDefault|TInput` lowers `TInput` to a name like any other, and what
            // it means is whatever the receiver was written with -- which is the half the contract
            // reader could not see until [`CallFacts::receiver`] existed.
            phrost_sema::ReturnBranch::Ty(ty) => {
                if let Some(supplied) = self.supplied_for_branch(at, &scope, ty) {
                    return Some(supplied);
                }
                // A name nothing declares and nothing supplied is a template the receiver was
                // written without -- `$request->request` is a plain `InputBag` where `$request->query`
                // is an `InputBag<string>`. Answering it as written puts a variable called `TInput` in
                // a tooltip; declining lets the declared return stand, which is a real type.
                self.names_something_real(ty)
                    .then_some(Held::Stored { ty, scope })
            }
            // The branch is a `@template` and the call names the class -- the same claim
            // `ReturnsArgumentClass` makes, for one branch of two.
            //
            // Read off the call's literals, or off the text for a call the completion chain scanned
            // -- which has none, and was answered by the test above and then left with a template it
            // could not fill: `$container->make(Repo::class)->` offered nothing where goto answered.
            phrost_sema::ReturnBranch::ArgumentClass(position) => {
                let literals = arguments_from(at.literals, params, position)
                    .into_iter()
                    .filter(|argument| argument.kind == phrost_hir::body::ArgumentLiteral::Text)
                    .map(|argument| argument.value.as_str());
                let named = literals
                    .chain(at.written_strings_from(params, position))
                    .filter_map(|written| self.written_class(at.offset, written))
                    .map(Held::Class)
                    .collect::<Vec<_>>();
                // One of them for an ordinary parameter; one *or* another for a variadic, which is
                // what `bar(A::class, B::class)` says its result is.
                (!named.is_empty()).then(|| Self::one_of(named))
            }
            // The branch is a `@template` the declaration bound to a parameter's own type, so
            // whatever was passed is what comes back. The plainest generic there is.
            phrost_sema::ReturnBranch::ArgumentType {
                parameter,
                absent,
                evaluated,
            } => {
                // A parameter taking the value *or a callable producing it* -- `T|(\Closure(): T)`
                // -- evaluates a closure it is handed, so what arrives is the closure's result.
                let producer = evaluated;
                // A value that may be a closure the callee calls, whose result nothing here can
                // name: `value($view)` on a `\Closure|string $view`. The branch then answers
                // nothing rather than the closure itself.
                let mut unknowable = false;
                let passed = at
                    .written_arguments_from(params, parameter)
                    .into_iter()
                    .filter_map(|(written, spread)| {
                        if producer
                            && !spread
                            && matches!(
                                at.hir.expr(written),
                                phrost_hir::body::ExprData::Closure { .. }
                            )
                        {
                            return self.callback_held(at, written);
                        }
                        let path = phrost_body::value_path_of(at.hir, written)?;
                        let held =
                            self.resolve_path(at.offset, at.hir, &path, at.budget.checked_sub(1)?)?;
                        // `...$groups` hands over each group, so the argument is one of them.
                        let held = match spread {
                            true => self.element_of_held(at.offset, at.hir, &held)?,
                            false => held,
                        };
                        if producer && self.may_be_called(at, &held) {
                            // What calling it gives, where its type says; otherwise nothing can.
                            let invoked = self.invoked_return(at.hir, &held);
                            unknowable |= invoked.is_none();
                            return invoked;
                        }
                        // A record does not survive being passed somewhere. What comes back is
                        // whatever the function made of the argument, and a shape names exactly the
                        // keys that are in there -- too precise a claim to carry across a call that
                        // may have put more in. `array_merge(['modern' => '', 'fresh' => ''],
                        // $colors)` is the case: the first argument is a sort-order skeleton of
                        // empty strings, the second is a map of objects, and reading the result as
                        // the skeleton reported every property taken off the loop below as a
                        // property of a string. Four findings on WordPress, all wrong.
                        //
                        // Weakened rather than refused, because `array` is what this answered
                        // before records existed and it is still true of the result.
                        Some(match held {
                            Held::Record(_) => Held::Builtin(phrost_model::BuiltinTy::Array),
                            held => held,
                        })
                    })
                    .collect::<Vec<_>>();
                if unknowable {
                    return None;
                }
                match passed.is_empty() {
                    // The call left it out, so what the *declaration* puts there is what arrives:
                    // `get('token')` on `get($key, mixed $default = null)` is a `?string` where
                    // `get('token', '')` is a `string`. 62 of 67 calls on one real project.
                    true => absent.map(Held::Builtin),
                    false => Some(Self::one_of(passed)),
                }
            }
            // The branch is a `@template` the declaration bound to what a container parameter holds,
            // so the answer is the element of whatever was passed -- a type rather than a literal,
            // which is why this needs the call and not just its literals.
            phrost_sema::ReturnBranch::ArgumentElement { parameter, absent } => {
                let passed = self.argument_held(at, params, parameter)?;
                let element = self.value_contained(at.offset, at.hir, &passed)?;
                Some(beside_the_sentinel(element, absent))
            }
            // What names one slot of the argument, for the reason `ArgumentKeys` below gives: the
            // key follows from how the container was written, and `array_key_first` over an
            // `array<string, Row>` is a `string` the declaration could only call `K`.
            phrost_sema::ReturnBranch::ArgumentKey { parameter, absent } => {
                let passed = self.argument_held(at, params, parameter)?;
                let key = self.iteration_key_of_held(at.offset, at.hir, &passed)?;
                Some(beside_the_sentinel(key, absent))
            }
            // The same element, handed back in a container -- `array_values($rows)` is a list of
            // whatever `$rows` held, and the branches disagreed only about how it is keyed.
            phrost_sema::ReturnBranch::ArgumentElements { parameter } => {
                let passed = self.argument_held(at, params, parameter)?;
                let element = self.value_contained(at.offset, at.hir, &passed)?;
                Some(Held::ListOf(Box::new(element)))
            }
            // Whatever was passed, in a container: `array_fill(0, 3, 'value')` is a list of strings.
            // Read exactly as the bare template is, so a spread, a record and a left-out argument
            // mean here what they mean there -- and never as a producer, which lowering refused.
            phrost_sema::ReturnBranch::ArgumentTypes { parameter } => {
                let passed = self.held_of_branch(
                    at,
                    phrost_sema::ReturnBranch::ArgumentType {
                        parameter,
                        absent: None,
                        evaluated: false,
                    },
                    params,
                    scope,
                )?;
                Some(Held::ListOf(Box::new(passed)))
            }
            // Keyed by what one argument holds and holding what the other does. Where the keys
            // argument says nothing about its elements, the key is what any key is -- an
            // `int|string` -- and not the `int` a list would answer, which is the one thing a table
            // of names is not.
            phrost_sema::ReturnBranch::CombinedElements { keys, values } => {
                let passed = self.argument_held(at, params, values)?;
                let value = self.value_contained(at.offset, at.hir, &passed)?;
                let key = self
                    .argument_held(at, params, keys)
                    .and_then(|keys| self.element_of_held(at.offset, at.hir, &keys))
                    .unwrap_or_else(|| {
                        Self::one_of(vec![
                            Held::Builtin(phrost_model::BuiltinTy::Int),
                            Held::Builtin(phrost_model::BuiltinTy::String),
                        ])
                    });
                Some(Held::MapOf {
                    key: Box::new(key),
                    value: Box::new(value),
                })
            }
            // A container of what *names each slot* of the argument, which is a different question
            // from what it holds: `array_keys` over a `list<Row>` is a list of `int` and over an
            // `array<string, Row>` a list of `string`, and neither is written in an argument the
            // template could bind. The same question `foreach ($x as $k)` asks.
            phrost_sema::ReturnBranch::ArgumentKeys { parameter } => {
                let passed = self.argument_held(at, params, parameter)?;
                let key = self.iteration_key_of_held(at.offset, at.hir, &passed)?;
                Some(Held::ListOf(Box::new(key)))
            }
            phrost_sema::ReturnBranch::ArgumentColumn { parameter, column } => Some(Held::ListOf(
                Box::new(self.column_of(at, params, parameter, column)?),
            )),
            // A container of what the callback gives back. Read off the closure written at the call,
            // because that is the only place the type appears: `array_map` says its result is made
            // of `U` and nothing in the declaration can say what `U` is.
            phrost_sema::ReturnBranch::CallbackResults { parameter } => {
                let written = at.written_argument(params, parameter)?;
                Some(Held::ListOf(Box::new(self.callback_held(at, written)?)))
            }
            // The same, handed back as itself: every `reduce` gives out what its callback produced.
            phrost_sema::ReturnBranch::CallbackResult { parameter } => {
                let written = at.written_argument(params, parameter)?;
                self.callback_held(at, written)
            }
            // A conditional slot, settled the way a conditional return is -- by the one reader of
            // a test, so a slot and a whole return cannot read `$groupBy is string` differently.
            // Unsettled, the slot is either branch; one that answers nothing leaves the slot
            // unfilled, which its template's bound then stands in for.
            phrost_sema::ReturnBranch::Conditional(conditional) => {
                let conditional = *conditional;
                match self.test_holds(
                    at,
                    params,
                    conditional.argument,
                    &conditional.test,
                    conditional.by_default,
                    None,
                ) {
                    Some(true) => self.held_of_branch(at, conditional.then, params, scope),
                    Some(false) => self.held_of_branch(at, conditional.otherwise, params, scope),
                    None => {
                        let then =
                            self.held_of_branch(at, conditional.then, params, scope.clone())?;
                        let otherwise =
                            self.held_of_branch(at, conditional.otherwise, params, scope)?;
                        Some(Self::one_of(vec![then, otherwise]))
                    }
                }
            }
            // Read by nothing, so the slot is left for its template's bound.
            phrost_sema::ReturnBranch::Unfilled => None,
            // A class with arguments inside another's slot: `static<int, TValue>` for each group
            // of a `groupBy()`. The class under the call's scope, as the outer one is.
            phrost_sema::ReturnBranch::Applied { class, arguments } => {
                let filled = arguments
                    .into_vec()
                    .into_iter()
                    .map(|argument| self.held_of_branch(at, argument, params, scope.clone()))
                    .collect();
                let class = self
                    .class_location(self.class_of_stored(class, scope.as_ref())?.as_str())?
                    .fqn;
                self.applied_instance_of(class, filled)
            }
        }
    }

    /// Whether a value handed to a parameter that calls a callable may be one: a callable, a
    /// `Closure`, or a union holding either.
    fn may_be_called(&self, at: &CallFacts<'_>, held: &Held) -> bool {
        match held {
            Held::Callable(_) => true,
            Held::OneOf(operands) => operands
                .iter()
                .any(|operand| self.may_be_called(at, operand)),
            Held::Nullable(inner) => self.may_be_called(at, inner),
            // A union written in this file is asked operand by operand, like a declared one.
            Held::Written(id)
                if let phrost_hir::TypeRefData::Union(parts) = at.hir.type_ref(*id) =>
            {
                at.hir
                    .type_ref_list(*parts)
                    .iter()
                    .any(|&part| self.may_be_called(at, &Held::Written(part)))
            }
            // A declared union is one interned type, so its operands are asked one by one too.
            Held::Stored { ty, .. }
                if let phrost_ty::TypeRef::Union(operands) = self.skeleton.types().get(*ty) =>
            {
                self.skeleton
                    .types()
                    .operands(*operands)
                    .iter()
                    .any(|&operand| {
                        self.may_be_called(
                            at,
                            &Held::Stored {
                                ty: operand,
                                scope: None,
                            },
                        )
                    })
            }
            _ => {
                self.kind_of_held(at.hir, held) == Some(BuiltinTy::Callable)
                    || self
                        .class_of_held(at.offset, at.hir, held)
                        .is_some_and(|class| class.as_str().eq_ignore_ascii_case("Closure"))
            }
        }
    }

    /// What invoking a callback that is not a closure literal gives back.
    ///
    /// `$this->convert(...)`, `[$this, 'convert']`, `self::convert(...)`, an invokable object, or a
    /// variable holding any of them: each resolves to a callable carrying its return, the same answer `$callback()`
    /// reads. Only the literal was read here, so `array_map($this->convert(...), $rows)` -- the
    /// spelling PHP 8.1 made for exactly this -- was a list of nothing.
    fn invoked_value(
        &self,
        at: &CallFacts<'_>,
        expr: phrost_hir::ExprId,
    ) -> Option<crate::resolve::Held> {
        if let Some(named) = self.callable_named_by_a_string(at, expr) {
            return Some(named);
        }
        let path = phrost_body::value_path_of(at.hir, expr)?;
        let written_at = at.hir.expr(expr).range().start();
        let held = self.resolve_path(written_at, at.hir, &path, at.budget.checked_sub(1)?)?;
        // An array is called as the `[$object, 'method']` pair it may be, written here or in a
        // variable -- see `invoked_pair`. Asked of an array only, since the pair is read off the
        // path and following a local there is a walk.
        if self.kind_of_held(at.hir, &held) == Some(BuiltinTy::Array) {
            return self.invoked_pair(written_at, at.hir, &path, at.budget);
        }
        // An object is called through its `__invoke`, which is the one callable a value of a class is.
        self.invoked_return(at.hir, &held).or_else(|| {
            let class = self.class_of_held(at.offset, at.hir, &held)?;
            self.member_held(class.as_str(), phrost_sema::MemberKind::Method, "__invoke")
        })
    }

    /// The text a string literal holds, when it holds it at rest: quotes off, and nothing
    /// interpolated into it.
    fn quoted_text(&self, at: &CallFacts<'_>, expr: phrost_hir::ExprId) -> Option<String> {
        let phrost_hir::body::ExprData::Literal {
            kind: phrost_hir::body::LiteralKind::String,
            range,
        } = at.hir.expr(expr)
        else {
            return None;
        };
        let text = self
            .current()?
            .text()
            .get(usize::from(range.start())..usize::from(range.end()))?;
        let inside = text.get(1..text.len().checked_sub(1)?)?;
        (!inside.is_empty() && !inside.contains('$')).then(|| inside.to_owned())
    }

    /// One column of every row: the field a shape names, or the property an object holds.
    ///
    /// Read off the row's type at the key the call wrote, the same reading `$row['name']` and
    /// `$row->name` get one at a time. A key nothing can name -- a variable, an expression --
    /// reaches every field at once, and a `null` column is the whole row.
    fn column_of(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        parameter: u16,
        column: u16,
    ) -> Option<Held> {
        let rows = self.argument_held(at, params, parameter)?;
        let row = self.value_contained(at.offset, at.hir, &rows)?;
        let written = at.written_argument(params, column)?;
        if matches!(
            at.hir.expr(written),
            phrost_hir::body::ExprData::Literal {
                kind: phrost_hir::body::LiteralKind::Null,
                ..
            }
        ) {
            return Some(row);
        }
        let keys = self.literal_keys(at, written);
        if keys.is_empty() {
            return self
                .shape_values(at.offset, at.hir, &row)
                .or_else(|| self.element_of_held(at.offset, at.hir, &row));
        }
        let class = self.class_of_held(at.offset, at.hir, &row);
        let columns = keys
            .iter()
            .map(|key| match &class {
                Some(class) => {
                    self.member_held(class.as_str(), phrost_sema::MemberKind::Property, key)
                }
                None => self.indexed_of_held(
                    at.offset,
                    at.hir,
                    &row,
                    Some(key),
                    crate::resolve::Subscript::Written,
                ),
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self::one_of(columns))
    }

    /// The keys a column argument can be: one literal, or each branch of a ternary between two.
    ///
    /// `array_column($tables, $qualified ? 'schema_qualified_name' : 'name')` is how Laravel picks a
    /// column, and reading it as "any key" answered every field the row has. Empty for anything
    /// else, which is a key nothing here can name.
    fn literal_keys(&self, at: &CallFacts<'_>, written: phrost_hir::ExprId) -> Vec<String> {
        match at.hir.expr(written) {
            phrost_hir::body::ExprData::Conditional {
                then_branch: Some(then),
                else_branch,
                ..
            } => {
                let (then, otherwise) = (
                    self.literal_keys(at, *then),
                    self.literal_keys(at, *else_branch),
                );
                match then.is_empty() || otherwise.is_empty() {
                    true => Vec::new(),
                    false => then.into_iter().chain(otherwise).collect(),
                }
            }
            _ => self.quoted_text(at, written).into_iter().collect(),
        }
    }

    /// What the runtime function a string callback names gives back.
    ///
    /// `array_map('trim', $lines)`, `array_map('intval', $ids)`: the commonest callback PHP has, read
    /// as fully qualified because a string is not affected by the file's `use` statements, and only
    /// where the text is the whole value.
    ///
    /// **Only the runtime's own functions**, whose stubs say what one call on one element returns --
    /// often conditionally. A project's function declares its return for every argument at once:
    /// WordPress's `esc_sql` says `string|array`, "in the same type as supplied", and read for a
    /// string element that made thirteen `implode(', ', array_map('esc_sql', $names))` calls pass
    /// arrays of arrays. A first-class callable or `[$this, 'm']` is the author choosing the
    /// declaration; a string name reaching into the project is not, and answering nothing there
    /// is what it answered before.
    fn callable_named_by_a_string(
        &self,
        at: &CallFacts<'_>,
        expr: phrost_hir::ExprId,
    ) -> Option<crate::resolve::Held> {
        let inside = self.quoted_text(at, expr)?;
        // A single-quoted string doubles nothing but its own quote and a backslash, and a namespace
        // separator is written `\` as often as `\` -- both mean one.
        let name = inside.as_str().replace("\\\\", "\\");
        let name = name.trim_start_matches('\\');
        if name.contains("::") {
            return None;
        }
        let declared = self.function_location(name)?;
        if self
            .skeleton
            .file(declared.file)
            .is_none_or(|file| file.origin() != phrost_model::SourceOrigin::Stub)
        {
            return None;
        }
        Some(crate::resolve::Held::Stored {
            ty: self.function_return(name)?,
            scope: None,
        })
    }

    /// What a closure gives back.
    ///
    /// Its declared return type first -- `fn (Row $r): Item => ...` -- because that is what the
    /// author committed to, and an inferred type must never displace a written one. Failing that,
    /// whatever the body produces: one expression for an arrow function, whose body *is* the
    /// return, and the union of every `return` for a block closure -- see [`crate::closures`] for
    /// why a body settles this and a named function's body does not.
    ///
    /// Asked of a closure written at a call, where the callback's own result is the only place the
    /// caller's element type appears, and of one read as a value, where it is what invoking the
    /// variable holding it gives back.
    pub(crate) fn callback_held(
        &self,
        at: &CallFacts<'_>,
        expr: phrost_hir::ExprId,
    ) -> Option<crate::resolve::Held> {
        let phrost_hir::body::ExprData::Closure {
            kind,
            return_type,
            body,
            ..
        } = at.hir.expr(expr)
        else {
            return self.invoked_value(at, expr);
        };
        if let Some(declared) = return_type {
            return Some(Held::Written(*declared));
        }
        if *kind != phrost_hir::body::ClosureKind::ArrowFunction {
            return self.inferred_closure_return(at, expr, *body);
        }
        let value = only_expression(at.hir, *body)?;
        let path = phrost_body::value_path_of(at.hir, value)?;
        // At the expression's own position, so a name written inside the arrow function resolves in
        // its scope rather than in the caller's.
        let inside = at.hir.expr(value).range().start();
        self.resolve_path(inside, at.hir, &path, at.budget.checked_sub(1)?)
    }

    /// What the call passed for one parameter, resolved to what it holds.
    pub(crate) fn argument_held(
        &self,
        at: &CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        position: u16,
    ) -> Option<Held> {
        let written = at.written_argument(params, position)?;
        let path = phrost_body::value_path_of(at.hir, written)?;
        self.resolve_path(at.offset, at.hir, &path, at.budget.checked_sub(1)?)
    }

    /// The class a catalog names for a receiver that a previous call produced.
    ///
    /// `$storage = $manager->getStorage('node')` then `$storage->load($id)`. Nothing at the `load`
    /// call names the entity type and the receiver's class cannot either -- `SqlContentEntityStorage`
    /// is the storage for most of Drupal's content entity types at once -- so the only thing in the
    /// source that says `Node` is the id one link back.
    ///
    /// `declared` decides the shape and the catalog decides only the class. `load()` admits null and
    /// keeps admitting it, because an id that matches no row really does return null; that is the
    /// difference from [`Self::returned_from_argument`], where the declaration said `object` and had
    /// nothing worth keeping.
    ///
    /// Gated in the order the same lesson taught: there has to *be* a previous call with a literal,
    /// then the method has to be contracted, then the previous call has to be the right one, then the
    /// literal has to be a key -- and only then is the hierarchy climbed. `load` is a common name in
    /// a Drupal project and `getStorage` is not, so the third gate settles nearly all of them.
    pub(crate) fn returned_through(
        &self,
        hir: &FileHir,
        class_fqn: &str,
        method: &str,
        produced_by: Option<(&Name, &Name)>,
        declared: Option<&Held>,
    ) -> Option<Held> {
        let (through, key) = produced_by?;
        let contracts = self.skeleton.returns();

        let (named, yields) = contracts
            .forwarded_bindings(method)
            .iter()
            .filter(|binding| binding.through.eq_ignore_ascii_case(through.as_str()))
            .filter_map(|binding| {
                let class = contracts.catalog_class(&binding.catalog, key.as_str())?;
                let applies = class_fqn.eq_ignore_ascii_case(&binding.class)
                    || self.descends_from(class_fqn, &binding.class, HIERARCHY_DEPTH);
                applies.then_some((class, binding.yields))
            })
            .next()?;

        // Through the catalog rather than straight to a name, for the reason
        // `returned_from_argument` gives: a catalog may name a class the index does not have.
        let held = Held::Class(self.class_location(named)?.fqn);

        // An array of them says nothing about null and cannot: `loadMultiple()` returns an empty
        // array, never a null, so the declaration's nullability belongs to the array and not to what
        // it holds. Only the one-of-it case consults it.
        if yields == Forwarded::ManyOf {
            return Some(Held::ListOf(Box::new(held)));
        }

        match declared.map(|declared| self.null_reach_of(hir, declared)) {
            Some(SentinelReach::Possible | SentinelReach::Certain) => Some(Self::nullable(held)),
            _ => Some(held),
        }
    }
}

/// The text inside a single- or double-quoted literal with nothing to interpolate or escape.
fn unquoted(written: &str) -> Option<&str> {
    let inner = written
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            written
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })?;
    (!inner.contains(['\\', '$', '\'', '"'])).then_some(inner)
}

/// Whether an argument is an array literal that plainly holds something: one element written as
/// itself rather than spread from another array, which may be empty.
///
/// The one non-emptiness the written code states outright rather than declares -- a declared
/// `non-empty-list<V>` is [`IndexView::declared_non_empty`]'s. `array_key_first(['one' => 1])` read
/// as `string|int|null`, and the `strlen()` around it was reported for a null that cannot arrive.
fn holds_an_element(hir: &FileHir, written: phrost_hir::ExprId) -> bool {
    let phrost_hir::body::ExprData::ArrayLiteral { elements, .. } = hir.expr(written) else {
        return false;
    };
    hir.expr_list(*elements).iter().any(|element| {
        !matches!(
            hir.expr(*element),
            phrost_hir::body::ExprData::ArrayElement {
                kind: phrost_hir::body::ArrayElementKind::Spread,
                ..
            }
        )
    })
}

/// Whether a string's text passes a string test: holds anything, and -- for `non-falsy-string` --
/// is not the one other falsy string, `'0'`.
pub(super) fn string_holds(written: &str, falsy_too: bool) -> bool {
    let wanted = match falsy_too {
        true => BuiltinTy::NonFalsyString,
        false => BuiltinTy::NonEmptyString,
    };
    BuiltinTy::string_holding(written.as_bytes()).string_rank() >= wanted.string_rank()
}
