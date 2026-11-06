use expect_test::expect;

use super::check_hir;

#[test]
fn method_and_property_tags_declare_members() {
    check_hir(
        r#"
//- /src/C.php
<?php
/**
 * @method InvocationStubber method($constraint)
 * @method static self make(string $name, int $times = 1)
 * @property-read string $name
 * @property int|null $count
 */
interface Stub
{
    public function expects(int $order): void;
}
"#,
        expect![[r#"
            interface Stub @ 8:11-8:15  [doc]
              method method($constraint): InvocationStubber  public @ 3:30-3:36
              method make(string $name, int $times = ...): self  public static @ 4:24-4:28
              property $name: string  public private(set) @ 5:26-5:31
              property $count: int|null  public @ 6:23-6:29
              method expects(int $order): void  public @ 10:21-10:28
        "#]],
    );
}

#[test]
fn a_written_declaration_wins_over_a_tag_claiming_the_same_name() {
    // Flattening lets a later declaration replace an earlier one, so the tags are emitted first --
    // which is also where they are written. It is the written method that runs.
    check_hir(
        r#"
//- /src/C.php
<?php
/**
 * @method void run()
 */
class Service
{
    public function run(): int {}
}
"#,
        expect![[r#"
            class Service @ 5:7-5:14  [doc]
              method run(): void  public @ 3:17-3:20
              method run(): int  public @ 7:21-7:24
        "#]],
    );
}

#[test]
fn a_docblock_without_member_tags_is_never_parsed_for_them() {
    check_hir(
        r#"
//- /src/C.php
<?php
/**
 * Ordinary prose, with an @author tag and no members.
 */
class Service
{
}
"#,
        expect![[r#"
            class Service @ 5:7-5:14  [doc]
        "#]],
    );
}
