//! Everywhere one declaration is used.
//!
//! Matching is by what a name *resolves to*, never by the text. Two classes called `Repo` in
//! different namespaces are two declarations, and a search that returned both would be worse than
//! useless for the thing anyone wants it for -- deciding whether a change is safe.
//!
//! # One file at a time
//!
//! This answers about the file a view is over. A project-wide answer is that, run over every file
//! the project owns, which is a workspace's job because only a workspace can open one.

mod refusal;
mod rename;
mod sweep;

pub use refusal::rename_refusal;
mod family;

use family::member_family;
pub use rename::{is_reserved_word, is_valid_name, rename_range, rename_span};
pub use sweep::{ConstructionQuery, SweepQuery};

use phrost_model::{Name, TextRange, TextSize};
use phrost_view::{IndexView, MemberAccessOutcome, MemberKind};

/// The declaration being searched for.
///
/// Identified by where it is declared, not by the name at the cursor: `$repo->find()` and
/// `$other->find()` are the same reference only when they reach the same declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReferenceTarget {
    /// A class, interface, trait, or enum, fully qualified.
    ///
    /// Every use that *reaches* this declaration, however the file spells it: a file that wrote
    /// `use ...\UUIDv7 as QuoteId;` and then `new QuoteId()` is using this class, and a reader
    /// asking where it is used means that site too. What a rename may rewrite is narrower --
    /// [`Self::ClassUnderItsOwnName`].
    Class(Name),
    /// The same class, and only the places that spell it by its own name.
    ///
    /// What a rename may rewrite. A use written `QuoteId` reaches the class through a binding the
    /// rename does not touch, so replacing the spelling breaks exactly the sites the alias was
    /// keeping correct. The `use` line's own path is written under the real name and is edited with
    /// everything else; the alias is renamed on its own, from a cursor on it -- see [`Self::Alias`].
    ///
    /// Produced only by [`rename_target`], like [`Self::Alias`] and [`Self::MemberFamily`].
    ClassUnderItsOwnName(Name),
    /// A free function, fully qualified.
    ///
    /// Its own variant and not a class-shaped one, because PHP's two symbol tables are two: a
    /// function is resolved with a rule no class has -- an unqualified name inside a namespace
    /// falls back to global -- and nothing that answers about classes answers about it. Reached as
    /// a [`Self::Class`] it was invisible twice over: the class-like projection skips a function
    /// name, and `class_location` answers nothing for one. So find-references on a free function
    /// listed neither its declaration nor a single call, and a rename moved only the places a
    /// *string* happened to name it -- which renames `'App\\alpha'` in an autoload map and leaves
    /// the function called `alpha`.
    Function(Name),
    /// A global constant, fully qualified.
    ///
    /// PHP's third symbol table, and third for the same reason the second is: nothing that answers
    /// about classes answers about one. Reached as a class-shaped target, find-references at a
    /// `const MAX = 10;` listed nothing at all -- not one use, not the declaration -- and the
    /// rename offered at that cursor came back with an empty edit and no error, which an editor
    /// shows as a dialogue that does nothing.
    Constant(Name),
    /// A member, by the class that declares it -- which is not necessarily the class it was reached
    /// through.
    Member {
        declared_by: Name,
        kind: MemberKind,
        name: Name,
    },
    /// A name no declaration answers, on a class whose `__get` or `__call` does.
    ///
    /// Drupal's field items are written this way -- `$this->file` on a `FieldItemBase` is a typed
    /// data property the accessor serves -- and a search at one had no declaration to be about, so
    /// it listed nothing while the name was written five times in the file. Two accesses are one
    /// member when the same accessor serves them: `answered_by` is the class declaring it.
    MagicMember {
        answered_by: Name,
        kind: MemberKind,
        name: Name,
    },
    /// A local variable, in one function, in one file.
    ///
    /// Carries the position it was asked about rather than a scope id, because which scope owns a
    /// name is a question about the body and the answer changes as the buffer does. Everything else
    /// here is a project-wide search; see [`Self::spans_files`].
    Local { name: Name, at: TextSize },
    /// The name one file gave an imported class: the `QuoteId` of `use ...\UUIDv7 as QuoteId;`.
    ///
    /// Not the class -- renaming the class leaves the binding saying `as QuoteId` and is correct to
    /// do so -- and not a reference to it either. A third thing, bound by a `use` line and reaching
    /// exactly as far as the file that wrote it, which is why it has no search name.
    ///
    /// Produced only by [`rename_target`]. A *search* at a use of `QuoteId` is about the class, and
    /// answering it with five ranges in one file would be a worse answer to a question nobody asked
    /// differently.
    Alias { name: Name },
    /// A member and every other declaration of it PHP makes one contract with.
    ///
    /// `AttachmentProcessorInterface::processBatch` is declared once and implemented six times, and
    /// an implementation left answering the old name no longer satisfies the interface. So a rename
    /// is about all of them at once, where a *search* is about the declaration that was clicked --
    /// which is why, like [`Self::Alias`], only [`rename_target`] produces this. The call hierarchy,
    /// the code lens and find-references all want the narrower reading.
    ///
    /// `declarers` is never empty and holds the class that was asked about first. See
    /// [`member_family`] for how far the walk goes and when it gives up.
    MemberFamily {
        declarers: Vec<Name>,
        kind: MemberKind,
        name: Name,
        /// Whether the family is known whole. A rename cannot go ahead on a partial one.
        complete: bool,
    },
    /// A key a framework reads out of a catalog: a service id, a route name, a permission.
    ///
    /// The key and not the class it may name. `\Drupal::service('app_signing.package')` resolves to
    /// a class for goto, and a search at it used to list where that *class* is written -- its
    /// imports and type hints -- and never the other places that ask for the service, which is the
    /// question a person standing on an id is asking. Which catalog is part of the identity: a
    /// route and a permission may share a spelling and are two keys.
    CatalogKey { catalog: Box<str>, key: Box<str> },
    /// The key of an element a class builds and reads back by it: Drupal's `$form['email']` and
    /// `$form_state->getValue('email')`. The class is the whole of where it means that, so `at` is
    /// a position inside it, the way a local's is inside its function.
    ElementKey { key: Box<str>, at: TextSize },
    /// An input a class declares in a property's string and reads back by name: Laravel's
    /// `{--queue=}` and `$this->option('queue')`. Named as the declaration writes it, `--queue`,
    /// since an argument and an option may share a spelling and are two inputs. The class is the
    /// whole of where it means that, so `at` is a position inside it.
    DeclaredInput { name: Box<str>, at: TextSize },
    /// A name a `use` clause gives a trait's method -- see `phrost_view::TraitAlias`. Neither the
    /// trait's method nor a declaration: the clause that writes it and the calls reaching it.
    TraitAlias(phrost_view::TraitAlias),
    /// A function's or a method's parameter with every call that names it -- see
    /// `phrost_view::NamedParameter`. What a rename reads a parameter as, and what a search at a
    /// call's label reads; a search at the parameter itself is about the variable.
    Parameter(phrost_view::NamedParameter),
}

impl ReferenceTarget {
    /// The name to grep candidate files for, or `None` for a target no other file can hold.
    ///
    /// The `None` is load-bearing rather than an optimisation: the project-wide sweep works by
    /// grepping for a name, and a local's name means a different variable in every function that
    /// uses the same spelling. A rename that swept for `$node` would rewrite a thousand unrelated
    /// variables.
    pub fn search_name(&self) -> Option<String> {
        match self {
            // Case as written: a function's name is matched case-insensitively when it is
            // resolved, and the sweep's grep is only a prefilter over the files to read. A
            // constant's is matched exactly, so its own spelling is the right needle either way.
            Self::Function(fqn) | Self::Constant(fqn) => {
                Some(phrost_model::short_name(fqn.as_str()).to_owned())
            }
            Self::Class(fqn) | Self::ClassUnderItsOwnName(fqn) => {
                // An anonymous class has no name anybody could write, so no other file can hold a
                // use of it and a sweep for the synthesised one matches nothing at all. `None` is
                // what every caller reads as "this file and no other", which is the whole of where
                // it can be used -- without it, find-references on `new class { ... }` came back
                // empty while the highlight drawn at the same cursor showed its uses.
                (!phrost_view::is_anonymous_class_name(fqn.as_str()))
                    .then(|| phrost_model::short_name(fqn.as_str()).to_owned())
            }
            Self::Member { name, .. }
            | Self::MemberFamily { name, .. }
            | Self::MagicMember { name, .. } => Some(name.as_str().to_owned()),
            // Every file, because a subclass anywhere may call it; the name is what each of them
            // writes.
            Self::TraitAlias(alias) => Some(alias.alias.as_str().to_owned()),
            // Every file calling it writes the callee's name, the declaration's own file included.
            Self::Parameter(parameter) => {
                Some(phrost_model::short_name(parameter.callable.as_str()).to_owned())
            }
            Self::Local { .. }
            | Self::Alias { .. }
            | Self::ElementKey { .. }
            | Self::DeclaredInput { .. } => None,
            // The longest word in it, which is what the index of each file's words can hold: a
            // service id is several words joined by dots, and any one of them rules out every file
            // that does not write it.
            Self::CatalogKey { key, .. } => key
                .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .max_by_key(|word| word.len())
                .filter(|word| !word.is_empty())
                .map(str::to_owned),
        }
    }
}

impl ReferenceTarget {
    /// What a project-wide sweep must look for in a file's text, and nothing for a target no other
    /// file can hold -- [`Self::search_name`], or a constructor's ways in.
    ///
    /// A constructor is reached by writing `new Repo()` and, for a subclass declaring none of its
    /// own, `new Child()` -- neither of which writes `__construct`. Swept for that word alone, every
    /// file that only constructs the class was no candidate: find-references on a constructor
    /// listed the declaration and nothing else, and a rename of one of its parameters left every
    /// `new Repo(table: ...)` naming the old one. The analysis-level tests read every file of a
    /// fixture and never noticed; the server greps first.
    pub fn sweep_query(&self, view: &IndexView<'_>) -> SweepQuery {
        let Some(name) = self.search_name() else {
            return SweepQuery::default();
        };
        let constructed = |name: &str| name.eq_ignore_ascii_case(ConstructionQuery::MEMBER);
        let classes: Vec<&phrost_model::Name> = match self {
            Self::Member {
                declared_by, name, ..
            } if constructed(name.as_str()) => vec![declared_by],
            Self::MemberFamily {
                declarers, name, ..
            } if constructed(name.as_str()) => declarers.iter().collect(),
            Self::Parameter(parameter) if constructed(parameter.callable.as_str()) => {
                parameter.container.iter().collect()
            }
            _ => return SweepQuery::word(name),
        };
        SweepQuery::construction(view, classes.into_iter().map(Name::as_str))
    }
}

/// What the cursor is on, as something that can be searched for.
///
/// `None` when the cursor is on neither a declaration nor a use of one: a keyword, a string, a
/// number.
pub fn reference_target(view: &IndexView<'_>, offset: TextSize) -> Option<ReferenceTarget> {
    // Before the declaration lookup, because a variable is not a declaration the def-map knows
    // about and asking it first costs a catalog lookup that cannot succeed.
    if let Some(variable) = view.variable_at(offset) {
        // `$this` is the language's, not the file's: there is nothing to rename and nothing to list.
        if variable.name.as_str() == "this" {
            return None;
        }
        return Some(local_target(view, variable.name, offset));
    }
    // A call's label, `id` in `find(id: 5)`, which is no variable and no declaration: the parameter
    // it names.
    if let Some((parameter, _)) = view.named_argument_at(offset) {
        // A promoted one names the property its constructor declares, whose search and rename
        // already take the label along.
        return Some(match view.promoted_by(&parameter) {
            Some(declared_by) => ReferenceTarget::Member {
                declared_by,
                kind: MemberKind::Property,
                name: parameter.name,
            },
            None => ReferenceTarget::Parameter(parameter),
        });
    }
    // A string a call reads as a key: before the declaration lookup, which would answer with the
    // class the key names -- see `ReferenceTarget::CatalogKey`.
    if let Some(used) = view.catalog_use_at(offset) {
        return Some(ReferenceTarget::CatalogKey {
            catalog: used.catalog,
            key: used.key,
        });
    }
    if let Some(alias) = view.trait_alias_clause_at(offset) {
        return Some(ReferenceTarget::TraitAlias(alias));
    }
    if let Some(key) = view.built_key_use_at(offset) {
        return Some(ReferenceTarget::ElementKey { key, at: offset });
    }
    if let Some(name) = view.declared_input_use_at(offset) {
        return Some(ReferenceTarget::DeclaredInput { name, at: offset });
    }
    // `$name` in a docblock is the parameter it documents -- the reading goto and hover give it --
    // so a search or a rename from the tag is the one from the signature.
    if let Some(documented) = crate::navigation::documented_parameter_at(view, offset) {
        return Some(local_target(
            view,
            documented.parameter.name,
            documented.parameter.name_range.start(),
        ));
    }

    let Some(location) = view.declaration_at(offset) else {
        return magic_member_at(view, offset);
    };

    // The declaration lookup answers a call to `greet` with the trait's `hello`, which is right for
    // goto and is the destructive answer here: a rename of it renamed `hello` and left `greet`.
    if location.container.is_some()
        && location.kind == phrost_model::SymbolKind::Method
        && let Some(alias) = view.trait_alias_called_at(offset, location.fqn.as_str())
    {
        return Some(ReferenceTarget::TraitAlias(alias));
    }

    Some(match location.container {
        Some(declared_by) => ReferenceTarget::Member {
            declared_by,
            kind: member_kind(location.kind)?,
            name: location.fqn,
        },
        None if location.kind == phrost_model::SymbolKind::Function => {
            ReferenceTarget::Function(location.fqn)
        }
        None if location.kind == phrost_model::SymbolKind::Constant => {
            ReferenceTarget::Constant(location.fqn)
        }
        // The alias token of a `use ... as Y` line is where `Y` comes into being, and is neither
        // the class nor a use of it -- see `phrost_view::AliasBinding`. A search there is about the
        // name the line binds, the way one at a local's first write is about the local: a search
        // for the class left the token it was asked at out of its own answer.
        None => match view
            .alias_at(offset)
            .filter(|alias| alias.range.contains_inclusive(offset))
        {
            Some(alias) => ReferenceTarget::Alias { name: alias.name },
            None => ReferenceTarget::Class(location.fqn),
        },
    })
}

/// The name under the cursor when a class's magic accessor, and no declaration, answers it.
fn magic_member_at(view: &IndexView<'_>, offset: TextSize) -> Option<ReferenceTarget> {
    let phrost_view::CursorTarget::MemberAccess {
        owner, name, kind, ..
    } = view.cursor_target(offset)?
    else {
        return None;
    };
    Some(ReferenceTarget::MagicMember {
        answered_by: magic_accessor_owner(view, owner.as_str(), kind)?,
        kind,
        name,
    })
}

/// The class declaring the accessor that serves an undeclared `kind` on `class`.
fn magic_accessor_owner(
    view: &IndexView<'_>,
    class: &str,
    kind: MemberKind,
) -> Option<phrost_model::Name> {
    let accessors: &[&str] = match kind {
        MemberKind::Property => &["__get", "__set"],
        MemberKind::Method => &["__call", "__callStatic"],
        MemberKind::ClassConstant | MemberKind::EnumCase => return None,
    };
    accessors.iter().find_map(|accessor| {
        view.member_location(class, MemberKind::Method, accessor)?
            .container
    })
}

/// What a rename at `offset` would change, which is not always what a search would look for.
///
/// One position differs, and it is the position where the two being the same destroyed code: a use
/// of `QuoteId` in a file that wrote `use ...\UUIDv7 as QuoteId;`. A *search* there is about the
/// class -- every use of it, however spelled, in every file. A *rename* there is about the alias:
/// the name at the cursor is `QuoteId`, it exists in this file and nowhere else, and the class is
/// not what the person is looking at. Renaming the class instead put the new name over `UUIDv7` at
/// thirty sites in six files -- the class's own declaration included -- while the editor's dialogue
/// promised to edit `"QuoteId"`.
/// The part of a member's family a rename can be refused on before the rest is walked: its own
/// declaration and every one above it.
///
/// Upwards is a header read per class and downwards is a pass over the class table per class, so
/// a rename that will be refused for a declaration above -- PHPUnit's `setUp()`, which every test
/// overrides -- spent 440 ms on Symfony walking the tests below it to say so. A family that
/// stops here, marked incomplete, for exactly that question; [`rename_target`] still answers the
/// whole. `None` for anything that is not a member.
pub fn rename_target_above(view: &IndexView<'_>, offset: TextSize) -> Option<ReferenceTarget> {
    let ReferenceTarget::Member {
        declared_by,
        kind,
        name,
    } = reference_target(view, offset)?
    else {
        return None;
    };
    Some(ReferenceTarget::MemberFamily {
        declarers: family::declared_above(view, &declared_by, kind, name.as_str()),
        kind,
        name,
        complete: false,
    })
}

pub fn rename_target(view: &IndexView<'_>, offset: TextSize) -> Option<ReferenceTarget> {
    let target = reference_target(view, offset)?;
    // A member is renamed across every declaration of it in its hierarchy, because PHP makes those
    // one contract: an implementation left answering the old name no longer satisfies the interface
    // that declared it. A search at the same position is about the one declaration.
    if let ReferenceTarget::Member {
        declared_by,
        kind,
        name,
    } = &target
    {
        let family = member_family(view, declared_by, *kind, name.as_str());
        return Some(ReferenceTarget::MemberFamily {
            declarers: family.declarers,
            kind: *kind,
            name: name.clone(),
            complete: family.complete,
        });
    }
    // A parameter callers can name is renamed at every call that names it, or the call stops with
    // `Unknown named parameter`. Its own declaration only: a rename asked in one method is about
    // that method, and an interface's spelling of the same parameter is the interface's.
    if let ReferenceTarget::Local { at, .. } = &target
        && let Some(parameter) = view.named_parameter_at(*at)
    {
        return Some(ReferenceTarget::Parameter(parameter));
    }
    // Only a class can be written under another name: a member is reached through its receiver and
    // a local is bound in the body, and neither goes through an import.
    let ReferenceTarget::Class(fqn) = target else {
        return Some(target);
    };

    Some(match view.alias_at(offset) {
        Some(alias) => ReferenceTarget::Alias { name: alias.name },
        None => ReferenceTarget::ClassUnderItsOwnName(fqn),
    })
}

/// A variable as something to search for: the local itself, or the property it promotes.
///
/// A promoted constructor parameter is a property as well, and the property is the half that leaves
/// the file. Answering `Local` there bounded the search to one function and left every
/// `$this->across` in the project behind.
fn local_target(view: &IndexView<'_>, name: Name, at: TextSize) -> ReferenceTarget {
    match view.promoted_property_owner(at, name.as_str()) {
        Some(declared_by) => ReferenceTarget::Member {
            declared_by,
            kind: MemberKind::Property,
            name,
        },
        None => ReferenceTarget::Local { name, at },
    }
}

/// Every place in the current file that reaches `target`.
///
/// Includes the declaration itself when it is in this file, because that is what an editor shows at
/// the top of the list and what a rename has to change too.
/// The `new` sites that run this member, when the member is a constructor.
///
/// Empty for everything else, which is what keeps the question off every other member's path: the
/// name is compared before the class table is asked anything.
fn constructions_reaching(
    view: &IndexView<'_>,
    declared_by: &str,
    name: &str,
) -> Vec<phrost_model::TextRange> {
    match name == "__construct" {
        true => view.constructions_of(declared_by),
        false => Vec::new(),
    }
}

pub fn references_in(view: &IndexView<'_>, target: &ReferenceTarget) -> Vec<TextRange> {
    let mut found = match target {
        // The calls, the `use function` lines that bind the short name, and the strings that name
        // it -- an autoload map, a `call_user_func('App\\alpha')`. The last is the only one that
        // ever worked, which is what made the gap read as a rename that edits a string and nothing
        // else.
        ReferenceTarget::Function(fqn) => {
            let mut uses = view
                .function_references()
                .iter()
                .filter(|reference| reference.resolved.as_ref() == Some(fqn))
                .map(|reference| reference.range)
                .collect::<Vec<_>>();
            uses.extend(view.function_imports_of(fqn.as_str()));
            uses.extend(view.string_class_references(fqn.as_str()));
            uses
        }
        // The uses, the `use const` lines, and the strings: `constant('App\\MAX')` and
        // `defined('App\\MAX')` name one without writing a name reference at all.
        ReferenceTarget::Constant(fqn) => {
            let mut uses = view.constant_references(fqn.as_str());
            uses.extend(view.string_class_references(fqn.as_str()));
            uses
        }
        ReferenceTarget::Class(fqn) => class_uses(view, fqn, Spelling::Any),
        // Every access of the name the same accessor serves, since there is no declaration to
        // match against.
        ReferenceTarget::MagicMember {
            answered_by,
            kind,
            name,
        } => view
            .member_accesses_named(name.as_str(), *kind)
            .iter()
            .filter(|access| {
                let MemberAccessOutcome::Dynamic { class } = &access.outcome else {
                    return false;
                };
                written_alike(access.kind, *kind)
                    && magic_accessor_owner(view, class.as_str(), *kind)
                        .is_some_and(|owner| owner == *answered_by)
            })
            .map(|access| access.range)
            .collect(),
        ReferenceTarget::ClassUnderItsOwnName(fqn) => class_uses(view, fqn, Spelling::ItsOwnName),
        ReferenceTarget::Member {
            declared_by,
            kind,
            name,
        } => {
            let mut uses = member_uses(view, declared_by.as_str(), *kind, name.as_str());
            // The constructor body's own `$across`, for a property some parameter promoted. It is
            // written as a local and reaches nothing a member sweep looks at, so a rename that
            // stopped here left the body reading a variable nothing binds any more.
            if *kind == MemberKind::Property {
                uses.extend(
                    view.promoted_parameter_occurrences(declared_by.as_str(), name.as_str()),
                );
                // And `new Money(amount: 5)`, which names the parameter declaring the property.
                uses.extend(view.named_arguments_to_promoting(declared_by.as_str(), name.as_str()));
            }
            // `use T { greet as welcome; }`, which names a method and which PHP checks when the
            // class is declared: a clause naming a method no used trait has is a fatal error, so a
            // rename that stepped over one produced a class that no longer loads.
            if *kind == MemberKind::Method {
                uses.extend(view.trait_adaptation_uses(declared_by.as_str(), name.as_str()));
                // And the spelling a constructor is nearly always reached by. `new Repo()` names
                // neither the member nor, for a subclass that declares none of its own, the class
                // that declares it -- so a member sweep finds the declaration and nothing else, and
                // a list of one reads as a constructor nobody uses. The call hierarchy has known
                // this all along through the same query; this is the other half of it.
                uses.extend(constructions_reaching(
                    view,
                    declared_by.as_str(),
                    name.as_str(),
                ));
            }
            uses
        }
        // Every declaration of one member, and every use reaching any of them. Deduplicated at the
        // end like everything else here, because two declarers in one hierarchy can both be reached
        // by one access written against the subtype.
        ReferenceTarget::MemberFamily {
            declarers,
            kind,
            name,
            ..
        } => {
            let mut uses = Vec::new();
            for declared_by in declarers {
                uses.extend(member_uses(
                    view,
                    declared_by.as_str(),
                    *kind,
                    name.as_str(),
                ));
                if *kind == MemberKind::Property {
                    uses.extend(
                        view.promoted_parameter_occurrences(declared_by.as_str(), name.as_str()),
                    );
                    uses.extend(
                        view.named_arguments_to_promoting(declared_by.as_str(), name.as_str()),
                    );
                }
                if *kind == MemberKind::Method {
                    uses.extend(view.trait_adaptation_uses(declared_by.as_str(), name.as_str()));
                    uses.extend(constructions_reaching(
                        view,
                        declared_by.as_str(),
                        name.as_str(),
                    ));
                }
            }
            uses
        }
        // Already complete, binding included: the body answers with every occurrence in the scope
        // that owns the name, which is the whole of where the variable exists.
        //
        // And the docblock above a parameter: `@param string $text` names it, and a rename that left
        // the tag spelling the old name would hand the project a `doc-param-name` finding.
        ReferenceTarget::Local { name, at } => {
            let mut found: Vec<TextRange> = view
                .local_occurrences(*at, name.as_str())
                .into_iter()
                .map(|occurrence| occurrence.range)
                .collect();
            let mentions = view.docblock_mentions(name.as_str(), &found);
            found.extend(mentions);
            found.sort_by_key(|range| range.start());
            return found;
        }
        // Also complete, the `use` line's own token included: the alias exists in this file and the
        // projection lists every place the file writes it.
        ReferenceTarget::Alias { name } => return view.alias_occurrences(name.as_str()),
        // What is between the quotes, the way a class named by a string is listed: the delimiters
        // are not the key. The declaration is not PHP and is not in this file -- the protocol layer
        // adds it from the catalog's own record.
        // The element's own key and every read of it, which is the declaration included.
        ReferenceTarget::ElementKey { key, at } => return view.built_key_occurrences(*at, key),
        // Where the declaration writes it and every read of it, which is all of where it is named in
        // PHP.
        ReferenceTarget::DeclaredInput { name, at } => {
            return view.declared_input_occurrences(*at, name);
        }
        // The clause is one of the occurrences, so there is no declaration to add.
        ReferenceTarget::TraitAlias(alias) => return view.trait_alias_occurrences(alias),
        // The parameter's occurrences where it is declared, as a local's, and the labels naming it
        // wherever it is called.
        ReferenceTarget::Parameter(parameter) => return view.parameter_uses(parameter),
        ReferenceTarget::CatalogKey { catalog, key } => {
            return view
                .catalog_uses()
                .into_iter()
                .filter(|used| used.catalog == *catalog && used.key == *key)
                .map(|used| match used.range.len() >= TextSize::from(2) {
                    true => TextRange::new(
                        used.range.start() + TextSize::from(1),
                        used.range.end() - TextSize::from(1),
                    ),
                    false => used.range,
                })
                .collect();
        }
    };

    found.extend(declaration_in(view, target));
    found.sort_by_key(|range| range.start());
    found.dedup();
    found
}

/// Which spellings of a class name count as reaching it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Spelling {
    /// Every one, which is what a use *is*: the import decided what the file may write.
    Any,
    /// Only the class's own short name -- the sites a rename may replace.
    ItsOwnName,
}

/// Uses of a class, matched by the declaration each one resolves to.
fn class_uses(view: &IndexView<'_>, fqn: &Name, spelling: Spelling) -> Vec<TextRange> {
    let short = phrost_model::short_name(fqn.as_str());
    view.name_references()
        .iter()
        .filter(|reference| reference.resolved.as_ref() == Some(fqn))
        .filter(|reference| match spelling {
            Spelling::Any => true,
            // Case-insensitively, because PHP's class names are.
            Spelling::ItsOwnName => {
                phrost_model::short_name(reference.path.as_str()).eq_ignore_ascii_case(short)
            }
        })
        .map(|reference| reference.range)
        .chain(
            // And the class named by a string, which holds no name reference to walk:
            // `class_exists('App\\Repo')`, a container definition, a Drupal annotation. Renamed
            // with everything else, because leaving it is how a rename half works -- and an alias
            // cannot reach one, so both spellings want the same list.
            view.string_class_references(fqn.as_str()),
        )
        .collect()
}

/// Uses of a member, matched by the declaration each one reaches.
///
/// Shared with the call hierarchy rather than copied for it: a method call resolves through its
/// receiver, and two answers in this codebase to what that means is two sets of results a reader
/// would have to reconcile.
pub(crate) fn member_uses(
    view: &IndexView<'_>,
    declared_by: &str,
    kind: MemberKind,
    name: &str,
) -> Vec<TextRange> {
    // By name before anything is resolved. A sweep asks this of every candidate file in the project,
    // and resolving every receiver in each of them to find the one access named `find` is the single
    // most expensive thing a request can do.
    // The class the access reached is not necessarily the one that declared the member. An inherited
    // method reached through a subclass is the same declaration, and a rename that missed those
    // would leave the project broken.
    // Remembered per class: a file reaches the member through one or two classes however many times
    // it is written, and asking where each access's member is declared is a member lookup with a
    // signature rendered -- `'account_id' => $this->author->account_id` three hundred times in one
    // Laravel job was 13 ms of a highlight once columns were members.
    let known = std::cell::RefCell::new(Vec::<(Box<str>, bool)>::new());
    let declares = |class: &str| {
        if let Some(&(_, answer)) = known.borrow().iter().find(|(seen, _)| &**seen == class) {
            return answer;
        }
        let answer = view
            .member_location(class, kind, name)
            .and_then(|location| location.container)
            .is_some_and(|container| container.as_str() == declared_by);
        known.borrow_mut().push((class.into(), answer));
        answer
    };
    let mut uses: Vec<TextRange> = view
        .member_accesses_named(name, kind)
        .iter()
        .filter(|access| {
            let MemberAccessOutcome::Resolved { class } = &access.outcome else {
                return false;
            };
            written_alike(access.kind, kind) && declares(class.as_str())
        })
        .map(|access| access.range)
        .collect();
    // And a framework's string naming a method of the class it is written in -- Drupal's
    // `'#submit' => ['::saveNote']`. Goto already followed it; a rename that did not edit it left
    // the form calling a method that no longer exists.
    if kind == MemberKind::Method {
        uses.extend(
            view.own_method_strings(name)
                .into_iter()
                .filter(|(_, class)| declares(class.as_str()))
                .map(|(range, _)| range),
        );
        // And PHP's own string spelling of a static callable, `'App\Repo::find'` -- what
        // `add_action()` and `usort()` are handed, and what goto has followed all along.
        uses.extend(
            view.static_method_strings(name)
                .into_iter()
                .filter(|(_, class)| declares(class.as_str()))
                .map(|(range, _)| range),
        );
        // And a property a framework reads through the method of the same name: Eloquent's
        // `$post->author` loads `author()`. Goto already lands on the method, so a rename that left
        // the property spelling behind turned every such read into a property nothing answers.
        uses.extend(
            view.member_accesses_named(name, MemberKind::Property)
                .iter()
                .filter(|access| {
                    let MemberAccessOutcome::Resolved { class } = &access.outcome else {
                        return false;
                    };
                    view.member_location(class.as_str(), MemberKind::Property, name)
                        .is_some_and(|location| {
                            location.kind == phrost_model::SymbolKind::Method
                                && location
                                    .container
                                    .is_some_and(|container| container.as_str() == declared_by)
                        })
                })
                .map(|access| access.range),
        );
    }
    uses
}

/// Whether two member kinds are written the same way at a use site.
///
/// A class constant and an enum case are read with identical syntax -- `Stage::Shipped` and
/// `Config::VERSION` differ only in what the declaration turns out to be -- so a search by syntax
/// cannot tell them apart, and comparing the *declared* kind against the *written* one matched
/// nothing at all. Every reference to every enum case in a project was missing, and the declaration
/// alone came back, which reads as a case nothing uses.
///
/// Which it really is still decides the answer: the declaration lookup below is asked with the
/// target's own kind, so a constant and a case of the same name on the same class stay apart.
fn written_alike(found: MemberKind, wanted: MemberKind) -> bool {
    if found == wanted {
        return true;
    }
    matches!(
        (found, wanted),
        (MemberKind::ClassConstant, MemberKind::EnumCase)
            | (MemberKind::EnumCase, MemberKind::ClassConstant)
    )
}

/// The target's own declaration, when it is written in this file.
fn declaration_in(view: &IndexView<'_>, target: &ReferenceTarget) -> Vec<TextRange> {
    let Some(current) = view.current_file_id() else {
        return Vec::new();
    };
    let located = match target {
        ReferenceTarget::Function(fqn) => vec![view.function_location(fqn.as_str())],
        ReferenceTarget::Constant(fqn) => vec![view.constant_location(fqn.as_str())],
        ReferenceTarget::Class(fqn) | ReferenceTarget::ClassUnderItsOwnName(fqn) => {
            vec![view.class_location(fqn.as_str())]
        }
        ReferenceTarget::Member {
            declared_by,
            kind,
            name,
        } => vec![view.member_location(declared_by.as_str(), *kind, name.as_str())],
        // One file can hold several of a family's declarations -- an interface and the class
        // implementing it are written in one file often enough -- so this is a list rather than the
        // first one found.
        ReferenceTarget::MemberFamily {
            declarers,
            kind,
            name,
            ..
        } => declarers
            .iter()
            .map(|declared_by| view.member_location(declared_by.as_str(), *kind, name.as_str()))
            .collect(),
        // A local's binding is one of the occurrences, and an alias's `use` line is too, so neither
        // has anything separate to add.
        ReferenceTarget::Local { .. }
        | ReferenceTarget::Alias { .. }
        | ReferenceTarget::ElementKey { .. }
        | ReferenceTarget::DeclaredInput { .. }
        | ReferenceTarget::TraitAlias(_)
        | ReferenceTarget::Parameter(_) => return Vec::new(),
        // Declared nowhere, which is the whole of what makes it magic.
        ReferenceTarget::MagicMember { .. } => return Vec::new(),
        // Declared in configuration, which is not a file this view is over.
        ReferenceTarget::CatalogKey { .. } => return Vec::new(),
    };

    located
        .into_iter()
        .flatten()
        .filter(|location| location.file == current)
        .map(|location| location.name_range)
        .collect()
}

fn member_kind(kind: phrost_model::SymbolKind) -> Option<MemberKind> {
    use phrost_model::SymbolKind;

    Some(match kind {
        SymbolKind::Method => MemberKind::Method,
        SymbolKind::Property => MemberKind::Property,
        SymbolKind::ClassConstant => MemberKind::ClassConstant,
        SymbolKind::EnumCase => MemberKind::EnumCase,
        _ => return None,
    })
}
