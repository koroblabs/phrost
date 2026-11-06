//! Names where they are used rather than declared.

use expect_test::expect;

use super::check;

#[test]
fn member_access_in_every_spelling() {
    check(
        r#"<?php

function run($repo): void
{
    $repo->count;
    $repo?->count;
    $repo->find(1);
    $repo?->find(1);
    Repo::find(1);
    Repo::$seen;
    Repo::LIMIT;
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      run              function   declaration
            3:13     $repo            parameter  declaration
            3:21     void             type       
            5:4      $repo            parameter  
            5:11     count            property   
            6:4      $repo            parameter  
            6:12     count            property   
            7:4      $repo            parameter  
            7:11     find             method     
            7:16     1                number     
            8:4      $repo            parameter  
            8:12     find             method     
            8:17     1                number     
            9:4      Repo             class      
            9:10     find             method     static
            9:15     1                number     
            10:4     Repo             class      
            10:10    $seen            property   static
            11:4     Repo             class      
            11:10    LIMIT            property   static readonly
        "#]],
    );
}

#[test]
fn a_call_is_a_function_and_a_bare_name_is_a_constant() {
    // Both are an identifier in the tree and only the shape around them says which. This is the
    // pair the blind arm would get wrong, and the rank is what stops it.
    check(
        r#"<?php

function run(): void
{
    strlen('a');
    PHP_EOL;
    App\helper();
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      run              function   declaration
            3:16     void             type       
            5:4      strlen           function   
            5:11     'a'              string     
            6:4      PHP_EOL          variable   readonly
            7:4      App\             namespace  
            7:8      helper           function   
        "#]],
    );
}

#[test]
fn a_class_like_name_in_use_is_reported_as_a_class() {
    // `Reads` is an interface and `Suit` is an enum, and nothing here can know that: the
    // declarations are in other files. This is the honest edge of a syntax-only answer.
    check(
        r#"<?php

function run($value): void
{
    $repo = new Repo();
    if ($value instanceof Reads) {
        Suit::Hearts;
    }
    try {
        run($value);
    } catch (RuntimeException $error) {
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      run              function   declaration
            3:13     $value           parameter  declaration
            3:22     void             type       
            5:4      $repo            variable   
            5:12     new              keyword    
            5:16     Repo             class      
            6:4      if               keyword    
            6:8      $value           parameter  
            6:15     instanceof       keyword    
            6:26     Reads            class      
            7:8      Suit             class      
            7:14     Hearts           property   static readonly
            9:4      try              keyword    
            10:8     run              function   
            10:12    $value           parameter  
            11:6     catch            keyword    
            11:13    RuntimeException class      
            11:30    $error           variable   
        "#]],
    );
}

#[test]
fn type_hints_name_classes_and_builtins_are_left_to_the_grammar() {
    check(
        r#"<?php

function run(int $id, ?Repo $repo, string|Reads $mixed): ?Base
{
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      function         keyword    
            3:9      run              function   declaration
            3:13     int              type       
            3:17     $id              parameter  declaration
            3:23     Repo             class      
            3:28     $repo            parameter  declaration
            3:35     string           type       
            3:42     Reads            class      
            3:48     $mixed           parameter  declaration
            3:58     Base             class      
        "#]],
    );
}

#[test]
fn an_attribute_is_a_decorator() {
    check(
        r#"<?php

#[Route('/x')]
#[App\Attribute\Deprecated]
class Repo
{
    #[Inject]
    public int $count = 0;
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:2      Route            decorator  
            3:8      '/x'             string     
            4:2      App\Attribute\   namespace  
            4:16     Deprecated       decorator  
            5:0      class            keyword    
            5:6      Repo             class      declaration
            7:6      Inject           decorator  
            8:4      public           keyword    
            8:11     int              type       
            8:15     $count           property   declaration
            8:24     0                number     
        "#]],
    );
}

#[test]
fn a_static_call_on_self_or_parent_leaves_the_keyword_alone() {
    // `self`, `static` and `parent` are keywords in the tree, not names, so the grammar keeps them.
    // The member after them is still reported.
    check(
        r#"<?php

class Repo extends Base
{
    public function find(): void
    {
        self::make();
        static::make();
        parent::find();
        self::LIMIT;
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            3:11     extends          keyword    
            3:19     Base             class      
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     find             method     declaration
            5:28     void             type       
            7:8      self             keyword    
            7:14     make             method     static
            8:8      static           keyword    
            8:16     make             method     static
            9:8      parent           keyword    
            9:16     find             method     static
            10:8     self             keyword    
            10:14    LIMIT            property   static readonly
        "#]],
    );
}
