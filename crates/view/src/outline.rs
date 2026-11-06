//! What a file declares, as a tree.
//!
//! Read from the skeleton rather than from the open buffer, so an outline is the same whether the
//! file happens to be open -- the same reason goto is. It also means this answers for any indexed
//! file, which is what a workspace-wide search needs.

use phrost_index::ClassSkeleton;
use phrost_model::{SymbolKind, TextRange, short_name};

use crate::view::{IndexView, member_symbol_kind};

/// One declaration, and whatever it contains.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OutlineNode {
    /// The short name, as a reader sees it in a list; the container is the tree above it.
    ///
    /// A plain string rather than an interned name: it is a slice of the fully-qualified one, and
    /// interning a display fragment would make it look like a symbol in its own right.
    pub name: Box<str>,
    pub kind: SymbolKind,
    /// The declaration rendered as PHP, for the detail line beside the name.
    pub detail: Option<Box<str>>,
    /// The name alone -- what selecting the entry should reveal.
    pub name_range: TextRange,
    /// The whole declaration, so an editor can fold or highlight the region it covers.
    pub full_range: TextRange,
    pub children: Vec<OutlineNode>,
    /// Whether the declaration says it is deprecated -- an editor draws the row struck through.
    pub deprecated: bool,
}

impl IndexView<'_> {
    /// Everything the current file declares, in source order, classes holding their members.
    ///
    /// Empty when the request is about a file this index has never seen, which is a normal thing for
    /// a client to ask -- a scratch buffer, or a file outside the project.
    pub fn outline(&self) -> Vec<OutlineNode> {
        let Some(skeleton) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
        else {
            return Vec::new();
        };

        let mut nodes = skeleton
            .scope()
            .symbols()
            .iter()
            .enumerate()
            .map(|(index, symbol)| {
                // An anonymous class is listed, under the name PHP itself gives it. Its members are
                // the reason: `new class { public function reset(): void {} }` is how Symfony and
                // Laravel write a test double, and filtering the class out took every method of one
                // with it -- eleven in one Laravel file, invisible to "go to symbol in file" and to
                // the breadcrumb. The synthesised name carries the offset that makes it unique; the
                // reader sees `class@anonymous`, which is what PHP's own errors say.
                let short = phrost_hir::display_class_name(short_name(symbol.fqn.as_str()));
                let class = skeleton
                    .classes()
                    .iter()
                    .find(|class| class.fqn() == &symbol.fqn);

                OutlineNode {
                    name: Box::from(short),
                    kind: symbol.kind,
                    // Only a function renders one here: a class's shape is its members, which are
                    // the children.
                    detail: skeleton
                        .function_of(u32::try_from(index).unwrap_or(u32::MAX))
                        .map(|function| function.signature(short, self.skeleton.types()).into()),
                    name_range: symbol.name_range,
                    full_range: symbol.full_range,
                    children: class.map_or_else(Vec::new, |class| self.members_of(class)),
                    deprecated: symbol.is_deprecated(),
                }
            })
            .collect::<Vec<_>>();

        nodes.sort_by_key(|node| node.full_range.start());
        nodes
    }

    fn members_of(&self, class: &ClassSkeleton) -> Vec<OutlineNode> {
        let mut members = class
            .members()
            .iter()
            .map(|row| OutlineNode {
                name: Box::from(row.name.as_str()),
                kind: member_symbol_kind(row.kind),
                detail: Some(row.signature(self.skeleton.types()).into()),
                name_range: row.name_range,
                full_range: row.full_range,
                children: Vec::new(),
                deprecated: row.modifiers.is_deprecated(),
            })
            .collect::<Vec<_>>();
        members.sort_by_key(|member| member.full_range.start());
        members
    }
}
