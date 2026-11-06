//! What a name written in a configuration file refers to.
//!
//! The other half of a framework project. A Drupal module's PHP is indexed and navigable; its YAML
//! declares the services, routes, permissions and links that PHP is reached *through*, and every one
//! of those files is full of names the index already holds. Measured on one real checkout: 1 414
//! references in 72 of the project's own YAML files.
//!
//! Nothing new is indexed for this. The catalogs are filled at bootstrap by whoever fills them, and
//! a class name is a class name -- what was missing is the statement of which key means which table,
//! which arrives from the framework seam as data.
//!
//! # Not a YAML parser
//!
//! A reference is one scalar on one line, under one key, in a file whose name says what kind it is.
//! That is the whole of what deciding this needs, and it is what a line scanner gives -- so a
//! configuration buffer costs a scan of the line the cursor is on rather than a parse of the file.
//! What a parser would buy is anchors, merge keys and multi-line scalars, and no reference in any of
//! these files is written with one.

use phrost_index::{ConfigTarget, Place};

use crate::{location::SymbolLocation, view::IndexView};

/// What a name in a configuration file turned out to be.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigReferent {
    /// A class, interface, trait or enum, and where it is declared.
    Declaration(Box<SymbolLocation>),
    /// A key of a catalog whose entries name a class -- a service id.
    ///
    /// Both halves, because a reader wants the class and a hover wants to say which id gave it.
    CatalogClass {
        class: Box<str>,
        location: Box<SymbolLocation>,
    },
    /// A key of a catalog whose entries are not classes: a route, a permission, a library.
    ///
    /// `place` is `None` for a key the framework *generates* rather than reads. Drupal derives
    /// `entity.node.canonical` from an entity type's link templates, so it is a real route with a
    /// real path and no line in any file -- and sending a reader to the entity class it came from
    /// would be sending them to something that is not the route. The description is what is left,
    /// and for a link template it is the path, which is the same line a route read from YAML shows.
    CatalogEntry {
        place: Option<Place>,
        detail: Box<str>,
    },
    /// A key of a **closed** catalog that holds no such key.
    ///
    /// Only a closed one, because only whoever fills a catalog knows whether a key missing from it
    /// does not exist: Drupal's container is built from a list of files that can be read in full, so
    /// an unregistered service id is a mistake -- while a route can be generated at runtime, and a
    /// name missing from that catalog may still be real. An open catalog answers nothing here, which
    /// is the same silence the PHP-side report already keeps.
    Unregistered { catalog: Box<str> },
}

/// One reference found in a configuration file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigReference {
    /// The name as written, without whatever prefix the rule recognised it by.
    pub name: Box<str>,
    /// Where the name is, as a byte range into the file's text -- the prefix excluded, so a jump and
    /// a report both underline the name rather than the `@` in front of it.
    pub range: std::ops::Range<usize>,
    pub referent: ConfigReferent,
}

impl IndexView<'_> {
    /// The reference at `offset` in a configuration file, if the position is on one.
    ///
    /// `file_name` rather than a path, because that is the whole of what decides which rules apply:
    /// a framework names a configuration file after the extension that owns it.
    pub fn config_reference_at(
        &self,
        file_name: &str,
        text: &str,
        offset: usize,
    ) -> Option<ConfigReference> {
        if self.reads_twig(file_name) {
            return self.twig_reference_at(text, offset);
        }
        let found = named_in(self.skeleton.returns(), file_name, text)
            .find(|named| named.range.contains(&offset) || named.range.end == offset)?;
        Some(ConfigReference {
            name: text[named_range(&found)].into(),
            range: found.range.clone(),
            referent: self.referent_of(found.target, &text[named_range(&found)])?,
        })
    }

    /// Whether any rule claims to read a file by this name.
    ///
    /// Asked before the file is opened, which is the whole point of it: a project's own
    /// configuration is 1 551 files on a real checkout and 53 of them are named by a rule, so
    /// reading them all to find out costs 54 ms of every start against 5.4 ms.
    pub fn covers_configuration(&self, file_name: &str) -> bool {
        self.reads_twig(file_name)
            || self
                .skeleton
                .returns()
                .config_references()
                .iter()
                .any(|rule| rule.covers(file_name))
    }

    /// Every name a configuration file writes that some rule says names code, with where it is.
    ///
    /// The third reader of the one walk, and the reason it is worth having: the other two answer
    /// about a buffer somebody has open, and this one is asked of every configuration file the
    /// project owns, once, while it is being indexed. A service's class is a *reference* to that
    /// class -- `references` should list it, a rename must rewrite it, and a class named only
    /// there is not dead code -- and none of that was reachable while the only way to read a
    /// configuration file was to open it.
    ///
    /// Class-like names only. A catalog key is a name in its own right and has its own table; what
    /// is wanted here is the ones that name a declaration, which is what a reference is.
    pub fn config_class_sites(&self, file_name: &str, text: &str) -> Vec<ConfigClassSite> {
        named_in(self.skeleton.returns(), file_name, text)
            .filter_map(|found| {
                let range = named_range(&found);
                let written = &text[range.clone()];
                // A callable is `Class::method`, and the class half is the reference. The method
                // half is a member reference and needs the class resolved to say which one, which
                // is a question for the reader rather than for a scanner.
                let (class, method) = match found.target {
                    ConfigTarget::Class => (written, None),
                    ConfigTarget::Callable => match written.split_once("::") {
                        Some((class, method)) => (class, Some(method)),
                        None => (written, None),
                    },
                    // A bare word under `factory` is a method of the service written beside it,
                    // so there is no class here to name -- the site says which member is called
                    // and leaves the owner to whoever resolves the id.
                    ConfigTarget::Factory => match written.split_once("::") {
                        Some((class, method)) => (class, Some(method)),
                        None if written.contains('\\') => (written, None),
                        None => ("", Some(written)),
                    },
                    _ => return None,
                };
                let at = |offset: usize, len: usize| {
                    Some(phrost_model::TextRange::at(
                        phrost_model::TextSize::from(u32::try_from(offset).ok()?),
                        phrost_model::TextSize::from(u32::try_from(len).ok()?),
                    ))
                };
                Some(ConfigClassSite {
                    name: class.trim_start_matches('\\').into(),
                    range: at(range.start, class.len())?,
                    member: match method {
                        // Past the class and the `::`, which is what makes this a range in the
                        // file rather than an offset into a name nobody else has. A bare factory
                        // method has no class before it, so the same arithmetic lands on the word.
                        Some(method) => {
                            Some((method.into(), at(range.end - method.len(), method.len())?))
                        }
                        None => None,
                    },
                })
            })
            .collect()
    }

    /// Every name in a configuration file that its own framework says should be there and is not.
    ///
    /// The judgement itself is not made here, for the reason [`catalog_uses`](Self::catalog_uses)
    /// gives: whether a miss is worth reporting depends on whether the catalog claims to hold every
    /// key there is, and that is the caller's rule -- the same rule, in the same words, that already
    /// decides it for a key written in PHP.
    pub fn config_problems(&self, file_name: &str, text: &str) -> Vec<ConfigProblem> {
        if self.reads_twig(file_name) {
            return self.twig_problems(text);
        }
        let contracts = self.skeleton.returns();
        let mut out = Vec::new();
        for found in named_in(contracts, file_name, text) {
            let range = named_range(&found);
            let name = &text[range.clone()];
            match found.target {
                ConfigTarget::CatalogKey(catalog) | ConfigTarget::CatalogKeys { catalog, .. } => {
                    let known = contracts.catalog_holds(catalog, name);
                    if known {
                        continue;
                    }
                    out.push(ConfigProblem {
                        range,
                        used: crate::catalog_uses::CatalogUse {
                            key: name.into(),
                            // The protocol wants a `TextRange`, and a configuration buffer is not in
                            // the index -- so the offsets are into the text handed in, which is what
                            // the caller has too.
                            range: phrost_model::TextRange::default(),
                            catalog: catalog.clone(),
                            known,
                            // What `closed` decides is whether a miss proves the line cannot run,
                            // and behind the optional mark it does not: the container passes null.
                            // So the miss is judged as an open catalog's -- a near miss is still a
                            // typo, and an absence is what the mark was written for.
                            closed: !found.optional && contracts.is_closed_catalog(catalog),
                            nearest: contracts.nearest_catalog_key(catalog, name).map(Into::into),
                        },
                    });
                }
                // A class is left to a later slice: measured on the reference project, 280 of 280
                // `class:` values name a class the index holds, so the report would be right and
                // silent -- and a module a checkout has configured but not installed is a state
                // nobody has measured yet.
                ConfigTarget::Class | ConfigTarget::Callable | ConfigTarget::Factory => {}
            }
        }
        out
    }

    /// What one name means, given what its rule said it would be.
    fn referent_of(&self, target: &ConfigTarget, name: &str) -> Option<ConfigReferent> {
        let contracts = self.skeleton.returns();
        match target {
            ConfigTarget::Class => self.declaration_named(name),
            // The class half is what a reader wants; the method, when one is written, is where they
            // want to land. A callable naming a class that is not there answers nothing, the same as
            // a class would.
            // A bare word under `factory` names a method of the service beside it, and this
            // scanner does not know which service that is -- so it is a reference the index holds
            // and not somewhere a reader can be sent, which is the honest answer rather than
            // resolving it as a class that does not exist.
            ConfigTarget::Factory if !name.contains("::") && !name.contains('\\') => None,
            ConfigTarget::Callable | ConfigTarget::Factory => match name.split_once("::") {
                Some((class, method)) => self
                    .member_location(qualified(class), phrost_sema::MemberKind::Method, method)
                    .map(|location| ConfigReferent::Declaration(Box::new(location)))
                    .or_else(|| self.declaration_named(class)),
                None => self.declaration_named(name),
            },
            ConfigTarget::CatalogKey(catalog) | ConfigTarget::CatalogKeys { catalog, .. } => {
                // A key that names a class first, because that is the one with somewhere to go.
                if let Some(class) = contracts.catalog_class(catalog, name) {
                    let class = class.to_owned();
                    if let Some(location) = self.class_location(&class) {
                        return Some(ConfigReferent::CatalogClass {
                            class: class.into(),
                            location: Box::new(location),
                        });
                    }
                }
                if contracts.catalog_holds(catalog, name) {
                    return Some(ConfigReferent::CatalogEntry {
                        place: contracts.catalog_placed_at(catalog, name).cloned(),
                        detail: contracts.catalog_detail(catalog, name).unwrap_or("").into(),
                    });
                }
                // Held by nothing. Only a closed catalog may say so.
                contracts
                    .is_closed_catalog(catalog)
                    .then(|| ConfigReferent::Unregistered {
                        catalog: catalog.clone(),
                    })
            }
        }
    }

    /// A class-like written the way a configuration file writes one, with or without the separator.
    fn declaration_named(&self, name: &str) -> Option<ConfigReferent> {
        self.class_location(qualified(name))
            .map(|location| ConfigReferent::Declaration(Box::new(location)))
    }
}

/// A class name the way the index is keyed by it: fully qualified, with no leading separator.
///
/// Both spellings are written in these files -- `\\Drupal\\node\\Controller\\NodeController` in a
/// route and `Drupal\\node\\NodeStorage` in a service -- and they name one class.
fn qualified(name: &str) -> &str {
    name.trim_start_matches('\\')
}

/// A name a configuration file writes that its catalog does not hold.
///
/// Carries a [`CatalogUse`](crate::CatalogUse) rather than a message, so the decision about what to
/// say -- and whether to say anything -- is the one already written for a key in PHP.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigProblem {
    /// Where the name is, as a byte range into the file's text.
    pub range: std::ops::Range<usize>,
    pub used: crate::catalog_uses::CatalogUse,
}

/// One name worth offering where a configuration value is being written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigCandidate {
    /// What gets inserted, as configuration writes it.
    pub name: Box<str>,
    /// What it names, which is what tells a reader they picked the right one: the class a service
    /// id builds, the URL a route serves, the title a permission is shown under. Empty for a key
    /// nothing describes, which is still a key worth offering.
    pub detail: Box<str>,
    pub kind: ConfigCandidateKind,
}

/// Which of the two things a configuration value can be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfigCandidateKind {
    /// A key of a catalog: a service id, a route name, a permission, a library.
    CatalogKey,
    /// A class-like, fully qualified.
    Class,
    /// A namespace a class path goes on into, written with its separator.
    Namespace,
    /// A method of the class a callable names, written as the whole `Class::method`.
    Method,
}

impl IndexView<'_> {
    /// What could be written at `offset` in a configuration buffer, and what it would replace.
    ///
    /// The ids are the whole point of this. A person writing `class:` copies the qualified name from
    /// the file they just wrote; nobody remembers which of 1 856 service ids builds the manager they
    /// want, and that is exactly what a list is for -- 745 of one real project's own YAML references
    /// are container ids, against 280 classes.
    ///
    /// `limit` is the caller's, because an empty prefix matches every key a catalog holds -- the same
    /// reason [`catalog_keys`](phrost_index::ReturnContracts::catalog_keys) takes one.
    pub fn config_completions(
        &self,
        file_name: &str,
        text: &str,
        offset: usize,
        limit: usize,
    ) -> Option<(std::ops::Range<usize>, Vec<ConfigCandidate>)> {
        if self.reads_twig(file_name) {
            return self.twig_name_completions(text, offset, limit);
        }
        let contracts = self.skeleton.returns();
        let (line, line_start) = line_around(text, offset);
        let key = key_of(text, line_start);
        let written = writing_at(line, offset - line_start)?;
        let typed = &line[written.start..offset - line_start];

        for rule in contracts.config_references() {
            if !rule.covers(file_name) || !rule.matches(key, typed) {
                continue;
            }
            let prefix = rule.name_in(typed);
            let mut from = line_start + written.start + (typed.len() - prefix.len());
            let mut to = line_start + written.end;
            // A scalar holding several names is narrowed to the one being written, the same way the
            // reference reader narrows it to the one being read.
            let prefix = match &rule.names {
                ConfigTarget::CatalogKeys { separators, .. } => {
                    let (piece, at) = piece_at(prefix, offset - from, separators)?;
                    from += at;
                    to = to.min(from + piece.len());
                    piece
                }
                _ => prefix,
            };
            let items = match &rule.names {
                ConfigTarget::CatalogKey(catalog) | ConfigTarget::CatalogKeys { catalog, .. } => {
                    contracts
                        .catalog_keys(catalog, prefix, limit)
                        .into_iter()
                        .map(|key| ConfigCandidate {
                            name: key.into(),
                            detail: contracts.catalog_detail(catalog, key).unwrap_or("").into(),
                            kind: ConfigCandidateKind::CatalogKey,
                        })
                        .collect()
                }
                ConfigTarget::Class => self.configured_classes(prefix, limit),
                // `Class::method`: the class until the `::` is written, a method of it after.
                ConfigTarget::Callable | ConfigTarget::Factory => match prefix.split_once("::") {
                    Some((class, method)) => self.configured_methods(class, method, limit),
                    None => self.configured_classes(prefix, limit),
                },
            };
            return Some((from..to, items));
        }
        None
    }
}

impl IndexView<'_> {
    /// The namespaces a written class path goes on into and the classes under it, spelled the way
    /// the value was -- with its leading separator when it has one.
    ///
    /// Fully qualified, as configuration always is: no file's imports apply to a YAML value, so
    /// the path is read from the root. A class here is one PHP can build, which is what each of
    /// these keys hands it -- a service, a controller, a form, a factory.
    fn configured_classes(&self, written: &str, limit: usize) -> Vec<ConfigCandidate> {
        let anchor = match written.starts_with('\\') {
            true => "\\",
            false => "",
        };
        let path = written.trim_start_matches('\\');
        let under = self.under(&path.to_ascii_lowercase());
        let mut found: Vec<ConfigCandidate> = self
            .spelled(under.namespaces, path, limit)
            .into_iter()
            .map(|namespace| ConfigCandidate {
                name: format!("{anchor}{namespace}\\").into(),
                detail: "".into(),
                kind: ConfigCandidateKind::Namespace,
            })
            .collect();
        found.extend(
            self.ranked_names(under.names, crate::NameKinds::ClassOnly, limit)
                .into_iter()
                .map(|candidate| ConfigCandidate {
                    name: format!(
                        "{anchor}{}",
                        candidate.fqn.as_str().trim_start_matches('\\')
                    )
                    .into(),
                    detail: "".into(),
                    kind: ConfigCandidateKind::Class,
                }),
        );
        found.truncate(limit);
        found
    }

    /// The public methods of the class written before `::`, as the whole value they complete.
    ///
    /// Public, because the framework calls them from outside the class. Matched the way a member
    /// list is, so `fi` offers `findAll` as readily as `find`.
    fn configured_methods(&self, class: &str, typed: &str, limit: usize) -> Vec<ConfigCandidate> {
        let Some(located) = self.class_location(class.trim_start_matches('\\')) else {
            return Vec::new();
        };
        let mut found: Vec<ConfigCandidate> = self
            .member_candidates(located.fqn.as_str(), phrost_model::Visibility::Public)
            .into_iter()
            .filter(|member| member.kind == phrost_sema::MemberKind::Method)
            .filter(|member| !member.name.as_str().starts_with("__"))
            .filter(|member| phrost_model::matches_loosely(member.name.as_str(), typed))
            .map(|member| ConfigCandidate {
                name: format!("{class}::{}", member.name).into(),
                detail: "".into(),
                kind: ConfigCandidateKind::Method,
            })
            .collect();
        found.truncate(limit);
        found
    }
}

/// The value being written at `at`, as a range into `line`.
///
/// Unlike [`scalar_at`], an *empty* one counts: `route_name: ` with the caret after the space is a
/// place a name goes, and it is where a person asks for the list. So this works from the value
/// region -- everything after the first `: ` or `- ` -- rather than from a scalar that has to exist.
///
/// Inside quotes the whole quoted run is the value, and that is not a nicety: a permission is
/// `administer warehouse`, so stopping at a space would make the prefix empty after the first word
/// and offer every permission the project has under the caret of somebody who has already typed
/// half of one.
fn writing_at(line: &str, at: usize) -> Option<std::ops::Range<usize>> {
    let value_from = match line.find(": ") {
        Some(colon) => colon + 2,
        None => match line.trim_end().ends_with(':') {
            true => line.len(),
            false => line.find("- ").map(|dash| dash + 2)?,
        },
    };
    if at < value_from || at > line.len() {
        return None;
    }
    if let Some(quoted) = quoted_around(line, value_from, at) {
        return Some(quoted);
    }

    let bytes = line.as_bytes();
    let mut from = at;
    while from > value_from && is_written_into_a_name(bytes[from - 1]) {
        from -= 1;
    }
    let mut to = at;
    while to < bytes.len() && is_written_into_a_name(bytes[to]) {
        to += 1;
    }
    Some(from..to)
}

/// The quoted run `at` sits inside, without its quotes, or nothing when it sits outside every one.
///
/// An unclosed quote counts and runs to the end of the line, which is what a value half typed is.
fn quoted_around(line: &str, from: usize, at: usize) -> Option<std::ops::Range<usize>> {
    let bytes = line.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        let quote = match bytes[index] {
            byte @ (b'\'' | b'"') => byte,
            _ => {
                index += 1;
                continue;
            }
        };
        let opens = index + 1;
        let closes = line[opens..]
            .find(char::from(quote))
            .map_or(line.len(), |found| opens + found);
        if (opens..=closes).contains(&at) {
            return Some(opens..closes);
        }
        index = closes.saturating_add(1).min(line.len());
    }
    None
}

/// Whether a byte is part of a name being written, rather than the punctuation around it.
///
/// The `@` is in, because it is how the id it opens is recognised and a caret typed after one is
/// still writing that id. Everything YAML uses to hold a value is out.
fn is_written_into_a_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'\\' | b'/' | b'-' | b':' | b'@')
}

/// A class-like name written in a configuration file, and where.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigClassSite {
    /// Fully qualified, with no leading separator -- the spelling the def-map keys on.
    pub name: Box<str>,
    /// The class's own bytes, so a rename replaces the name and not the `Class::method` around it.
    pub range: phrost_model::TextRange,
    /// The method half of a `Class::method`, and its own bytes.
    ///
    /// A route names its controller that way -- 145 of them on one real project -- so the method
    /// is a reference like the class is, and renaming it without this leaves a route pointing at
    /// a method that is gone. Two ranges rather than one for the same reason the class has its
    /// own: each half is renamed on its own.
    pub member: Option<(Box<str>, phrost_model::TextRange)>,
}

/// One name a rule recognised, and what the rule said it would be.
struct Named<'a> {
    target: &'a ConfigTarget,
    /// The name's own bytes in the file, trimmed of whatever the rule recognised it by.
    range: std::ops::Range<usize>,
    /// Written with the rule's optional mark, so that nothing registering it is not a mistake.
    optional: bool,
}

/// The line's own key, when it is written as a qualified name, as a range into the line.
///
/// The separator is the whole test. `app.provider:` is an id somebody chose and `Drupal\app\Foo:`
/// is a class, and a container written both ways in one file -- which is the normal state of a
/// Drupal project -- needs them told apart by what they say rather than by where they sit.
fn qualified_key(line: &str) -> Option<std::ops::Range<usize>> {
    let trimmed = line.trim_start();
    // Not a list item and not a value: a key with something after the colon is read by the rules.
    let (key, rest) = trimmed.split_once(':')?;
    if trimmed.starts_with('-') || !rest.trim().is_empty() || !key.contains('\\') {
        return None;
    }
    let key = key.trim_end();
    let start = line.len() - trimmed.len();
    (!key.is_empty() && !key.contains(' ')).then(|| start..start + key.len())
}

/// Where a found name is, as a range that can index the text.
fn named_range(found: &Named<'_>) -> std::ops::Range<usize> {
    found.range.clone()
}

/// Every name in the file that some rule claims, in source order.
///
/// One walk for both readers. Asking per position and asking for all of them are the same question
/// over a different span, and answering them separately is how the two come to disagree about what
/// counts as a name -- which for a scanner is the whole of what it does.
fn named_in<'a>(
    contracts: &'a phrost_index::ReturnContracts,
    file_name: &'a str,
    text: &'a str,
) -> impl Iterator<Item = Named<'a>> + 'a {
    let rules: Vec<_> = contracts
        .config_references()
        .iter()
        .filter(|rule| rule.covers(file_name))
        .collect();

    let mut at = 0;
    let mut lines = Vec::new();
    for line in text.split('\n') {
        lines.push((at, line));
        at += line.len() + 1;
    }

    let class_keyed = contracts.class_keys_cover(file_name);

    lines.into_iter().flat_map(move |(line_start, line)| {
        let key = key_of(text, line_start);
        let mut found = Vec::new();
        // A key that is itself a class, which is how a container written for autowiring names
        // one. Only a *qualified* key: an ordinary service id has no separator in it, and the
        // rules below match values and would never see this line at all.
        if class_keyed && let Some(at) = qualified_key(line) {
            found.push(Named {
                target: &ConfigTarget::Class,
                range: line_start + at.start..line_start + at.end,
                optional: false,
            });
        }
        for scalar in scalars_in(line) {
            let written = &line[scalar.clone()];
            let Some(rule) = rules.iter().find(|rule| rule.matches(key, written)) else {
                continue;
            };
            let name = rule.name_in(written);
            let optional = rule.may_be_absent(written);
            let from = line_start + scalar.start + (written.len() - name.len());
            match &rule.names {
                // Each side of the scalar is a name of its own: Drupal writes
                // `_permission: 'a+b'` for *either* and `'a,b'` for *both*.
                ConfigTarget::CatalogKeys { separators, .. } => {
                    let mut piece_from = 0;
                    for piece in name.split(|ch| separators.contains(ch)) {
                        let trimmed = piece.trim();
                        if !trimmed.is_empty() {
                            let offset = piece_from + (piece.len() - piece.trim_start().len());
                            found.push(Named {
                                target: &rule.names,
                                range: from + offset..from + offset + trimmed.len(),
                                optional,
                            });
                        }
                        piece_from += piece.len() + 1;
                    }
                }
                _ => {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() {
                        let offset = name.len() - name.trim_start().len();
                        found.push(Named {
                            target: &rule.names,
                            range: from + offset..from + offset + trimmed.len(),
                            optional,
                        });
                    }
                }
            }
        }
        found
    })
}

/// Every scalar the line holds, as ranges into it, with the quotes taken off.
///
/// The same reading [`scalar_at`] made at one position, made for the whole line: a framework's own
/// files write several on one, and `arguments: ['@a', '@b']` is 94 lines of one real project.
fn scalars_in(line: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = line.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'#' if index == 0 || bytes[index - 1].is_ascii_whitespace() => break,
            quote @ (b'\'' | b'"') => {
                let from = index + 1;
                let to = line[from..]
                    .find(char::from(quote))
                    .map_or(line.len(), |at| from + at);
                found.push(from..to);
                index = to.saturating_add(1).min(line.len());
            }
            byte if breaks_a_scalar(bytes, index, byte) => index += 1,
            _ => {
                let from = index;
                while index < bytes.len() && !breaks_a_scalar(bytes, index, bytes[index]) {
                    index += 1;
                }
                let after = line[index..].trim_start_matches(' ');
                let is_key = after.starts_with(':')
                    && after[1..].chars().next().is_none_or(char::is_whitespace);
                if !is_key {
                    found.push(from..index);
                }
            }
        }
    }
    found
}

/// The line `offset` is on, and where that line starts in the file.
fn line_around(text: &str, offset: usize) -> (&str, usize) {
    let offset = offset.min(text.len());
    let start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
    (&text[start..end], start)
}

/// The key a line's value is written under.
///
/// The line's own key when it has one (`class: Foo`), and otherwise the key of the block it is a
/// list item of -- `- '@id'` under `arguments:`. Nothing deeper: the rules are written against the
/// innermost key, which is what a person reading the file sees above the value.
fn key_of(text: &str, line_start: usize) -> &str {
    let line = &text[line_start
        ..text[line_start..]
            .find('\n')
            .map_or(text.len(), |at| line_start + at)];
    let trimmed = line.trim_start();
    if !trimmed.starts_with('-')
        && let Some(key) = trimmed.split_once(':').map(|(key, _)| key.trim())
        && !key.is_empty()
        && !key.contains(' ')
    {
        return key;
    }

    // A list item, or a line with no key of its own. The nearest line above it that is less
    // indented and does have one governs it.
    let indent = line.len() - trimmed.len();
    let mut at = line_start;
    while at > 0 {
        let previous_end = at - 1;
        let previous_start = text[..previous_end]
            .rfind('\n')
            .map_or(0, |found| found + 1);
        let previous = &text[previous_start..previous_end];
        let content = previous.trim_start();
        let previous_indent = previous.len() - content.len();
        if !content.is_empty()
            && previous_indent < indent
            && let Some((key, _)) = content.split_once(':')
        {
            return key.trim().trim_start_matches("- ").trim();
        }
        at = previous_start;
    }
    ""
}

/// Whether the byte at `index` ends a bare scalar.
///
/// YAML's own delimiters, plus the `-` of a block list item -- which is only a delimiter when a
/// space follows it, or `-3` would be two scalars. A `:` is one only when a space or the end of the
/// line follows, which is what keeps `Drupal\Foo::bar` whole.
fn breaks_a_scalar(bytes: &[u8], index: usize, byte: u8) -> bool {
    match byte {
        b' ' | b'\t' | b'[' | b']' | b'{' | b'}' | b',' | b'\'' | b'"' | b'!' | b'&' | b'*' => true,
        b'-' => bytes.get(index + 1).is_none_or(|next| *next == b' '),
        b':' => bytes
            .get(index + 1)
            .is_none_or(|next| next.is_ascii_whitespace()),
        _ => false,
    }
}

/// The piece of a multi-name scalar that `at` is inside, and where it starts.
fn piece_at<'a>(value: &'a str, at: usize, separators: &str) -> Option<(&'a str, usize)> {
    let mut from = 0;
    for (index, ch) in value.char_indices() {
        if !separators.contains(ch) {
            continue;
        }
        if at <= index {
            return Some((&value[from..index], from));
        }
        from = index + ch.len_utf8();
    }
    (at <= value.len()).then(|| (&value[from..], from))
}
