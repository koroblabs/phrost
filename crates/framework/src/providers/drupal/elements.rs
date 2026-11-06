//! Which render element an array is asking for.
//!
//! Drupal's render and form arrays are the shape most of its code is written in, and nothing about
//! them is a call: `['#type' => 'textfield', '#title' => $this->t('Name')]` names a registered
//! element, and the only thing in the source that says so is the key beside it. There is no
//! receiver to resolve, no method to bind and no argument list -- so
//! [`ArgumentBinding`](crate::rule::ArgumentBinding) cannot reach it and
//! [`ArrayKeyBinding`](crate::rule::ArrayKeyBinding) exists for this.
//!
//! The other half is where the ids come from. An element is a plugin, declared by an attribute on
//! the class implementing it -- `#[FormElement('textfield')]` -- with the id written *positionally*,
//! which is the newer of Drupal's two spellings and now the usual one.
//!
//! # Why this catalog is open
//!
//! The key is not only Drupal's. `#`-prefixed keys are a *convention*, and a project can borrow
//! it for something else: one real project writes a mapping DSL of its own under `'#type' =>
//! 'String'`, `'Object'`, `'Float'`, and 671 of its 1 676 `'#type'` sites are that rather than a
//! render array. Reporting those would light up correct code eight times over the project's entire
//! current diagnostic surface.
//!
//! So this catalog is never named in
//! [`closed_catalogs`](crate::FrameworkProvider::closed_catalogs). Completion, goto and hover all
//! work on an open catalog; only the report is withheld, which is the half that could be wrong.
//!
//! # What is not here
//!
//! **`#theme`.** It looks like the same shape and is not. Of 63 literal values in one project, 53
//! are template *suggestions* -- `view__foo`, `table__bar` -- and Drupal resolves those by falling
//! back along `__` until something exists, so the string deliberately names a hook that does not. A
//! catalog would be wrong about 53 of 63.
//!
//! **The properties an element accepts, read from `getInfo()`.** `#title` and `#required` are the
//! second and third most written keys in a real project and appear in no element's `getInfo()`
//! anywhere, so that source cannot answer. The one that can is the class's own docblock, which is
//! what [`DOCUMENTED_KEYS`] reads -- see it for why a bullet is exact enough where a heading is not.

use crate::rule::{ArrayKeyBinding, AttributeArgument, AttributeKey, DocumentedKeys};

/// Where a render element id is a key. Filled from the element declarations in the project.
pub(super) const CATALOG: &str = "element";

/// The array key whose value names an element.
///
/// One key, because one key is what the framework defines: `#type` is read by the renderer to pick
/// the element plugin, and every other `#`-prefixed key is a property of whichever element that
/// chose.
pub(super) static ARRAY_KEYS: &[ArrayKeyBinding] = &[ArrayKeyBinding {
    key: "#type",
    catalog: CATALOG,
}];

/// Where the `#`-prefixed properties beside `#type` are written down.
///
/// Drupal declares an element's properties nowhere the runtime can see: `getInfo()` returns
/// defaults and most properties have none, so `#title`, `#required` and `#maxlength` are in no
/// PHP structure at all. What every element class does have is a docblock listing them, one bullet
/// each -- and the convention is followed by core and contrib alike because the API topic page is
/// generated from exactly these lines.
///
/// The bullet is the whole marker, with no heading above it. `Textfield` writes `Properties:` and
/// `FormElementBase` writes "Here is a list of properties that are used during the rendering and
/// form processing of form elements"; anchoring on a word would find the leaf class's five and lose
/// the sixty a form element inherits, which is the wrong half to keep.
pub(super) static DOCUMENTED_KEYS: &[DocumentedKeys] = &[DocumentedKeys {
    discriminator: ARRAY_KEYS[0].key,
    catalog: CATALOG,
    prefix: "#",
}];

/// The attributes that declare an element, and where its id is written.
///
/// Two attributes and one catalog, because the two are one namespace at runtime: a form element is
/// a render element with input handling, `\Drupal\Core\Render\ElementInfoManager` registers both
/// into the same table, and `['#type' => 'textfield']` reaches whichever kind declared the id.
///
/// Positional, which is the whole reason lowering had to learn a new trick: the attribute classes
/// declare a single `string $id` constructor parameter and every declaration in core writes it bare.
/// Naming them here is also what tells lowering to keep their literals, and to read the older
/// `@RenderElement("horizontal_tabs")` annotation spelling -- see [`AttributeArgument`]. That second
/// half is worth five ids on one real checkout, `horizontal_tabs` among them, which used to be this
/// module's example of an element declared in code that runs and in no index.
pub(super) static ATTRIBUTE_KEYS: &[AttributeKey] = &[
    AttributeKey {
        attribute: "RenderElement",
        argument: AttributeArgument::At(0),
        catalog: CATALOG,
    },
    AttributeKey {
        attribute: "FormElement",
        argument: AttributeArgument::At(0),
        catalog: CATALOG,
    },
];
