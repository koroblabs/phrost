//! What can be written at a position, and what accepting it does.
//!
//! One module per kind of answer, because a completion request is several unrelated questions
//! wearing one name: the members of a receiver, the names in the catalog, the paths a `use` line
//! takes, the keys of a shape, the locals in scope. This file holds the shared shape of an answer
//! and the reading of which question was asked; each answer is next door.

mod arguments;
mod chain;
mod doc_tags;
mod keys;
mod keywords;
mod members;
mod names;
mod overrides;
mod type_names;
mod variables;

use phrost_model::{SymbolKind, TextRange, TextSize};

use crate::HoverBlock;
use phrost_syntax_features::regions;
use phrost_view::{CommentPlace, ImportEdit, IndexView, MemberKind};

use self::{
    arguments::named_arguments,
    keys::{built_keys, catalog_keys, declared_inputs, documented_keys, shape_keys},
    keywords::keywords,
    members::{Access, callable_methods, configured_methods, members},
    names::{Spelling, import_paths, names},
    overrides::overrides,
    type_names::type_keywords,
    variables::{VariablePlace, compacted_names, documented_parameters, variables},
};
use crate::context::{CompletionContext, NamePlace, TypePlace};

/// How many top-level names to offer.
///
/// A one-letter prefix matches thousands on a real project, and a client filters and shows a dozen.
/// The cap is about not building a list nobody reads; it is not a claim that the rest do not exist,
/// which is why [`CompletionList::is_complete`] says so.
const NAME_LIMIT: usize = 200;

/// Everything worth offering at a position.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CompletionList {
    pub items: Vec<CompletionItem>,
    /// The text every item replaces: what has already been typed.
    ///
    /// Reported rather than left to the client to work out. A client computes the range from its own
    /// idea of a word, and PHP's names do not match it -- `\App\Nested\Rep` looks like the word
    /// `Rep`, so replacing only that with the qualified name it was offered leaves
    /// `\App\Nested\App\Nested\Repo` in the file.
    ///
    /// One range for the whole list, because there is one prefix. Filled in by [`completions`],
    /// which is the only function here that knows where the cursor is.
    pub replace: TextRange,
    /// False when the list is not everything that could be offered here, so the client asks again
    /// rather than filtering what it has and concluding a name does not exist.
    ///
    /// Two things make it false and they are one answer to the client: a list cut short by a limit,
    /// and a list filtered by what was already typed. The second is nearly every list -- see
    /// [`completions`], which sets it once for all of them.
    pub is_complete: bool,
}

/// What accepting a call-shaped item writes after the name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallShape {
    /// `name()`, with the cursor after: there is no argument to type.
    Empty,
    /// `name()`, with the cursor between them.
    Arguments,
}

/// Text an accepted item writes in place of the label, and where the cursor lands in it.
///
/// The offset and not a marker character, because the text is PHP and every character a marker
/// could use appears in it. What a client is told to do with the offset -- a snippet, or nothing at
/// all for a client that cannot place a cursor -- is the protocol layer's decision.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Insertion {
    pub text: String,
    /// A byte offset into [`Self::text`]. `None` leaves the cursor wherever the client puts it,
    /// which is the end.
    pub cursor: Option<usize>,
}

impl Insertion {
    /// Text the cursor has nothing to say about: a qualified name, a quoted key, a named argument.
    pub(crate) fn plain(text: String) -> Self {
        Self { text, cursor: None }
    }
}

/// Everything accepting a call-shaped item writes after the name.
///
/// Two decisions rather than one enum of four, because they are made in different places and for
/// different reasons: the brackets follow from the *member* -- it is a method, and it takes
/// arguments or it does not -- and the semicolon follows from the *position*, which knows nothing
/// about what is being offered there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Call {
    pub shape: CallShape,
    /// The parameters a call must be given, in order and without their `$`.
    ///
    /// Handed over rather than written here, for the reason [`Insertion`] gives about the cursor: a
    /// snippet is something a client either understands or reads as literal characters, and which
    /// of those is true is the protocol layer's to know. Empty for a call with none, and for one
    /// whose parameters all have defaults -- see
    /// [`MemberCandidate::parameters`](phrost_view::MemberCandidate::parameters).
    pub parameters: Box<[phrost_model::Name]>,
    /// Whether the `;` this statement is missing is written after the brackets.
    ///
    /// Only ever true where the access is the whole of a statement and nothing has terminated it --
    /// see [`crate::context::CompletionContext`]. A `;` anywhere else makes the buffer invalid, and
    /// a character somebody deletes after every accepted completion is worse than one they add.
    pub terminates: bool,
}

/// One thing to offer, and what accepting it does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompletionItem {
    /// What the person reads and filters against.
    pub label: String,
    pub kind: SymbolKind,
    /// The one line shown beside the label in the list: a member's type, or a name's fully-qualified
    /// form.
    ///
    /// Short on purpose. This used to be the whole declaration, and a hundred rows of
    /// `public function loadByItemNumber(int|string $itemNumber, ?int $supplierId = ...):
    /// ?Application` is a hundred rows the client truncates -- so the part a reader is choosing
    /// between, the type, was the part cut off.
    pub detail: Option<String>,
    /// What a second request needs to describe this item in full.
    ///
    /// The list does not carry the declaration. A client shows the declaration of the *one* row a
    /// person has highlighted, so building two hundred of them per keystroke was work thrown away --
    /// and the cost grew with the size of the class rather than with what was typed. The protocol
    /// calls the second request `completionItem/resolve`; this is what it needs to answer.
    pub describe: Option<Describe>,
    /// Whether the declaration this row offers says it is deprecated.
    ///
    /// On the row rather than fetched with the declaration, because it decides how the row is
    /// *drawn*: a client strikes a deprecated item through, and a fact that arrives only when
    /// somebody highlights the item has arrived after they picked it.
    pub deprecated: bool,
    /// What to insert, when it differs from the label.
    pub insert: Option<Insertion>,
    /// The parentheses and the semicolon accepting this item should write, when it is a call.
    ///
    /// The *fact* and not the text: whether a client can be sent a snippet is one of its own
    /// capabilities, and this layer knows nothing about clients. `None` when there is nothing to
    /// write -- a property, a keyword, or a call whose parentheses are already there.
    pub call: Option<Call>,
    /// A `use` line to add at the same time, when accepting this item needs one.
    ///
    /// Separate from [`Self::insert`] because it lands somewhere else in the file: the protocol
    /// calls these additional edits, and a client applies both or neither.
    pub import: Option<ImportEdit>,
    /// Shown after everything else rather than among it.
    ///
    /// For a member that is real, reachable and almost never what somebody is reaching for -- PHP's
    /// `__`-prefixed methods. They were *hidden* before, on the argument that the language calls
    /// them rather than a person; that argument is wrong for four of them, and the counts say so:
    /// `->__toString()` is written 51 times in Drupal core and 59 in Symfony, `->__invoke()` 33 and
    /// 18, `->__construct()` and `->__serialize()` seventeen and fifteen. What the complaint behind
    /// the old rule was actually about is *crowding* -- a class with a handful of them pushing the
    /// wanted members off the visible part of the list -- and ranking answers that without deciding
    /// on somebody's behalf that a member does not exist.
    pub demoted: bool,
}

/// Enough to find one member again, for a request that arrives after the list.
///
/// By name rather than by index into the list, because a client may resolve an item long after the
/// buffer moved on -- and a stale index would describe a different member with confidence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberRef {
    /// The class the list was built from, not the one that declared the member: what a reader is
    /// shown is what they would reach through the receiver they typed.
    pub class: String,
    pub kind: MemberKind,
    pub name: String,
}

/// One row described the way hover describes the same declaration.
///
/// The other half of [`CompletionItem::describe`]. Nothing about it depends on the completion that
/// produced the reference, which is what lets it answer a request arriving on its own.
///
/// The blocks hover builds, not a second rendering of them: the two cards are about the same
/// declarations, and the only difference anybody chose is *when* they are built -- a list per
/// keystroke, a card for the one item somebody highlighted. Everything else that differed was
/// drift, and a probe comparing the two found it at every site it asked.
pub fn describe_completion(view: &IndexView<'_>, target: &Describe) -> Option<Vec<HoverBlock>> {
    let location = match target {
        Describe::Member(member) => {
            view.member_location(member.class.as_str(), member.kind, &member.name)?
        }
        Describe::TopLevel { fqn, kind } => match kind {
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait | SymbolKind::Enum => {
                view.class_location(fqn.as_str())?
            }
            // A free function answered *nothing* before it had a branch of its own, because every
            // top-level candidate was looked up as a class and a function has no class header.
            SymbolKind::Function => view.function_location(fqn.as_str())?,
            SymbolKind::Constant => view.constant_location(fqn.as_str())?,
            _ => return None,
        },
        // Its own return, because there is no declaration to render and no docblock range to read:
        // the whole of what this row is, is a sentence in somebody else's class. The key is the
        // declaration line, which is what a person highlighting it is checking they typed right.
        Describe::DocumentedKey { class, name } => {
            let documented = view
                .documented_keys_of(class)
                .into_iter()
                .find(|key| &*key.name == name)?;
            let mut blocks = vec![HoverBlock::Code(match &documented.kind {
                Some(kind) => format!("{name}: {kind}"),
                None => name.clone(),
            })];
            if !documented.summary.is_empty() {
                blocks.push(HoverBlock::Separator);
                blocks.push(HoverBlock::Documentation(documented.summary.into_string()));
            }
            return Some(blocks);
        }
    };

    Some(crate::hover::declaration_blocks(view, &location))
}

/// A completion row described in full: what it is, and what its author said about it.
///
/// Three fields rather than one string because the three are escaped differently when they reach
/// markdown -- the declaration is code, the prose is the author's own markup and passes through
/// untouched, and a `@param array<int, Foo>` line is full of angle brackets a renderer would read as
/// HTML. The same split hover makes, and for the same reason.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Described {
    /// The declaration as PHP would write it.
    pub declaration: String,
    /// The docblock's prose, when it has any.
    pub prose: Option<String>,
    /// The `@tag` lines, in source order.
    pub tags: Vec<String>,
}

/// What a `completionItem/resolve` needs in order to describe one row.
///
/// Two shapes because the two are found in different tables: a member is a row of a class's
/// flattened table, and everything else is a declaration of its own. The kind travels with the name
/// because it decides which table to look in -- describing a function as a class is what made a
/// function answer nothing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Describe {
    Member(MemberRef),
    TopLevel {
        fqn: String,
        kind: SymbolKind,
    },
    /// A key a framework's own documentation names, which no declaration anywhere holds.
    ///
    /// The class rather than the cursor, because a resolve arrives long after the buffer moved on
    /// and the array the key was offered inside may no longer be there. What the class documents
    /// does not move, so the two names re-find the row without a position.
    DocumentedKey {
        class: String,
        name: String,
    },
}

/// The one thing worth offering outside PHP: the tag that would start it.
///
/// Only while it is being typed. A list that appears in the middle of a template's prose is noise,
/// and the client's own word completion is a better answer there than anything this crate knows.
fn opening_tag(text: &str, offset: TextSize) -> CompletionList {
    let Some(replace) = regions::opening_tag_typed(text, offset) else {
        return CompletionList::default();
    };
    CompletionList {
        items: vec![CompletionItem {
            demoted: false,
            deprecated: false,
            label: "<?php".to_owned(),
            kind: SymbolKind::Keyword,
            detail: None,
            describe: None,
            // Spelled out because the label is not what a client would insert on its own: the
            // range begins at the `<`, which a client's idea of a word does not include.
            insert: Some(Insertion::plain("<?php".to_owned())),
            call: None,
            import: None,
        }],
        replace,
        is_complete: true,
    }
}

/// Offers what can be written at `offset`.
///
/// Nothing here guesses. A receiver whose type needs inference — any variable other than `$this` —
/// offers nothing at all rather than the members of some plausible class, because a wrong
/// suggestion accepted by a keystroke is worse than no suggestion.
pub fn completions(view: &IndexView<'_>, offset: TextSize) -> CompletionList {
    let Some(text) = view.current_text() else {
        return CompletionList::default();
    };

    // Before anything reads a prefix, because outside PHP's tags there is no prefix to read.
    // Everything there is literal text the page prints, and a name offered in it is a name in a
    // paragraph -- which is what the top of every new file was: `<?php` half typed reads as the
    // word `php`, and the answer was two hundred `PHP_*` constants over the tag being written.
    if !regions::writing_php(text, offset) {
        return opening_tag(text, offset);
    }

    // Read with the comments before the cursor blanked, which is what the scanner's parentheses
    // count needs -- see `IndexView::code_text_before`.
    let code = view.code_text_before(offset);
    let Some(context) = CompletionContext::at(code.as_deref().unwrap_or(text), offset) else {
        return CompletionList::default();
    };

    // A subscript, before the string check: `$row['` is inside a string as far as any lexer is
    // concerned, and a shape's keys are what is legal there.
    //
    // Only when the shape actually names some. `[` opens an array literal and an attribute as well
    // as a subscript, and a receiver with no shape behind it has nothing to say -- so an empty
    // answer here is not an answer, and the request carries on to whatever it would have been.
    if let CompletionContext::Key {
        receiver,
        quoted,
        prefix,
    } = &context
    {
        let keys = shape_keys(
            view,
            offset,
            receiver,
            *quoted,
            context.replaced(offset),
            prefix,
        );
        if !keys.items.is_empty() {
            return keys;
        }
    }

    // Inside a string, the only thing worth offering is what the string is *for*. A call whose first
    // argument indexes a project catalog -- a service id, an entity type -- has a knowable list, and
    // that list is the only statement anywhere of what is legal there.
    //
    // Everything else gets nothing. A class name is not what someone typing an id is reaching for,
    // and a long list over one hides whatever the editor would have suggested from its own history.
    if view.inside_string_literal(offset) {
        // `[$this, 'vali|']`: what the string is for is a method of the first element. Asked first
        // because the scanner has already ruled it in or out, so a string that is not one of these
        // pays nothing.
        if let CompletionContext::Callable {
            receiver,
            prefix,
            statically,
        } = &context
        {
            let mut methods = callable_methods(view, offset, receiver, prefix, *statically);
            if !methods.items.is_empty() {
                methods.replace = context.replaced(offset);
                methods.is_complete &= methods.replace.is_empty();
                return methods;
            }
        }
        // `$mock->method('fi|')`: a method of what the double stands in for. From the tree, which a
        // string has once its quotes are closed -- and an editor closes them as the first is typed.
        if let Some(subjects) = view.stand_in_subjects_at(offset) {
            let mut methods = configured_methods(view, &subjects, context.prefix());
            if !methods.items.is_empty() {
                methods.replace = context.replaced(offset);
                methods.is_complete &= methods.replace.is_empty();
                return methods;
            }
        }
        if view.inside_a_compacted_name(offset) {
            let mut list = compacted_names(view, offset, context.prefix());
            list.replace = context.replaced(offset);
            list.is_complete &= list.replace.is_empty();
            return list;
        }
        let mut ids = catalog_keys(view, offset);
        if !ids.items.is_empty() {
            // Filtered by what was typed, like every other list here -- so the client asks again
            // rather than narrowing this one with a function of its own.
            ids.is_complete &= ids.replace.is_empty();
            return ids;
        }
        // A form element's key, read back from the form state: what the class's builder wrote.
        let mut elements = built_keys(view, offset);
        if !elements.items.is_empty() {
            elements.replace = context.replaced(offset);
            elements.is_complete &= elements.replace.is_empty();
            return elements;
        }
        // An input a command's signature declares, read back by its name. The whole literal is
        // replaced, as an id is: an input's name may hold a dash, where the word before the cursor
        // ends.
        let mut inputs = declared_inputs(view, offset);
        if !inputs.items.is_empty() {
            inputs.is_complete &= inputs.replace.is_empty();
            return inputs;
        }
        // A class or function named by its path, which goto already follows from a string --
        // `'Symfony\Component\Form\Extension\Core\Type\TextType'`, `class_exists('App\Repo')`.
        // After the catalog, because a service id may be a class name and the catalog knows which.
        if let Some(mut paths) = paths_in_a_string(view, offset, text, context.prefix()) {
            paths.replace = context.replaced(offset);
            paths.is_complete &= paths.replace.is_empty();
            return paths;
        }
        // The other half of a framework's array: the id goes in a value, and the keys beside it are
        // what the class that id names documents. Second because a value position is the cheaper
        // question and answering it rules this one out -- an entry cannot be both.
        return documented_keys(view, offset);
    }

    // A `$` in a docblock names a parameter of the declaration under it -- in the `@param` line or
    // in a sentence -- and those are the only variables a comment has.
    if let CompletionContext::Variable { prefix } = &context
        && view.comment_at(offset).is_some()
    {
        let mut list = documented_parameters(view, offset, prefix).unwrap_or(CompletionList {
            items: Vec::new(),
            is_complete: true,
            replace: TextRange::empty(offset),
        });
        list.replace = context.replaced(offset);
        list.is_complete &= list.replace.is_empty();
        return list;
    }

    // An `@` where a tag starts, in a docblock: the tags what it documents can hold.
    if let Some(tags) = doc_tags::doc_tags(view, offset) {
        return tags;
    }

    // A comment. Prose gets nothing: a list of class names over a sentence fires on every word
    // somebody writes and suggests none of them. A type tag gets the same names as code, spelled
    // the way a comment has to spell them.
    let spelling = match view.comment_at(offset) {
        Some(CommentPlace::Prose) => {
            return CompletionList {
                items: Vec::new(),
                is_complete: true,
                replace: TextRange::empty(offset),
            };
        }
        // Spelled the way this file's docblocks spell a class -- from the root, or as the code
        // names it. See `IndexView::docblocks_name_classes_short`.
        Some(CommentPlace::TypeTag | CommentPlace::TraitTag | CommentPlace::ReferenceTag) => {
            match view.docblocks_name_classes_short() {
                true => Spelling::Meant,
                false => Spelling::Qualified,
            }
        }
        None => Spelling::Imported,
    };

    // A `@var`, `@param` or `@return` is a type position too, and a richer one: the docblock
    // language has a vocabulary of its own -- `list`, `class-string`, `non-empty-string` -- that PHP
    // itself has no word for. The tag says so outright, which is more than the text scanner below
    // can tell inside a comment.
    let context = match (view.comment_at(offset), &context) {
        (Some(CommentPlace::TypeTag), CompletionContext::Name { prefix, .. }) => {
            CompletionContext::Name {
                prefix: hyphenated(view, offset, prefix),
                place: NamePlace::Type(TypePlace::Docblock),
            }
        }
        // `@use HasFactory<UserFactory>` names the trait a class body uses, which no type position
        // accepts; `@see` points at any declaration at all.
        (Some(CommentPlace::TraitTag), CompletionContext::Name { prefix, .. }) => {
            CompletionContext::Name {
                prefix: prefix.clone(),
                place: NamePlace::UseTrait,
            }
        }
        (Some(CommentPlace::ReferenceTag), CompletionContext::Name { prefix, .. }) => {
            CompletionContext::Name {
                prefix: prefix.clone(),
                place: NamePlace::Anywhere,
            }
        }
        _ => context,
    };

    let mut list = match &context {
        CompletionContext::Variable { prefix } => {
            variables(view, offset, prefix, VariablePlace::Expression)
        }
        // A closure's capture list, which takes a variable of the enclosing scope and nothing else
        // -- so the names that would otherwise fill a bare identifier position are all wrong here.
        CompletionContext::Capture {
            prefix, already, ..
        } => variables(view, offset, prefix, VariablePlace::Capture { already }),
        // Names, plus the parameters of the call the cursor is inside. Both, because a bare
        // identifier at an argument position is genuinely ambiguous -- `Stage` and `stage:` are
        // both things a person types there -- and offering only one of the two hides the other.
        CompletionContext::Name { prefix, place } => {
            let mut list = names(view, offset, prefix, spelling, *place);
            // Ahead of the names, because at either place the shorter list is the more specific
            // answer: the callee has five parameters and PHP has thirty-odd keywords, where a
            // one-letter prefix matches a thousand class names.
            let first = match place {
                NamePlace::Argument => named_arguments(view, offset, prefix),
                // A statement may start in a class body too, and there a bare word is nearly always
                // the beginning of a member declaration -- so the methods the supertypes left to
                // write go ahead of even the keywords. Outside a class body this costs a range
                // lookup and returns nothing.
                NamePlace::Statement => {
                    let mut first = overrides(view, offset, prefix);
                    first.extend(keywords(prefix));
                    first
                }
                // The words PHP allows where a type goes, which no other place offers and this
                // one is mostly made of: `string` and `int` are typed far more often than any
                // class name. A property's place also takes the declaration keywords, since
                // `public fu` is on its way to `function` as readily as to a type.
                NamePlace::Type(place) => {
                    let mut first = type_keywords(*place, prefix);
                    if *place == TypePlace::Property {
                        first.extend(keywords(prefix));
                    }
                    first
                }
                NamePlace::New
                | NamePlace::Anywhere
                | NamePlace::Implements
                | NamePlace::Extends { .. }
                | NamePlace::UseTrait
                | NamePlace::Attribute => Vec::new(),
            };
            list.items.splice(0..0, first);
            list
        }
        // The same keyword, two languages. A `use` in a class body names a trait, and the class the
        // cursor is in is what tells them apart -- a text scanner cannot, which is why the split is
        // here and not in `CompletionContext`.
        CompletionContext::Import { prefix } => match view.class_body_at(offset).is_some() {
            true => names(view, offset, prefix, spelling, NamePlace::UseTrait),
            false => import_paths(view, offset, prefix),
        },
        // Answered above when the receiver held a shape. Reaching here means it did not, and a
        // `[` that opens an array literal or an attribute asks for a name like any other place.
        CompletionContext::Key { prefix, .. } => {
            names(view, offset, prefix, spelling, NamePlace::Anywhere)
        }
        // Answered above when the quote opened a real string. Reaching here means it did not -- a
        // pair written in a comment -- and a method of something nobody is calling is a guess.
        CompletionContext::Callable { .. } => CompletionList {
            items: Vec::new(),
            is_complete: true,
            replace: TextRange::empty(offset),
        },
        CompletionContext::Member {
            receiver,
            prefix,
            whole_statement,
        } => members(
            view,
            offset,
            receiver,
            prefix,
            Access::Instance,
            *whole_statement,
        ),
        CompletionContext::StaticMember {
            receiver,
            prefix,
            whole_statement,
            ..
        } => members(
            view,
            offset,
            receiver,
            prefix,
            Access::Static,
            *whole_statement,
        ),
    };
    list.replace = context.replaced(offset);
    // A list filtered by what has already been typed is not the whole list, whatever else is true
    // of it -- and saying otherwise hands the client a set it will narrow with a *different*
    // function. The filter here is a loose subsequence match over member names, where a client
    // applies its own to whatever it was sent; and when the prefix shrinks -- a backspace, which is
    // half of how anyone types -- a name that now matches is one it never received. So anything
    // typed means "ask again", which is what the flag is for.
    list.is_complete &= list.replace.is_empty();
    list
}

/// The names a string being written as a qualified path can go on into.
///
/// Only a string that is a path from its opening quote, and only once it holds a separator after
/// its first segment: a string is data far more often than a name, and nobody writes `App\Re` in
/// a message by accident -- the rule the cursor's reading of a function in a string already
/// follows. A string's name is read from the root whatever the file imports, which is exactly how
/// a `use` line reads one, so the answer is that line's.
fn paths_in_a_string(
    view: &IndexView<'_>,
    offset: TextSize,
    text: &str,
    prefix: &str,
) -> Option<CompletionList> {
    let before = text.get(..usize::from(offset).checked_sub(prefix.len())?)?;
    if !before.ends_with(['\'', '"']) || !prefix.trim_start_matches('\\').contains('\\') {
        return None;
    }
    let mut list = import_paths(view, offset, prefix);
    if list.items.is_empty() {
        return None;
    }
    // A leading separator is the author's, and harmless in a string; what was written stays.
    if prefix.starts_with('\\') {
        for item in &mut list.items {
            let written = item
                .insert
                .as_ref()
                .map_or(item.label.as_str(), |insert| insert.text.as_str());
            item.insert = Some(Insertion::plain(format!("\\{written}")));
        }
    }
    Some(list)
}

/// The typed prefix with the hyphens a docblock type may hold.
///
/// `non-empty-string` and `class-string` are one word to the tag language and three to every
/// identifier scanner, so a person who has typed `non-` has an empty prefix and an empty range to
/// replace -- and accepting `non-empty-array` there leaves `non-non-empty-array` in the file. Read
/// here rather than in `CompletionContext`, where a `-` is a minus sign and widening the identifier
/// would take the operator with it.
fn hyphenated(view: &IndexView<'_>, offset: TextSize, prefix: &str) -> String {
    let Some(text) = view.current_text() else {
        return prefix.to_owned();
    };
    let Some(before) = text.get(..usize::from(offset) - prefix.len()) else {
        return prefix.to_owned();
    };
    // Backwards over `-` and the word parts between them, and no further: a space or a `*` ends it.
    let head = before.trim_end_matches(|ch: char| ch.is_alphanumeric() || ch == '_' || ch == '-');
    match before[head.len()..].contains('-') {
        true => format!("{}{prefix}", &before[head.len()..]),
        false => prefix.to_owned(),
    }
}
