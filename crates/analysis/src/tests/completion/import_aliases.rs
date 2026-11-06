//! A name the file bound with `as` is offered where it can be written.
//!
//! Completion's names come from the catalog, which holds declarations, and an alias is not one --
//! so `use Illuminate\Support\Collection as BaseCollection;` gave the file a name completion had
//! never heard of. Laravel aliases its imports throughout; `new BaseColl` offered nothing there.

use expect_test::expect;

use crate::tests::{Query, check};

const DECLARED: &str = r#"
//- /src/Support/Collection.php
<?php
namespace App\Support;

class Collection {}
interface Process {}
function render(): string {}
"#;

fn check_declared(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{DECLARED}\n{client}"), queries, expect);
}

#[test]
fn an_aliased_class_after_new() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

use App\Support\Collection as BaseCollection;
use App\Support\Process as ProcessContract;

function make(): object
{
    return new BaseColl@here@
}
"#,
        &[Query::completion("new", "here")],
        expect![[r#"
            new
            - BaseCollection [class]  App\Support\Collection
            - ... and the client must ask again
            - accepting the first leaves `return new BaseCollection`
        "#]],
    );
}

#[test]
fn an_aliased_interface_is_not_offered_after_new_but_is_after_implements() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

use App\Support\Process as ProcessContract;

class Runner implements ProcessCon@implements@
{
    public function make(): object
    {
        return new ProcessCon@new@
    }
}
"#,
        &[
            Query::completion("implements", "implements"),
            Query::completion("new", "new"),
        ],
        expect![[r#"
            implements
            - ProcessContract [interface]  App\Support\Process
            - ... and the client must ask again
            - accepting the first leaves `class Runner implements ProcessContract`
            new
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_aliased_function() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

use function App\Support\render as show;

function page(): string
{
    return sho@here@
}
"#,
        &[Query::completion("function", "here")],
        expect![[r#"
            function
            - show [function]  App\Support\render
            - ... and the client must ask again
            - accepting the first leaves `return show`
        "#]],
    );
}

/// `use Psl\Graph;` names a namespace, not a class, and binds `Graph` all the same -- the way PSL
/// writes every call, `Graph\add_edge(...)`. The first segment offered nothing.
#[test]
fn a_namespace_imported_by_name_is_offered_as_its_last_segment() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

use App\Support;
use App\Support\Collection;

function make(): string
{
    return Supp@here@
}
"#,
        &[Query::completion("namespace import", "here")],
        expect![[r#"
            namespace import
            - Support [namespace]  App\Support  insert `Support\`
            - ... and the client must ask again
            - accepting the first leaves `return Support\`
        "#]],
    );
}
