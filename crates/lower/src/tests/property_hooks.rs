//! PHP 8.4 property hooks, lowered as scopes of their own.
//!
//! Nothing covered these before, which is why the gap went unnoticed: the generic children walk
//! stepped around every hook body, so goto, hover, rename and every diagnostic were silent inside
//! one. The bodies below are what would have caught it.

use expect_test::expect;

use super::{check_bodies, check_hir};

#[test]
fn a_hook_body_is_lowered_into_the_hook_and_not_into_the_file() {
    // The whole of the bug: the hook bodies used to appear nowhere at all, and `?? statement` for
    // the class was the only trace of the class in the file's own body. It still is -- what changed
    // is that the hooks now have bodies of their own, and the file's body must *not* have gained
    // copies of their expressions.
    check_bodies(
        r#"
//- /src/Person.php
<?php
class Person
{
    private string $raw = '';

    public string $name {
        get => strtoupper($this->raw);
        set(string $value) => $this->raw = trim($value);
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
                  string @ 4:27-4:29
            Person::$name::get()
              body @ 7:16-7:38
                return @ 7:16-7:38
                  call strtoupper() @ 7:16-7:38
                    args
                      property ->raw @ 7:27-7:37
                        receiver
                          $this @ 7:27-7:32
            Person::$name::set($value)
              body @ 8:31-8:56
                assign @ 8:31-8:56
                  property ->raw @ 8:31-8:41
                    receiver
                      $this @ 8:31-8:36
                  =
                    call trim() @ 8:44-8:56
                      args
                        $value @ 8:49-8:55
        "#]],
    );
}

#[test]
fn a_block_bodied_hook_holds_its_statements() {
    check_bodies(
        r#"
//- /src/Person.php
<?php
class Person
{
    private string $raw = '';

    public string $name {
        get {
            return $this->raw;
        }
        set {
            $this->raw = $value;
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-15:1
                ?? declaration (arrival) @ 2:1-14:2
                  string @ 4:27-4:29
            Person::$name::get()
              body @ 7:13-9:10
                return @ 8:13-8:31
                  property ->raw @ 8:20-8:30
                    receiver
                      $this @ 8:20-8:25
            Person::$name::set($value)
              body @ 10:13-12:10
                assign @ 11:13-11:32
                  property ->raw @ 11:13-11:23
                    receiver
                      $this @ 11:13-11:18
                  =
                    $value @ 11:26-11:32
        "#]],
    );
}

#[test]
fn an_abstract_hook_has_no_body() {
    // `get;` in an interface declares that implementers must provide one. Distinguishing that from
    // "nobody asked for bodies" is the same rule a method's body follows.
    check_bodies(
        r#"
//- /src/HasName.php
<?php
interface HasName
{
    public string $name { get; }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-6:1
                ?? declaration (arrival) @ 2:1-5:2
            HasName::$name::get()
              <no body>
        "#]],
    );
}

#[test]
fn a_hook_is_visible_in_the_declaration_snapshot() {
    // The declaration side, which is lowered even in skeleton mode: only the body is not. The
    // implicit `$value` is there too, and it carries the property's own type -- PHP says so, and
    // the source does not.
    check_hir(
        r#"
//- /src/Person.php
<?php
class Person
{
    public string $name {
        get => 'x';
        set { $this->raw = $value; }
    }

    public function __construct(public private(set) int $count { get => 1; }) {}
}
"#,
        expect![[r#"
            class Person @ 2:7-2:13
              property $name: string  public hooks @ 4:19-4:24
                hook get() @ 5:9-5:12
                hook set(string $value) @ 6:9-6:12
              method __construct(public int $count)  public @ 9:21-9:32
              property $count: int  public private(set) virtual-get @ 9:57-9:63
                hook get() @ 9:66-9:69
        "#]],
    );
}

#[test]
fn a_hook_on_a_promoted_parameter_belongs_to_the_property_it_declares() {
    check_bodies(
        r#"
//- /src/Money.php
<?php
class Money
{
    public function __construct(
        public int $cents {
            get => $this->cents * 2;
        },
    ) {}
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Money::__construct
              body @ 8:7-8:9
            Money::$cents::get()
              body @ 6:20-6:36
                return @ 6:20-6:36
                  arithmetic * @ 6:20-6:36
                    property ->cents @ 6:20-6:32
                      receiver
                        $this @ 6:20-6:25
                    int @ 6:35-6:36
        "#]],
    );
}

/// Backed or virtual, which is a fact about the *text* of the hooks and nothing else.
///
/// PHP's own rule: a property is backed if any of its hooks uses `$this->propName`, and virtual
/// otherwise. Read here because a hook's body is lowered only for the project's own files, and the
/// distinction has to hold for a dependency's class as much as for one's own -- so it comes off the
/// written span, which every file has.
#[test]
fn a_hook_that_uses_the_backing_store_makes_the_property_backed() {
    check_hir(
        r#"
//- /src/Lazy.php
<?php
class Lazy
{
    public ?string $cached {
        get { return $this->cached ??= 'computed'; }
    }

    public string $computed {
        get => $this->raw;
    }

    public string $cachedPrefix {
        get => 'x' . $this->cachedPrefixOther;
    }

    public string $required { get; }
}
"#,
        expect![[r#"
            class Lazy @ 2:7-2:11
              property $cached: ?string  public backed-get @ 4:20-4:27
                hook get() @ 5:9-5:12
              property $computed: string  public virtual-get @ 8:19-8:28
                hook get() @ 9:9-9:12
              property $cachedPrefix: string  public virtual-get @ 12:19-12:32
                hook get() @ 13:9-13:12
              property $required: string  public declared-get @ 16:19-16:28
                hook get() @ 16:31-16:34
        "#]],
    );
}
