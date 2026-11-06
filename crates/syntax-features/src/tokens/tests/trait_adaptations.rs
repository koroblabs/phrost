//! The two method names inside `use T { ... }`.
//!
//! Every other word of that line is a keyword or a class, so an uncoloured method reads as the
//! only prose in a declaration. The walk hands over the adaptation as one node and never the two
//! shapes inside it, which is why an arm written on either of those fires nowhere.

use expect_test::expect;

use super::check;

#[test]
fn an_alias_names_the_method_it_renames_and_the_name_it_gives_it() {
    check(
        r#"<?php

class Repo
{
    use Logs { log as writeLine; }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            5:4      use              keyword    
            5:8      Logs             class      
            5:15     log              method     
            5:19     as               keyword    
            5:22     writeLine        method     declaration
        "#]],
    );
}

#[test]
fn a_precedence_adaptation_names_the_trait_and_the_method() {
    check(
        r#"<?php

class Repo
{
    use Logs, Quiet { Logs::log insteadof Quiet; }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            5:4      use              keyword    
            5:8      Logs             class      
            5:14     Quiet            class      
            5:22     Logs             class      
            5:28     log              method     
            5:32     insteadof        keyword    
            5:42     Quiet            class      
        "#]],
    );
}
