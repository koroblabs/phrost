//! The `use` block, which an editor may collapse on open and nothing else.

use expect_test::expect;

use super::check;

#[test]
fn a_run_of_use_statements_is_one_block() {
    check(
        r#"<?php

namespace App;

use App\Nested\Repo;
use App\Nested\Reads;
use function App\helper;
use const App\LIMIT;

class Other
{
}
"#,
        expect![[r#"
            imports 5:0-8:20  use App\Nested\Repo;
            block   11:0-12:1  {
        "#]],
    );
}

#[test]
fn a_blank_line_between_imports_does_not_split_them() {
    // People group imports with blank lines and still mean one section. An editor that collapses
    // imports on open should leave one line behind, not three.
    check(
        r#"<?php

use App\Repo;

use Other\Thing;
$x = 1;
"#,
        expect![[r#"
            imports 3:0-5:16  use App\Repo;
        "#]],
    );
}

#[test]
fn a_comment_between_imports_splits_them() {
    // Nearly always a heading for the group under it. Breaking there gives two blocks that each
    // mean something instead of one that hides the heading.
    check(
        r#"<?php

use App\First;
use App\Second;
// Third-party.
use Other\Third;
use Other\Fourth;
$x = 1;
"#,
        expect![[r#"
            imports 3:0-4:15  use App\First;
            imports 6:0-7:17  use Other\Third;
        "#]],
    );
}

#[test]
fn a_single_import_is_not_a_block() {
    check("<?php\n\nuse App\\Repo;\n$x = 1;\n", expect![""]);
}

#[test]
fn a_group_import_written_over_several_lines_folds_by_its_braces_as_well() {
    check(
        r#"<?php

use App\Nested\{
    Reads,
    Repo,
};
$x = 1;
"#,
        expect![[r#"
            imports 3:0-6:2  use App\Nested\{
            block   3:15-6:1  use App\Nested\{
        "#]],
    );
}
