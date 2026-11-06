//! Writing the constructor a class does not have.
//!
//! The one refactor whose whole content is already in the file being edited: a constructor's
//! parameters are the class's own slots, so each takes the property's name and the property's type,
//! spelled exactly as the property spells it. Nothing is invented and nothing has to be resolved.
//!
//! A refactor and not a quick fix. Nothing is wrong with a class that fills its properties some
//! other way -- a factory, a `setUp`, an ORM hydrating them -- so this is offered and never
//! preferred, and a keystroke that applies the single fix must not reach it.
//!
//! Which properties, and which classes, is
//! [`constructible_at`](phrost_view::IndexView::constructible_at): it refuses one that already has a
//! constructor, and leaves out a property with a default, a static one, and a hooked one, each for a
//! reason written there.

use phrost_model::TextRange;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};

pub(super) struct GenerateConstructor;

impl Offer for GenerateConstructor {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::ClassDeclaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        // On the name, like the offer to implement what is missing: what is being added belongs to
        // the declaration, and a cursor in the comment above it is asking about the comment.
        let Some(class) = sites
            .declared_classes()
            .find(|class| concerns(class.name_range, at))
        else {
            return;
        };
        let Some(body) = view.class_body_for(class.name_range.start()) else {
            return;
        };
        // Two `__construct` in a class is a parse error, and this offer exists to write the first.
        if body.declares("__construct") {
            return;
        }
        // A property with a default is a memoised slot rather than something a caller supplies; a
        // static one belongs to the class and `$this->` cannot reach it; a hooked one is not a slot
        // at all, and assigning to it runs its `set` hook or fails.
        let filled: Vec<_> = body
            .properties
            .iter()
            .filter(|property| !property.has_default && !property.is_static && !property.has_hooks)
            .collect();
        if filled.is_empty() {
            return;
        }

        let mut written = String::from("\n\n");
        written.push_str(&body.indent);
        written.push_str("public function __construct(");
        written.push_str(
            &filled
                .iter()
                .map(|property| match &property.ty {
                    Some(ty) => format!("{ty} ${}", property.name),
                    // A property that declares no type gives its parameter none either. Inventing
                    // `mixed` would be writing a narrower claim than the class makes -- an untyped
                    // property accepts anything, including nothing being passed at all.
                    None => format!("${}", property.name),
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        written.push_str(")\n");
        written.push_str(&body.indent);
        written.push_str("{\n");
        for property in &filled {
            written.push_str(&body.indent);
            written.push_str(&body.step);
            written.push_str(&format!("$this->{0} = ${0};\n", property.name));
        }
        written.push_str(&body.indent);
        written.push('}');

        out.push(CodeAction {
            title: "Generate a constructor".to_owned(),
            // After the whole property block, which is where PHP is conventionally
            // written and where a reader looks for it.
            edits: vec![TextEdit::replace(
                TextRange::empty(body.after_properties),
                written,
            )],
            preferred: false,
            kind: ActionKind::Refactor,
            fixes: None,
        });
    }
}
