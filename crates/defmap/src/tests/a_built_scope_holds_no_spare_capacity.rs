//! A scope keeps no memory it will never use.
//!
//! A `FileScope` is built once per file and never appended to, so whatever its vectors hold beyond
//! their length is held for the life of the process. On a 30 916-file checkout that was **13.4 MB**
//! -- and the tell was that the *cache* path did not have it: `from_parts` is handed vectors of a
//! known length, so the scope table was 27.4 MB built and 14.0 MB read back from disk. A cold
//! session carried the difference until it ended and a warm one did not.
//!
//! What this pins is not the number, which will move, but the property: a built scope is no larger
//! than the same scope rebuilt from its own contents. Two of its lists are sized ahead from counts
//! the HIR gives in O(1), and the three import lists -- which cannot be, since which of PHP's symbol
//! tables a `use` line goes to is known only when it is read -- are tightened afterwards.
//!
//! # Why it does not enumerate the lists
//!
//! Because the defect this guards against is *a list added in one place and not the others*, which is
//! the shape every defect in the session that produced it had. A test naming the five lists would be
//! a sixth place to remember, and the one nobody updated would be invisible again.
//!
//! So it compares `heap_size` against a scope rebuilt through the same `from_parts` chain the cache
//! uses, from this one's own slices. `to_vec` allocates exactly a length, so the rebuilt scope is
//! tight everywhere -- including the import lists inside each namespace -- and the comparison covers
//! any list the memory report counts, named here or not.

use lspkit_memsize::MemSize as _;
use phrost_hir::ImportKind;
use phrost_model::Interner;
use phrost_syntax::ParseSession;

use crate::{FileScope, NamespaceScope, ScopeImports};

/// PHP's three symbol tables, which is what an import list is one of.
const TABLES: [ImportKind; 3] = [
    ImportKind::ClassLike,
    ImportKind::Function,
    ImportKind::Constant,
];

/// A file with several of everything, because a list holding one item has no room to waste.
///
/// Nine `use` lines across the three symbol tables, ten declarations and three attribute literals:
/// each list crosses a doubling boundary, which is where a grown vector keeps the most.
const SOURCE: &str = r#"<?php

namespace App\Reports;

use App\Nested\One;
use App\Nested\Two;
use App\Nested\Three;
use App\Nested\Four;
use App\Nested\Five as Aliased;
use function App\helper;
use function App\second;
use const App\LIMIT;
use const App\OTHER;

#[Plugin(id: 'one', label: 'One')]
class First {}

class Second {}
class Third {}
interface Fourth {}
trait Fifth {}
enum Sixth {}

function seventh(): void {}
function eighth(): void {}

const NINTH = 1;
const TENTH = 2;
"#;

#[test]
fn a_built_scope_is_no_larger_than_the_same_scope_rebuilt_from_its_contents() {
    let mut interner = Interner::new();
    let session = ParseSession::new();
    let parsed = session.parse(SOURCE);
    let hir = super::lower(&mut interner, 0, &parsed);
    let built = FileScope::build(&mut interner, &hir);

    // The fixture has to actually fill the lists, or this passes over nothing: a vector never pushed
    // to has capacity zero and is tight for free. Nine imports and ten declarations, checked here so
    // that a fixture edited later cannot quietly stop testing anything.
    assert!(
        built.symbols().len() >= 10,
        "the fixture stopped declaring enough to fill a list: {} symbols",
        built.symbols().len()
    );
    let imported: usize = built
        .scopes()
        .iter()
        .flat_map(|scope| TABLES.map(|kind| scope.imports().table(kind).len()))
        .sum();
    assert!(
        imported >= 9,
        "the fixture stopped importing enough: {imported}"
    );

    // Through the public `from_parts` chain, which is the path a warm start takes -- so this is the
    // cold scope measured against the warm one, which is how the waste was found in the first place.
    let tight = FileScope::from_parts(
        built.file(),
        built
            .scopes()
            .iter()
            .map(|scope| {
                let [class_like, function, constant] =
                    TABLES.map(|kind| scope.imports().table(kind).to_vec());
                NamespaceScope::from_parts(
                    scope.namespace(),
                    scope.name().cloned(),
                    scope.range(),
                    ScopeImports::from_parts(class_like, function, constant),
                )
            })
            .collect(),
        built.symbols().to_vec(),
        built.attribute_literals().to_vec(),
    );

    assert_eq!(
        built.heap_size(),
        tight.heap_size(),
        "a built scope holds {} bytes a rebuilt one does not, and holds them for the life of the \
         process -- a list in `FileScope::build` is grown by pushing and neither sized ahead nor \
         tightened",
        built.heap_size() - tight.heap_size()
    );
}
