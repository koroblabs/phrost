//! A table as every migration so far leaves it: created, changed, renamed and dropped in the order
//! the framework runs them, which is by file name and not by the order the files were read.
//!
//! The rule is the one `configured` installs: `Db\Schema` is the facade, `up()` the method that
//! runs, `migrations` the directory, and a two-word builder -- `text`, `number`, `forget`, `move`.

use expect_test::{Expect, expect};

use super::{Order, Step, build, check_incremental, fixture_files};

/// Every table the fixture's migrations leave, column by column, with where each was defined.
fn check(fixture: &str, order: Order, expect: Expect) {
    let built = build(&fixture_files(fixture), order);
    let schema = built.skeleton.schema();
    let rendered = schema
        .tables()
        .into_iter()
        .map(|(table, columns)| {
            let columns = columns
                .iter()
                .map(|column| {
                    let path = built
                        .skeleton
                        .file(column.file)
                        .map(|file| built.skeleton.display_path(file).to_owned())
                        .unwrap_or_default();
                    format!(
                        "  {}: {}{} @ {path} {:?}\n",
                        column.name,
                        if column.nullable { "?" } else { "" },
                        column.holds.as_str(),
                        column.at
                    )
                })
                .collect::<String>();
            format!("{table}\n{columns}")
        })
        .collect::<String>();
    expect.assert_eq(&rendered);
}

const FIXTURE: &str = r#"
//- /database/migrations/2020_01_01_books.php
<?php
use Db\Schema;

return new class {
    public function up(): void
    {
        Schema::create('books', function ($table) {
            $table->number('id');
            $table->text('title');
            $table->text('note')->nullable();
            $table->index('title');
        });
        Schema::create('temporary', function ($table) {
            $table->number('id');
        });
    }

    public function down(): void
    {
        Schema::drop('books');
    }
};

//- /database/migrations/2020_02_01_change_books.php
<?php
return new class {
    public function up(): void
    {
        \Db\Schema::table('books', function ($t) {
            $t->forget('note');
            $t->move('title', 'name');
            $t->number('pages')->nullable();
            if (true) {
                $t->text('summary');
            }
        });
        \Db\Schema::create('shelf_items', fn ($t) => $t->text('label'));
        \Db\Schema::rename('shelf_items', 'shelves');
        \Db\Schema::drop('temporary');
    }
};

//- /tests/BooksTest.php
<?php
return new class {
    public function up(): void
    {
        \Db\Schema::create('books', function ($t) {
            $t->text('only_in_a_test');
        });
    }
};
"#;

/// A column dropped, one renamed and one added later, a table renamed and one dropped -- and a test
/// building a table of its own, and `down()`, which undoes the migration, both left out.
#[test]
fn the_migrations_leave_each_table_as_they_ran() {
    check(
        FIXTURE,
        Order::ByPath,
        expect![[r#"
            books
              id: int @ database/migrations/2020_01_01_books.php 158..160
              name: string @ database/migrations/2020_01_01_books.php 190..195
              pages: ?int @ database/migrations/2020_02_01_change_books.php 208..213
              summary: string @ database/migrations/2020_02_01_change_books.php 279..286
            shelves
              label: string @ database/migrations/2020_02_01_change_books.php 379..384
        "#]],
    );
}

/// The same tables whichever order the files were read in: the framework runs them by name.
#[test]
fn the_order_files_are_read_in_changes_nothing() {
    let by_path = build(&fixture_files(FIXTURE), Order::ByPath);
    let reversed = build(&fixture_files(FIXTURE), Order::ReversedByPath);
    let render = |built: &super::Built| {
        built
            .skeleton
            .schema()
            .tables()
            .into_iter()
            .map(|(table, columns)| {
                let columns = columns
                    .iter()
                    .map(|column| {
                        let file = built
                            .skeleton
                            .file(column.file)
                            .map(|file| built.skeleton.display_path(file).to_owned());
                        format!(
                            "{} {:?} {} {file:?} {:?}",
                            column.name, column.holds, column.nullable, column.at
                        )
                    })
                    .collect::<Vec<_>>();
                format!("{table}: {columns:?}")
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(render(&by_path), render(&reversed));
}

/// What a file says about the schema is withdrawn with it, and the tables composed again.
#[test]
fn deleting_a_migration_withdraws_what_it_did() {
    check_incremental(
        FIXTURE,
        &[Step::delete(
            "database/migrations/2020_02_01_change_books.php",
        )],
        expect![[r#"
            state ready
            file database/migrations/2020_01_01_books.php [User]
              migration creates table books
                defines id: int @ 8:29-8:31
                defines title: string @ 9:27-9:32
                defines note: ?string @ 10:27-10:31
              migration creates table temporary
                defines id: int @ 14:29-14:31
              class class@anonymous#33@820f5ad4 [unnameable] @ 4:12-4:17
              use Db\Schema as Schema  [class]
              class@anonymous#33@820f5ad4 is class
              class@anonymous#33@820f5ad4::method up(): void  public  @ 5:21-5:23 (full 5:5-16:6)
              class@anonymous#33@820f5ad4::method down(): void  public  @ 18:21-18:25 (full 18:5-21:6)
            file tests/BooksTest.php [User]
              class class@anonymous#17@13487a01 [unnameable] @ 2:12-2:17
              class@anonymous#17@13487a01 is class
              class@anonymous#17@13487a01::method up(): void  public  @ 3:21-3:23 (full 3:5-8:6)
            catalog
              [class] class@anonymous#17@13487a01 -> tests/BooksTest.php (class@anonymous#17@13487a01)
              [class] class@anonymous#33@820f5ad4 -> database/migrations/2020_01_01_books.php (class@anonymous#33@820f5ad4)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

/// The tables are composed again after a change, rather than answered from before it: asked once,
/// then asked after the later migration is gone.
#[test]
fn the_tables_are_composed_again_after_a_change() {
    let mut built = build(&fixture_files(FIXTURE), Order::ByPath);
    let names = |built: &super::Built| {
        built
            .skeleton
            .schema()
            .tables()
            .into_iter()
            .map(|(table, columns)| {
                let columns = columns
                    .iter()
                    .map(|column| &*column.name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{table}: {columns}\n")
            })
            .collect::<String>()
    };
    let before = names(&built);
    let file = built
        .file_id("database/migrations/2020_02_01_change_books.php")
        .expect("the fixture has it");
    built.skeleton.remove(file);
    expect![[r#"
        books: id, name, pages, summary
        shelves: label
        --
        books: id, title, note
        temporary: id
    "#]]
    .assert_eq(&format!("{before}--\n{}", names(&built)));
}
