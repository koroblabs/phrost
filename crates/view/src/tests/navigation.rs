use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_type_hint_navigates_to_the_class_it_names() {
    check_view(
        r#"
//- /src/User.php
<?php
namespace App;

class User
{
    public function name(): string {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): ?Us@target@er {}
}
"#,
        &[
            Query::cursor("cursor", "target"),
            Query::goto("goto", "target"),
        ],
        expect![[r#"
            cursor
            - written name User (Relative) [class]
            goto
            - class App\User @ src/User.php 4:7-4:11
        "#]],
    );
}

#[test]
fn a_parent_named_through_an_import_navigates_across_files() {
    check_view(
        r#"
//- /vendor/acme/src/Model.php
<?php
namespace Acme\Orm;

class Model {}

//- /src/User.php
<?php
namespace App;

use Acme\Orm\Model;

class User extends Mo@parent@del {}
"#,
        &[Query::goto("goto parent", "parent")],
        expect![[r#"
            goto parent
            - class Acme\Orm\Model @ vendor/acme/src/Model.php 4:7-4:12
        "#]],
    );
}

#[test]
fn the_path_of_an_import_navigates_to_what_it_imports() {
    check_view(
        r#"
//- /vendor/acme/src/Model.php
<?php
namespace Acme\Orm;

class Model {}

//- /src/User.php
<?php
namespace App;

use Acme\Orm\Mo@import@del;
"#,
        &[
            Query::cursor("cursor", "import"),
            Query::goto("goto", "import"),
        ],
        expect![[r#"
            cursor
            - import Acme\Orm\Model [class]
            goto
            - class Acme\Orm\Model @ vendor/acme/src/Model.php 4:7-4:12
        "#]],
    );
}

#[test]
fn goto_on_a_declaration_lands_on_itself() {
    check_view(
        r#"
//- /src/User.php
<?php
namespace App;

class Us@decl@er {}
"#,
        &[Query::goto("goto", "decl")],
        expect![[r#"
            goto
            - class App\User @ src/User.php 4:7-4:11
        "#]],
    );
}

#[test]
fn a_name_that_resolves_to_nothing_is_reported_as_not_found() {
    // Not a guess, and not a fuzzy match on the short name. A deterministic miss is what lets a
    // diagnostic say "not declared" with confidence.
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo extends Not@missing@Installed {}
"#,
        &[Query::goto("goto", "missing")],
        expect![[r#"
            goto
            - (not found)
        "#]],
    );
}

#[test]
fn a_member_navigates_to_the_class_that_declared_it() {
    // Inherited from a grandparent and pulled in from a trait: both should land where the member
    // was written, not on the class that was asked about.
    check_view(
        r#"
//- /src/Grandparent.php
<?php
namespace App;

class Grandparent
{
    public function deep(): void {}
}

//- /src/Timestamps.php
<?php
namespace App;

trait Timestamps
{
    public ?string $createdAt = null;
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Grandparent
{
    use Timestamps;

    public function own(): void {}
}
"#,
        &[
            Query::method_goto("own", "App\\Repo", "own"),
            Query::method_goto("inherited", "App\\Repo", "deep"),
            Query::property_goto("from trait", "App\\Repo", "createdAt"),
            Query::members("members", "App\\Repo"),
        ],
        expect![[r#"
            own
            - method App\Repo::own @ src/Repo.php 8:21-8:24
            inherited
            - method App\Grandparent::deep @ src/Grandparent.php 6:21-6:25
            from trait
            - property App\Timestamps::createdAt @ src/Timestamps.php 6:20-6:30
            members
            - method deep(): void  public  [inherited App\Grandparent]
            - method own(): void  public  [own]
            - property ?string $createdAt  public  [trait App\Timestamps]
        "#]],
    );
}

#[test]
fn a_member_declaration_under_the_cursor_navigates_to_itself() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi@member@nd(): void {}
}
"#,
        &[
            Query::cursor("cursor", "member"),
            Query::goto("goto", "member"),
        ],
        expect![[r#"
            cursor
            - member declaration App\Repo::find [method]
            goto
            - method App\Repo::find @ src/Repo.php 6:21-6:25
        "#]],
    );
}

#[test]
fn a_fully_qualified_name_ignores_the_files_imports() {
    check_view(
        r#"
//- /src/A.php
<?php
namespace App;
class Model {}

//- /vendor/acme/src/Model.php
<?php
namespace Acme;
class Model {}

//- /src/User.php
<?php
namespace App;

use Acme\Model;

class User
{
    public function a(): \App\Mo@absolute@del {}
    public function b(): Mo@imported@del {}
}
"#,
        &[
            Query::goto("absolute", "absolute"),
            Query::goto("imported", "imported"),
        ],
        expect![[r#"
            absolute
            - class App\Model @ src/A.php 3:7-3:12
            imported
            - class Acme\Model @ vendor/acme/src/Model.php 3:7-3:12
        "#]],
    );
}

#[test]
fn the_narrowest_thing_under_the_cursor_wins() {
    // A qualified name's range covers the whole path, and the class declaration's name range is
    // separate -- but if two candidates ever do overlap, the inner one is what was clicked.
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function a(): Nested\Th@nested@ing {}
}
"#,
        &[Query::cursor("cursor", "nested")],
        expect![[r#"
            cursor
            - written name Nested\Thing (Relative) [class]
        "#]],
    );
}

#[test]
fn user_code_wins_over_vendor_when_both_declare_the_name() {
    check_view(
        r#"
//- /vendor/acme/src/Repo.php
<?php
namespace App;
class Repo {}

//- /src/Repo.php
<?php
namespace App;
class Repo {}

//- /src/Use.php
<?php
namespace App;

class Client
{
    public function a(): Re@which@po {}
}
"#,
        &[Query::goto("goto", "which")],
        expect![[r#"
            goto
            - class App\Repo @ src/Repo.php 3:7-3:11
        "#]],
    );
}
