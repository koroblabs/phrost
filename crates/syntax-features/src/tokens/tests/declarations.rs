//! Names at the place they are declared.

use expect_test::expect;

use super::check;

#[test]
fn a_class_with_a_header_and_members() {
    check(
        r#"<?php

namespace App\Nested;

abstract class Repo extends Base implements Reads, Writes
{
    use Counts;

    public const int LIMIT = 10;

    protected static array $seen = [];

    public readonly string $name;

    abstract public function find(int $id): void;

    final public static function make(): static
    {
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      namespace        keyword    
            3:10     App\             namespace  
            3:14     Nested           namespace  
            5:0      abstract         keyword    
            5:9      class            keyword    
            5:15     Repo             class      declaration abstract
            5:20     extends          keyword    
            5:28     Base             class      
            5:33     implements       keyword    
            5:44     Reads            interface  
            5:51     Writes           interface  
            7:4      use              keyword    
            7:8      Counts           class      
            9:4      public           keyword    
            9:11     const            keyword    
            9:17     int              type       
            9:21     LIMIT            property   declaration static readonly
            9:29     10               number     
            11:4     protected        keyword    
            11:14    static           keyword    
            11:21    array            type       
            11:27    $seen            property   declaration static
            13:4     public           keyword    
            13:11    readonly         keyword    
            13:20    string           type       
            13:27    $name            property   declaration readonly
            15:4     abstract         keyword    
            15:13    public           keyword    
            15:20    function         keyword    
            15:29    find             method     declaration abstract
            15:34    int              type       
            15:38    $id              parameter  declaration
            15:44    void             type       
            17:4     final            keyword    
            17:10    public           keyword    
            17:17    static           keyword    
            17:24    function         keyword    
            17:33    make             method     declaration static
            17:41    static           type       
        "#]],
    );
}

#[test]
fn an_interface_extends_interfaces_and_a_class_extends_a_class() {
    // The keyword settles what a name in a header is, which is more than can be said at a use
    // site. `implements` is always interfaces; `extends` on an interface is too.
    check(
        r#"<?php

interface Reads extends Countable, Stringable
{
    public function read(): string;
}

class Repo extends Base implements Reads
{
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      interface        keyword    
            3:10     Reads            interface  declaration
            3:16     extends          keyword    
            3:24     Countable        interface  
            3:35     Stringable       interface  
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     read             method     declaration
            5:28     string           type       
            8:0      class            keyword    
            8:6      Repo             class      declaration
            8:11     extends          keyword    
            8:19     Base             class      
            8:24     implements       keyword    
            8:35     Reads            interface  
        "#]],
    );
}

#[test]
fn traits_enums_and_their_members() {
    check(
        r#"<?php

trait Counts
{
    public int $total = 0;
}

enum Suit: string implements Reads
{
    case Hearts = 'h';
    case Spades = 's';

    public function label(): string
    {
        return 'x';
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      trait            keyword    
            3:6      Counts           class      declaration
            5:4      public           keyword    
            5:11     int              type       
            5:15     $total           property   declaration
            5:24     0                number     
            8:0      enum             keyword    
            8:5      Suit             enum       declaration
            8:11     string           type       
            8:18     implements       keyword    
            8:29     Reads            interface  
            10:4     case             keyword    
            10:9     Hearts           enumMember declaration
            10:18    'h'              string     
            11:4     case             keyword    
            11:9     Spades           enumMember declaration
            11:18    's'              string     
            13:4     public           keyword    
            13:11    function         keyword    
            13:20    label            method     declaration
            13:29    string           type       
            15:8     return           keyword    
            15:15    'x'              string     
        "#]],
    );
}

#[test]
fn functions_constants_and_property_hooks() {
    check(
        r#"<?php

const LIMIT = 10;

function build(string $type): void
{
}

class Repo
{
    public int $count = 0 {
        get => 1;
        set => 2;
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      const            keyword    
            3:6      LIMIT            variable   declaration readonly
            3:14     10               number     
            5:0      function         keyword    
            5:9      build            function   declaration
            5:15     string           type       
            5:22     $type            parameter  declaration
            5:30     void             type       
            9:0      class            keyword    
            9:6      Repo             class      declaration
            11:4     public           keyword    
            11:11    int              type       
            11:15    $count           property   declaration
            11:24    0                number     
            12:8     get              method     declaration
            12:15    1                number     
            13:8     set              method     declaration
            13:15    2                number     
        "#]],
    );
}

#[test]
fn a_promoted_parameter_is_reported_as_the_parameter_it_is_written_as() {
    // It declares a property as well, but the property has no range of its own to report, and this
    // is the parameter list.
    check(
        r#"<?php

class Repo
{
    public function __construct(
        private readonly Base $base,
        public int $count = 0,
    ) {
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      class            keyword    
            3:6      Repo             class      declaration
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     __construct      method     declaration
            6:8      private          keyword    
            6:16     readonly         keyword    
            6:25     Base             class      
            6:30     $base            parameter  declaration readonly
            7:8      public           keyword    
            7:15     int              type       
            7:19     $count           parameter  declaration
            7:28     0                number     
        "#]],
    );
}
