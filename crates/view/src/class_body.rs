//! A class's own body, as something that writes into it sees it.
//!
//! Three offers generate a member -- a constructor, an accessor, and the methods a supertype
//! requires -- and each needs the same four things: what the class declares, what it already has a
//! method for, where the next member goes, and what the body is indented by. One projection because
//! they are one question: a second reader of a class body is a second answer to "does it already
//! declare that", and the two would disagree the day one of them learned about a trait.
//!
//! A position query rather than a field on every class in the file, for the reason
//! [`documentable_at`](IndexView::documentable_at) is one: it renders a type per property, and doing
//! that for every class on every code-action request is work thrown away on the classes the cursor
//! is nowhere near.
//!
//! What is *reported* and what is *decided* are kept apart. A static property is here, and so is one
//! with a default and one with hooks, because which of those a generator may touch differs by
//! generator: a constructor takes neither a defaulted property nor a hooked one, and an accessor is
//! worth having for a defaulted one. Only what no generator can use -- a property PHP would not let
//! either of them reach -- is filtered, and that is nothing, so nothing is.

use phrost_hir::MemberKind;
use phrost_model::{Name, TextSize, Visibility};

use crate::view::IndexView;

/// One property the class declares itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredProperty {
    /// Without the leading `$`, as names are stored everywhere else.
    pub name: Name,
    /// The type as the property spells it. `None` for one that declares none.
    ///
    /// As written rather than qualified, which is the opposite of what a stub for an *inherited*
    /// method needs: a member generated from this is written into the same file as the property it
    /// copies, where the file's own imports already make a short name mean what it says.
    pub ty: Option<String>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_readonly: bool,
    pub has_hooks: bool,
    /// Whether the declaration writes a value for itself.
    ///
    /// Read from the text, because the HIR keeps that a property *is* declared and not what it was
    /// declared as: a default is an expression, and expressions live with bodies.
    pub has_default: bool,
}

/// A class body, and everything writing a member into it needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassBody {
    /// Just inside the opening brace, which is where a constant goes: PSR-12 puts them above the
    /// properties, and a reader looks for them there.
    ///
    /// Found from the text rather than stored, because between a class's name and its body PHP
    /// writes only `extends`, `implements` and the names they take -- none of which can hold a
    /// brace. One place that knows it, since three offers would each have written the scan.
    pub opens: TextSize,
    /// Where a member goes at the end: after the last thing in the body.
    pub at: TextSize,
    /// Where one goes after the property block, which is where PHP conventionally writes a
    /// constructor and where a reader looks for it. The same as [`Self::at`] for a body that is all
    /// properties.
    pub after_properties: TextSize,
    /// What the body's own members are indented by.
    pub indent: String,
    /// One step further in, for what goes inside a member's braces.
    ///
    /// The difference between the body's indentation and the class's, which is the only place a
    /// file's step can be read from: nothing configures it, and guessing from the language writes
    /// PSR-12's four spaces into a Drupal file that uses two.
    pub step: String,
    /// In the order the class declares them, which is the order a caller will read.
    pub properties: Vec<DeclaredProperty>,
    /// The constants the class declares itself, for a generator that adds one.
    pub constants: Vec<Name>,
    /// The methods the class declares itself, folded, for asking whether one is already there.
    ///
    /// Its own declarations and not the flattened table: a generator writes into *this* class, and a
    /// method it inherits is one it may legitimately override.
    pub methods: Vec<Name>,
}

impl ClassBody {
    /// Whether the class already declares a method of this name.
    ///
    /// Case-insensitively, which is how PHP compares a method name -- so a class with `getTitle` is
    /// not offered a `gettitle`.
    pub fn declares(&self, method: &str) -> bool {
        self.methods
            .iter()
            .any(|name| name.as_str().eq_ignore_ascii_case(method))
    }
}

impl IndexView<'_> {
    /// The body of the class whose name `offset` is on.
    pub fn class_body_for(&self, offset: TextSize) -> Option<ClassBody> {
        let open = self.current()?;
        let hir = open.hir();
        let text = open.text();

        let (_, class) = hir
            .classes()
            .filter(|(_, class)| !class.is_anonymous)
            .find(|(_, class)| class.name_range.contains_inclusive(offset))?;

        let members = hir.member_list(class.members);
        let mut properties = Vec::new();
        let mut methods = Vec::new();
        let mut constants = Vec::new();
        let mut after_properties = None;
        let mut at = None;

        for &id in members {
            let member = hir.member(id);
            at = Some(member.full_range.end());
            match member.kind {
                MemberKind::Method => methods.push(member.name.clone()),
                MemberKind::ClassConstant | MemberKind::EnumCase => {
                    constants.push(member.name.clone());
                }
                MemberKind::Property => {
                    after_properties = Some(member.full_range.end());
                    properties.push(DeclaredProperty {
                        name: member.name.clone(),
                        ty: member
                            .type_ref
                            .map(|written| phrost_hir::render::render_type_ref(hir, written)),
                        visibility: member.visibility,
                        is_static: member.modifiers.is_static(),
                        is_readonly: member.modifiers.is_readonly(),
                        has_hooks: member.modifiers.hooks.are_written(),
                        has_default: text
                            .get(
                                usize::from(member.full_range.start())
                                    ..usize::from(member.full_range.end()),
                            )
                            .is_some_and(|written| written.contains('=')),
                    });
                }
            }
        }

        let opens = {
            let from = usize::from(class.name_range.end());
            let brace = text.get(from..)?.find('{')?;
            TextSize::try_from(from + brace + 1).ok()?
        };
        // A body with nothing in it: the one position it has is the one it just found.
        let at = at.unwrap_or(opens);
        let outer = self.indentation_at(class.full_range.start());
        // A body with no member to read has no indentation of its own yet, so the first thing
        // written into it takes the class's plus a step -- and the step is then the language's,
        // because the file has given no other. Falling back to the class's own would put a member
        // at the same depth as the `class` keyword.
        let indent = match members.first() {
            Some(&id) => self.indentation_at(hir.member(id).full_range.start()),
            None => format!(
                "{outer}{}",
                match outer.contains('\t') {
                    true => "\t",
                    false => "    ",
                }
            ),
        };
        Some(ClassBody {
            opens,
            at,
            after_properties: after_properties.unwrap_or(at),
            step: indent
                .strip_prefix(&outer)
                .filter(|step| !step.is_empty())
                .map_or_else(
                    // A body indented in some way the class's own indentation is not a prefix of.
                    // Nothing better is available, so the language's own step it is.
                    || match indent.contains('\t') {
                        true => "\t".to_owned(),
                        false => "    ".to_owned(),
                    },
                    str::to_owned,
                ),
            indent,
            properties,
            constants,
            methods,
        })
    }
}

impl IndexView<'_> {
    /// The literal at `at`, when the position is on one.
    ///
    /// Here rather than reached through `phrost_body` at the feature layer, so a feature keeps
    /// asking one crate: the view is the only index API a feature may see, and a body function
    /// reached around it is the edge the dependency rule exists to keep.
    pub fn literal_at(&self, at: phrost_model::TextRange) -> Option<phrost_model::TextRange> {
        phrost_body::literal_at(self.current()?.hir(), at)
    }
}
