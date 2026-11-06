//! Docblock annotations, and what each one would be written as instead.
//!
//! Two halves of one question. The declarations a Doctrine-style annotation can sit on are the
//! class-likes in the open file, which nothing else here projects -- [`documented_declarations`] is
//! about function-likes, because everything else that reads a docblock is about parameters and
//! returns. And the class the annotation would become is a search of the project, because the
//! framework does not tabulate its plugin types: it puts the attribute one namespace segment along
//! from the annotation and lets the name do the rest.
//!
//! What is *in* the docblock is nobody's business here. The offer that reads this has to take the
//! annotation's arguments apart to rewrite them, so finding the tag is the same pass, and a
//! projection that also found it would be a second reader of one syntax.
//!
//! [`documented_declarations`]: IndexView::documented_declarations

use phrost_model::{Name, TextRange, TextSize, short_name};

use crate::{documentation::Docblock, view::IndexView};

/// A class-like declaration in the open file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredClass {
    /// Fully qualified, as the index is keyed by it.
    pub fqn: Name,
    /// The name alone, which is what a request about the declaration is at.
    pub name_range: TextRange,
    /// The whole declaration, `class` keyword to closing brace.
    pub full_range: TextRange,
    /// The comment above it, `/**` and `*/` included, and where it is. `None` for one with none.
    pub docblock: Option<Docblock>,
    /// The declaration's own indentation, so anything written above it lines up with it.
    pub indent: String,
    /// Where the last thing inside the body ends, which is where another member goes after it.
    ///
    /// `None` for a body with nothing in it, whose one position is just past the opening brace --
    /// and which the caller finds from [`Self::full_range`] rather than being told, because an
    /// empty body's brace is the only thing in it.
    pub last_member_ends: Option<TextSize>,
    /// What the body's own members are indented by, read off the first of them.
    ///
    /// The fact rather than the ingredients, and it is worth carrying because nothing else can
    /// supply it: no configuration says what a project indents with, and guessing from the language
    /// gives PSR-12's four spaces to a Drupal file that uses two. `None` for a body with no member
    /// to read, where a caller has only the guess.
    pub member_indent: Option<String>,
}

/// How far down the ranked search to look for the attribute an annotation names.
///
/// The same number [`ImportClass`](../../phrost_analysis/actions/index.html) scans, and for the same
/// reason: the search is by prefix, so asking for `Block` also returns `BlockContent`,
/// `BlockManager` and everything else that starts with it.
const SCANNED: usize = 200;

impl IndexView<'_> {
    /// Every class-like in the open file, in source order.
    ///
    /// Anonymous classes left out: `new class { ... }` has no name to write an attribute above and
    /// no name anything could resolve.
    pub fn declared_classes(&self) -> Vec<DeclaredClass> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let mut found: Vec<DeclaredClass> = hir
            .classes()
            .filter(|(_, class)| !class.is_anonymous)
            .filter_map(|(id, class)| {
                Some(DeclaredClass {
                    // The declared symbol rather than the written name, for the reason
                    // `class_body_at` gives: the written one is the short name and the catalog is
                    // keyed by the qualified one. Reading `class.name` here made every lookup miss
                    // and the offer that used it silently do nothing.
                    fqn: open
                        .scope()
                        .symbol_of(phrost_defmap::DeclarationSource::Class(id))?
                        .fqn
                        .clone(),
                    name_range: class.name_range,
                    full_range: class.full_range,
                    docblock: class
                        .docblock
                        .map(|id| hir.docblock(id))
                        .map(|data| Docblock {
                            text: data.text.clone(),
                            range: data.range,
                        }),
                    indent: self.indentation_at(class.full_range.start()),
                    last_member_ends: hir
                        .member_list(class.members)
                        .iter()
                        .map(|&id| hir.member(id).full_range.end())
                        .max(),
                    member_indent: hir
                        .member_list(class.members)
                        .iter()
                        .map(|&id| hir.member(id).full_range.start())
                        .min()
                        .map(|at| self.indentation_at(at)),
                })
            })
            .collect();

        found.sort_by_key(|class| class.name_range.start());
        found
    }

    /// The attribute class an annotation of this name would become.
    ///
    /// A search rather than a table, because the framework's own answer is a search: the attribute
    /// class has the annotation's short name and sits in the namespace segment the framework named.
    /// `None` when no framework said which segment that is, and when nothing in the index is called
    /// that -- which is the whole gate on offering the conversion at all, since a docblock tag that
    /// names no class is prose.
    pub fn attribute_for_annotation(&self, annotation: &str) -> Option<Name> {
        let segment = self.skeleton.returns().attribute_namespace()?;
        self.search_symbols(annotation, SCANNED)
            .into_iter()
            .filter(|location| location.kind.is_class_like())
            .filter(|location| {
                // The search matches a prefix; only the exact short name is the same annotation.
                short_name(location.fqn.as_str()).eq_ignore_ascii_case(annotation)
            })
            .find(|location| namespace_segment(location.fqn.as_str()) == Some(segment))
            .map(|location| location.fqn)
    }

    /// The class a nested annotation constructs, for one written as an argument's value.
    ///
    /// `@Translation("Content")` is not an attribute and never was -- it is an object spelled in a
    /// docblock -- so it comes from what the framework stated rather than from the search above.
    pub fn class_for_annotation_value(&self, annotation: &str) -> Option<&str> {
        self.skeleton.returns().class_for_annotation(annotation)
    }
}

/// The segment before the last one: `Drupal\rest\Attribute\RestResource` -> `Attribute`.
fn namespace_segment(fqn: &str) -> Option<&str> {
    let namespace = fqn.rsplit_once('\\')?.0;
    Some(
        namespace
            .rsplit_once('\\')
            .map_or(namespace, |(_, last)| last),
    )
}
