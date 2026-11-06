//! An array shape in a docblock, coloured.

use expect_test::expect;

use super::check;

#[test]
fn an_array_shape_written_across_lines_is_coloured_on_every_line() {
    check(
        r#"<?php
class C
{
    /**
     * @param array{
     *   context_values?: mixed,
     *   metadata?: mixed
     * } $options
     */
    public function send(array $options): void {}
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      class            keyword    
            2:6      C                class      declaration
            4:4      /**              comment    
            5:0           *           comment    
            5:7      @param           decorator  
            5:13                      comment    
            5:14     array{           type       
            6:0           *           comment    
            6:9      context_values   property   
            6:23     ?: mixed,        type       
            7:0           *           comment    
            7:9      metadata         property   
            7:17     ?: mixed         type       
            8:0           *           comment    
            8:7      }                type       
            8:8                       comment    
            8:9      $options         parameter  
            9:0           */          comment    
            10:4     public           keyword    
            10:11    function         keyword    
            10:20    send             method     declaration
            10:25    array            type       
            10:31    $options         parameter  declaration
            10:42    void             type       
        "#]],
    );
}

#[test]
fn an_array_shape_on_one_line_keeps_its_keys_apart_from_its_types() {
    check(
        r#"<?php
class C
{
    /** @param array{name: string, size: int} $shape */
    public function take(array $shape): void {}
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      class            keyword    
            2:6      C                class      declaration
            4:4      /**              comment    
            4:8      @param           decorator  
            4:14                      comment    
            4:15     array{           type       
            4:21     name             property   
            4:25     : string,        type       
            4:35     size             property   
            4:39     : int}           type       
            4:45                      comment    
            4:46     $shape           parameter  
            4:52      */              comment    
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     take             method     declaration
            5:25     array            type       
            5:31     $shape           parameter  declaration
            5:40     void             type       
        "#]],
    );
}
