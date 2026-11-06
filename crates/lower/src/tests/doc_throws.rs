//! `@throws` as a name reference.
//!
//! The tag names a class and nothing else -- it is not the type *of* any declaration -- so it shows
//! up nowhere in the declaration snapshot. What it must produce is a name reference: that is what
//! goto lands on, what find-references counts, and what tells the unused-import check the name was
//! mentioned. So this asserts on the reference table directly.
//!
//! Lowering also keeps the ids now, on the declaration, for the index to resolve into the sparse
//! table a caller reads -- but the renderer does not print them and this is not where that is
//! tested. What is *stored* is the index's question, and
//! `phrost_index`'s `what_a_declaration_documents_itself_as_throwing_is_in_the_dump_and_survives_the_cache`
//! is where it is pinned, resolved and through the codec.

use expect_test::expect;

use super::check_name_refs;

#[test]
fn a_thrown_class_is_recorded() {
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

class Repo
{
    /**
     * @throws \App\NotFound
     */
    public function find(): void {}
}
"#,
        expect![[r#"
            App\NotFound  [ClassLike] @ 6:16-6:29  [doc]
        "#]],
    );
}

#[test]
fn every_thrown_class_is_recorded_and_not_only_the_first() {
    // `find_map`, which the single-type tag lookup uses, would keep one of these and silently lose
    // the rest -- and an import used only by a later one would then read as unused.
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

class Repo
{
    /**
     * @throws NotFound
     * @throws Denied
     * @throws Broken
     */
    public function find(): void {}
}
"#,
        expect![[r#"
            NotFound  [ClassLike] @ 6:16-6:24  [doc]
            Denied  [ClassLike] @ 7:16-7:22  [doc]
            Broken  [ClassLike] @ 8:16-8:22  [doc]
        "#]],
    );
}

#[test]
fn a_free_function_throws_too() {
    check_name_refs(
        r#"
//- /src/go.php
<?php

/** @throws Denied */
function go(): void {}
"#,
        expect![[r#"
            Denied  [ClassLike] @ 3:13-3:19  [doc]
        "#]],
    );
}

#[test]
fn a_docblock_without_the_tag_records_nothing_extra() {
    // The prefilter's own test: no `@throws`, no parse, no reference.
    check_name_refs(
        r#"
//- /src/Repo.php
<?php

class Repo
{
    /** Finds one. */
    public function find(): void {}
}
"#,
        expect![[r#"
            <no name references>
        "#]],
    );
}
