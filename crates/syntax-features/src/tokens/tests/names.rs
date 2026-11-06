//! Where a namespace ends and the name begins.

use expect_test::expect;

use super::check;

#[test]
fn a_qualified_name_is_split_at_the_last_separator() {
    check(
        r#"<?php

$repo = new App\Nested\Repo();
$other = new \App\Other();
$root = new \Repo();
$plain = new Repo();
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      $repo            variable   
            3:8      new              keyword    
            3:12     App\Nested\      namespace  
            3:23     Repo             class      
            4:0      $other           variable   
            4:9      new              keyword    
            4:13     \App\            namespace  
            4:18     Other            class      
            5:0      $root            variable   
            5:8      new              keyword    
            5:12     \                namespace  
            5:13     Repo             class      
            6:0      $plain           variable   
            6:9      new              keyword    
            6:13     Repo             class      
        "#]],
    );
}

#[test]
fn an_import_is_reported_as_the_kind_of_thing_the_keyword_names() {
    // `use function` and `use const` put a name in a different symbol table, and the keyword is the
    // only thing that says so.
    check(
        r#"<?php

use App\Nested\Repo;
use App\Nested\Reads as Readable;
use function App\helper;
use const App\LIMIT;
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      use              keyword    
            3:4      App\Nested\      namespace  
            3:15     Repo             class      
            4:0      use              keyword    
            4:4      App\Nested\      namespace  
            4:15     Reads            class      
            4:21     as               keyword    
            4:24     Readable         class      
            5:0      use              keyword    
            5:4      function         keyword    
            5:13     App\             namespace  
            5:17     helper           function   
            6:0      use              keyword    
            6:4      const            keyword    
            6:10     App\             namespace  
            6:14     LIMIT            variable   readonly
        "#]],
    );
}

#[test]
fn a_group_import_carries_its_kind_to_every_item() {
    check(
        r#"<?php

use App\Nested\{Reads, Writes};
use function App\Helpers\{first, second};
use App\Mixed\{Thing, function act, const CAP};
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      use              keyword    
            3:4      App\             namespace  
            3:8      Nested           namespace  
            3:16     Reads            class      
            3:23     Writes           class      
            4:0      use              keyword    
            4:4      function         keyword    
            4:13     App\             namespace  
            4:17     Helpers          namespace  
            4:26     first            function   
            4:33     second           function   
            5:0      use              keyword    
            5:4      App\             namespace  
            5:8      Mixed            namespace  
            5:15     Thing            class      
            5:22     function         keyword    
            5:31     act              function   
            5:36     const            keyword    
            5:42     CAP              variable   readonly
        "#]],
    );
}

#[test]
fn a_namespace_declaration_is_a_namespace_all_the_way_to_the_end() {
    // Not a namespace prefix and a class: the blind arm would split it that way, and this is the
    // overlap that would produce.
    check(
        "<?php\n\nnamespace App\\Nested\\Deep;\n",
        expect![[r#"
            1:0      <?php            keyword    
            3:0      namespace        keyword    
            3:10     App\Nested\      namespace  
            3:21     Deep             namespace  
        "#]],
    );
}
