//! What PHP accepts, one rule per test.
//!
//! The hierarchy is a stub here on purpose. Every nominal question is delegated, so what these
//! tests cover is the half this crate actually owns: how the connectives compose, and which
//! conversions the engine performs under each setting of `strict_types`.

use phrost_model::{BuiltinTy, Interner, RelativeTy};

use super::{ClassHierarchy, Compatibility, Fit};
use crate::{TypeId, TypeTable};

/// A hierarchy spelled out by hand.
///
/// Anything not listed is [`Fit::Unknown`], which is what a real implementation must also do for a
/// class the index has never seen.
#[derive(Default)]
struct Classes {
    accepts: Vec<(&'static str, &'static str)>,
    rejects: Vec<(&'static str, &'static str)>,
    satisfies: Vec<(&'static str, BuiltinTy)>,
    /// Classes the index is to be treated as holding. Empty means every one of them, which is what
    /// every test written before `declared` existed assumes -- they name a class and expect it to
    /// be judged, not waved through for being unknown.
    undeclared: Vec<&'static str>,
}

impl ClassHierarchy for Classes {
    fn relate(&self, subject: &phrost_model::Name, expected: &phrost_model::Name) -> Fit {
        let pair = (subject.as_str(), expected.as_str());
        if self.accepts.iter().any(|&(a, b)| (a, b) == pair) {
            Fit::Accepted
        } else if self.rejects.iter().any(|&(a, b)| (a, b) == pair) {
            Fit::Rejected
        } else {
            Fit::Unknown
        }
    }

    fn declared(&self, class: &phrost_model::Name) -> bool {
        !self.undeclared.iter().any(|name| *name == class.as_str())
    }

    fn satisfies(&self, subject: &phrost_model::Name, expected: BuiltinTy) -> Fit {
        if self
            .satisfies
            .iter()
            .any(|&(name, ty)| name == subject.as_str() && ty == expected)
        {
            Fit::Accepted
        } else {
            Fit::Unknown
        }
    }
}

/// Everything a test needs to name a type and ask one question.
struct Fixture {
    types: TypeTable,
    interner: Interner,
    classes: Classes,
}

impl Fixture {
    fn new() -> Self {
        Self {
            types: TypeTable::new(),
            interner: Interner::default(),
            classes: Classes::default(),
        }
    }

    fn builtin(&mut self, ty: BuiltinTy) -> TypeId {
        self.types.builtin(ty)
    }

    fn named(&mut self, fqn: &str) -> TypeId {
        let name = self.interner.name(fqn);
        self.types.named(name)
    }

    fn fit(&self, value: TypeId, expected: TypeId) -> Fit {
        Compatibility::new(&self.types, &self.classes, false).fit(value, expected)
    }

    fn fit_strict(&self, value: TypeId, expected: TypeId) -> Fit {
        Compatibility::new(&self.types, &self.classes, true).fit(value, expected)
    }
}

#[test]
fn a_type_fits_itself() {
    let mut fixture = Fixture::new();
    let string = fixture.builtin(BuiltinTy::String);

    assert_eq!(fixture.fit(string, string), Fit::Accepted);
}

#[test]
fn strict_types_is_what_decides_a_scalar() {
    // The whole reason the directive had to become an indexed fact. The identical call is a
    // coercion in one file and a TypeError in the next.
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let string = fixture.builtin(BuiltinTy::String);

    assert_eq!(fixture.fit(string, int), Fit::Accepted);
    assert_eq!(fixture.fit_strict(string, int), Fit::Rejected);
}

#[test]
fn int_widens_to_float_even_under_strict_types() {
    // PHP keeps exactly one implicit conversion when strict types are on, and this is it.
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let float = fixture.builtin(BuiltinTy::Float);

    assert_eq!(fixture.fit_strict(int, float), Fit::Accepted);
    assert_eq!(fixture.fit_strict(float, int), Fit::Rejected);
}

#[test]
fn mixed_takes_anything_and_promises_nothing() {
    let mut fixture = Fixture::new();
    let mixed = fixture.builtin(BuiltinTy::Mixed);
    let string = fixture.builtin(BuiltinTy::String);

    assert_eq!(fixture.fit_strict(string, mixed), Fit::Accepted);
    // The other direction is the leniency policy: a `mixed` may well hold a string.
    assert_eq!(fixture.fit_strict(mixed, string), Fit::Unknown);
}

#[test]
fn never_fits_everywhere_because_no_value_arrives() {
    let mut fixture = Fixture::new();
    let never = fixture.builtin(BuiltinTy::Never);
    let string = fixture.builtin(BuiltinTy::String);

    assert_eq!(fixture.fit_strict(never, string), Fit::Accepted);
}

#[test]
fn a_void_call_evaluates_to_null() {
    // PHP has no `void` *value*. A call to a `: void` function evaluates to NULL, so consuming one
    // where a nullable is wanted runs and prints NULL -- no error, no deprecation. Rejecting it put
    // a red squiggle on working code.
    let mut fixture = Fixture::new();
    let void = fixture.builtin(BuiltinTy::Void);
    let int = fixture.builtin(BuiltinTy::Int);
    let null = fixture.builtin(BuiltinTy::Null);
    let maybe_int = fixture.types.nullable(int);

    assert_eq!(fixture.fit_strict(void, maybe_int), Fit::Accepted);
    assert_eq!(fixture.fit_strict(void, null), Fit::Accepted);
    // And still refused where a bare null would be.
    assert_eq!(fixture.fit_strict(void, int), Fit::Rejected);
}

#[test]
fn void_is_not_a_value() {
    let mut fixture = Fixture::new();
    let void = fixture.builtin(BuiltinTy::Void);
    let string = fixture.builtin(BuiltinTy::String);

    assert_eq!(fixture.fit(void, string), Fit::Rejected);
    assert_eq!(fixture.fit(string, void), Fit::Rejected);
}

#[test]
fn null_needs_a_nullable_target() {
    let mut fixture = Fixture::new();
    let null = fixture.builtin(BuiltinTy::Null);
    let string = fixture.builtin(BuiltinTy::String);
    let maybe = fixture.types.nullable(string);

    assert_eq!(fixture.fit(null, string), Fit::Rejected);
    assert_eq!(fixture.fit(null, maybe), Fit::Accepted);
    assert_eq!(fixture.fit(string, maybe), Fit::Accepted);
    // The half that has nowhere to go is what rejects this.
    assert_eq!(fixture.fit(maybe, string), Fit::Rejected);
}

#[test]
fn a_nullable_fits_a_nullable_by_its_held_type() {
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let string = fixture.builtin(BuiltinTy::String);
    let maybe_int = fixture.types.nullable(int);
    let maybe_string = fixture.types.nullable(string);

    assert_eq!(fixture.fit(maybe_int, maybe_string), Fit::Accepted);
    assert_eq!(fixture.fit_strict(maybe_int, maybe_string), Fit::Rejected);
}

#[test]
fn every_branch_of_the_value_has_to_land_somewhere() {
    // `int|string` given to an `int`: the string branch is a value the callee cannot take, so the
    // argument is wrong however often it happens to be an int at runtime.
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let string = fixture.builtin(BuiltinTy::String);
    let either = fixture.types.union(&[int, string]);

    assert_eq!(fixture.fit_strict(either, int), Fit::Rejected);
    assert_eq!(fixture.fit(either, int), Fit::Accepted);
}

#[test]
fn one_branch_of_the_target_is_enough() {
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let string = fixture.builtin(BuiltinTy::String);
    let either = fixture.types.union(&[int, string]);

    assert_eq!(fixture.fit_strict(int, either), Fit::Accepted);
}

#[test]
fn a_union_is_judged_branch_by_branch_and_not_as_a_whole() {
    // The ordering that matters: `int|string` fits `string|int` because *each* branch finds *a*
    // home, not because one branch of the target swallows both.
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let string = fixture.builtin(BuiltinTy::String);
    let float = fixture.builtin(BuiltinTy::Float);
    let value = fixture.types.union(&[int, string]);
    let target = fixture.types.union(&[string, float]);

    // int fits float, string fits string -- and no single branch of the target takes both.
    assert_eq!(fixture.fit_strict(value, target), Fit::Accepted);
}

#[test]
fn an_intersection_target_needs_every_part() {
    let mut fixture = Fixture::new();
    let countable = fixture.named("Countable");
    let traversable = fixture.named("Traversable");
    let both = fixture.types.intersection(&[countable, traversable]);
    let collection = fixture.named("Collection");

    fixture.classes.accepts.push(("Collection", "Countable"));
    assert_eq!(fixture.fit(collection, both), Fit::Unknown);

    fixture.classes.accepts.push(("Collection", "Traversable"));
    assert_eq!(fixture.fit(collection, both), Fit::Accepted);
}

#[test]
fn an_intersection_value_satisfies_a_target_by_any_of_its_parts() {
    let mut fixture = Fixture::new();
    let countable = fixture.named("Countable");
    let traversable = fixture.named("Traversable");
    let both = fixture.types.intersection(&[countable, traversable]);

    assert_eq!(fixture.fit(both, countable), Fit::Accepted);
}

/// Each part of the target met by *some* part of the value -- not one value part meeting every
/// target part, which no part of `MockObject&Connection` does for `MockObject&Connection`.
#[test]
fn an_intersection_value_meets_an_intersection_target_part_by_part() {
    let mut fixture = Fixture::new();
    let mock = fixture.named("MockObject");
    let connection = fixture.named("Connection");
    let value = fixture.types.intersection(&[mock, connection]);
    let target = fixture.types.intersection(&[connection, mock]);
    let nullable_target = fixture.types.nullable(target);

    fixture.classes.rejects.push(("MockObject", "Connection"));
    fixture.classes.rejects.push(("Connection", "MockObject"));
    assert_eq!(fixture.fit(value, target), Fit::Accepted);
    assert_eq!(fixture.fit(value, nullable_target), Fit::Accepted);

    // And a part nothing in the value meets is still a rejection.
    let logger = fixture.named("Logger");
    let wider = fixture.types.intersection(&[connection, logger]);
    fixture.classes.rejects.push(("MockObject", "Logger"));
    fixture.classes.rejects.push(("Connection", "Logger"));
    assert_eq!(fixture.fit(value, wider), Fit::Rejected);
}

#[test]
fn a_nominal_question_is_the_hierarchys_to_answer() {
    let mut fixture = Fixture::new();
    let dog = fixture.named("Dog");
    let animal = fixture.named("Animal");
    let invoice = fixture.named("Invoice");

    // Nothing said, nothing claimed.
    assert_eq!(fixture.fit(dog, animal), Fit::Unknown);

    fixture.classes.accepts.push(("Dog", "Animal"));
    assert_eq!(fixture.fit(dog, animal), Fit::Accepted);

    fixture.classes.rejects.push(("Dog", "Invoice"));
    assert_eq!(fixture.fit(dog, invoice), Fit::Rejected);
}

#[test]
fn every_class_like_is_an_object_without_asking() {
    let mut fixture = Fixture::new();
    let dog = fixture.named("Dog");
    let object = fixture.builtin(BuiltinTy::Object);

    assert_eq!(fixture.fit_strict(dog, object), Fit::Accepted);
    // And back the other way is the leniency policy again: the object may be a Dog.
    assert_eq!(fixture.fit_strict(object, dog), Fit::Unknown);
}

#[test]
fn no_object_is_ever_a_scalar_or_an_array() {
    let mut fixture = Fixture::new();
    let dog = fixture.named("Dog");
    let int = fixture.builtin(BuiltinTy::Int);
    let array = fixture.builtin(BuiltinTy::Array);

    assert_eq!(fixture.fit(dog, int), Fit::Rejected);
    assert_eq!(fixture.fit(dog, array), Fit::Rejected);
}

#[test]
fn strict_types_turns_off_stringable() {
    // A `__toString` is a conversion, and the directive is precisely the switch that forbids
    // conversions -- so a Stringable object is still a TypeError for a `string` parameter.
    let mut fixture = Fixture::new();
    let name = fixture.named("Name");
    let string = fixture.builtin(BuiltinTy::String);
    fixture.classes.satisfies.push(("Name", BuiltinTy::String));

    assert_eq!(fixture.fit(name, string), Fit::Accepted);
    assert_eq!(fixture.fit_strict(name, string), Fit::Rejected);
}

#[test]
fn true_and_false_are_halves_of_bool() {
    let mut fixture = Fixture::new();
    let truth = fixture.builtin(BuiltinTy::True);
    let falsity = fixture.builtin(BuiltinTy::False);
    let boolean = fixture.builtin(BuiltinTy::Bool);

    assert_eq!(fixture.fit_strict(truth, boolean), Fit::Accepted);
    assert_eq!(fixture.fit_strict(truth, falsity), Fit::Rejected);
    // The half that is not the one asked for is a branch that cannot land, as `int|string` is
    // against an `int`. A guard that settled which half it is narrows before this is asked.
    assert_eq!(fixture.fit_strict(boolean, truth), Fit::Rejected);
}

#[test]
fn weak_mode_converts_nothing_to_a_half_of_bool() {
    // PHP 8.5.10 without `strict_types`: `1`, `1.0` and `"x"` passed to a declared `true` are each
    // a `TypeError`, where a declared `bool` takes all three.
    let mut fixture = Fixture::new();
    let truth = fixture.builtin(BuiltinTy::True);
    let boolean = fixture.builtin(BuiltinTy::Bool);

    for scalar in [BuiltinTy::Int, BuiltinTy::Float, BuiltinTy::String] {
        let value = fixture.builtin(scalar);
        assert_eq!(fixture.fit(value, truth), Fit::Rejected);
        assert_eq!(fixture.fit(value, boolean), Fit::Accepted);
    }
}

#[test]
fn an_array_is_iterable_but_an_iterable_need_not_be_an_array() {
    let mut fixture = Fixture::new();
    let array = fixture.builtin(BuiltinTy::Array);
    let iterable = fixture.builtin(BuiltinTy::Iterable);

    assert_eq!(fixture.fit_strict(array, iterable), Fit::Accepted);
    assert_eq!(fixture.fit_strict(iterable, array), Fit::Unknown);
}

#[test]
fn a_shape_is_an_array_and_an_array_is_not_a_shape() {
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let key = fixture.interner.name("id");
    let field = fixture.types.field(key, false, int);
    let shape = fixture.types.shape(&[field], false);
    let array = fixture.builtin(BuiltinTy::Array);
    let string = fixture.builtin(BuiltinTy::String);

    assert_eq!(fixture.fit_strict(shape, array), Fit::Accepted);
    // The keys might be there; nothing here can say.
    assert_eq!(fixture.fit_strict(array, shape), Fit::Unknown);
    assert_eq!(fixture.fit_strict(shape, string), Fit::Rejected);
    assert_eq!(fixture.fit_strict(string, shape), Fit::Rejected);
}

#[test]
fn a_generic_is_judged_by_its_base() {
    // Type arguments are docblock-only and unchecked everywhere else in the engine, so reading them
    // here would produce findings nothing else can corroborate.
    let mut fixture = Fixture::new();
    let collection = fixture.named("Collection");
    let user = fixture.named("User");
    let invoice = fixture.named("Invoice");
    let of_users = fixture.types.generic(collection, &[user]);
    let of_invoices = fixture.types.generic(collection, &[invoice]);

    assert_eq!(fixture.fit_strict(of_users, collection), Fit::Accepted);
    assert_eq!(fixture.fit_strict(of_users, of_invoices), Fit::Accepted);
}

#[test]
fn self_and_static_are_left_undecided() {
    let mut fixture = Fixture::new();
    let this = fixture.types.relative(RelativeTy::SelfTy);
    let dog = fixture.named("Dog");

    assert_eq!(fixture.fit_strict(this, dog), Fit::Unknown);
    assert_eq!(fixture.fit_strict(dog, this), Fit::Unknown);
}

#[test]
fn a_callable_is_too_many_things_to_rule_out() {
    // A string, a two-element array and a Closure are all callables, so almost nothing about one
    // can be proven wrong from the vocabulary alone.
    let mut fixture = Fixture::new();
    let callable = fixture.builtin(BuiltinTy::Callable);
    let string = fixture.builtin(BuiltinTy::String);
    let dog = fixture.named("Dog");

    assert_eq!(fixture.fit_strict(callable, string), Fit::Unknown);
    assert_eq!(fixture.fit_strict(string, callable), Fit::Unknown);
    assert_eq!(fixture.fit_strict(callable, dog), Fit::Unknown);
}

#[test]
fn an_undecidable_branch_silences_a_union_but_not_an_intersection() {
    // The asymmetry the two combinators exist for. On the target's side one unknown branch might
    // have been the one that fits, so nothing is reported. On the value's side one branch proven
    // impossible is enough, whatever the others do.
    let mut fixture = Fixture::new();
    let dog = fixture.named("Dog");
    let animal = fixture.named("Animal");
    let int = fixture.builtin(BuiltinTy::Int);
    let string = fixture.builtin(BuiltinTy::String);

    // `Dog` against `Animal|int`: unrelated to int, undecided against Animal.
    let target = fixture.types.union(&[animal, int]);
    assert_eq!(fixture.fit_strict(dog, target), Fit::Unknown);

    // `Dog|string` against `int`: the string branch is merely undecided under weak rules, but the
    // Dog branch is impossible outright.
    let value = fixture.types.union(&[dog, string]);
    assert_eq!(fixture.fit(value, int), Fit::Rejected);
}

/// A record literal judged against the shape a declaration names, key by key.
mod records {
    use super::*;
    use crate::compat::{RecordField, ValueTy};

    /// `array{id: int, name: string}`, and a record to judge against it.
    fn shaped(fixture: &mut Fixture) -> TypeId {
        let int = fixture.builtin(BuiltinTy::Int);
        let string = fixture.builtin(BuiltinTy::String);
        let id = fixture.interner.name("id");
        let name = fixture.interner.name("name");
        let fields = [
            fixture.types.field(id, false, int),
            fixture.types.field(name, false, string),
        ];
        fixture.types.shape(&fields, false)
    }

    fn record(fixture: &mut Fixture, pairs: &[(&str, ValueTy)]) -> ValueTy {
        ValueTy::Record {
            fields: pairs
                .iter()
                .map(|(key, ty)| RecordField {
                    key: fixture.interner.name(key),
                    value: ty.clone(),
                    optional: false,
                })
                .collect(),
            open: false,
        }
    }

    fn fit(fixture: &Fixture, value: &ValueTy, expected: TypeId) -> Fit {
        Compatibility::new(&fixture.types, &fixture.classes, false).fit_value(value, expected)
    }

    #[test]
    fn a_key_the_shape_refuses_is_the_whole_finding() {
        let mut fixture = Fixture::new();
        let expected = shaped(&mut fixture);
        let row = ValueTy::Named(fixture.interner.name("App\\Row"));
        let value = record(
            &mut fixture,
            &[("id", row), ("name", ValueTy::Builtin(BuiltinTy::String))],
        );
        assert_eq!(fit(&fixture, &value, expected), Fit::Rejected);
    }

    #[test]
    fn every_key_fitting_is_accepted() {
        let mut fixture = Fixture::new();
        let expected = shaped(&mut fixture);
        let value = record(
            &mut fixture,
            &[
                ("id", ValueTy::Builtin(BuiltinTy::Int)),
                ("name", ValueTy::Builtin(BuiltinTy::String)),
            ],
        );
        assert_eq!(fit(&fixture, &value, expected), Fit::Accepted);
    }

    /// A key the shape requires and the record does not hold. The record is what the literal wrote
    /// *and* every slot written into it since -- see `locals/filled.rs` in the view -- so a key it
    /// lacks is one nothing put there.
    #[test]
    fn a_required_key_the_record_lacks_is_a_finding() {
        let mut fixture = Fixture::new();
        let expected = shaped(&mut fixture);
        let value = record(&mut fixture, &[("id", ValueTy::Builtin(BuiltinTy::Int))]);
        assert_eq!(fit(&fixture, &value, expected), Fit::Rejected);
    }

    /// And one the shape marks optional is not.
    #[test]
    fn an_optional_key_the_record_lacks_is_not() {
        let mut fixture = Fixture::new();
        let int = fixture.builtin(BuiltinTy::Int);
        let string = fixture.builtin(BuiltinTy::String);
        let id = fixture.interner.name("id");
        let name = fixture.interner.name("name");
        let fields = [
            fixture.types.field(id, false, int),
            fixture.types.field(name, true, string),
        ];
        let expected = fixture.types.shape(&fields, false);
        let value = record(&mut fixture, &[("id", ValueTy::Builtin(BuiltinTy::Int))]);
        assert_eq!(fit(&fixture, &value, expected), Fit::Accepted);
    }

    fn with_extra(fixture: &mut Fixture) -> ValueTy {
        record(
            fixture,
            &[
                ("id", ValueTy::Builtin(BuiltinTy::Int)),
                ("name", ValueTy::Builtin(BuiltinTy::String)),
                ("extra", ValueTy::Builtin(BuiltinTy::Bool)),
            ],
        )
    }

    /// A key a shape does not name is not judged: `array{id: int, name: string}` on a parameter is
    /// the keys the callee reads, and PHPStan reads it so unless `bleedingEdge` seals it.
    #[test]
    fn a_key_a_shape_does_not_name_is_not_a_finding() {
        let mut fixture = Fixture::new();
        let expected = shaped(&mut fixture);
        let value = with_extra(&mut fixture);
        assert_eq!(fit(&fixture, &value, expected), Fit::Accepted);
    }

    /// And one written with `...` takes it, which is what the `...` is for.
    #[test]
    fn a_key_an_open_shape_does_not_name_is_not_a_finding() {
        let mut fixture = Fixture::new();
        let id = fixture.interner.name("id");
        let name = fixture.interner.name("name");
        let int = fixture.builtin(BuiltinTy::Int);
        let string = fixture.builtin(BuiltinTy::String);
        let fields = [
            fixture.types.field(id, false, int),
            fixture.types.field(name, false, string),
        ];
        let expected = fixture.types.shape(&fields, true);
        let value = with_extra(&mut fixture);
        assert_eq!(fit(&fixture, &value, expected), Fit::Accepted);
    }

    /// A string key a literal wrote is a claim about every run, and a list is keyed by ints. An
    /// integer key says nothing: the record cannot tell `[0 => $a]` from `[$a]`, PHP's own numbering.
    #[test]
    fn a_written_string_key_is_not_a_lists_and_a_numbered_entry_is_not_a_maps_finding() {
        let mut fixture = Fixture::new();
        let array = fixture.builtin(BuiltinTy::Array);
        let int = fixture.builtin(BuiltinTy::Int);
        let string = fixture.builtin(BuiltinTy::String);
        let list = fixture.types.generic(array, &[int, string]);
        let map = fixture.types.generic(array, &[string, int]);
        let keyed = record(
            &mut fixture,
            &[("name", ValueTy::Builtin(BuiltinTy::String))],
        );
        let numbered = record(&mut fixture, &[("0", ValueTy::Builtin(BuiltinTy::Int))]);
        assert_eq!(fit(&fixture, &keyed, list), Fit::Rejected);
        assert_eq!(fit(&fixture, &numbered, map), Fit::Accepted);
    }

    /// Beside a written string key an integer one is a key of the map the literal is:
    /// `['disabled', 'href' => $url]` hands a string-keyed parameter an int key on every run, and is
    /// no list either. Alone, a numbered entry still says nothing -- see the test above.
    #[test]
    fn a_numbered_entry_beside_a_written_key_is_a_maps_finding() {
        let mut fixture = Fixture::new();
        let array = fixture.builtin(BuiltinTy::Array);
        let int = fixture.builtin(BuiltinTy::Int);
        let string = fixture.builtin(BuiltinTy::String);
        let list = fixture.types.generic(array, &[int, string]);
        let map = fixture.types.generic(array, &[string, string]);
        let either_key = fixture.types.union(&[int, string]);
        let keyed_by_either = fixture.types.generic(array, &[either_key, string]);
        let hybrid = record(
            &mut fixture,
            &[
                ("0", ValueTy::Builtin(BuiltinTy::String)),
                ("href", ValueTy::Builtin(BuiltinTy::String)),
            ],
        );
        assert_eq!(fit(&fixture, &hybrid, map), Fit::Rejected);
        assert_eq!(fit(&fixture, &hybrid, list), Fit::Rejected);
        assert_eq!(fit(&fixture, &hybrid, keyed_by_either), Fit::Accepted);
    }

    /// `array<int>|array<string>` is all of one or all of the other, so each operand is asked for
    /// the whole record -- and a union mixing a container with a scalar keeps its element unjudged.
    #[test]
    fn a_union_of_containers_is_asked_one_container_at_a_time() {
        let mut fixture = Fixture::new();
        let array = fixture.builtin(BuiltinTy::Array);
        let int = fixture.builtin(BuiltinTy::Int);
        let string = fixture.builtin(BuiltinTy::String);
        let ints = fixture.types.generic(array, &[int]);
        let strings = fixture.types.generic(array, &[string]);
        let either = fixture.types.union(&[ints, strings]);
        let or_a_string = fixture.types.union(&[ints, string]);
        let mixed = record(
            &mut fixture,
            &[
                ("0", ValueTy::Builtin(BuiltinTy::Int)),
                ("1", ValueTy::Builtin(BuiltinTy::String)),
            ],
        );
        let all_ints = record(&mut fixture, &[("0", ValueTy::Builtin(BuiltinTy::Int))]);
        let strict = |value: &ValueTy, expected| {
            Compatibility::new(&fixture.types, &fixture.classes, true).fit_value(value, expected)
        };
        assert_eq!(strict(&mixed, either), Fit::Rejected);
        assert_eq!(strict(&all_ints, either), Fit::Accepted);
        assert_eq!(strict(&mixed, or_a_string), Fit::Accepted);
    }
}

/// A class the index holds nothing for cannot reject anything, in either direction.
///
/// The report built out of an absence: a `@return Row` whose `Row` is a type alias the file
/// imported and nothing expanded, or a `@template` nobody declared, names a class the project does
/// not have -- and the array the method really hands back was rejected against it. `relate` already
/// said a class nothing declares says nothing; a *builtin* against a class never reached it.
#[test]
fn an_undeclared_class_rejects_nothing() {
    let mut fixture = Fixture::new();
    fixture.classes.undeclared = vec!["Row", "AlsoAbsent"];
    let row = fixture.named("Row");
    let absent = fixture.named("AlsoAbsent");
    let declared = fixture.named("Real");
    let array = fixture.builtin(BuiltinTy::Array);
    let string = fixture.builtin(BuiltinTy::String);

    // The value's side and the expectation's, since a name that is not a class may be either.
    assert_eq!(fixture.fit_strict(array, row), Fit::Unknown);
    assert_eq!(fixture.fit_strict(row, array), Fit::Unknown);
    assert_eq!(fixture.fit_strict(string, absent), Fit::Unknown);

    // And the control: a class the index does hold still rejects what it is not, or the guard
    // would have silenced every judgement rather than the ones built on nothing.
    assert_eq!(fixture.fit_strict(array, declared), Fit::Rejected);
    assert_eq!(fixture.fit_strict(declared, array), Fit::Rejected);
}

/// A declared shape judged against another, which is what a value typed by a docblock is when it is
/// handed on.
mod shapes {
    use super::*;

    /// A shape of `(key, optional)` fields holding ints, sealed unless `open`.
    fn shape(fixture: &mut Fixture, keys: &[(&str, bool)], open: bool) -> TypeId {
        let int = fixture.builtin(BuiltinTy::Int);
        let fields = keys
            .iter()
            .map(|(key, optional)| {
                let key = fixture.interner.name(key);
                fixture.types.field(key, *optional, int)
            })
            .collect::<Vec<_>>();
        fixture.types.shape(&fields, open)
    }

    #[test]
    fn the_same_keys_fit() {
        let mut fixture = Fixture::new();
        let value = shape(&mut fixture, &[("id", false), ("name", false)], false);
        let expected = shape(&mut fixture, &[("name", false), ("id", false)], false);
        assert_eq!(fixture.fit(value, expected), Fit::Accepted);
    }

    /// `array{foo: int, buz: int}` handed to `array{foo: int}`: the key the declaration needs is
    /// there, and the one it does not name is not its business, written with `...` or not.
    #[test]
    fn a_wider_shape_fits_a_narrower_one() {
        let mut fixture = Fixture::new();
        let value = shape(&mut fixture, &[("foo", false), ("buz", false)], false);
        let expected = shape(&mut fixture, &[("foo", false)], false);
        assert_eq!(fixture.fit(value, expected), Fit::Accepted);
        let open = shape(&mut fixture, &[("foo", false)], true);
        assert_eq!(fixture.fit(value, open), Fit::Accepted);
    }

    /// A key that may be absent is the missing key on the runs that leave it out.
    #[test]
    fn a_key_that_may_be_absent_does_not_fill_one_that_is_required() {
        let mut fixture = Fixture::new();
        let value = shape(&mut fixture, &[("foo", true)], false);
        let expected = shape(&mut fixture, &[("foo", false)], false);
        assert_eq!(fixture.fit(value, expected), Fit::Rejected);
        let optional = shape(&mut fixture, &[("foo", true)], false);
        assert_eq!(fixture.fit(value, optional), Fit::Accepted);
    }

    /// A list is keyed `0` upwards in order, and the same two keys the other way round are not one.
    #[test]
    fn a_list_shape_refuses_its_keys_out_of_order() {
        let mut fixture = Fixture::new();
        let int = fixture.builtin(BuiltinTy::Int);
        let first = fixture.interner.name("0");
        let second = fixture.interner.name("1");
        let fields = [
            fixture.types.field(first, false, int),
            fixture.types.field(second, false, int),
        ];
        let listed = fixture.types.shape_of(&fields, false, true);
        let in_order = shape(&mut fixture, &[("0", false), ("1", false)], false);
        assert_eq!(fixture.fit(in_order, listed), Fit::Accepted);
        let reversed = shape(&mut fixture, &[("1", false), ("0", false)], false);
        assert_eq!(fixture.fit(reversed, listed), Fit::Rejected);
    }

    /// A value that may hold keys it does not name may hold the required one: that question has
    /// no answer. Whatever else it holds is not asked.
    #[test]
    fn an_open_value_is_not_missing_a_key_it_may_hold() {
        let mut fixture = Fixture::new();
        let value = shape(&mut fixture, &[("foo", false)], true);
        let wants_more = shape(&mut fixture, &[("foo", false), ("bar", false)], true);
        assert_eq!(fixture.fit(value, wants_more), Fit::Unknown);
        let sealed = shape(&mut fixture, &[("foo", false)], false);
        assert_eq!(fixture.fit(value, sealed), Fit::Accepted);
    }
}

#[test]
fn a_string_that_may_be_empty_is_not_one_that_holds_something() {
    // `string` is `''|non-empty-string`, and the empty half has nowhere to land -- the reason a
    // `bool` is refused where `true` is declared, and how PHPStan reads the pair. In either mode,
    // because no conversion is involved.
    let mut fixture = Fixture::new();
    let string = fixture.builtin(BuiltinTy::String);
    let non_empty = fixture.builtin(BuiltinTy::NonEmptyString);
    let non_falsy = fixture.builtin(BuiltinTy::NonFalsyString);

    assert_eq!(fixture.fit(string, non_empty), Fit::Rejected);
    assert_eq!(fixture.fit(string, non_falsy), Fit::Rejected);
    assert_eq!(fixture.fit(non_empty, non_falsy), Fit::Rejected);
    assert_eq!(fixture.fit_strict(string, non_empty), Fit::Rejected);
}

#[test]
fn a_refinement_is_a_string_wherever_a_string_goes() {
    let mut fixture = Fixture::new();
    let string = fixture.builtin(BuiltinTy::String);
    let non_empty = fixture.builtin(BuiltinTy::NonEmptyString);
    let non_falsy = fixture.builtin(BuiltinTy::NonFalsyString);
    let int = fixture.builtin(BuiltinTy::Int);
    let nullable = fixture.types.nullable(non_empty);

    assert_eq!(fixture.fit_strict(non_empty, string), Fit::Accepted);
    assert_eq!(fixture.fit_strict(non_falsy, string), Fit::Accepted);
    assert_eq!(fixture.fit_strict(non_falsy, non_empty), Fit::Accepted);
    assert_eq!(fixture.fit_strict(non_empty, nullable), Fit::Accepted);
    // And converts where a string converts, which is weak mode only.
    assert_eq!(fixture.fit(non_falsy, int), Fit::Accepted);
    assert_eq!(fixture.fit_strict(non_falsy, int), Fit::Rejected);
}

#[test]
fn weak_mode_converts_a_scalar_to_what_it_spells() {
    // An int or a float always spells some digits -- `0` spells `'0'`, which holds something and is
    // falsy -- `true` spells `'1'` and `false` spells `''`. Strict mode converts none of them.
    let mut fixture = Fixture::new();
    let int = fixture.builtin(BuiltinTy::Int);
    let float = fixture.builtin(BuiltinTy::Float);
    let bool_ = fixture.builtin(BuiltinTy::Bool);
    let true_ = fixture.builtin(BuiltinTy::True);
    let false_ = fixture.builtin(BuiltinTy::False);
    let non_empty = fixture.builtin(BuiltinTy::NonEmptyString);
    let non_falsy = fixture.builtin(BuiltinTy::NonFalsyString);

    assert_eq!(fixture.fit(int, non_empty), Fit::Accepted);
    assert_eq!(fixture.fit(int, non_falsy), Fit::Rejected);
    assert_eq!(fixture.fit(float, non_empty), Fit::Accepted);
    assert_eq!(fixture.fit(true_, non_falsy), Fit::Accepted);
    assert_eq!(fixture.fit(false_, non_empty), Fit::Rejected);
    assert_eq!(fixture.fit(bool_, non_empty), Fit::Rejected);
    assert_eq!(fixture.fit_strict(int, non_empty), Fit::Rejected);
}

#[test]
fn a_class_string_names_a_class_and_so_holds_something() {
    // The value side: what `get_class()` hands back is never `''` and never `'0'`. The declaration
    // side is untouched -- a `class-string` parameter still takes any string, as it did before.
    let mut fixture = Fixture::new();
    let string = fixture.builtin(BuiltinTy::String);
    let object = fixture.builtin(BuiltinTy::Object);
    let non_falsy = fixture.builtin(BuiltinTy::NonFalsyString);
    let class_string = fixture.types.generic(string, &[object]);

    assert_eq!(fixture.fit_strict(class_string, non_falsy), Fit::Accepted);
    assert_eq!(fixture.fit_strict(string, class_string), Fit::Accepted);
}

#[test]
fn a_union_of_two_strings_is_one_holding_what_both_do() {
    let mut fixture = Fixture::new();
    let string = fixture.builtin(BuiltinTy::String);
    let non_empty = fixture.builtin(BuiltinTy::NonEmptyString);
    let non_falsy = fixture.builtin(BuiltinTy::NonFalsyString);
    let int = fixture.builtin(BuiltinTy::Int);

    assert_eq!(fixture.types.union(&[non_empty, string]), string);
    assert_eq!(fixture.types.union(&[non_falsy, non_empty]), non_empty);
    assert_eq!(fixture.types.union(&[non_falsy, non_falsy]), non_falsy);
    let with_int = fixture.types.union(&[non_falsy, int, string]);
    assert_eq!(fixture.types.render(with_int), "string|int");
}

#[test]
fn two_containers_differing_only_in_a_string_are_one() {
    // What `preg_split()`'s undecided conditional answers: the same list twice, once of strings that
    // hold something.
    let mut fixture = Fixture::new();
    let string = fixture.builtin(BuiltinTy::String);
    let non_empty = fixture.builtin(BuiltinTy::NonEmptyString);
    let int = fixture.builtin(BuiltinTy::Int);
    let array = fixture.builtin(BuiltinTy::Array);
    let refined = fixture.types.generic(array, &[int, non_empty]);
    let plain = fixture.types.generic(array, &[int, string]);
    let ints = fixture.types.generic(array, &[int, int]);

    assert_eq!(fixture.types.union(&[refined, plain]), plain);
    let kept = fixture.types.union(&[refined, ints]);
    assert_eq!(
        fixture.types.render(kept),
        "array<int, non-empty-string>|array<int, int>"
    );
}
