//! Lifting a literal into a class constant.
//!
//! The third extract, and the one with almost nothing to refuse: a constant is evaluated once at
//! class load and a literal has no side effects, so moving one changes neither when it runs nor how
//! often. What is left is where it goes and what it is called.
//!
//! Above the properties, which is where PSR-12 puts a constant and where a reader looks for one.
//! Named after what it says -- `'reduced payment'` becomes `REDUCED_PAYMENT` -- because a constant
//! whose name repeats its value is worth less than the literal was, and a number has no words to
//! take, so it becomes `VALUE` for somebody to rename.
//!
//! Only the one occurrence, and deliberately. Replacing every equal literal in the class is what a
//! person usually wants and sometimes emphatically does not: two `'id'` in one class are as likely
//! to be two different things as one, and a refactor that guesses wrong there has changed the
//! meaning of code nobody was looking at.

use phrost_view::ClassBody;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};

pub(super) struct ExtractConstant;

impl Offer for ExtractConstant {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Expression
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        let Some(literal) = view.literal_at(at) else {
            return;
        };
        // The class the literal is written inside, which is the one gaining the constant.
        let Some(class) = view
            .declared_classes()
            .into_iter()
            .find(|class| class.full_range.contains_range(literal))
        else {
            return;
        };
        let Some(body) = view.class_body_for(class.name_range.start()) else {
            return;
        };
        let written = view.text_in(literal).unwrap_or_default().to_owned();
        let name = unused(&named_after(&written), &body);

        out.push(CodeAction {
            title: format!("Extract {written} into a constant"),
            edits: vec![
                TextEdit::insert(
                    body.opens,
                    format!("\n{}private const {name} = {written};\n", body.indent),
                ),
                TextEdit::replace(literal, format!("self::{name}")),
            ],
            preferred: false,
            kind: ActionKind::Refactor,
            fixes: None,
        });
    }
}

/// The name a literal suggests, in the shape PHP writes a constant.
///
/// The words of a string, upper-cased and joined by underscores. A number has no words, so it gets
/// a name to be renamed -- which is honest: nothing in `4096` says what it is for, and a generated
/// `CONST_4096` would only look as though it did.
fn named_after(written: &str) -> String {
    let inside = written.trim_matches(['\'', '"']);
    let name: String = inside
        .chars()
        .map(|ch| match ch.is_ascii_alphanumeric() {
            true => ch.to_ascii_uppercase(),
            false => '_',
        })
        .collect();
    let name = name.trim_matches('_').replace("__", "_");
    match name.is_empty() || name.starts_with(|ch: char| ch.is_ascii_digit()) {
        true => "VALUE".to_owned(),
        false => name,
    }
}

/// The name, or the first numbered form of it the class does not already declare.
fn unused(wanted: &str, body: &ClassBody) -> String {
    let held =
        |name: &str| body.declares(name) || body.constants.iter().any(|held| held.as_str() == name);
    if !held(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|nth| format!("{wanted}_{nth}"))
        .find(|name| !held(name))
        .unwrap_or_else(|| wanted.to_owned())
}
