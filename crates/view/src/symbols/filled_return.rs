//! A bare generic return a framework fills from the receiver: Laravel 11's `Builder::paginate()`,
//! declared `LengthAwarePaginator` where later releases write `LengthAwarePaginator<int, TModel>`.
//! See the seam's `FilledReturn`.

use phrost_model::Name;

use crate::{resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// `hinted`, the bare generic class `name` declares, with the arguments a framework states for
    /// it -- each template of the declaring class as `arguments`, the receiver's, supply it.
    ///
    /// `declaring` is the class the declaration was written in, or `None` for the class asked
    /// about. `None` unless a rule names the method on that class or one it descends from, the
    /// returned class takes exactly as many arguments, and every template the rule names was
    /// supplied: a page of half its arguments is a guess.
    pub(crate) fn filled_return(
        &self,
        class_fqn: &str,
        declaring: Option<&Name>,
        name: &str,
        hinted: &Name,
        arguments: &[Held],
    ) -> Option<Held> {
        let rules = self.skeleton.returns().filled_returns(name);
        if rules.is_empty() {
            return None;
        }
        let owner = declaring.map_or(class_fqn, Name::as_str);
        let rule = rules.iter().find(|rule| {
            owner.eq_ignore_ascii_case(&rule.class)
                || self.descends_from(owner, &rule.class, crate::resolve::HIERARCHY_DEPTH)
        })?;
        let templates = self.class_templates(hinted.as_str())?;
        if templates.len() != rule.arguments.len() {
            return None;
        }
        let ruled = self.class_location(&rule.class)?.fqn;
        let filled = rule
            .arguments
            .iter()
            .map(|argument| match argument {
                phrost_index::FilledArgumentRule::Builtin(builtin) => Some(Held::Builtin(*builtin)),
                phrost_index::FilledArgumentRule::Template(template) => {
                    let position = self
                        .class_templates(ruled.as_str())?
                        .iter()
                        .position(|declared| declared.name.as_str() == &**template)?;
                    self.supplied_argument_by_position(
                        class_fqn,
                        ruled.as_str(),
                        position,
                        arguments,
                    )
                }
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Held::Applied {
            class: self.class_location(hinted.as_str())?.fqn,
            arguments: filled.into_boxed_slice(),
        })
    }
}
