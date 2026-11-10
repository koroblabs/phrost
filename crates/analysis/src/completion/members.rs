//! The members of what a receiver holds, and which of them the operator can reach.

use phrost_model::{SymbolKind, TextSize, matches_loosely};
use phrost_view::{IndexView, MemberKind, ReceiverClasses};

use super::{
    Call, CallShape, CompletionItem, CompletionList, Describe, MemberRef,
    chain::{chain_root, chain_steps},
};
use crate::context::{Receiver, ReceiverRoot};

/// Which members an access operator can reach.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Access {
    /// `->` reaches instance methods and properties only.
    Instance,
    /// `::` reaches statics, constants, and cases — and instance methods too, which is how
    /// `parent::doThing()` works.
    Static,
}

/// The members of what the receiver holds, filtered by [`matches_loosely`] rather than by prefix.
///
/// Reported from an editor: typing `fund` on a class with `isShipped`, `getShipDate` and `setShipDate`
/// offered nothing, because none of the three *starts* with it. PHP member names carry their subject
/// in the middle, so a prefix test is the wrong test here -- and a client cannot recover from it,
/// since it can only narrow what it was sent.
pub(super) fn members(
    view: &IndexView<'_>,
    offset: TextSize,
    receiver: &Receiver,
    typed: &str,
    access: Access,
    whole_statement: bool,
) -> CompletionList {
    let steps = chain_steps(receiver);
    // Where the name being typed starts, which is where the receiver is read: at the cursor, the
    // end of `$item = $item->res|` is the end of an assignment, and a write that has finished there
    // has replaced the very value whose members are being asked for -- the guard that made
    // `$item` a resource gone with it, and nothing offered while goto on the same name answered.
    let receiver_at = offset - TextSize::of(typed);
    // Several classes, because PHP has two ways for a value to have more than one type, and they
    // read in opposite directions -- see `ReceiverClasses`.
    // A value before `::` that names a class, rather than having one -- see
    // `IndexView::class_a_chain_names`.
    let named_by_a_value = match (access, chain_root(&receiver.root)) {
        (Access::Static, Some(root)) => view.class_a_chain_names(receiver_at, root, &steps),
        _ => None,
    };
    let reached = match (named_by_a_value.clone(), chain_root(&receiver.root)) {
        (Some(class), _) => ReceiverClasses::All(vec![class]),
        (None, Some(root)) => view.chain_classes(receiver_at, root, &steps),
        // Nothing readable before the operator. Offering the enclosing class's members here is the
        // guess that makes a completion list look authoritative and be wrong.
        (None, None) => ReceiverClasses::All(Vec::new()),
    };

    if reached.is_empty() {
        return CompletionList {
            items: Vec::new(),
            is_complete: true,
            ..CompletionList::default()
        };
    }
    let classes = reached.classes();
    // Whether what is before the operator stands for the object the code runs as: `$this`,
    // `self`, `parent` -- and the name of the enclosing class or of one it descends from, which is
    // the last two spelled out. See `IndexView::runs_within`.
    let relative = match &receiver.root {
        ReceiverRoot::This | ReceiverRoot::SelfClass | ReceiverRoot::Parent => true,
        ReceiverRoot::Class(_) if matches!(access, Access::Static) && receiver.steps.is_empty() => {
            classes
                .iter()
                .any(|class| view.runs_within(offset, class.as_str()))
        }
        _ => false,
    };

    // Visibility is decided per class, because PHP decides it by comparing classes rather than by
    // looking at the receiver: inside `Repo`, `$sibling->own` compiles and `$this->` is not special.
    //
    // Each candidate keeps the class it came from. Two readers need it and both were wrong without
    // it: a `completionItem/resolve` looks the member up again and would have looked in the wrong
    // table, and an operand that does not answer this name is what the row has to say.
    let mut candidates: Vec<Reached> = Vec::new();
    // Each class's members, built once: the union check below asks every one of them about every
    // row, and building a class's list per row -- types rendered and all -- was quadratic in the
    // class's size. Measured on a union of two 200-member classes: 9.4 ms for one completion, the
    // worst site on the reference project and four-fifths of the budget.
    let mut answered: Vec<std::collections::HashSet<(MemberKind, phrost_model::Name)>> =
        Vec::with_capacity(classes.len());
    for (operand, class) in classes.iter().enumerate() {
        let visibility = view.visible_from(offset, class.as_str());
        let members = view.member_candidates(class.as_str(), visibility);
        answered.push(
            members
                .iter()
                .map(|member| (member.kind, member.name.clone()))
                .collect(),
        );
        for member in members {
            // Only a second operand can collide with what is already there, so an ordinary receiver
            // -- which is every one that is not composite -- pays nothing for this. The first
            // operand that declares a name wins, which is the order they are written in.
            if operand > 0
                && candidates
                    .iter()
                    .any(|held| held.member.kind == member.kind && held.member.name == member.name)
            {
                continue;
            }
            candidates.push(Reached {
                owner: class.clone(),
                member,
                on_every_operand: true,
            });
        }
    }

    // A value that is *one of* several classes really may be any of them, so a member only some of
    // them answer is a member the call may fatal on -- and the row says which class declares it
    // rather than being left out.
    //
    // Withheld until this was measured on real code, and the measurement is what changed it: the
    // checker cannot report such a call either, because a union here is built by inference that is
    // approximate in places (see `PathRoot::EitherOf`) and a report on an approximate union is a
    // report on our own imprecision. Offering nothing therefore prevented no mistake; it only left
    // two unrelated operands with an empty list, which is what an editor shows when a server is
    // broken. Naming the operand is what makes the risk visible where hiding the row did not.
    if let ReceiverClasses::Any {
        classes,
        every_operand_named,
    } = &reached
        && (classes.len() > 1 || !every_operand_named)
    {
        for candidate in &mut candidates {
            // An operand naming no class answers no name at all, so a union holding one can never
            // have a member on every operand -- `Alpha|string` reached with `->` may be a string,
            // and the row saying `Alpha only` is how that is said rather than hidden.
            let row = (candidate.member.kind, candidate.member.name.clone());
            candidate.on_every_operand =
                *every_operand_named && answered.iter().all(|names| names.contains(&row));
        }
        // The safe ones first, because the row a person accepts without reading is the first one:
        // a name every operand answers cannot be wrong here, and one only some of them answer can.
        // Stable, so the order inside each group is the order the classes were written in.
        candidates.sort_by_key(|candidate| !candidate.on_every_operand);
    }

    // Which of the classes answer a name they do not declare through `::` -- see
    // `MemberCandidate::forwarded`. Asked of the rows already in hand: `__callStatic` is a member
    // like any other, inherited like any other.
    let calls_statically: Vec<phrost_model::Name> = candidates
        .iter()
        .filter(|reached| {
            reached.member.kind == MemberKind::Method
                && reached
                    .member
                    .name
                    .as_str()
                    .eq_ignore_ascii_case("__callStatic")
        })
        .map(|reached| reached.owner.clone())
        .collect();

    let mut items = candidates
        .into_iter()
        .filter(|reached| {
            // A call only: `__callStatic` answers `User::where()` and nothing answers a property
            // read through `::`.
            // And a reopened one this scope could not call directly, which PHP sends the same way.
            let refused_here = || {
                !reached
                    .member
                    .visibility
                    .is_visible_from(view.visible_from(offset, reached.owner.as_str()))
            };
            let forwarded_statically = reached.member.kind == MemberKind::Method
                && (reached.member.forwarded || (reached.member.opened && refused_here()))
                && calls_statically.contains(&reached.owner);
            reachable(
                reached.member.kind,
                reached.member.is_static || forwarded_statically,
                access,
                relative,
            )
        })
        .filter(|reached| matches_loosely(reached.member.name.as_str(), typed))
        .map(|reached| {
            let Reached {
                owner,
                member,
                on_every_operand,
            } = reached;
            CompletionItem {
                deprecated: member.deprecated,
                demoted: demoted(
                    member.kind,
                    member.name.as_str(),
                    member.is_static,
                    access,
                    relative,
                ),
                // A static property is the one member PHP writes with a sigil, and `Foo::foo` is not
                // how it is reached. Through `::` an instance property is unreachable and is already
                // filtered out, so every property left here is one of these.
                label: match (access, member.kind) {
                    (Access::Static, MemberKind::Property) => format!("${}", member.name),
                    _ => member.name.as_str().to_owned(),
                },
                // A method is a call, and accepting one should leave a call rather than a name somebody
                // then has to add brackets to. Refused where the brackets are already written: re-typing
                // the name in `$app->getFoo(1)` must not leave `getFoo()(1)`.
                //
                // The `;` rides along only where the access is the whole statement -- and only for a
                // method, because `$repo->size;` is a statement that does nothing and a property is
                // reached to read it, not to end a line.
                call: match member.kind {
                    MemberKind::Method if !view.opens_call_after(offset) => Some(Call {
                        shape: match member.takes_arguments {
                            true => CallShape::Arguments,
                            false => CallShape::Empty,
                        },
                        parameters: member.parameters.clone(),
                        terminates: whole_statement,
                    }),
                    _ => None,
                },
                kind: crate::render::symbol_kind_of(member.kind),
                // The type alone in the list, the whole declaration beside the selected row -- and the
                // operand's name when only some of a union's operands answer this, because that is the
                // one thing a reader needs in order to decide whether the call is safe here.
                detail: match on_every_operand {
                    true => detail_of(member.kind, member.ty.as_deref()),
                    false => Some(match detail_of(member.kind, member.ty.as_deref()) {
                        Some(detail) => format!("{detail} — {} only", short_name(owner.as_str())),
                        None => format!("{} only", short_name(owner.as_str())),
                    }),
                },
                // The class this row came from, not the first of them: an intersection reaches
                // `expects()` through `MockObject` and `getId()` through `Application`, and a resolve
                // that looked both up in the first operand's table answered nothing for half the list.
                describe: Some(Describe::Member(MemberRef {
                    class: owner.as_str().to_owned(),
                    kind: member.kind,
                    name: member.name.as_str().to_owned(),
                })),
                insert: None,
                // A member is reached through its receiver, not through a `use` line.
                import: None,
            }
        })
        .collect::<Vec<_>>();

    // Whether a catalog's keys were cut at the limit, which the client is told so that it asks
    // again as the typed letters narrow them.
    let mut truncated = false;
    if access == Access::Instance
        && let Some(root) = chain_root(&receiver.root)
    {
        truncated = undeclared_properties(view, offset, root, &steps, classes, typed, &mut items);
    }
    if let Some(root) = chain_root(&receiver.root) {
        prefixed_methods(
            view,
            offset,
            root,
            &steps,
            classes,
            typed,
            whole_statement,
            &mut items,
        );
        fluent_methods(
            view,
            offset,
            root,
            &steps,
            classes,
            typed,
            whole_statement,
            &mut items,
        );
    }

    // `::class` is a constant every class-like has and none declares, so no member table holds it
    // and the goto path special-cases it. It is worth more than one item's weight: a service id, an
    // event name, and a mocked class are all written that way.
    //
    // Not on a string that names a class: `$class::class` is a `TypeError` on PHP 8.5.10 -- "Cannot
    // use "::class" on string" -- where `$object::class` is the instance's class.
    if access == Access::Static && named_by_a_value.is_none() && matches_loosely("class", typed) {
        let class = &classes[0];
        items.push(CompletionItem {
            demoted: false,
            // `Foo::class` is the language's, not the class's -- a deprecated class still has it.
            deprecated: false,
            label: "class".to_owned(),
            kind: SymbolKind::ClassConstant,
            // The name the expression evaluates to, which is the thing worth knowing about it.
            detail: Some(class.as_str().to_owned()),
            describe: None,
            insert: None,
            call: None,
            import: None,
        });
    }

    CompletionList {
        items,
        is_complete: !truncated,
        ..CompletionList::default()
    }
}

/// The properties a framework says the receiver has, after the ones it declares.
///
/// Where goto already goes: `$node->get('field_address')->state` lands on the key the field kind's
/// table writes and `$field->entity` on the class a binding says it holds, and a list offering only
/// the interface's methods there was the one half of the rule that had not learned either. A name
/// the member table holds is the declaration's and is not offered twice.
///
/// Whether a catalog's keys were cut at the limit.
fn undeclared_properties(
    view: &IndexView<'_>,
    offset: TextSize,
    root: phrost_view::ChainRoot<'_>,
    steps: &[phrost_view::ChainStep<'_>],
    classes: &[phrost_model::Name],
    typed: &str,
    items: &mut Vec<CompletionItem>,
) -> bool {
    let mut truncated = false;
    for class in classes {
        let found = view.undeclared_properties(
            offset,
            root,
            steps,
            class.as_str(),
            typed,
            super::NAME_LIMIT,
        );
        truncated |= found.iter().filter(|property| property.keyed).count() >= super::NAME_LIMIT;
        for property in found {
            let name = &*property.name;
            let known = items.iter().any(|item| {
                item.label == name
                    && matches!(
                        &item.describe,
                        Some(Describe::Member(MemberRef {
                            kind: MemberKind::Property,
                            ..
                        })) | None
                    )
            });
            if known || !matches_loosely(name, typed) {
                continue;
            }
            items.push(CompletionItem {
                deprecated: false,
                // A catalog's keys after everything else: on an entity they are every field the
                // project has, and ranking is what answers a crowded list -- see `demoted`.
                demoted: property.keyed,
                label: name.to_owned(),
                kind: crate::render::symbol_kind_of(MemberKind::Property),
                detail: detail_of(MemberKind::Property, property.ty.as_deref()),
                // Described as the class whose body states it, which is where goto lands and what
                // hover shows; a kind only the framework's own table describes has no such class.
                describe: property.declarer.map(|declarer| {
                    Describe::Member(MemberRef {
                        class: declarer.as_str().to_owned(),
                        kind: MemberKind::Property,
                        name: name.to_owned(),
                    })
                }),
                insert: None,
                call: None,
                import: None,
            });
        }
    }
    truncated
}

/// The methods a framework finds under a prefix, by the name a call writes: Eloquent's
/// `scopePublished` as `published`, on the model and on a builder for it.
///
/// Where goto already lands. Offered after the declared members and never in place of one: a name
/// the receiver declares is the declaration's, and that row is already in the list.
#[allow(clippy::too_many_arguments)]
fn prefixed_methods(
    view: &IndexView<'_>,
    offset: TextSize,
    root: phrost_view::ChainRoot<'_>,
    steps: &[phrost_view::ChainStep<'_>],
    classes: &[phrost_model::Name],
    typed: &str,
    whole_statement: bool,
    items: &mut Vec<CompletionItem>,
) {
    for class in classes {
        for candidate in view.prefixed_candidates(offset, root, steps, class.as_str(), typed) {
            let name = &*candidate.name;
            if items.iter().any(|item| item.label == name) {
                continue;
            }
            items.push(CompletionItem {
                deprecated: false,
                demoted: false,
                label: name.to_owned(),
                kind: crate::render::symbol_kind_of(MemberKind::Method),
                detail: Some(format!(
                    "{}::{}",
                    short_name(candidate.model.as_str()),
                    candidate.declared
                )),
                describe: Some(Describe::Member(MemberRef {
                    class: candidate.model.as_str().to_owned(),
                    kind: MemberKind::Method,
                    name: candidate.declared.as_str().to_owned(),
                })),
                insert: None,
                call: (!view.opens_call_after(offset)).then(|| Call {
                    shape: match candidate.takes_arguments {
                        true => CallShape::Arguments,
                        false => CallShape::Empty,
                    },
                    parameters: candidate.parameters.clone(),
                    terminates: whole_statement,
                }),
                import: None,
            });
        }
    }
}

/// The methods a framework's `__call` answers by a prefix and a word the model states: Eloquent's
/// `whereTitle` for the `title` a model lists.
///
/// Described by the word, which is the one thing written down: the card is the property's. Last, and
/// never in place of a declared member or a scope of the same name.
#[allow(clippy::too_many_arguments)]
fn fluent_methods(
    view: &IndexView<'_>,
    offset: TextSize,
    root: phrost_view::ChainRoot<'_>,
    steps: &[phrost_view::ChainStep<'_>],
    classes: &[phrost_model::Name],
    typed: &str,
    whole_statement: bool,
    items: &mut Vec<CompletionItem>,
) {
    for class in classes {
        for candidate in view.fluent_candidates(offset, root, steps, class.as_str(), typed) {
            let name = &*candidate.name;
            if items.iter().any(|item| item.label == name) {
                continue;
            }
            items.push(CompletionItem {
                deprecated: false,
                demoted: false,
                label: name.to_owned(),
                kind: crate::render::symbol_kind_of(MemberKind::Method),
                detail: Some(format!(
                    "{}::${}",
                    short_name(candidate.model.as_str()),
                    candidate.word
                )),
                describe: Some(Describe::Member(MemberRef {
                    class: candidate.model.as_str().to_owned(),
                    kind: MemberKind::Property,
                    name: candidate.word.to_string(),
                })),
                insert: None,
                call: (!view.opens_call_after(offset)).then(|| Call {
                    shape: CallShape::Arguments,
                    parameters: Box::default(),
                    terminates: whole_statement,
                }),
                import: None,
            });
        }
    }
}

/// The methods a callable pair can name: `[$this, 'vali|']`, `[Repo::class, 'fi|']`.
///
/// The members an operator would offer on the same receiver, and nothing else of them: PHP checks a
/// pair against exactly the rules `->` and `::` follow -- visibility from where the pair is
/// written, and only a static method through a class named outright. Methods only, because a pair
/// names nothing else; and no brackets, because the name is the whole of what goes in the string.
pub(super) fn callable_methods(
    view: &IndexView<'_>,
    offset: TextSize,
    receiver: &Receiver,
    typed: &str,
    statically: bool,
) -> CompletionList {
    let access = match statically {
        true => Access::Static,
        false => Access::Instance,
    };
    let mut list = members(view, offset, receiver, typed, access, false);
    list.items.retain(|item| {
        matches!(
            &item.describe,
            Some(Describe::Member(MemberRef {
                kind: MemberKind::Method,
                ..
            }))
        )
    });
    for item in &mut list.items {
        item.call = None;
    }
    list
}

/// The methods a test double can be configured with: those of what it stands in for.
///
/// Public and protected, because a double replaces both and is configured with either by name; never
/// a call, because what is written is a string.
pub(super) fn configured_methods(
    view: &IndexView<'_>,
    subjects: &[phrost_model::Name],
    typed: &str,
) -> CompletionList {
    let mut items = Vec::new();
    for class in subjects {
        for member in view.member_candidates(class.as_str(), phrost_model::Visibility::Protected) {
            if member.kind != MemberKind::Method
                || member.is_static
                || !matches_loosely(member.name.as_str(), typed)
                // A double answers these itself and cannot be told to.
                || member.name.as_str().starts_with("__")
                || items
                    .iter()
                    .any(|item: &CompletionItem| item.label == member.name.as_str())
            {
                continue;
            }
            items.push(CompletionItem {
                deprecated: member.deprecated,
                demoted: false,
                label: member.name.as_str().to_owned(),
                kind: crate::render::symbol_kind_of(member.kind),
                detail: detail_of(member.kind, member.ty.as_deref()),
                describe: Some(Describe::Member(MemberRef {
                    class: class.as_str().to_owned(),
                    kind: member.kind,
                    name: member.name.as_str().to_owned(),
                })),
                insert: None,
                call: None,
                import: None,
            });
        }
    }
    CompletionList {
        items,
        is_complete: true,
        ..CompletionList::default()
    }
}

/// One member the receiver reaches, and which of its classes it came from.
struct Reached {
    /// The class whose table this row was read out of -- what a resolve has to look in again.
    owner: phrost_model::Name,
    member: phrost_view::MemberCandidate,
    /// Whether every operand of a union answers this name.
    ///
    /// Always true for an ordinary receiver and for an intersection, where the value really is all
    /// of them. False only where the value is *one of* several and this name is not on all of them,
    /// which is the row that has to say so.
    on_every_operand: bool,
}

/// The last segment of a fully-qualified name, for a column with room for one word.
fn short_name(fqn: &str) -> &str {
    fqn.rsplit('\\').next().unwrap_or(fqn)
}

/// The one line shown beside a member in the list.
///
/// `: void` for a method, because that is the arrow PHP writes and it reads as "returns"; the type
/// on its own for a property or a constant, because that is where PHP writes it. Nothing for an
/// enum case: its type is the enum, which the reader is already looking at.
fn detail_of(kind: MemberKind, ty: Option<&str>) -> Option<String> {
    let ty = ty?;
    match kind {
        MemberKind::Method => Some(format!(": {ty}")),
        MemberKind::Property | MemberKind::ClassConstant => Some(ty.to_owned()),
        MemberKind::EnumCase => None,
    }
}

/// Whether a member is one to show *after* the rest rather than among them.
///
/// PHP reserves every function name beginning with `__` for itself, and these used to be left out
/// of the list entirely, on the argument that the language calls them rather than a person. That
/// argument is wrong for four of them and the counts say so: `->__toString()` is written by hand 51
/// times in Drupal core and 59 in Symfony, `->__invoke()` 33 and 18, `->__construct()` and
/// `->__serialize()` seventeen and fifteen. They are ordinary members of the language, which is
/// what a person reaching for one is entitled to assume.
///
/// What the complaint behind the old rule was about is **crowding**: a class with a handful of them
/// pushes the members that were actually wanted off the visible part of the list. Ranking answers
/// that; hiding answered a different question nobody asked.
///
/// Only methods: the reservation is on function names, and nothing magic is a property.
///
/// **A static method reached through `->` is demoted for the same reason**, and it is the same
/// argument a second time. It used to be left out unless the receiver named the enclosing class,
/// on the ground that `$app->fromNode()` is a style error every linter flags and reading it in a
/// list is surprising. True, and it is the crowding complaint again wearing the words of a
/// correctness one -- the engine looks the method up on the class and calls it, with no error and
/// no deprecation. Ranking answers crowding; hiding answers a question nobody asked.
///
/// Not on `$this->`, which is the half of the old rule that was a measurement rather than a
/// judgement: `$this->create(...)` on a static factory is written by hand six times in one real
/// project and five in Drupal core, so on a receiver naming the enclosing class it is an ordinary
/// member and belongs among them.
fn demoted(kind: MemberKind, name: &str, is_static: bool, access: Access, relative: bool) -> bool {
    if kind != MemberKind::Method {
        return false;
    }
    name.starts_with("__") || (is_static && matches!(access, Access::Instance) && !relative)
}

/// Whether the operator can reach this member, given what is on the left of it.
///
/// `$obj->CONST` is not PHP, and neither is `$obj->staticProperty`. Offering them would suggest code
/// that does not run.
///
/// Both halves need the receiver's *root* and not just the operator, which is what this took a
/// second pass to get right.
///
/// Through `->`, a static method is legal PHP -- the engine looks it up on the class and calls it,
/// with no error and no deprecation. So it is reachable, and [`demoted`] is where the argument
/// against *suggesting* it is answered: it is offered last rather than not at all, which is what
/// this file already decided about magic methods after making the opposite mistake.
///
/// Through `::`, an instance method is the mirror image: `parent::__construct()` and
/// `self::helper()` are ordinary PHP, and `Unrelated::instanceMethod()` is a fatal error since PHP
/// 8.0. So that one is allowed for a relative receiver and refused for a named class -- unless the
/// name is the enclosing class's own or an ancestor's, which is `self` or `parent` written out and
/// arrives here as `relative`.
fn reachable(kind: MemberKind, is_static: bool, access: Access, relative: bool) -> bool {
    match access {
        Access::Instance => match kind {
            MemberKind::Method => true,
            // A static property is not: `$obj->registry` reads an instance property that does not
            // exist. The asymmetry is PHP's, not a policy of ours.
            MemberKind::Property => !is_static,
            _ => false,
        },
        // `Stage::$name` is a static property access, so an instance property is never reachable
        // through `::` -- and an enum's `$name` and `$value` made that visible, because they are the
        // first instance properties most classes reached this way have.
        Access::Static => match kind {
            MemberKind::Property => is_static,
            MemberKind::Method => is_static || relative,
            _ => true,
        },
    }
}
