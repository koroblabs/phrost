//! The type a body's `return` statements agree on, for a declaration that names none.
//!
//! `public function create() { return new Bar(); }` says what it hands back as plainly as a hint
//! would, and until now nothing read it: the method's return was `None`, so every value it produced
//! was untyped and every member reached through one was unresolvable. On the reference project one
//! such method -- `ProductCoordinator::getCoordinator()` -- costs twenty-four unresolved accesses in
//! a single file, and 275 of its own methods declare no return at all.
//!
//! # Why it is here rather than in the layer that resolves types
//!
//! The body belongs to whichever file declares the method, and phrost lowers a body only for an
//! open buffer. What it *does* do at ingest is lower the bodies of project code, keep the
//! declarations they held and drop the rest -- so this is a fact derived where the body is still in
//! hand and stored with the declaration, in the shape §8 records for `is_conditional`.
//!
//! # All or nothing
//!
//! A body whose returns this cannot read settles nothing, rather than settling from the ones it
//! can: `return new Bar();` beside `return $this->make();` is a union of two things and reading
//! only the first would state a claim narrower than the code. So a single unreadable return gives
//! the whole declaration back to whatever it said before, which is nothing.
//!
//! # Two answers, because a type and a path are different things
//!
//! [`Self::returned_from_body`] answers with a *type*, for the shapes that name one outright --
//! `new Bar()`, `$this`, `null`, `[new Row()]`. Everything else names a type only after
//! resolution, which is a whole index away from here, and that used to be the end of it.
//!
//! [`Self::returned_path_from_body`] is the other half: `return $this->getDatabase()->select(...)`
//! names no type and describes one exactly -- a root and a list of member names. That *description*
//! is small enough for a closed file to keep, which is the whole point: Drupal core has 4 181
//! methods declaring neither a return type nor a `@return`, and 1 235 of them are this shape. A
//! reader following `$this->select('t')->fields(...)` in a migrate source got nothing at the second
//! hop, because the first is one of those wrappers.

use mago_syntax::cst::{
    Block, Expression, ForBody, ForeachBody, FunctionLikeParameterList, IfBody, Statement,
    SwitchBody, WhileBody,
};
use phrost_hir::{ReturnRootData, ReturnedKeyData, ReturnedPathData, TypeRefData, TypeRefId};
use phrost_model::BuiltinTy;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// What the `return` statements of `block` agree on, or `None` when they say nothing this reads.
    pub(crate) fn returned_from_body(
        &mut self,
        block: &Block<'p>,
        parameters: &FunctionLikeParameterList<'p>,
    ) -> Option<TypeRefId> {
        let mut returned = Vec::new();
        collect_returns(block.statements.as_slice(), &mut returned);
        if returned.is_empty() {
            return None;
        }

        let mut operands: Vec<TypeRefId> = Vec::with_capacity(returned.len());
        for value in returned {
            let id = self.returned_type_id(block, parameters, value?)?;
            if !operands
                .iter()
                .any(|seen| self.same_written_type(*seen, id))
            {
                operands.push(id);
            }
        }

        match operands.len() {
            0 => None,
            1 => Some(operands[0]),
            // Two branches handing back two classes is a union, which is what the code said.
            _ => {
                let parts = self.builder.alloc_type_ref_list(&operands);
                Some(self.builder.add_type_ref(TypeRefData::Union(parts)))
            }
        }
    }

    /// The type one returned expression names, for the shapes that name one outright.
    ///
    /// Deliberately short. A factory returns `new Thing()` and a nullable one returns `null` beside
    /// it; everything else -- a call, a variable, a property -- names a type only after resolution,
    /// which is a whole index away from here. The list is closed on purpose rather than
    /// approximated, for the reason the module states.
    pub(crate) fn returned_type(&mut self, value: &Expression<'p>) -> Option<TypeRefData> {
        match value {
            Expression::Instantiation(new) => match new.class {
                Expression::Identifier(identifier) => {
                    Some(TypeRefData::Named(self.name_ref(identifier)))
                }
                // `new static()` and `new self()`, which are what a fluent factory writes.
                Expression::Static(_) => {
                    Some(TypeRefData::Relative(phrost_model::RelativeTy::Static))
                }
                Expression::Self_(_) => {
                    Some(TypeRefData::Relative(phrost_model::RelativeTy::SelfTy))
                }
                _ => None,
            },
            // `return $this;`, which is every fluent setter in the language.
            Expression::Variable(mago_syntax::cst::Variable::Direct(variable))
                if variable.name == b"$this" =>
            {
                Some(TypeRefData::Relative(phrost_model::RelativeTy::Static))
            }
            Expression::Literal(mago_syntax::cst::Literal::Null(_)) => {
                Some(TypeRefData::Builtin(BuiltinTy::Null))
            }
            // `return [new Row()];` -- a list of one thing, which is what makes the `foreach` over
            // the result resolve. Every element has to be the same `new`, because an array whose
            // elements disagree holds a union this does not build.
            Expression::Array(array) => {
                let element =
                    self.same_element(array.elements.iter().map(|element| match element {
                        mago_syntax::cst::ArrayElement::Value(written) => Some(written.value),
                        _ => None,
                    }))?;
                let base = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::Array));
                let key = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::Int));
                let args = self.builder.alloc_type_ref_list(&[key, element]);
                Some(TypeRefData::Generic {
                    base,
                    args,
                    non_empty: false,
                })
            }
            _ => None,
        }
    }

    /// The one type every element of an array literal names, for a literal that is not empty.
    fn same_element(
        &mut self,
        elements: impl Iterator<Item = Option<&'p Expression<'p>>>,
    ) -> Option<TypeRefId> {
        let mut found: Option<TypeRefId> = None;
        let mut any = false;
        for element in elements {
            any = true;
            let data = self.returned_type(element?)?;
            let id = self.builder.add_type_ref(data);
            match found {
                Some(seen) if !self.same_written_type(seen, id) => return None,
                _ => found = Some(id),
            }
        }
        any.then_some(found?)
    }

    /// Whether two of [`Self::returned_type`]'s answers are the same claim.
    ///
    /// Comparing the ids is not enough and reads as though it is: every `new Row()` allocates a
    /// type reference and a name reference of its own, so `return [new Row(), new Row()];` compares
    /// two ids that differ and settles nothing, for a literal whose elements plainly agree. What
    /// has to be compared is what was *written* -- for a class, its path, case-insensitively, the
    /// way PHP looks one up.
    ///
    /// The same comparison [`phrost_hir::refined_type_ref`] makes and deliberately not shared with
    /// it: that one reads a finished file and this one reads a builder mid-flight.
    pub(crate) fn same_written_type(&self, left: TypeRefId, right: TypeRefId) -> bool {
        match (self.builder.type_ref(left), self.builder.type_ref(right)) {
            (TypeRefData::Named(left), TypeRefData::Named(right)) => self
                .builder
                .name_ref(*left)
                .path
                .as_str()
                .eq_ignore_ascii_case(self.builder.name_ref(*right).path.as_str()),
            (TypeRefData::Builtin(left), TypeRefData::Builtin(right)) => left == right,
            (TypeRefData::Relative(left), TypeRefData::Relative(right)) => left == right,
            (
                TypeRefData::Generic {
                    base: left,
                    args: left_args,
                    ..
                },
                TypeRefData::Generic {
                    base: right,
                    args: right_args,
                    ..
                },
            ) => {
                let (left, right) = (*left, *right);
                let left_args = self.builder.type_ref_list(*left_args).to_vec();
                let right_args = self.builder.type_ref_list(*right_args).to_vec();
                left_args.len() == right_args.len()
                    && self.same_written_type(left, right)
                    && std::iter::zip(left_args, right_args)
                        .all(|(left, right)| self.same_written_type(left, right))
            }
            // The four above are everything [`Self::returned_type`] builds. A pair of different
            // shapes is two different claims, which is the answer this hands back for them.
            _ => false,
        }
    }
}

/// Every `return` that belongs to *this* body, as the expression it hands back.
///
/// `None` for a `return;`, which hands back nothing and is what makes a body's returns disagree
/// with each other -- the caller reads that as "settles nothing".
///
/// The walk goes into every construct a statement can nest another in and into none that opens a
/// function-like of its own: a closure's `return` is the closure's, and reading it as the enclosing
/// method's is a claim about a value that method never produces. Nine places, which is the list
/// §8 records for a nested declaration -- the same walk, for the other reason to make it.
fn collect_returns<'p>(statements: &[Statement<'p>], out: &mut Vec<Option<&'p Expression<'p>>>) {
    for statement in statements {
        match statement {
            Statement::Return(node) => out.push(node.value),
            Statement::Block(node) => collect_returns(node.statements.as_slice(), out),
            Statement::If(node) => match &node.body {
                IfBody::Statement(body) => {
                    collect_returns(std::slice::from_ref(body.statement), out);
                    for clause in body.else_if_clauses.iter() {
                        collect_returns(std::slice::from_ref(clause.statement), out);
                    }
                    if let Some(clause) = &body.else_clause {
                        collect_returns(std::slice::from_ref(clause.statement), out);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    collect_returns(body.statements.as_slice(), out);
                    for clause in body.else_if_clauses.iter() {
                        collect_returns(clause.statements.as_slice(), out);
                    }
                    if let Some(clause) = &body.else_clause {
                        collect_returns(clause.statements.as_slice(), out);
                    }
                }
            },
            Statement::Switch(node) => match &node.body {
                SwitchBody::BraceDelimited(body) => {
                    for case in body.cases.iter() {
                        collect_returns(case.statements(), out);
                    }
                }
                SwitchBody::ColonDelimited(body) => {
                    for case in body.cases.iter() {
                        collect_returns(case.statements(), out);
                    }
                }
            },
            Statement::Try(node) => {
                collect_returns(node.block.statements.as_slice(), out);
                for clause in node.catch_clauses.iter() {
                    collect_returns(clause.block.statements.as_slice(), out);
                }
                if let Some(clause) = &node.finally_clause {
                    collect_returns(clause.block.statements.as_slice(), out);
                }
            }
            Statement::While(node) => match &node.body {
                WhileBody::Statement(body) => collect_returns(std::slice::from_ref(body), out),
                WhileBody::ColonDelimited(body) => collect_returns(body.statements.as_slice(), out),
            },
            Statement::DoWhile(node) => collect_returns(std::slice::from_ref(node.statement), out),
            Statement::For(node) => match &node.body {
                ForBody::Statement(body) => collect_returns(std::slice::from_ref(body), out),
                ForBody::ColonDelimited(body) => collect_returns(body.statements.as_slice(), out),
            },
            Statement::Foreach(node) => match &node.body {
                ForeachBody::Statement(body) => collect_returns(std::slice::from_ref(body), out),
                ForeachBody::ColonDelimited(body) => {
                    collect_returns(body.statements.as_slice(), out);
                }
            },
            _ => {}
        }
    }
}

impl<'p> Lowerer<'_, 'p> {
    /// The member chain a body hands back, for a declaration that names no type and whose returned
    /// expression names none either.
    ///
    /// A root and the member names applied to it, in order: `$this->getDatabase()->select($t)` is
    /// the receiver and `[getDatabase, select]`. Names only -- no arguments, no ranges, nothing
    /// bound to a tree -- because what this is for is being *stored*, and the file it came from is
    /// closed by the time anybody asks.
    ///
    /// Only when [`Self::returned_from_body`] found nothing, and only for a body whose returns all
    /// agree, for the reason the module states. A single `return` is what nearly every one of these
    /// is: a wrapper exists to forward.
    ///
    /// A returned *variable* is read through the one assignment that gave it a value, by the rules
    /// `returned_local` states for a type: `$connection = Mockery::mock(Connection::class); ...;
    /// return $connection;` is how a test helper builds a fixture, and it describes the chain
    /// `Mockery::mock` as surely as returning the call would. Laravel's own tests write that shape
    /// for every connection, session and filesystem they fake, and each untyped helper left every
    /// `->expects()` downstream of it unresolved.
    /// Whether the body's one `return` is a call on `$this` -- `return $this->hasMany(...)`.
    ///
    /// Read off the syntax and nothing else, because [`Self::returned_path_from_body`] records a
    /// name reference for a root it resolves, and a caller that asks only to throw most answers
    /// away would record each of those twice: a `new Relay()` returned beside a `Relay` hint was
    /// reported undefined twice at one position.
    ///
    /// A chain of calls counts, down to the `$this` it starts from: `return
    /// $this->allRevisions()->where(...)->orderBy(...);` is the relation `allRevisions()` builds,
    /// narrowed, and only the walk from `$this` can say which model it holds.
    pub(crate) fn returns_a_call_on_this(block: &Block<'p>) -> bool {
        let mut returned = Vec::new();
        collect_returns(block.statements.as_slice(), &mut returned);
        let [Some(Expression::Call(mago_syntax::cst::Call::Method(call)))] = returned.as_slice()
        else {
            return false;
        };
        let mut object = call.object;
        loop {
            match object {
                Expression::Call(mago_syntax::cst::Call::Method(inner)) => object = inner.object,
                Expression::Variable(mago_syntax::cst::Variable::Direct(variable)) => {
                    return variable.name == b"$this";
                }
                _ => return false,
            }
        }
    }

    /// Whether the body's one `return` is a chain of calls built from a class rather than from
    /// `$this`: `return User::query()->with('roles')->paginate($count);`, or the same through a local
    /// the body gave that start once -- `$query = User::query()->...; return $query->paginate()`.
    ///
    /// Syntax only, for the reason [`Self::returns_a_call_on_this`] gives. A chain through a
    /// property is the getter shape and does not count: `return $this->repository->all();` says
    /// what a collaborator returns, which its own declaration already says.
    pub(crate) fn returns_a_built_chain(
        &self,
        block: &Block<'p>,
        parameters: &FunctionLikeParameterList<'p>,
    ) -> bool {
        let mut returned = Vec::new();
        collect_returns(block.statements.as_slice(), &mut returned);
        let [Some(value)] = returned.as_slice() else {
            return false;
        };
        let mut value: &Expression<'p> = value;
        let (mut calls, mut hops) = (0, LOCAL_HOPS);
        loop {
            match value {
                Expression::Call(mago_syntax::cst::Call::Method(call)) => {
                    calls += 1;
                    value = call.object;
                }
                Expression::Call(mago_syntax::cst::Call::StaticMethod(call)) => {
                    return calls < CHAIN_LIMIT
                        && matches!(
                            call.class,
                            Expression::Identifier(_)
                                | Expression::Static(_)
                                | Expression::Self_(_)
                        );
                }
                Expression::Variable(mago_syntax::cst::Variable::Direct(variable))
                    if variable.name != b"$this" =>
                {
                    let (Some(left), Some(assigned)) = (
                        hops.checked_sub(1),
                        self.the_one_value_of(block, parameters, value),
                    ) else {
                        return false;
                    };
                    hops = left;
                    value = assigned;
                }
                _ => return false,
            }
            if calls >= CHAIN_LIMIT {
                return false;
            }
        }
    }

    pub(crate) fn returned_path_from_body(
        &mut self,
        block: &Block<'p>,
        parameters: &FunctionLikeParameterList<'p>,
    ) -> Option<ReturnedPathData> {
        let mut returned = Vec::new();
        collect_returns(block.statements.as_slice(), &mut returned);
        // One returned expression. Two that disagree describe a union this cannot express, and two
        // that agree are not worth the walk to find out.
        let [value] = returned.as_slice() else {
            return None;
        };
        let value = (*value)?;
        let value = self
            .the_one_value_of(block, parameters, value)
            .unwrap_or(value);
        let mut steps = Vec::new();
        // Overwritten as the walk climbs, so what survives is the *root-most* call's -- which after
        // the reverse below is the first step's. See `ReturnedPathData::key`.
        let mut key = None;
        let root = self.returned_root(
            Locals { block, parameters },
            value,
            &mut steps,
            &mut key,
            LOCAL_HOPS,
        )?;
        steps.reverse();
        Some(ReturnedPathData {
            root,
            steps: steps.into_boxed_slice(),
            key,
        })
    }

    /// Walks a chain from its tail back to its root, pushing each member name as it goes.
    ///
    /// Bounded, because a chain long enough to matter is not one anybody wrote: the longest in
    /// Drupal core is three.
    ///
    /// A local in the chain is read through the one value this body gave it, so `$document = new
    /// DOMDocument(); return $document->createElement($tag);` roots at the `new` -- the same reading
    /// [`Lowerer::the_one_value_of`] makes for a returned local, one link further in. Symfony's
    /// DomCrawler tests build every field through a helper of that shape, and each one came back
    /// untyped. `hops` bounds the reading separately from the steps, because `$a = $a;` adds none.
    fn returned_root(
        &mut self,
        locals: Locals<'_, 'p>,
        value: &Expression<'p>,
        steps: &mut Vec<phrost_model::Name>,
        key: &mut Option<ReturnedKeyData>,
        hops: u8,
    ) -> Option<ReturnRootData> {
        if steps.len() > CHAIN_LIMIT {
            return None;
        }
        match value {
            // `$this`, the root every chain in a method hangs off.
            Expression::Variable(mago_syntax::cst::Variable::Direct(variable))
                if variable.name == b"$this" =>
            {
                Some(ReturnRootData::Receiver)
            }
            Expression::Variable(mago_syntax::cst::Variable::Direct(_)) => {
                let assigned = self.the_one_value_of(locals.block, locals.parameters, value)?;
                self.returned_root(locals, assigned, steps, key, hops.checked_sub(1)?)
            }
            // `new Foo()` as a root rather than as a whole answer: `new Query($this)->execute()`
            // is a chain, where a bare `new Foo()` is a type and `returned_from_body` has it.
            Expression::Instantiation(new) => match new.class {
                Expression::Identifier(identifier) => {
                    Some(ReturnRootData::Named(self.name_ref(identifier)))
                }
                _ => None,
            },
            Expression::Call(mago_syntax::cst::Call::Method(call)) => {
                let (Some(name), None) = self.member_selector(&call.method) else {
                    return None;
                };
                steps.push(name);
                *key = self.first_argument_key(&call.argument_list);
                self.returned_root(locals, call.object, steps, key, hops)
            }
            Expression::Call(mago_syntax::cst::Call::StaticMethod(call)) => {
                let (Some(name), None) = self.member_selector(&call.method) else {
                    return None;
                };
                steps.push(name);
                *key = self.first_argument_key(&call.argument_list);
                match call.class {
                    Expression::Identifier(identifier) => {
                        Some(ReturnRootData::Named(self.name_ref(identifier)))
                    }
                    Expression::Static(_) | Expression::Self_(_) => Some(ReturnRootData::Receiver),
                    _ => None,
                }
            }
            Expression::Access(mago_syntax::cst::Access::Property(access)) => {
                let (Some(name), None) = self.member_selector(&access.property) else {
                    return None;
                };
                steps.push(name);
                // A property read carries no argument, and it must not leave an earlier step's
                // standing: the key belongs to the root-most *call*.
                *key = None;
                self.returned_root(locals, access.object, steps, key, hops)
            }
            _ => None,
        }
    }
}

/// The first positional argument of a call, when it is a string literal or a `Foo::class`.
///
/// What a contract reads: a catalog a service id, an entity type id, a field name; a factory the
/// class it builds. The first, because that is what such a contract means by "the argument" -- the
/// same rule `literal_call` states for the chain a receiver was produced by.
impl<'p> crate::lowerer::Lowerer<'_, 'p> {
    fn first_argument_key(
        &mut self,
        list: &mago_syntax::cst::ArgumentList<'p>,
    ) -> Option<ReturnedKeyData> {
        let written = match list.arguments.first()? {
            mago_syntax::cst::Argument::Positional(argument) if argument.ellipsis.is_none() => {
                argument.value
            }
            _ => return None,
        };
        if let Expression::Access(mago_syntax::cst::Access::ClassConstant(access)) = written
            && let mago_syntax::cst::ClassLikeConstantSelector::Identifier(constant) =
                &access.constant
            && constant.value.eq_ignore_ascii_case(b"class")
            && let Expression::Identifier(named) = access.class
        {
            let (anchor, path) = self.written_name(named);
            return Some(ReturnedKeyData::Class { path, anchor });
        }
        let text = crate::lowerer::string_literal(written)?;
        Some(ReturnedKeyData::Text(self.interner.name(&text)))
    }
}

/// How many members a chain may hold. Three is the longest Drupal core writes, and a Laravel query
/// is longer: BookStack's paginated user list is `User::query()` and six more calls.
const CHAIN_LIMIT: usize = 8;

/// How many locals a chain may be read through on its way to its root.
const LOCAL_HOPS: u8 = 3;

/// The body a returned chain's locals are bound in, and the parameters that are not locals at all.
#[derive(Clone, Copy)]
struct Locals<'b, 'p> {
    block: &'b Block<'p>,
    parameters: &'b FunctionLikeParameterList<'p>,
}
