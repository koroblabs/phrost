//! What an enum has that it never declared.

use expect_test::expect;
use phrost_model::Visibility::Public;

use super::{Source, check};
use crate::{MemberKind, flatten_class};

/// The standard library's two enum interfaces, as the prelude declares them.
fn with_enum_interfaces(source: Source) -> Source {
    source
        .interface("UnitEnum", |c| {
            c.property("name", Public).static_method("cases", Public)
        })
        .interface("BackedEnum", |c| {
            c.extends("UnitEnum")
                .property("value", Public)
                .static_method("from", Public)
                .static_method("tryFrom", Public)
        })
}

#[test]
fn a_plain_enum_gets_name_and_cases() {
    let source = with_enum_interfaces(Source::new())
        .php_enum("Suit", |c| c.enum_case("Hearts").enum_case("Spades"));

    check(
        &source,
        "Suit",
        expect![[r#"
            method cases()  public static  [interface UnitEnum]
            property $name  public  [interface UnitEnum]
            case Hearts  public  [own]
            case Spades  public  [own]
        "#]],
    );
}

#[test]
fn a_backed_enum_also_gets_value_from_and_tryfrom() {
    let source = with_enum_interfaces(Source::new())
        .backed_enum("Stage", |c| c.enum_case("Draft").enum_case("Shipped"));

    check(
        &source,
        "Stage",
        expect![[r#"
            method cases()  public static  [interface UnitEnum]
            method from()  public static  [interface BackedEnum]
            method tryFrom()  public static  [interface BackedEnum]
            property $name  public  [interface UnitEnum]
            property string $value  public  [interface BackedEnum]
            case Draft  public  [own]
            case Shipped  public  [own]
        "#]],
    );
}

#[test]
fn a_case_answers_a_constant_lookup() {
    // `Stage::Shipped` is written as a constant access, and PHP resolves it to the case. A lookup
    // that demanded the exact kind would miss every enum case anyone writes.
    let source =
        with_enum_interfaces(Source::new()).backed_enum("Stage", |c| c.enum_case("Shipped"));
    let table = flatten_class(&source, "Stage");

    assert!(
        table
            .find(MemberKind::ClassConstant, "Shipped")
            .is_some_and(|member| member.row.kind == MemberKind::EnumCase)
    );
    // Still case-sensitive, as constants and cases both are.
    assert!(table.find(MemberKind::ClassConstant, "shipped").is_none());
}

#[test]
fn without_the_standard_library_the_table_says_it_is_incomplete() {
    // A missing `UnitEnum` means the members are unknown, not absent. Anything that reports an
    // undefined member has to be able to tell those apart.
    let source = Source::new().php_enum("Suit", |c| c.enum_case("Hearts"));
    let table = flatten_class(&source, "Suit");

    assert!(table.is_incomplete());
    assert!(table.find(MemberKind::Property, "name").is_none());
}
