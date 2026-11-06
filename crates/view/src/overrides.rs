//! An override PHP will refuse to load.
//!
//! A class whose method does not fit the one it replaces is a fatal error at *class load* -- before
//! any of it runs -- and the declaration it has to fit is in another file. So this is the same kind
//! of claim as the argument checks and answerable in the same one place.
//!
//! # What is checked, and what is deliberately not
//!
//! Five things, each exact and each a guaranteed fatal: visibility reduced, `static` gained or lost,
//! fewer parameters than the parent declares, a new parameter with no default, and overriding a
//! `final`.
//!
//! **Not the types.** PHP requires a parameter type to widen and a return type to narrow, and
//! deciding either needs a subtyping relation over unions, intersections, nullables and PHP's own
//! coercions -- a type engine rather than a rule. Half a subtyping relation reports code that is
//! correct, which is worse than reporting nothing, so the types are left alone and said to be.

use phrost_model::{Name, TextRange, Visibility};
use phrost_sema::{MemberKind, MemberRow};

use crate::{symbols::INHERIT_DEPTH, view::IndexView};

/// One method that does not fit the declaration it replaces.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BadOverride {
    /// The method's own name, which is what a report underlines.
    pub range: TextRange,
    pub method: Name,
    /// The class whose declaration it has to fit.
    pub parent: Name,
    pub because: Incompatible,
}

/// Why an override does not fit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Incompatible {
    /// `public` in the parent, narrower here. PHP allows widening only.
    VisibilityReduced { was: Visibility, now: Visibility },
    /// One is `static` and the other is not.
    StaticnessChanged { was_static: bool },
    /// The parent takes more than this accepts at all.
    FewerParameters { was: usize, now: usize },
    /// A parameter the parent does not have, and nothing to fall back on.
    AddedRequired { name: Name },
    /// The parent said `final`.
    ParentIsFinal,
}

/// One `#[\Override]` with nothing above it to override.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmptyOverride {
    /// The member's own name, which is what a report underlines.
    pub range: TextRange,
    pub method: Name,
    /// Whether it is a constructor, which PHP refuses whatever the parent declares.
    pub is_constructor: bool,
    /// Whether it is a property, which PHP 8.5 lets carry the attribute under the method's rule.
    pub is_property: bool,
}

impl IndexView<'_> {
    /// Every `#[\Override]` in the current file that PHP will refuse to load the class for.
    ///
    /// PHP 8.3's attribute exists to catch a parent method that was renamed underneath a child, and
    /// it is a *fatal at class load* rather than a warning: `C::m() has #[\Override] attribute, but
    /// no matching parent method exists`. What counts as a match was asked of PHP 8.5.10 rather
    /// than read off the RFC, and three of the answers are not what the wording suggests:
    ///
    /// * a **trait** that supplies the method does not count -- a trait is not a parent -- while a
    ///   parent that got the method *from* a trait does;
    /// * a **private** method on the parent does not count, because PHP treats a private method and
    ///   a child's method of the same name as unrelated;
    /// * a **constructor** never counts, whatever the parent declares, which is the same exemption
    ///   [`Self::bad_overrides`] makes for the opposite reason.
    ///
    /// Only methods the class declares *itself*, so a `#[\Override]` written inside a trait is not
    /// reported: whether it has something to override depends on the class using the trait, and
    /// the declaration alone cannot say. A miss rather than a wrong answer.
    pub fn empty_overrides(&self) -> Vec<EmptyOverride> {
        let Some(skeleton) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
        else {
            return Vec::new();
        };
        let mut out = Vec::new();

        for class in skeleton.classes() {
            // A trait's own method is judged where it lands, and nowhere else can say.
            if class.kind() == phrost_sema::ClassKind::Trait {
                continue;
            }
            for member in class.members() {
                if !member.modifiers.is_override() {
                    continue;
                }
                let is_property = match member.kind {
                    MemberKind::Method => false,
                    MemberKind::Property => true,
                    _ => continue,
                };
                if is_property {
                    if !self.property_to_override(class.fqn().as_str(), member.name.as_str()) {
                        out.push(EmptyOverride {
                            range: member.name_range,
                            method: member.name.clone(),
                            is_constructor: false,
                            is_property,
                        });
                    }
                    continue;
                }
                let is_constructor = member.name.as_str().eq_ignore_ascii_case("__construct");
                let matched = !is_constructor
                    && (self.something_to_override(class.fqn().as_str(), member.name.as_str())
                        // Every class that declares `__toString` implements `Stringable`, whether
                        // it says so or not -- PHP 8 made the interface implicit -- so the method
                        // always has one above it. Asked of 8.5.10: a bare class with a
                        // `#[\Override] __toString` loads, and `instanceof Stringable` is true.
                        || member.name.as_str().eq_ignore_ascii_case("__toString"));
                if !matched {
                    out.push(EmptyOverride {
                        range: member.name_range,
                        method: member.name.clone(),
                        is_constructor,
                        is_property,
                    });
                }
            }
        }

        out
    }

    /// Every method in the current file that does not fit the one it overrides.
    pub fn bad_overrides(&self) -> Vec<BadOverride> {
        let Some(skeleton) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
        else {
            return Vec::new();
        };
        let mut out = Vec::new();

        for class in skeleton.classes() {
            for member in class.members() {
                if member.kind != MemberKind::Method {
                    continue;
                }
                // PHP exempts constructors from every one of these: a subclass may declare a
                // constructor with any signature it likes, which is what makes dependency injection
                // in a framework possible at all.
                if member.name.as_str().eq_ignore_ascii_case("__construct") {
                    continue;
                }
                let Some((parent, overridden)) =
                    self.overridden(class.fqn().as_str(), member.name.as_str())
                else {
                    continue;
                };
                // A private method is not an override. PHP treats the two as unrelated, so a child
                // may declare whatever it likes under the same name.
                if overridden.visibility == Visibility::Private {
                    continue;
                }

                out.extend(mismatch(member, &overridden).map(|because| BadOverride {
                    range: member.name_range,
                    method: member.name.clone(),
                    parent: parent.clone(),
                    because,
                }));
            }
        }

        out
    }

    /// Whether anything above `class_fqn` declares `name` in a way PHP counts as overridable.
    ///
    /// A parent class or an interface, through [`Self::overridden`] -- and *also* an **abstract**
    /// method in a used trait, which that one deliberately does not look at. The distinction is
    /// PHP's and it is sharp: an abstract trait method is a requirement the class fulfils, so
    /// `#[\Override]` on it loads, while a *concrete* trait method is silently replaced by the
    /// class's own and `#[\Override]` on that is the fatal. Both were run.
    fn something_to_override(&self, class_fqn: &str, name: &str) -> bool {
        if self
            .overridden(class_fqn, name)
            .is_some_and(|(_, row)| row.visibility != Visibility::Private)
        {
            return true;
        }
        let Some(header) = self.class_header(class_fqn) else {
            return false;
        };
        header.uses.iter().any(|used| {
            self.members(used.as_str())
                .find(MemberKind::Method, name)
                .is_some_and(|found| found.row.modifiers.is_abstract())
        })
    }

    /// Whether a parent or an interface declares a property `name` a child may replace.
    ///
    /// The method's rule, asked of PHP 8.5.10 for properties too: a parent's **private** property
    /// does not count, nor does one the class takes from its *own* trait, while a parent that got
    /// the property from a trait counts, and so does an interface's hooked property.
    fn property_to_override(&self, class_fqn: &str, name: &str) -> bool {
        let Some(header) = self.class_header(class_fqn) else {
            return false;
        };
        header
            .extends
            .iter()
            .chain(header.implements.iter())
            .any(|above| {
                self.members(above.as_str())
                    .find(MemberKind::Property, name)
                    .is_some_and(|found| {
                        !found.forwarded && found.row.visibility != Visibility::Private
                    })
            })
    }

    /// The nearest declaration above `class_fqn` of a method called `name`, and which class holds it.
    ///
    /// Through the flattened table of each supertype rather than by climbing one level at a time:
    /// flattening is memoized for the life of the view, and a class's parents are the classes every
    /// other projection has already asked about.
    pub(crate) fn overridden(&self, class_fqn: &str, name: &str) -> Option<(Name, MemberRow)> {
        let header = self.class_header(class_fqn)?;

        // Traits are not checked: a trait's method becomes the composing class's own, and PHP's rules
        // for a conflict there are about the `insteadof` clause rather than about compatibility.
        for above in header.extends.iter().chain(header.implements.iter()) {
            // A method a `@mixin` forwards is not declared above, so a class writing the name is
            // the first declaration of it rather than an override.
            if let Some(found) = self
                .members(above.as_str())
                .find(MemberKind::Method, name)
                .filter(|found| !found.forwarded)
            {
                let owner = match &found.provenance {
                    phrost_sema::MemberProvenance::Own
                    | phrost_sema::MemberProvenance::Trait(_) => above.clone(),
                    phrost_sema::MemberProvenance::Inherited(fqn)
                    | phrost_sema::MemberProvenance::Interface(fqn) => fqn.clone(),
                };
                return Some((owner, found.row.clone()));
            }
        }

        // One level is enough for a name the parents' own tables did not hold, because those tables
        // are already flattened. The bound is here for a broken file whose hierarchy is a cycle.
        let _ = INHERIT_DEPTH;
        None
    }
}

/// What is wrong with `child` as a replacement for `parent`, or nothing when it fits.
///
/// One reason and not a list. A declaration that breaks two rules at once is one mistake to fix, and
/// two reports on the same name is how a list stops being read.
fn mismatch(child: &MemberRow, parent: &MemberRow) -> Option<Incompatible> {
    if parent.modifiers.is_final() {
        return Some(Incompatible::ParentIsFinal);
    }
    if child.modifiers.is_static() != parent.modifiers.is_static() {
        return Some(Incompatible::StaticnessChanged {
            was_static: parent.modifiers.is_static(),
        });
    }
    // Ordered, so "more restrictive" is a comparison: Public < Protected < Private.
    if child.visibility > parent.visibility {
        return Some(Incompatible::VisibilityReduced {
            was: parent.visibility,
            now: child.visibility,
        });
    }
    // A variadic accepts everything after it, so a parent that ends in one cannot be under-served.
    let parent_takes = parent.params.len();
    if !parent.params.iter().any(|param| param.is_variadic) && child.params.len() < parent_takes {
        return Some(Incompatible::FewerParameters {
            was: parent_takes,
            now: child.params.len(),
        });
    }
    // Anything beyond what the parent declares has to be optional, or a caller holding the parent's
    // signature would call it with too few.
    child
        .params
        .iter()
        .skip(parent_takes)
        .find(|param| !param.has_default && !param.is_variadic)
        .map(|param| Incompatible::AddedRequired {
            name: param.name.clone(),
        })
}
