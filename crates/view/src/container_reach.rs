//! Static calls into the container, written in a class the container builds.
//!
//! What counts as either is the framework's to say -- see `ContainerReachRule` -- and this is only
//! the walk: which classes in the file the container constructs, and which static calls in their
//! instance methods fetch from it.
//!
//! Instance methods only. A static method has no instance to have been handed anything, and the
//! factory that builds the class -- `create(ContainerInterface $container)` -- is one, and reads
//! the container by design. Traits are left out for the reason `phpstan-drupal` leaves them out: a
//! trait is written for every class that uses it, and a lazy accessor over a static call is how a
//! trait serves classes the container does not build.

use phrost_hir::body::ExprData;
use phrost_model::{Name, TextRange};

use crate::{resolve::HIERARCHY_DEPTH, view::IndexView};

/// One static call that reaches into the container from a class it builds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContainerReach {
    /// The call, from the class it is made on to the end of its arguments.
    pub range: TextRange,
    /// The class written before the `::`, as the declaration spells it.
    pub class: Name,
    pub method: Name,
    /// The class the call is written in, which the container constructs. A `String` for the reason
    /// `DocumentedDeclaration::owner` is one: this projection cannot intern.
    pub built: String,
    /// What the framework says to hand the class instead.
    pub instead: Box<str>,
}

impl IndexView<'_> {
    /// Every static call into the container written in an instance method of a class the
    /// container builds, in source order.
    pub fn container_reaches(&self) -> Vec<ContainerReach> {
        let contracts = self.skeleton.returns();
        // The gate every other project pays: no framework said it has a container.
        if contracts.container_reaches().is_empty() {
            return Vec::new();
        }
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        // The instance methods of every class the container builds, by the span they cover.
        let mut methods: Vec<(TextRange, String)> = Vec::new();
        for (_, class) in hir.classes() {
            if class.kind != phrost_hir::ClassKind::Class {
                continue;
            }
            let fqn = self.qualified_name(class);
            let built = contracts
                .container_interfaces()
                .iter()
                .any(|interface| self.descends_from(&fqn, interface, HIERARCHY_DEPTH))
                || contracts.container_builds_by_catalog(&fqn);
            if !built {
                continue;
            }
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.kind == phrost_hir::MemberKind::Method
                    && member.body.is_some()
                    && !member.modifiers.is_static()
                {
                    methods.push((member.full_range, fqn.clone()));
                }
            }
        }
        if methods.is_empty() {
            return Vec::new();
        }

        let mut found = Vec::new();
        phrost_body::visit_every_expression(hir, &mut |hir, expr| {
            let ExprData::StaticCall {
                class: target,
                name: Some(method),
                range,
                ..
            } = hir.expr(expr)
            else {
                return;
            };
            let Some((_, built)) = methods.iter().find(|(span, _)| span.contains_range(*range))
            else {
                return;
            };
            let Some(class) = self.class_of_target(range.start(), hir, *target) else {
                return;
            };
            let reaches = contracts.container_reaches().iter().find(|rule| {
                let named = class.as_str().eq_ignore_ascii_case(&rule.class)
                    || (rule.descendants
                        && self.descends_from(class.as_str(), &rule.class, HIERARCHY_DEPTH));
                named
                    && (rule.methods.is_empty()
                        || rule
                            .methods
                            .iter()
                            .any(|wanted| wanted.eq_ignore_ascii_case(method.as_str())))
            });
            if let Some(rule) = reaches {
                found.push(ContainerReach {
                    range: *range,
                    class,
                    method: method.clone(),
                    built: built.clone(),
                    instead: rule.instead.clone(),
                });
            }
        });
        found.sort_by_key(|reach| reach.range.start());
        found
    }
}
