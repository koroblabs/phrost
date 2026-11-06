//! What else may sit in an array whose type key names a class.
//!
//! `['#type' => 'textfield', '#ma|']`. Nothing in the project declares `#maxlength` -- it is not a
//! property, not a constant, not an argument -- and the only statement anywhere that it exists is a
//! bullet in the docblock of the class `textfield` names. That is a framework's convention, so what
//! a bullet looks like and which key chooses the class both come from the seam; what is here is the
//! walk that puts the two together.
//!
//! Read at the request rather than indexed. The list is a few dozen lines of prose per class on a
//! hierarchy four deep, wanted only while somebody is typing a key inside one particular array --
//! so holding it for every element in the project would be memory spent on a question almost never
//! asked, and reading four docblocks when it is asked costs nothing.

use phrost_index::DocumentedKey;
use phrost_model::{Name, TextRange, TextSize};

use crate::{resolve::HIERARCHY_DEPTH, view::IndexView};

/// What the array around a cursor accepts, and what it already holds.
///
/// One projection rather than three, because all four parts come out of the same walk and a caller
/// that asked for them separately would walk the file's expressions once per part.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedKeys {
    /// What an accepted item replaces: the text inside the quotes being typed.
    pub replace: TextRange,
    /// The class whose documentation these came from, for a resolve arriving after the buffer moved.
    pub class: String,
    /// Every key the class and its ancestors document, nearest declarer first.
    pub keys: Vec<DocumentedKey>,
    /// The keys this array already writes, which are the ones not to offer back.
    pub written: Vec<Name>,
    /// What has been typed so far, read from the literal rather than from the lexical context.
    ///
    /// The context reader stops at the first character that cannot be in an identifier, and every
    /// one of these keys begins with one -- so a person who has typed `#ti` would be filtered
    /// against `ti`, matching `#weight` as loosely as `#title`.
    pub prefix: String,
}

impl IndexView<'_> {
    /// The keys a framework's own documentation says the array around `offset` accepts.
    ///
    /// Empty unless every part lines up: a framework contributed a convention, the cursor is inside
    /// an array literal, that array writes the discriminating key with a literal value, and the
    /// value is a key of the catalog the convention names. Each of those failing is a different
    /// ordinary thing -- a plain array, a `'#type' => $variable`, a project's own `#`-keyed DSL --
    /// and offering a guess at any of them would put form-element properties into code that has
    /// nothing to do with forms.
    pub fn documented_keys_at(&self, offset: TextSize) -> Option<DocumentedKeys> {
        let contracts = self.skeleton.returns();
        // The gate every array-literal walk here shares: a project with no framework detected has
        // contributed no convention, and finding the enclosing array means walking the file.
        let rules = contracts.key_documentation();
        if rules.is_empty() {
            return None;
        }

        let open = self.current()?;
        // The key being typed is a string, so the whole of it is what an accepted item replaces --
        // the same rule a catalog id follows, and for the same reason: somebody correcting `#titl`
        // wants a valid key, not a spliced one.
        let replace = self.inside_the_quotes(open.hir().string_at(offset)?)?;

        let siblings = phrost_body::keyed_siblings_at(open.hir(), offset);
        if siblings.is_empty() {
            return None;
        }

        for rule in rules {
            let entry = siblings
                .iter()
                .find(|entry| entry.key.as_str() == &*rule.discriminator)?;
            let written = open
                .text()
                .get(usize::from(entry.value.start())..usize::from(entry.value.end()))?;
            // The value as the source wrote it, quotes included, because the range of a literal is
            // the range an edit would replace. A `$variable` or a constant leaves nothing to strip
            // and reaches no catalog key, which is the honest answer for an array whose type is
            // decided at runtime.
            let key = written.trim_matches(['\'', '"']);
            let Some(class) = contracts.catalog_class(&rule.catalog, key) else {
                continue;
            };
            let keys = self.documented_up_the_chain(class, rule);
            if keys.is_empty() {
                continue;
            }
            return Some(DocumentedKeys {
                replace,
                class: class.to_owned(),
                keys,
                written: siblings.into_iter().map(|entry| entry.key).collect(),
                prefix: open
                    .text()
                    .get(usize::from(replace.start())..usize::from(offset))
                    .unwrap_or_default()
                    .to_owned(),
            });
        }

        None
    }

    /// Every key one named class and its ancestors document.
    ///
    /// The same walk [`Self::documented_keys_at`] makes, addressed by the class instead of by a
    /// cursor: a `completionItem/resolve` arrives long after the buffer moved on, and what the
    /// class documents has not moved even if the array has.
    pub fn documented_keys_of(&self, class: &str) -> Vec<DocumentedKey> {
        self.skeleton
            .returns()
            .key_documentation()
            .iter()
            .map(|rule| self.documented_up_the_chain(class, rule))
            .find(|found| !found.is_empty())
            .unwrap_or_default()
    }

    /// Every key the class and its ancestors document, nearest declarer first.
    ///
    /// The chain and not the class, because that is where the list actually is: a form element
    /// documents its own five properties and inherits sixty from the two base classes above it, and
    /// the inherited ones are the ones everybody writes. `extends` only -- an interface declares no
    /// properties and a trait's documentation is about the trait.
    ///
    /// Nearest wins on a repeated name, which is the same rule member lookup follows: a leaf class
    /// that re-documents `#default_value` is saying what it means *here*.
    fn documented_up_the_chain(
        &self,
        class: &str,
        rule: &phrost_index::DocumentedKeysRule,
    ) -> Vec<DocumentedKey> {
        let mut found: Vec<DocumentedKey> = Vec::new();
        let mut current = Some(class.to_owned());

        for _ in 0..HIERARCHY_DEPTH {
            let Some(fqn) = current.take() else {
                break;
            };
            let Some(location) = self.class_location(&fqn) else {
                break;
            };
            if let Some(docblock) = self.documentation(&location) {
                for key in rule.keys_in(&docblock) {
                    if !found.iter().any(|held| held.name == key.name) {
                        found.push(key);
                    }
                }
            }
            current = self
                .class_header(&fqn)
                .and_then(|header| header.extends.first().map(|name| name.to_string()));
        }

        found
    }
}
