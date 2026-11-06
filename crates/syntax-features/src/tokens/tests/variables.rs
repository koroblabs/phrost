//! Telling a parameter from a local, which is the one thing here a grammar cannot do at all.

use expect_test::expect;

use super::check;

#[test]
fn a_variable_is_a_parameter_only_inside_the_function_that_declares_it() {
    check(
        r#"<?php

function first(int $id): void
{
    $local = $id;
}

function second(): void
{
    $id = 1;
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      first            function   declaration
            3:15     int              type       
            3:19     $id              parameter  declaration
            3:25     void             type       
            5:4      $local           variable   
            5:13     $id              parameter  
            8:0      function         keyword    
            8:9      second           function   declaration
            8:19     void             type       
            10:4     $id              variable   
            10:10    1                number     
        "#]],
    );
}

#[test]
fn a_closure_starts_with_an_empty_scope() {
    // PHP closures capture nothing unless they say so, so the enclosing parameter is not one here.
    // A captured variable is not reported as a parameter either: it is a local of the closure.
    check(
        r#"<?php

function outer(int $id): void
{
    $make = function (string $name) use ($id) {
        return $name . $id;
    };
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      outer            function   declaration
            3:15     int              type       
            3:19     $id              parameter  declaration
            3:25     void             type       
            5:4      $make            variable   
            5:12     function         keyword    
            5:22     string           type       
            5:29     $name            parameter  declaration
            5:36     use              keyword    
            5:41     $id              variable   
            6:8      return           keyword    
            6:15     $name            parameter  
            6:23     $id              variable   
        "#]],
    );
}

#[test]
fn an_arrow_function_keeps_the_enclosing_parameters() {
    // It captures by value automatically, so `$id` inside it is still the parameter of `outer`.
    // Getting this wrong is invisible in a snapshot of one function and obvious in a real file.
    check(
        r#"<?php

function outer(int $id): void
{
    $make = fn (string $name) => $name . $id;
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      outer            function   declaration
            3:15     int              type       
            3:19     $id              parameter  declaration
            3:25     void             type       
            5:4      $make            variable   
            5:12     fn               keyword    
            5:16     string           type       
            5:23     $name            parameter  declaration
            5:33     $name            parameter  
            5:41     $id              parameter  
        "#]],
    );
}

#[test]
fn a_nested_arrow_function_sees_both_levels() {
    check(
        r#"<?php

function outer(int $id): void
{
    $make = fn (string $name) => fn () => $name . $id;
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      outer            function   declaration
            3:15     int              type       
            3:19     $id              parameter  declaration
            3:25     void             type       
            5:4      $make            variable   
            5:12     fn               keyword    
            5:16     string           type       
            5:23     $name            parameter  declaration
            5:33     fn               keyword    
            5:42     $name            parameter  
            5:50     $id              parameter  
        "#]],
    );
}

#[test]
fn this_is_left_to_the_grammar() {
    // Every editor colours it as a keyword already, and reporting it as a local reads as a
    // downgrade. The member after it is still reported.
    check(
        r#"<?php

class Repo
{
    public function find(int $id): void
    {
        $this->count = $id;
        $this->store($id);
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     find             method     declaration
            5:25     int              type       
            5:29     $id              parameter  declaration
            5:35     void             type       
            7:8      $this            keyword    
            7:15     count            property   
            7:23     $id              parameter  
            8:8      $this            keyword    
            8:15     store            method     
            8:21     $id              parameter  
        "#]],
    );
}

#[test]
fn a_method_parameter_is_not_in_scope_in_a_sibling_method() {
    check(
        r#"<?php

class Repo
{
    public function first(int $id): void
    {
        $id;
    }

    public function second(): void
    {
        $id;
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     first            method     declaration
            5:26     int              type       
            5:30     $id              parameter  declaration
            5:36     void             type       
            7:8      $id              parameter  
            10:4     public           keyword    
            10:11    function         keyword    
            10:20    second           method     declaration
            10:30    void             type       
            12:8     $id              variable   
        "#]],
    );
}
