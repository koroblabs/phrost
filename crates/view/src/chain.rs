use phrost_body::{ElementKey, PathRoot, PathStep, ValuePath};
use phrost_model::{Name, TextSize};
use phrost_sema::MemberKind;

use crate::{
    resolve::Held,
    view::{FOLLOW_LIMIT, IndexView},
};

/// How many answers one step may carry forward.
///
/// A chain through an intersection branches, and a branch that branches again multiplies. Real code
/// intersects two or three types, so this is a backstop against a pathological declaration rather
/// than a limit anybody meets.
const BREADTH: usize = 8;

/// What a written receiver starts from.
///
/// Built by whoever scanned the text — completion reads backwards from the cursor, because at the
/// moment someone asks, `$this->repo->` is not an expression and the tree has nothing to say about
/// it. Borrowed rather than owned: this lives for one request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChainRoot<'a> {
    /// `$this` or `static` -- what the value is, so a guard can narrow it. The same distinction
    /// `PathRoot` draws, for the same reason.
    This,
    /// `self` -- the class the code is written in, never narrowed.
    SelfClass,
    Parent,
    /// A local variable, without its `$`.
    Local(&'a str),
    /// A written class name.
    Class(&'a str),
    /// A free function's result: `reset($rows)->`, `make_thing()->`.
    ///
    /// The arguments as written, because what some of these return is decided by what they were
    /// given -- the same reason a [`ChainStep::Member`] carries them.
    Function {
        name: &'a str,
        /// Each argument as written, in position order.
        arguments: &'a [Box<str>],
    },
}

/// One thing applied to what came before it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChainStep<'a> {
    /// `->name` or `::name`. [`MemberKind::Property`] for a field read, [`MemberKind::Method`] for
    /// a call's result.
    Member {
        name: &'a str,
        kind: MemberKind,
        /// What the call was written with, as far as text can say -- one entry per position, `None`
        /// for a slot the reader could not make out. Empty for anything that is not a call.
        ///
        /// Carried because for some methods an argument *is* part of the type, and a chain is the one
        /// place nothing else can say: the statement being typed does not parse, so there is no
        /// expression to read the arguments off.
        arguments: &'a [Option<Box<str>>],
    },
    /// `::name` -- a class constant or an enum case.
    ///
    /// One step and not two because the text cannot tell them apart: neither is written with a
    /// sigil, and `Suit::Hearts` and `Config::DEFAULT` are the same six characters of syntax. They
    /// are stored under different kinds, so both tables are asked.
    Constant(&'a str),
    /// `(...)` on a value: PHP calls the object's `__invoke`.
    Invoked,
    /// `[...]` -- one element out of what came before.
    ///
    /// `Some` for a key this can name, `None` for a computed one where every slot holds the same
    /// thing as far as anyone here can tell -- see [`ChainKey`] for why a *variable* key is one it
    /// can name.
    Element(Option<ChainKey<'a>>),
}

/// A written index, as far as characters can identify the slot it names.
///
/// Two kinds and not one, because they answer different questions and the value paths this becomes
/// have made the distinction since they existed (`ElementKey`). A literal is what a *shape* is
/// keyed by: `array{app: Application}` indexed by `app` is not what it holds in general. A variable
/// names no shape key and still identifies the slot -- `$rows[$id]` read twice is the same element,
/// which is what lets a guard written about it narrow the read below.
///
/// The scanner used to keep the first and throw the second away, so `assert($rows[$id] instanceof
/// Widget); $rows[$id]->` offered nothing while the same two lines with `'a'` for `$id` offered
/// everything -- the scanner disagreeing with the parsed path about a vocabulary both share.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChainKey<'a> {
    Literal(&'a str),
    /// The local's name, without its `$`.
    Variable(&'a str),
}

impl<'a> ChainKey<'a> {
    /// The key a *shape* is keyed by, which only a literal is: `array{app: Application}` says
    /// nothing about `$rows[$id]` however well that identifies a slot.
    pub fn literal(self) -> Option<&'a str> {
        match self {
            Self::Literal(key) => Some(key),
            Self::Variable(_) => None,
        }
    }
}

/// The classes a written receiver reaches, and how to read the list.
///
/// The distinction is the difference between PHP's two composite types, and it decides what may be
/// offered: `A&B` is one value that is both, so everything either declares can be reached, while
/// `A|B` is one value that is one of them, so only what both declare can.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReceiverClasses {
    /// The value is all of these at once. Their member tables together.
    All(Vec<Name>),
    /// The value is one of these, and nothing here knows which.
    ///
    /// A caller that offers members offers all of them and says which operand a name is only on;
    /// one that *reports* may say nothing at all, because a union here is often built by inference
    /// that is approximate in places -- see `PathRoot::EitherOf` -- and a report on an approximate
    /// union is a report on our own imprecision.
    Any {
        classes: Vec<Name>,
        /// Whether every operand of the union is one of them.
        ///
        /// `Alpha|string` names one class and reaches nothing at all through its other operand, so
        /// a member offered from `Alpha` is a member the value may not have -- exactly the risk a
        /// name only one of `Reader|Writer` declares carries, and worth saying the same way. Without
        /// this the list would read as certain, since one class is not "several".
        every_operand_named: bool,
    },
}

impl ReceiverClasses {
    pub fn classes(&self) -> &[Name] {
        match self {
            Self::All(classes) | Self::Any { classes, .. } => classes,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.classes().is_empty()
    }
}

impl IndexView<'_> {
    /// The classes a written receiver chain ends at.
    ///
    /// `$this->repo->find()` is three questions -- what is `$this`, what does its `repo` hold, what
    /// does that class's `find` return -- and each one has to succeed for the next to mean anything.
    /// A miss anywhere gives nothing, which is the honest answer: a partial chain names a class the
    /// receiver is not.
    ///
    /// More than one answer where the value genuinely has more than one API. A chain through an
    /// intersection reaches the members of every operand, and a completion list is the one reader
    /// that wants all of them rather than one: `$this->application->` on an `Application&MockObject`
    /// should offer `expects()` and `getId()` together.
    ///
    /// The two composites are told apart by the variant rather than folded together, because what a
    /// reader may *say* about them differs: an intersection's operands are all true at once, and a
    /// union's are one of several. See [`ReceiverClasses::Any`].
    ///
    /// # Why this carries types and not class names
    ///
    /// A step can be a subscript, and what a subscript gives back is not a class the step before it
    /// named: `array<Item>` and `array{app: Application}` name no class at all while saying exactly
    /// what they hold. Walking over class names would have to end the chain there -- which is what
    /// it used to do, and why `$items[0]->` offered nothing on a corpus where it is written
    /// everywhere. So the walk carries [`Held`] and reduces to names only at the end.
    pub fn chain_classes(
        &self,
        offset: TextSize,
        root: ChainRoot<'_>,
        steps: &[ChainStep<'_>],
    ) -> ReceiverClasses {
        let held = self.chain_held(offset, root, steps);
        let Some(hir) = self.current().map(|open| open.hir()) else {
            return ReceiverClasses::All(Vec::new());
        };

        // A union first, because it is the answer the others would quietly replace with a worse
        // one: `A|B` where both extend `Base` has a common ancestor, and reducing to it hides every
        // method the two declare themselves.
        if let [one] = held.as_slice()
            && let Some(operands) = self.union_operands(offset, hir, one)
        {
            // The operands that name a class, rather than all of them or none. An operand naming
            // none -- `mixed`, `string`, `array` -- says nothing about what the call reaches, and
            // requiring every one of them to name a class is what left `@return ParameterBag|mixed`
            // on Laravel's `Request::json()` offering an empty list while goto on the very name
            // written after the `->` resolved. That it was not all of them is carried rather than
            // dropped, so the list can say so: see `ReceiverClasses::Any`.
            let named = operands
                .iter()
                .filter_map(|operand| operand.class.clone())
                .collect::<Vec<_>>();
            if !named.is_empty() {
                return ReceiverClasses::Any {
                    every_operand_named: named.len() == operands.len(),
                    classes: named,
                };
            }
        }

        let mut classes = Vec::new();
        for one in &held {
            let named = match self.class_of_held(offset, hir, one) {
                Some(class) => vec![class],
                None => self.intersection_of_held(offset, hir, one),
            };
            for class in named {
                if !classes.contains(&class) {
                    classes.push(class);
                }
            }
        }
        ReceiverClasses::All(classes)
    }

    /// The same walk, stopping at what the chain holds rather than at the classes it names.
    ///
    /// A caller that asks about the *container* needs this: the keys of `array{app: Application}`
    /// are a fact about the shape, and reducing it to a class name first would throw the shape away
    /// -- it names no class at all.
    /// The class a value before `::` names, when it names one rather than being one.
    ///
    /// `$class::make()` on `$class = Foo::class`, or on a property declared `class-string<Foo>`, is
    /// a static call on `Foo` -- and the same chain read the way `->` reads it is a string, which has
    /// no members at all. So the list was empty where goto, which reads the call as a class target,
    /// answered. Of Laravel's 739 static accesses on a value, 503 completed the name written there
    /// before this and 512 after, none fewer; most of the rest are values nothing states the class
    /// of. `None` for anything else, an instance included, whose `$obj::CONST` reads the class it
    /// has as [`Self::chain_classes`] already answers.
    pub fn class_a_chain_names(
        &self,
        offset: TextSize,
        root: ChainRoot<'_>,
        steps: &[ChainStep<'_>],
    ) -> Option<Name> {
        // Only a value can name a class this way: `self::`, `static::` and `Foo::` already are one.
        if !matches!(root, ChainRoot::Local(_) | ChainRoot::Function { .. }) && steps.is_empty() {
            return None;
        }
        let hir = self.current()?.hir();
        self.written_path(offset, root, steps)
            .and_then(|path| {
                self.class_a_path_names(offset, hir, &path, crate::resolve::HIERARCHY_DEPTH)
            })
            .or_else(|| {
                self.chain_held(offset, root, steps)
                    .iter()
                    .find_map(|held| self.class_named_by_held_class_string(offset, hir, held))
            })
    }

    pub(crate) fn chain_held(
        &self,
        offset: TextSize,
        root: ChainRoot<'_>,
        steps: &[ChainStep<'_>],
    ) -> Vec<Held> {
        let Some(hir) = self.current().map(|open| open.hir()) else {
            return Vec::new();
        };
        // The receiver as the path the body layer would name it by, when every piece of it has
        // already been written somewhere in the file -- see `written_path`. Built once and used
        // for both checks below: a guard replacing the declaration outright, and a contract
        // refining whatever the walk below settles on.
        let subject = (!steps.is_empty())
            .then(|| self.written_path(offset, root, steps))
            .flatten();

        // What the body settled about the receiver replaces what its declaration said. Asked here
        // because this walk starts from characters rather than from an expression: `if
        // ($this->timeline instanceof Trip) { $this->timeline-> }` is the shape, and the receiver
        // being narrowed is a property rather than a local -- which `chain_root` already folds in.
        if let Some(subject) = &subject
            && let Some(settled) = phrost_body::slot_value(
                hir,
                offset,
                subject,
                &|hir, expr| self.call_never_returns(offset, hir, expr),
                &self.asserting(),
                &self.assigned_properties(hir),
            )
            && let Some(held) = self.resolve_path(
                settled.written_at.unwrap_or(offset),
                hir,
                &settled.value,
                FOLLOW_LIMIT,
            )
        {
            return vec![held];
        }

        let held = self.walked_chain(offset, hir, root, steps);

        // An `@assert` contract on a call earlier in the function -- the same claim
        // `held_after_steps` reads for a real expression, asked here through the one function that
        // answers it rather than a second copy of it (`IndexView::contract_narrowed`). Not folded
        // into the walk above: that walk carries the arguments a call was scanned with, and a path
        // built from characters -- `written_path` -- cannot, so it must only refine a single vague
        // result rather than replace how the result itself was reached.
        if let [one] = held.as_slice()
            && let Some(subject) = &subject
        {
            return vec![self.contract_narrowed(offset, hir, subject, FOLLOW_LIMIT, one.clone())];
        }

        held
    }

    /// The chain walked a step at a time, from what its root holds.
    fn walked_chain(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        root: ChainRoot<'_>,
        steps: &[ChainStep<'_>],
    ) -> Vec<Held> {
        let Some(start) = self.chain_root(offset, root) else {
            return Vec::new();
        };

        let mut held = vec![start];
        for (at, step) in steps.iter().enumerate() {
            let mut next = Vec::new();
            for current in &held {
                match step {
                    ChainStep::Member {
                        name,
                        kind,
                        arguments,
                    } => {
                        // Every operand of an intersection -- `&` means the member exists if any
                        // of them has it -- and otherwise whichever class owns the name, by the
                        // ladder goto climbs. That ladder had two rungs here, a single class and an
                        // intersection's parts, so a union whose operands are intersections --
                        // `PriceList&Mock|Product&Mock`, what an `if` choosing which class to mock
                        // leaves -- ended the chain at its first call while goto went on.
                        let classes = match self.intersection_of_held(offset, hir, current) {
                            parts if !parts.is_empty() => parts,
                            _ => self
                                .owner_of_member(offset, hir, current, *kind, name)
                                .into_iter()
                                .collect(),
                        };
                        let supplied = self.arguments_of_held(hir, current);
                        // The same facts the path layer builds for a real call expression, with the
                        // one thing text can still say about a call it cannot parse: how many
                        // arguments were written. See `CallFacts::written_arity`.
                        let facts = crate::symbols::CallFacts {
                            offset,
                            hir,
                            literals: &[],
                            call: phrost_body::CallSource::unread(),
                            budget: FOLLOW_LIMIT,
                            receiver: &supplied,
                            // Only for a call: the text gives a property read and a call with no
                            // arguments the same empty slice, and only the kind tells them apart.
                            written: (*kind == MemberKind::Method).then_some(*arguments),
                        };
                        next.extend(classes.iter().filter_map(|class| {
                            // What the call was written with, before what the declaration says: for
                            // some methods the argument *is* the type, and a declaration that
                            // returns `object` cannot say which one.
                            self.returned_for_written(
                                offset,
                                class.as_str(),
                                *kind,
                                name,
                                arguments,
                            )
                            // Then the method's own contract, which is how the path layer reads a
                            // conditional `@return` -- and reading it here is what makes the two
                            // agree. Without it `@return ($key is null ? InputBag : mixed)` fell
                            // through to the declaration, which such a method does not have, and
                            // then to its body: Laravel's `Request::json()` returns a property, so
                            // `$request->json()->` offered nothing while goto answered.
                            .or_else(|| {
                                (*kind == MemberKind::Method)
                                    .then(|| {
                                        self.returned_by_method_contract(
                                            class.as_str(),
                                            name,
                                            None,
                                            &facts,
                                        )
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                self.member_held_on(class.as_str(), *kind, name, None, &supplied)
                            })
                        }));
                    }
                    ChainStep::Constant(name) => {
                        let classes = match self.class_of_held(offset, hir, current) {
                            Some(one) => vec![one],
                            None => self.intersection_of_held(offset, hir, current),
                        };
                        next.extend(classes.iter().filter_map(|class| {
                            self.member_held_on(
                                class.as_str(),
                                MemberKind::ClassConstant,
                                name,
                                None,
                                &[],
                            )
                            .or_else(|| {
                                self.member_held_on(
                                    class.as_str(),
                                    MemberKind::EnumCase,
                                    name,
                                    None,
                                    &[],
                                )
                            })
                        }));
                    }
                    // PHP's own rule for calling an object. Written here as well as in the path
                    // layer because a receiver under the cursor has no expression to read.
                    //
                    // The same split as `held_after_steps`: a value whose type already said what
                    // invoking it gives back has no class to look `__invoke` up on. Without it
                    // `$make()->` offers nothing while `$q = $make(); $q->` offers everything --
                    // one shape answering two ways is the failure both copies of this step exist
                    // to avoid. All three spellings of that claim go through `invoked_return`.
                    ChainStep::Invoked => match self.invoked_return(hir, current) {
                        Some(returns) => next.push(returns),
                        None => {
                            let receiver = current;
                            let classes = match self.class_of_held(offset, hir, receiver) {
                                Some(one) => vec![one],
                                None => self.intersection_of_held(offset, hir, receiver),
                            };
                            let supplied = self.arguments_of_held(hir, receiver);
                            next.extend(classes.iter().filter_map(|class| {
                                self.member_held_on(
                                    class.as_str(),
                                    MemberKind::Method,
                                    "__invoke",
                                    None,
                                    &supplied,
                                )
                            }));
                        }
                    },
                    ChainStep::Element(key) => {
                        match self.indexed_of_held(
                            offset,
                            hir,
                            current,
                            key.and_then(ChainKey::literal),
                            // A chain is what the cursor stands after, and `foreach` is no step of
                            // one: every element step here was written as a subscript.
                            crate::resolve::Subscript::Written,
                        ) {
                            Some(indexed) => next.push(indexed),
                            // The slot says nothing, and an array built one slot at a time is
                            // described by what was put in it rather than by the `[]` it was
                            // assigned. Asked per step, because a container is often a slot itself:
                            // `$sums[$id] = $this->shape()` and then `$sums[$id]['paid']`.
                            //
                            // The key travels as text -- the scanner read it out of the file, and a
                            // key nothing in the project declares has no interned name to look up.
                            None => {
                                next.extend(self.written_into(offset, hir, root, &steps[..=at]))
                            }
                        }
                    }
                }
                if next.len() >= BREADTH {
                    break;
                }
            }
            if next.is_empty() {
                return Vec::new();
            }
            held = next;
        }

        held
    }

    /// What a method gives back for the arguments the text was written with.
    ///
    /// The contracts an argument can settle without resolving anything: a framework catalog, where an
    /// id names the class, and a `class-string<T>` parameter, where the argument names it outright --
    /// either as the whole result or as what the returned container holds. Both are answered from a
    /// string, which is all a chain has: the statement being typed does not parse, so there is no
    /// expression behind the call.
    ///
    /// And one that an *absent* argument settles, which is why the kind is asked for: the text gives
    /// a call with no arguments and a property read the same empty slice, and only the kind tells
    /// them apart.
    /// What a free function written at a chain's root hands back.
    ///
    /// Two answers, in the order a reader wants them. A contract keyed on an *argument* first --
    /// `reset($rows)` gives back a `Row` only because `$rows` holds them, and its declared return is
    /// `mixed`. Then the declaration's own return type, which is the whole of it for a project's own
    /// `make_thing(): Row`.
    ///
    /// Text and not a call expression, which is what this scanner has: the statement being completed
    /// does not parse, so there is no call node to hand `returned_by_function_contract`. The same
    /// bargain [`Self::returned_for_written`] makes for a method one step along, and the same
    /// discipline -- a written argument this cannot read leaves the declaration standing.
    fn returned_by_written_call(
        &self,
        offset: TextSize,
        name: &str,
        arguments: &[Box<str>],
    ) -> Option<Held> {
        // The path *and* the anchor, which is what `NameAnchor::of` hands back as a pair. Taking
        // only the anchor and passing the written name left `namespace\here()->` resolving a
        // function called `namespace\here` -- nothing -- while goto on the very name after the
        // `->` answered. The class half of this question (`written_class`) has taken both since
        // the `\Drupal::` note; this half had not.
        let (anchor, path) = phrost_hir::NameAnchor::of(name);
        let location = self.resolve_name(offset, path, anchor, phrost_hir::ImportKind::Function)?;
        let fqn = location.fqn.as_str();
        if let Some(held) = self.returned_from_function_text(fqn, arguments) {
            return Some(held);
        }
        if let Some(held) = self.returned_from_written_argument(offset, fqn, arguments) {
            return Some(held);
        }
        if let Some(held) = self.literal_case_for_text(offset, fqn, arguments) {
            return Some(held);
        }
        if let Some(held) = self.returned_from_written_call_contract(offset, fqn, arguments) {
            return Some(held);
        }
        Some(Held::Stored {
            ty: self.function_return(fqn)?,
            // A free function has no enclosing class, so `self` and `static` cannot appear in what
            // it returns.
            scope: None,
        })
    }

    /// The one branch a written argument can settle: what that argument holds, or what it holds
    /// *inside*.
    ///
    /// `reset`, `end`, `current` and `array_pop` all say "an element of the array you gave me", and
    /// every one of them is written with a plain `$local` at a chain's root. Resolving that local
    /// and taking its element is what the call expression would have given -- so the answer is the
    /// same one a completed statement gets, reached from the characters instead.
    ///
    /// Only a plain local. An argument written as anything else is a miss, which leaves the
    /// declaration's own return standing.
    fn returned_from_written_argument(
        &self,
        offset: TextSize,
        fqn: &str,
        arguments: &[Box<str>],
    ) -> Option<Held> {
        let hir = self.current()?.hir();
        let branches: &[phrost_sema::ReturnBranch] = match self.function_contract(fqn)? {
            phrost_sema::Contract::AgreedReturn(branch) => std::slice::from_ref(branch),
            phrost_sema::Contract::ConditionalReturn {
                then, otherwise, ..
            } => {
                // Whichever of the two names an argument: the test needs a call this cannot build,
                // and both branches of the shapes that reach here name the same argument.
                return [then, otherwise].into_iter().find_map(|branch| {
                    self.held_of_written_branch(offset, hir, branch.clone(), arguments)
                });
            }
            phrost_sema::Contract::ReturnsOneOf(branches) => branches,
            _ => return None,
        };
        branches
            .iter()
            .find_map(|branch| self.held_of_written_branch(offset, hir, branch.clone(), arguments))
    }

    /// One branch, against the argument text the scanner read.
    fn held_of_written_branch(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        branch: phrost_sema::ReturnBranch,
        arguments: &[Box<str>],
    ) -> Option<Held> {
        let written = |position: u16| {
            let text = arguments.get(usize::from(position))?;
            let local = phrost_body::local_name(hir, offset, text.strip_prefix('$')?)?;
            let path = phrost_body::ValuePath {
                root: phrost_body::PathRoot::Local(local),
                steps: Vec::new(),
                may_be_absent: false,
            };
            self.resolve_path(offset, hir, &path, FOLLOW_LIMIT)
        };
        match branch {
            phrost_sema::ReturnBranch::ArgumentType { parameter, .. } => written(parameter),
            phrost_sema::ReturnBranch::ArgumentElement { parameter, absent } => {
                let element = self.element_of_held(offset, hir, &written(parameter)?)?;
                Some(match absent {
                    Some(phrost_model::Absence::Null) => Self::nullable(element),
                    // A `false` the declaration admits is not a null, and this layer has one shape
                    // for "or nothing" -- so the honest answer is the element as it stands, which is
                    // what a member list is asked about anyway.
                    _ => element,
                })
            }
            phrost_sema::ReturnBranch::ArgumentElements { parameter } => {
                let element = self.element_of_held(offset, hir, &written(parameter)?)?;
                Some(Held::ListOf(Box::new(element)))
            }
            phrost_sema::ReturnBranch::ArgumentTypes { parameter } => {
                Some(Held::ListOf(Box::new(written(parameter)?)))
            }
            _ => None,
        }
    }

    fn returned_for_written(
        &self,
        offset: TextSize,
        class_fqn: &str,
        kind: MemberKind,
        method: &str,
        arguments: &[Option<Box<str>>],
    ) -> Option<Held> {
        let written = |position: usize| arguments.get(position)?.as_deref();
        self.returned_from_argument(class_fqn, method, written(0))
            .or_else(|| self.returned_by_key(class_fqn, method))
            .or_else(|| self.returned_named_after(class_fqn, method))
            .or_else(|| {
                self.returned_by_default(class_fqn, method, || {
                    (kind == MemberKind::Method)
                        .then(|| u16::try_from(arguments.len()).ok())
                        .flatten()
                })
            })
            .or_else(|| self.returned_class_written(offset, class_fqn, method, arguments))
    }

    /// What was written into the slot a chain's prefix ends at.
    ///
    /// The prefix rather than the whole chain, because this is asked while walking: the step that
    /// found nothing is the last one of it, and everything before names the container.
    fn written_into(
        &self,
        offset: TextSize,
        hir: &phrost_hir::FileHir,
        root: ChainRoot<'_>,
        upto: &[ChainStep<'_>],
    ) -> Option<Held> {
        let [before @ .., ChainStep::Element(key)] = upto else {
            return None;
        };
        let container = self.written_path(offset, root, before)?;
        let written =
            phrost_body::element_written(hir, offset, &container, key.and_then(ChainKey::literal))?;
        self.resolve_path(written.at, hir, &written.value, FOLLOW_LIMIT)
    }

    /// A written receiver as the path the body layer names it by, when it can be named.
    ///
    /// Only the roots a path can spell: a class written by name reaches the index through a
    /// reference this layer cannot build, and nothing narrows a static receiver anyway. Every name
    /// is looked up rather than interned -- a step naming something the project never declared
    /// cannot match a stored path, so failing there is the answer and not a shortcut.
    fn written_path(
        &self,
        offset: TextSize,
        root: ChainRoot<'_>,
        steps: &[ChainStep<'_>],
    ) -> Option<ValuePath> {
        let names = self.skeleton.interner();
        let root = match root {
            ChainRoot::This => PathRoot::This,
            ChainRoot::SelfClass => PathRoot::SelfClass,
            ChainRoot::Parent => PathRoot::Parent,
            ChainRoot::Local(name) => PathRoot::Local(phrost_body::local_name(
                self.current()?.hir(),
                offset,
                name,
            )?),
            // A guard is written about a *value*, and neither of these is one this layer can name:
            // a class is not a value, and a call's result has no slot to have been narrowed.
            ChainRoot::Class(_) | ChainRoot::Function { .. } => return None,
        };

        let steps = steps
            .iter()
            .map(|step| {
                Some(match step {
                    ChainStep::Member {
                        name,
                        kind: MemberKind::Method,
                        ..
                    } => PathStep::MethodReturn {
                        name: names.lookup_name(name)?,
                        // The scanner steps over an argument list whole, so what was passed is not
                        // part of what it read. A guard written about a call *with* a literal
                        // argument therefore does not match, which loses a narrowing rather than
                        // inventing one.
                        arguments: Box::default(),
                        // And for the same reason the list itself is empty: this scanner reads
                        // characters, so there is no call expression to point at.
                        call: phrost_body::CallSource::unread(),
                    },
                    ChainStep::Member { name, .. } => PathStep::Property(names.lookup_name(name)?),
                    // The scanner saw no call expression, for the reason the arguments above are
                    // empty: it reads characters.
                    ChainStep::Invoked => PathStep::Invoked {
                        call: phrost_body::CallSource::unread(),
                    },
                    ChainStep::Constant(name) => PathStep::ClassConstant(names.lookup_name(name)?),
                    // Through the file rather than the project's names, because an array key is
                    // *data*: `$constants['C']` says nothing about the project having declared a
                    // `C`, and a key lives only inside a body -- which a closed file never
                    // lowers. See `phrost_body::element_key_name` for what looking it up in the
                    // project's interner cost.
                    ChainStep::Element(key) => PathStep::Element(match key {
                        Some(ChainKey::Literal(key)) => Some(ElementKey::Literal(
                            phrost_body::element_key_name(self.current()?.hir(), key)?,
                        )),
                        // The local the index was written with, looked up the way a root local is
                        // -- it is a name in this body and not data, so it is the project's
                        // interner that holds it.
                        Some(ChainKey::Variable(name)) => Some(ElementKey::Variable(
                            phrost_body::local_name(self.current()?.hir(), offset, name)?,
                        )),
                        None => None,
                    }),
                })
            })
            .collect::<Option<Vec<_>>>()?;

        Some(ValuePath {
            root,
            steps,
            may_be_absent: false,
        })
    }

    /// What the chain starts from, as a type rather than a class.
    ///
    /// A local is the one root that can hold something no class name describes -- a parameter is
    /// where `array{...}`, `list<T>` and `A&B` are all written most -- so it is resolved to what it
    /// holds. The others name a class by construction.
    fn chain_root(&self, offset: TextSize, root: ChainRoot<'_>) -> Option<Held> {
        match root {
            // The same methods the body's own path layer calls for `PathRoot::This` and
            // `PathRoot::SelfClass`. This scanner exists because a receiver with nothing typed
            // after it has no expression in the body to look up -- not because the answer differs.
            ChainRoot::This => self.this_held(offset),
            ChainRoot::SelfClass => self.enclosing_class(offset).map(Held::Class),
            ChainRoot::Parent => self.parent_class(offset).map(Held::Class),
            // Through the path layer rather than straight to the local, so that everything
            // `resolve_path` applies to a value applies here too. An `@assert` tag is written about
            // what a call was *passed*, so it is matched against the argument as written -- and a
            // receiver resolved straight from the local never presents one.
            ChainRoot::Local(name) => {
                let hir = self.current()?.hir();
                let path = ValuePath {
                    root: PathRoot::Local(phrost_body::local_name(hir, offset, name)?),
                    steps: Vec::new(),
                    may_be_absent: false,
                };
                self.resolve_path(offset, hir, &path, FOLLOW_LIMIT)
            }
            ChainRoot::Class(name) => self.written_class(offset, name).map(Held::Class),
            // A function's result, which is a name in a different table from every root above it.
            // Its declared return is the whole answer for a project's own `make_thing(): Row`; for
            // `reset($rows)` it is `mixed`, and what makes that one useful is the argument -- see
            // `returned_for_written_arguments`.
            ChainRoot::Function { name, arguments } => {
                self.returned_by_written_call(offset, name, arguments)
            }
        }
    }
}
