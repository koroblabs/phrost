//! Writing the methods a class's supertypes require and it does not declare.
//!
//! `class Foo implements Bar {}` where nothing declares `Bar::baz()` is a fatal error at *class
//! load*, before any of the file runs -- so this is not a tidying action, it is the repair for a
//! finding the checker already makes. What is missing is
//! [`unimplemented_methods`](phrost_view::IndexView::unimplemented_methods), which owns every
//! judgement about *whether* a method is required; what each one looks like is
//! [`Layout::stub`](crate::stub::Layout), the same writer that fills a completion when somebody
//! types the name.
//!
//! # An empty body, on purpose
//!
//! Nothing is invented inside the braces. The class loads afterwards, and a method that has to
//! return something is then reported by `missing-return` -- which is the honest trade: a class-load
//! fatal becomes a list of bodies to write, each pointed at by the checker. A generated
//! `throw new \RuntimeException('Not implemented')` would take the second report away and leave the
//! code lying about itself.
//!
//! # What it may leave behind
//!
//! A method whose declaration cannot be written back exactly is not offered -- a **promoted**
//! parameter, which declares a property as well and cannot be reproduced from a signature. So the
//! title counts what will be written against what is missing, and says so when the two differ. Half
//! a repair with the rest still reported beats no repair at all, and it beats a stub that looks
//! right and is not.
//!
//! A parameter *default* used to be the other half of that refusal, and was the half that bit: the
//! index kept that there was one and not what it was, so this skipped every such method -- 21 % of
//! the reference project's own interfaces and 30 % of Drupal core's. The value is stored now.

use phrost_model::{TextRange, TextSize};
use phrost_view::DeclaredClass;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};
use crate::{diagnostics::unimplemented::UNIMPLEMENTED_METHOD, stub::Layout};

pub(super) struct ImplementRequired;

impl Offer for ImplementRequired {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::ClassDeclaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        // On the name and not in the docblock: what is being repaired is the declaration, and a
        // cursor in the comment above it is asking about the comment.
        let Some(class) = sites
            .declared_classes()
            .find(|class| concerns(class.name_range, at))
        else {
            return;
        };
        let Some(unmet) = view
            .unimplemented_methods()
            .into_iter()
            .find(|found| found.range == class.name_range)
        else {
            return;
        };

        let required: Vec<&str> = unmet.missing.iter().map(|one| one.name.as_str()).collect();
        let writable = view.overridable_methods(class.fqn.as_str(), |name| {
            required
                .iter()
                .any(|missing| missing.eq_ignore_ascii_case(name))
        });
        let writable: Vec<_> = writable
            .into_iter()
            .filter(|method| method.is_required)
            .collect();
        if writable.is_empty() {
            return;
        }

        let Some((range, text)) = Self::written(sites, class, &writable) else {
            return;
        };
        out.push(CodeAction {
            title: match writable.len() == unmet.missing.len() {
                true => format!(
                    "Implement the {} {} requires",
                    counted(writable.len(), "method"),
                    phrost_model::short_name(unmet.class.as_str())
                ),
                // Said out loud rather than silently doing less: the class will still not load, and
                // a person who was told "implement 4" and got 3 has no way to know why.
                false => format!(
                    "Implement {} of the {} {} requires",
                    writable.len(),
                    counted(unmet.missing.len(), "method"),
                    phrost_model::short_name(unmet.class.as_str())
                ),
            },
            edits: vec![TextEdit::replace(range, text)],
            // One answer, and it repairs a finding. Nothing to choose between.
            preferred: true,
            kind: ActionKind::QuickFix,
            fixes: Some(UNIMPLEMENTED_METHOD),
        });
    }
}

impl ImplementRequired {
    /// Where the stubs go and what they say.
    ///
    /// After the last member when there is one, and just inside the brace when there is not -- which
    /// is the one position an empty body has. The brace is found from the text rather than stored,
    /// because between a class's name and its body PHP writes only `extends`, `implements` and the
    /// names they take, none of which can hold one.
    fn written(
        sites: &SubjectSites<'_, '_>,
        class: &DeclaredClass,
        writable: &[phrost_view::OverridableMethod],
    ) -> Option<(TextRange, String)> {
        let text = sites.view().current_text()?;
        let body = sites.view().class_body_for(class.name_range.start())?;

        // The body's own indentation where there is a member to read it from, and the language's
        // otherwise. Nothing configures it, and guessing gives PSR-12's four spaces to a Drupal file
        // that uses two.
        let inside = body.indent.clone();
        let layout = Layout::inside(&inside);

        let mut written = String::new();
        for method in writable {
            written.push('\n');
            written.push('\n');
            written.push_str(&inside);
            // Trailing whitespace trimmed off each line. The shared writer leaves the body line as
            // indentation with nothing after it, because for a completion that is where the cursor
            // lands -- and an action has no cursor, so what it would leave behind is a blank line
            // full of spaces, which every PHP standard there is reports.
            let (stub, _) = layout.stub(method);
            written.push_str(
                &stub
                    .lines()
                    .map(str::trim_end)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        // After the last member, or -- for a body with nothing in it -- in place of whatever
        // whitespace separates its two braces. The second is a replacement rather than an insertion
        // so that `{}` and `{\n}` come out the same: inserting after the opening brace of `{}`
        // leaves the class's own `}` on the last stub's line.
        match class.last_member_ends {
            Some(end) => Some((TextRange::empty(end), written)),
            None => {
                let closes = text[..usize::from(class.full_range.end())].rfind('}')?;
                let opens = text[..closes].rfind(|ch: char| !ch.is_whitespace())? + 1;
                written.push('\n');
                written.push_str(&class.indent);
                Some((
                    TextRange::new(
                        TextSize::try_from(opens).ok()?,
                        TextSize::try_from(closes).ok()?,
                    ),
                    written,
                ))
            }
        }
    }
}

/// `1 method`, `4 methods`.
fn counted(many: usize, what: &str) -> String {
    match many {
        1 => format!("1 {what}"),
        _ => format!("{many} {what}s"),
    }
}
