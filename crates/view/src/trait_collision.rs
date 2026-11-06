//! Two traits that give one class the same method, with nothing to say which wins.
//!
//! `Trait method T2::m has not been applied as C::m, because of collision with T1::m` is a fatal at
//! *class load*, and it is the shape a trait *gains* a method into: two traits composed happily for
//! a year, one of them grows a `log()`, and every class using both stops loading. That is a change
//! in one file breaking another, which nothing but an index can see.
//!
//! # What PHP counts as a collision, which was run rather than read
//!
//! Asked of 8.5.10, one line at a time:
//!
//! * **Two concrete methods** of the same name from two traits collide.
//! * **One abstract, one concrete** does not: the abstract one is a requirement the other fulfils.
//! * **The same trait reached twice** does not -- directly, or because two used traits both `use`
//!   a third. It is one method however many paths lead to it, which is why the *declaring* trait
//!   is what gets counted and not the used one.
//! * **The class declaring the method itself** resolves it, and so does `insteadof`.
//! * **`as` on its own does not.** An alias adds a name; it does not choose between two.
//! * **A parent declaring it does not help**, which is the one a reader is most likely to assume.
//!
//! Properties collide differently: two declarations of one property compose when they are the same
//! declaration and are a fatal when they differ -- `T1 and T2 define the same property ($x) in the
//! composition of C. However, the definition differs`. Run against 8.5.10, a different type, a
//! different visibility, a default on one side only all differ, and so does a class declaring the
//! property beside a trait that does. Only what this can be *sure* differs is reported: a native
//! type each side writes as one plain name, visibility, `static` and `readonly`. Two written defaults
//! are not compared at all, and a **promoted** property is left alone -- it follows the constructor
//! that declares it, so `T1::__construct insteadof T2` takes `T2`'s promoted properties with it.
//! Laravel writes exactly that, and it was this check's last false positive.

use phrost_model::{Name, TextRange};
use phrost_sema::MemberKind;

use crate::view::IndexView;

/// One member two traits give a class, with nothing choosing between them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraitCollision {
    /// The class's own name, which is what a report underlines.
    pub range: TextRange,
    pub class: Name,
    pub member: Name,
    pub kind: MemberKind,
    /// The two traits that declare it.
    pub traits: (Name, Name),
}

impl IndexView<'_> {
    /// Every class in the current file whose traits collide.
    ///
    /// The flattening already decides this -- `MemberTable::conflicts` exists because picking a
    /// winner would make the server disagree with the runtime -- so all this does is say it out
    /// loud. Nothing did until now.
    pub fn trait_collisions(&self) -> Vec<TraitCollision> {
        let Some(file) = self.current_file() else {
            return Vec::new();
        };
        let Some(skeleton) = self.skeleton.file(file) else {
            return Vec::new();
        };
        let mut out = Vec::new();

        for class in skeleton.classes() {
            if class.uses().is_empty() {
                continue;
            }
            // The declaration that wins is where the report belongs, and where its range comes
            // from -- the same rule its neighbours here follow.
            let Some(location) = self.class_location(class.fqn().as_str()) else {
                continue;
            };
            if location.file != file {
                continue;
            }
            for conflict in self.members(class.fqn().as_str()).conflicts() {
                // Methods only. A property or a constant collides in PHP by *value* -- `T1 and T2
                // define the same property ($x)` comes out only when the defaults differ -- and two
                // written defaults are a different comparison from two names. Worse, a promoted
                // property follows the constructor that declares it, so an `insteadof` on
                // `__construct` takes the property with it: Laravel writes exactly that, and
                // reporting the property was the last false positive this check had.
                let collides = match conflict.kind {
                    MemberKind::Method => true,
                    MemberKind::Property => self.traits_define_differently(
                        conflict.first.as_str(),
                        conflict.second.as_str(),
                        conflict.member.as_str(),
                    ),
                    MemberKind::ClassConstant | MemberKind::EnumCase => false,
                };
                if !collides {
                    continue;
                }
                out.push(TraitCollision {
                    range: location.name_range,
                    class: class.fqn().clone(),
                    member: conflict.member.clone(),
                    kind: conflict.kind,
                    traits: (conflict.first.clone(), conflict.second.clone()),
                });
            }
            // The class's own property against a trait's, which the flattening does not record as a
            // conflict -- for a method the class's declaration simply wins, and for a property it is
            // the same fatal when the two differ.
            let constructor = class
                .members()
                .iter()
                .find(|member| {
                    member.kind == MemberKind::Method
                        && member.name.as_str().eq_ignore_ascii_case("__construct")
                })
                .cloned();
            for own in class.members() {
                if own.kind != MemberKind::Property
                    || constructor.as_ref().is_some_and(|constructor| {
                        constructor
                            .params
                            .iter()
                            .any(|param| param.name == own.name)
                    })
                {
                    continue;
                }
                for used in class.uses() {
                    let table = self.members(used.fqn.as_str());
                    if table.find(MemberKind::Method, "__construct").is_some() {
                        continue;
                    }
                    let Some(theirs) = table.find(MemberKind::Property, own.name.as_str()) else {
                        continue;
                    };
                    if !definitions_differ(self, own, &theirs.row) {
                        continue;
                    }
                    out.push(TraitCollision {
                        range: location.name_range,
                        class: class.fqn().clone(),
                        member: own.name.clone(),
                        kind: MemberKind::Property,
                        traits: (class.fqn().clone(), used.fqn.clone()),
                    });
                }
            }
        }

        out
    }

    /// Whether two traits declare a property in ways PHP refuses to compose, as far as this can be
    /// sure -- and never where either trait has a constructor, whose promoted properties an
    /// `insteadof` may have settled.
    fn traits_define_differently(&self, first: &str, second: &str, name: &str) -> bool {
        let first = self.members(first);
        let second = self.members(second);
        if first.find(MemberKind::Method, "__construct").is_some()
            || second.find(MemberKind::Method, "__construct").is_some()
        {
            return false;
        }
        match (
            first.find(MemberKind::Property, name),
            second.find(MemberKind::Property, name),
        ) {
            (Some(left), Some(right)) => definitions_differ(self, &left.row, &right.row),
            _ => false,
        }
    }
}

/// Whether two property declarations certainly differ in what PHP compares.
fn definitions_differ(
    view: &IndexView<'_>,
    left: &phrost_sema::MemberRow,
    right: &phrost_sema::MemberRow,
) -> bool {
    // A `@property` tag declares nothing PHP composes: Symfony's `ServiceSubscriberTrait` documents
    // `$container` beside every class that declares the real one.
    if left.from_docblock || right.from_docblock {
        return false;
    }
    if left.visibility != right.visibility
        || left.modifiers.is_static() != right.modifiers.is_static()
        || left.modifiers.is_readonly() != right.modifiers.is_readonly()
    {
        return true;
    }
    // One plain name a side, or none on one side: `?string` against `string|null` is one type
    // written two ways, and nothing here says how PHP compares a union.
    let plain = |ty: phrost_ty::TypeId| {
        matches!(
            view.skeleton.types().get(ty),
            phrost_ty::TypeRef::Builtin(_) | phrost_ty::TypeRef::Named(_)
        )
    };
    match (left.ty, right.ty) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(left), Some(right)) => left != right && plain(left) && plain(right),
    }
}
