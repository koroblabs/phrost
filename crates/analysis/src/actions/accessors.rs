//! The getters and setters a class's properties do not have.
//!
//! Two offers rather than one, because they are two decisions: a value object wants readers and
//! nothing else, and adding a writer to it is the opposite of what it is for. So a person picks,
//! and neither is preferred -- a property without an accessor is not a defect.
//!
//! Only what is missing. A class that already declares `getTitle()` is not offered a second, which
//! is a parse error, and the count in the title is what will actually be written.
//!
//! # The name a getter takes
//!
//! `get` and the property's name, except for a `bool`, where PHP writes `isActive()` and nobody
//! writes `getActive()`. And a property already named for the question it answers -- `$isActive`,
//! `$hasRows` -- keeps its own name, because `isIsActive()` is not a method anybody meant.
//!
//! # What has no accessor to generate
//!
//! A `readonly` property gets a getter and never a setter: PHP refuses the assignment outside the
//! constructor, so the setter would be a method that cannot run. A hooked property is not a slot --
//! `get` and `set` are already written on it. And a static one belongs to the class rather than to
//! an instance, so `$this->` cannot reach it.

use phrost_view::{ClassBody, DeclaredProperty};

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};

pub(super) struct GenerateAccessors;

/// Which of the two an offer writes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Accessor {
    Reads,
    Writes,
}

impl Offer for GenerateAccessors {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::ClassDeclaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        let Some(class) = sites
            .declared_classes()
            .find(|class| concerns(class.name_range, at))
        else {
            return;
        };
        let Some(body) = view.class_body_for(class.name_range.start()) else {
            return;
        };

        for accessor in [Accessor::Reads, Accessor::Writes] {
            let wanted: Vec<_> = body
                .properties
                .iter()
                .filter(|property| accessor.suits(property))
                .filter(|property| !body.declares(&accessor.named(property)))
                .collect();
            if wanted.is_empty() {
                continue;
            }

            let mut written = String::new();
            for property in &wanted {
                written.push_str("\n\n");
                written.push_str(&body.indent);
                written.push_str(&accessor.written(&body, property));
            }
            out.push(CodeAction {
                // Named for what it writes and how much of it: a menu of "Generate accessors"
                // twice is a menu nobody can use, and a person choosing needs to know whether this
                // touches the one property they are looking at or all eight.
                title: match wanted.as_slice() {
                    [only] => format!(
                        "Generate a {} for ${}",
                        match accessor {
                            Accessor::Reads => "getter",
                            Accessor::Writes => "setter",
                        },
                        only.name
                    ),
                    many => format!(
                        "Generate {} for {} properties",
                        match accessor {
                            Accessor::Reads => "getters",
                            Accessor::Writes => "setters",
                        },
                        many.len()
                    ),
                },
                edits: vec![TextEdit::insert(body.at, written)],
                preferred: false,
                kind: ActionKind::Refactor,
                fixes: None,
            });
        }
    }
}

impl Accessor {
    /// Whether a property can have this accessor at all.
    fn suits(self, property: &DeclaredProperty) -> bool {
        if property.is_static || property.has_hooks {
            return false;
        }
        match self {
            Self::Reads => true,
            // PHP refuses the assignment outside the constructor, so a setter here is a method that
            // cannot run.
            Self::Writes => !property.is_readonly,
        }
    }

    /// The method's name.
    fn named(self, property: &DeclaredProperty) -> String {
        let name = property.name.as_str();
        match self {
            Self::Writes => format!("set{}", capitalised(name)),
            Self::Reads => match property.ty.as_deref() {
                // A property already named for the question it answers keeps its own name: PHP
                // writes `isActive()` and `hasRows()`, and `isIsActive()` is not a method anybody
                // meant.
                Some("bool") if asks_a_question(name) => name.to_owned(),
                Some("bool") => format!("is{}", capitalised(name)),
                _ => format!("get{}", capitalised(name)),
            },
        }
    }

    /// The whole declaration, its braces and its one statement.
    fn written(self, body: &ClassBody, property: &DeclaredProperty) -> String {
        let name = property.name.as_str();
        let method = self.named(property);
        let (signature, statement) = match self {
            Self::Reads => (
                match &property.ty {
                    Some(ty) => format!("{method}(): {ty}"),
                    // A property that declares no type gives its getter no return type. Inventing
                    // `mixed` would be a claim the class does not make.
                    None => format!("{method}()"),
                },
                format!("return $this->{name};"),
            ),
            Self::Writes => (
                match &property.ty {
                    Some(ty) => format!("{method}({ty} ${name}): void"),
                    None => format!("{method}(${name}): void"),
                },
                format!("$this->{name} = ${name};"),
            ),
        };
        format!(
            "public function {signature}\n{0}{{\n{0}{1}{statement}\n{0}}}",
            body.indent, body.step
        )
    }
}

/// A name whose first word already asks something, which a getter's prefix would ask twice.
fn asks_a_question(name: &str) -> bool {
    ["is", "has", "can", "should"].iter().any(|word| {
        name.strip_prefix(word)
            .is_some_and(|rest| rest.starts_with(|ch: char| ch.is_ascii_uppercase()))
    })
}

/// The name with its first character upper-cased, which is how PHP writes an accessor.
fn capitalised(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}
