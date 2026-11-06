//! `foo(...)`, PHP 8.1's first-class callable.
//!
//! Its own node rather than a call with an odd argument, so every arm written for a call misses
//! it -- and the name is then the one method or function in the file drawn as plain text.

use expect_test::expect;

use super::check;

#[test]
fn every_spelling_of_a_first_class_callable_names_what_it_captures() {
    check(
        r#"<?php

class Repo
{
    public function run(): void
    {
        $method = $this->find(...);
        $static = Repo::make(...);
        $plain = strlen(...);
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     run              method     declaration
            5:27     void             type       
            7:8      $method          variable   
            7:18     $this            keyword    
            7:25     find             method     
            8:8      $static          variable   
            8:18     Repo             class      
            8:24     make             method     static
            9:8      $plain           variable   
            9:17     strlen           function   
        "#]],
    );
}
