//! What iterating a generator hands back, read from the `yield`s that produce it.
//!
//! `public function rows(): \Generator { yield new Row(); }` says what comes out of it as plainly
//! as a `@return \Generator<Row>` would, and PHP has no syntax for writing that in the signature --
//! the declaration can only name the generator. So every `foreach ($this->rows() as $row)` was
//! iterating something with no element type, and every member reached through one was unresolvable.
//! Symfony writes a generator or an `iterable` return in 955 of its files.
//!
//! # A generator's `return` is not the declaration's
//!
//! The half of this that is a *correction* rather than a gain. A `return` inside a generator hands
//! its value to `Generator::getReturn()` and never to the caller, so `returned_from_body` -- which
//! reads an undeclared method's returns as its type -- was reading a value that method does not
//! produce. `function each() { yield 1; return new Row(); }` typed the call `Row`, and a `Row`
//! member reached through it resolved against a class the value can never be.
//!
//! So both readings have to know: one to read the yields instead, the other to stay silent. That is
//! why this answers with three cases and not with an `Option` -- "not a generator" and "a generator
//! this cannot read" are the same absence and must not have the same consequence.
//!
//! # All or nothing
//!
//! The rule [`crate::returns`] states, for the same reason: a body whose yields disagree settles
//! nothing rather than settling from the ones it can read. Two `yield`s of two classes are a union
//! this does not build, and taking the first would state a claim narrower than the code.
//!
//! `yield from` is the case that settles nothing by construction: it hands on somebody else's
//! elements, and what those are takes a resolution this layer is a whole index away from.

use mago_span::HasSpan as _;
use mago_syntax::cst::{Block, Expression, Node, Yield};
use phrost_hir::{TypeRefData, TypeRefId};
use phrost_model::BuiltinTy;

use crate::lowerer::Lowerer;

/// The word the file-level prefilter tests for, and the one a body is scanned for.
pub(crate) const KEYWORD: &str = "yield";

/// What a `yield` makes of a declaration whose type nothing else settled.
///
/// Three cases rather than an `Option`, because the two absences have opposite consequences: a body
/// that is not a generator falls through to every other reading, and one that is falls through to
/// none of them.
pub(crate) enum Yielding {
    /// No `yield` belongs to this body. Every other reading of it applies.
    No,
    /// A generator, and what iterating it produces.
    Produces(TypeRefId),
    /// A generator whose yields name nothing this reads. The declaration keeps what it said, and
    /// -- the point of the case -- no `return` inside it is read as what the declaration produces.
    Unreadable,
}

impl<'p> Lowerer<'_, 'p> {
    /// What iterating the generator `block` declares would produce.
    ///
    /// The element type is written onto whatever the declaration already named -- `\Generator`,
    /// `iterable`, `\Traversable` -- so that the two are the same claim with the arguments added,
    /// which is the one relationship [`phrost_hir::refined_type_ref`] lets a docblock win. A
    /// declaration that named nothing gets `iterable`: a generator is one, and it is the spelling
    /// that needs no name reference invented for a name nobody wrote.
    ///
    /// Keys are read only when every `yield` writes one. PHP numbers the rest from zero, so a body
    /// that writes none has `int` keys and one that writes some has keys this cannot name -- and
    /// there the value alone is stated rather than a key guessed.
    pub(crate) fn yielded_from_body(
        &mut self,
        block: &Block<'p>,
        declared: Option<TypeRefId>,
    ) -> Yielding {
        if !self.body_may_yield(block) {
            return Yielding::No;
        }
        let yields = yields_in(block);
        if yields.is_empty() {
            return Yielding::No;
        }

        let mut value: Option<TypeRefId> = None;
        let mut key: Option<TypeRefId> = None;
        let mut keyed = 0usize;
        let mut keys_agree = true;
        for yielded in &yields {
            let written = match yielded {
                // `yield;` hands back null, which is an operand of a union this does not build.
                Yield::Value(node) => match node.value {
                    Some(value) => (None, value),
                    None => return Yielding::Unreadable,
                },
                Yield::Pair(node) => {
                    keyed += 1;
                    (Some(node.key), node.value)
                }
                Yield::From(_) => return Yielding::Unreadable,
            };
            let Some(agreed) = self.agrees(value, written.1) else {
                return Yielding::Unreadable;
            };
            value = Some(agreed);
            if let Some(written_key) = written.0 {
                // A key this cannot read is not a reason to lose the value: it only means the pair
                // half of the answer is dropped below. Recorded rather than left in `key`, because
                // a later key that *is* readable would otherwise look like the one they all agree
                // on.
                match self.key_agrees(key, written_key) {
                    Some(agreed) => key = Some(agreed),
                    None => keys_agree = false,
                }
            }
        }

        let value = match value {
            Some(value) => value,
            None => return Yielding::Unreadable,
        };
        let key = match keyed {
            // Every `yield` wrote a bare value, so PHP numbered the keys itself.
            0 => Some(
                self.builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::Int)),
            ),
            // Every one wrote a key, and they agreed on a type this reads.
            _ if keyed == yields.len() && keys_agree => key,
            // Some did and some did not, which is two key domains in one generator.
            _ => None,
        };

        let base = declared.unwrap_or_else(|| {
            self.builder
                .add_type_ref(TypeRefData::Builtin(BuiltinTy::Iterable))
        });
        let args = match key {
            Some(key) => self.builder.alloc_type_ref_list(&[key, value]),
            None => self.builder.alloc_type_ref_list(&[value]),
        };
        Yielding::Produces(self.builder.add_type_ref(TypeRefData::Generic {
            base,
            args,
            non_empty: false,
        }))
    }

    /// The type `written` names, when it is the one everything before it named too.
    fn agrees(
        &mut self,
        seen: Option<TypeRefId>,
        written: &'p Expression<'p>,
    ) -> Option<TypeRefId> {
        let data = self.returned_type(written)?;
        let id = self.builder.add_type_ref(data);
        match seen {
            Some(seen) if !self.same_written_type(seen, id) => None,
            _ => Some(id),
        }
    }

    /// [`Self::agrees`] for a key, which may also be a literal.
    ///
    /// A literal key is the commonest one written -- `yield 'name' => $value` -- and the reader of
    /// returned values does not read scalars, so every such generator lost its keys. Not the array
    /// rule, either: an array stores `'1'` as the integer `1`, and a generator hands its key over
    /// untouched, which PHP 8.5.10 confirms -- `yield '1' => x` is keyed by the string `"1"`.
    fn key_agrees(
        &mut self,
        seen: Option<TypeRefId>,
        written: &'p Expression<'p>,
    ) -> Option<TypeRefId> {
        let literal = match written {
            Expression::Literal(mago_syntax::cst::Literal::String(_)) => Some(BuiltinTy::String),
            Expression::Literal(mago_syntax::cst::Literal::Integer(_)) => Some(BuiltinTy::Int),
            Expression::Literal(mago_syntax::cst::Literal::Float(_)) => Some(BuiltinTy::Float),
            _ => None,
        };
        let Some(literal) = literal else {
            return self.agrees(seen, written);
        };
        let id = self.builder.add_type_ref(TypeRefData::Builtin(literal));
        match seen {
            Some(seen) if !self.same_written_type(seen, id) => None,
            _ => Some(id),
        }
    }

    /// Whether the text of this body holds the keyword at all.
    ///
    /// The prefilter `own_arguments` states, and it fails in the same harmless direction: a `yield`
    /// written in a comment or inside a longer identifier costs one walk that finds nothing. What it
    /// cannot do is miss one, since the walk it gates is over the same bytes.
    fn body_may_yield(&self, block: &Block<'p>) -> bool {
        if !self.file_may_yield() {
            return false;
        }
        let source = self.parsed_source();
        let span = block.span();
        let (start, end) = (span.start.offset as usize, span.end.offset as usize);
        // Clamped rather than trusted, for the reason `own_arguments` gives: a repaired parse can
        // carry a span past the text it was re-parsed from.
        source
            .get(start.min(source.len())..end.min(source.len()))
            .is_some_and(|text| text.contains(KEYWORD))
    }
}

/// Every `yield` that belongs to *this* body.
///
/// The generic node walk rather than the statement walk [`crate::returns`] uses, because a `yield`
/// is an expression and PHP puts it where one goes: `$sent = yield $line;` is how a coroutine reads
/// what was sent back into it, and `yield` inside a `match` arm or a ternary is ordinary.
///
/// It steps around everything that opens a scope of its own, for the reason
/// [`Lowerer::nested_expressions`] states and the one `saw_yield` exists for: a closure that yields
/// is itself the generator, and the function around it is not.
fn yields_in<'ast, 'p>(block: &'ast Block<'p>) -> Vec<&'ast Yield<'p>> {
    let mut found = Vec::new();
    let mut queue = vec![Node::Block(block)];

    while let Some(node) = queue.pop() {
        match node {
            // Taken whole. What is yielded is read from the node itself, and a `yield` nested
            // inside another's operand is the same generator's either way.
            Node::Yield(yielded) => found.push(yielded),
            Node::Closure(_)
            | Node::ArrowFunction(_)
            | Node::Function(_)
            | Node::Method(_)
            | Node::PropertyHook(_)
            | Node::AnonymousClass(_)
            | Node::Class(_)
            | Node::Interface(_)
            | Node::Trait(_)
            | Node::Enum(_) => {}
            other => queue.extend(other.children()),
        }
    }

    // Source order, because the queue is a stack and because which `yield` is read first decides
    // which type refs the file's arena allocates in which order.
    found.sort_by_key(|yielded| yielded.span().start.offset);
    found
}
