//! A template parameter where a docblock writes a type.
//!
//! Declared by a docblock and by nothing the catalog holds, so `@return TVa` offered every class
//! that began that way and not the name the line was about. Only the ones in scope: the docblock's
//! own and the class's around it. A template of another class in the same file means nothing here.

use expect_test::expect;

use crate::tests::{Query, check};

/// The class's template in a method's docblock, ahead of the keywords and the classes.
#[test]
fn a_method_docblock_names_the_template_of_its_class() {
    check(
        r#"
//- /src/TValueObject.php
<?php
namespace App;

class TValueObject {}

//- /src/Bag.php
<?php
namespace App;

/**
 * @template TKey of array-key
 * @template TValue
 */
class Bag
{
    /** @return TVa@here@|null */
    public function first() {}
}

/**
 * @template TValueElsewhere
 */
class Other {}
"#,
        &[Query::completion("the class's template", "here")],
        expect![[r#"
            the class's template
            - TValue [keyword]  template
            - TValueObject [class]  App\TValueObject  insert `\App\TValueObject`
            - ... and the client must ask again
            - accepting the first leaves `/** @return TValue|null */`
        "#]],
    );
}

/// A method's own template, in the docblock that declares it -- and a type alias beside it.
#[test]
fn a_docblock_names_what_it_declares_itself() {
    check(
        r#"
//- /src/Mapper.php
<?php
namespace App;

class Mapper
{
    /**
     * @template TMapped
     * @phpstan-type TMappedRow array{id: int}
     * @param TMa@here@ $value
     */
    public function map($value) {}
}
"#,
        &[Query::completion("the docblock's own", "here")],
        expect![[r#"
            the docblock's own
            - TMapped [keyword]  template
            - TMappedRow [keyword]  type alias
            - ... and the client must ask again
            - accepting the first leaves `* @param TMapped $value`
        "#]],
    );
}

/// The class's own docblock, naming its template in an `@extends` beside the declaration.
#[test]
fn a_class_docblock_names_its_own_template() {
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

/** @template TItem */
class Base {}

//- /src/Rows.php
<?php
namespace App;

/**
 * @template TRow
 * @extends Base<TR@here@>
 */
class Rows extends Base {}
"#,
        &[Query::completion("in the class's own docblock", "here")],
        expect![[r#"
            in the class's own docblock
            - TRow [keyword]  template
            - ... and the client must ask again
            - accepting the first leaves `* @extends Base<TRow>`
        "#]],
    );
}
