//! What a call or a property read gives back where it is written.
//!
//! A declaration says `get(): T`, `create(): TModel`, `make(): static` -- true of every call and
//! useless at any one of them: what a reader hovering `$box->get()` wants is that it is a `User`
//! *here*. Goto and completion already resolve that; the card showed only the declaration.

use phrost_hir::body::ExprData;
use phrost_model::TextSize;

use crate::{
    variables::Naming,
    view::{FOLLOW_LIMIT, IndexView},
};

impl IndexView<'_> {
    /// The type the call or access whose name is under the cursor evaluates to here, rendered as a
    /// card writes it -- `None` where the index cannot say.
    pub fn value_named_at(&self, offset: TextSize) -> Option<String> {
        let open = self.current()?;
        let hir = open.hir();
        let (expr, _) = hir
            .expressions()
            .filter_map(|(id, data)| {
                let named = match data {
                    ExprData::MethodCall { name_range, .. }
                    | ExprData::StaticCall { name_range, .. }
                    | ExprData::PropertyAccess { name_range, .. }
                    | ExprData::StaticPropertyAccess { name_range, .. } => *name_range,
                    ExprData::FunctionCall { name, .. } => hir.name_ref(*name).range,
                    _ => return None,
                };
                named.contains_inclusive(offset).then_some((id, named))
            })
            .min_by_key(|(_, named)| named.len())?;
        let path = phrost_body::value_path_of(hir, expr)?;
        let at = hir.expr(expr).range().start();
        let held = self.resolve_path(at, hir, &path, FOLLOW_LIMIT)?;
        self.render_held(at, hir, &held, Naming::Short)
    }
}
