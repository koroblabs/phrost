//! What a docblock says, coloured.
//!
//! To a PHP grammar all of this is one comment, so every token here is one the editor could not have
//! had. The tag names are the reason the feature exists: `@phpstan-assert-if-true` is not a word any
//! grammar knows, and it reads as prose until something says otherwise.

use expect_test::expect;

use super::check;

#[test]
fn a_tag_its_type_and_the_variable_it_is_about() {
    check(
        r#"<?php
class Repo
{
    /**
     * Loads one.
     *
     * @param int $id The id to load.
     * @param \App\Filter|null $filter
     * @return \App\Item|null
     * @throws \RuntimeException
     */
    public function find(int $id, $filter = null) {}
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      class            keyword    
            2:6      Repo             class      declaration
            4:4      /**              comment    
            5:0           * Loads one. comment    
            6:0           *           comment    
            7:0           *           comment    
            7:7      @param           decorator  
            7:13                      comment    
            7:14     int              type       
            7:17                      comment    
            7:18     $id              parameter  
            7:21      The id to load. comment    
            8:0           *           comment    
            8:7      @param           decorator  
            8:13                      comment    
            8:14     \App\Filter|null type       
            8:30                      comment    
            8:31     $filter          parameter  
            9:0           *           comment    
            9:7      @return          decorator  
            9:14                      comment    
            9:15     \App\Item|null   type       
            10:0          *           comment    
            10:7     @throws          decorator  
            10:14                     comment    
            10:15    \RuntimeException type       
            11:0          */          comment    
            12:4     public           keyword    
            12:11    function         keyword    
            12:20    find             method     declaration
            12:25    int              type       
            12:29    $id              parameter  declaration
            12:34    $filter          parameter  declaration
            12:44    null             keyword    
        "#]],
    );
}

#[test]
fn the_vendor_prefixed_tags_a_grammar_has_never_heard_of() {
    check(
        r#"<?php
class App
{
    /**
     * @phpstan-assert-if-true \App\Special $this
     */
    public function isSpecial(): bool {}

    /**
     * @psalm-assert !null $value
     */
    public function assertNotNull(mixed $value): void {}
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      class            keyword    
            2:6      App              class      declaration
            4:4      /**              comment    
            5:0           *           comment    
            5:7      @phpstan-assert-if-true decorator  
            5:30                      comment    
            5:31     \App\Special     type       
            5:43                      comment    
            5:44     $this            parameter  
            6:0           */          comment    
            7:4      public           keyword    
            7:11     function         keyword    
            7:20     isSpecial        method     declaration
            7:33     bool             type       
            9:4      /**              comment    
            10:0          *           comment    
            10:7     @psalm-assert    decorator  
            10:20     !               comment    
            10:22    null             type       
            10:26                     comment    
            10:27    $value           parameter  
            11:0          */          comment    
            12:4     public           keyword    
            12:11    function         keyword    
            12:20    assertNotNull    method     declaration
            12:34    mixed            type       
            12:40    $value           parameter  declaration
            12:49    void             type       
        "#]],
    );
}

#[test]
fn a_template_names_a_type_that_exists_nowhere_else() {
    check(
        r#"<?php
/**
 * @template T of \App\Item
 * @extends \IteratorAggregate<T>
 */
interface Collection {}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      /**              comment    
            3:0       *               comment    
            3:3      @template        decorator  
            3:12                      comment    
            3:13     T                type       
            3:14      of              comment    
            3:18     \App\Item        type       
            4:0       *               comment    
            4:3      @extends         decorator  
            4:11                      comment    
            4:12     \IteratorAggregate<T> type       
            5:0       */              comment    
            6:0      interface        keyword    
            6:10     Collection       interface  declaration
        "#]],
    );
}

#[test]
fn a_method_tag_is_a_whole_signature() {
    check(
        r#"<?php
/**
 * @method \App\Item find(int $id, ?string $stage)
 * @property-read \App\Owner $owner
 */
class Magic {}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      /**              comment    
            3:0       *               comment    
            3:3      @method          decorator  
            3:10                      comment    
            3:11     \App\Item        type       
            3:20                      comment    
            3:21     find             method     
            3:25     (                comment    
            3:26     int              type       
            3:29                      comment    
            3:30     $id              parameter  
            3:33     ,                comment    
            3:35     ?string          type       
            3:42                      comment    
            3:43     $stage           parameter  
            3:49     )                comment    
            4:0       *               comment    
            4:3      @property-read   decorator  
            4:17                      comment    
            4:18     \App\Owner       type       
            4:28                      comment    
            4:29     $owner           property   
            5:0       */              comment    
            6:0      class            keyword    
            6:6      Magic            class      declaration
        "#]],
    );
}

/// Prose stays prose, and a tag whose value says nothing about a type stays prose with it.
///
/// The `@see` beside them is the exception and is why this fixture keeps both: the sentence names
/// `\App\Item` in the middle of it and gets no colour, because nothing there says it is a name --
/// while the tag written to point at one does. The test was named for the first half alone and
/// pinned the second as prose, which is what it had always been.
#[test]
fn prose_and_a_tag_with_no_type_in_it_are_left_to_the_grammar() {
    check(
        r#"<?php
/**
 * A sentence about \App\Item, which is not a type position.
 *
 * @see \App\Other::method()
 * @deprecated use something else
 */
class Plain {}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      /**              comment    
            3:0       * A sentence about \App\Item, which is not a type position. comment    
            4:0       *               comment    
            5:0       *               comment    
            5:3      @see             decorator  
            5:7                       comment    
            5:8      \App\Other       type       
            5:18     ::method()       comment    
            6:0       *               comment    
            6:3      @deprecated      decorator  
            6:14      use something else comment    
            7:0       */              comment    
            8:0      class            keyword    
            8:6      Plain            class      declaration
        "#]],
    );
}

/// The commonest tag in a framework project, and the one the grammar has no chance with: to it a
/// docblock is a comment, and `{@inheritdoc}` is a word in the middle of one.
///
/// `{@see}` beside it because an inline tag can carry a value, and the braces are then emitted as
/// two runs around it rather than as one span over the whole -- two tokens over one character have
/// no meaning in this protocol.
#[test]
fn an_inline_tag_is_a_tag_too() {
    check(
        r#"<?php
class Child extends Base
{
    /** {@inheritdoc} */
    public function label(): string {}

    /**
     * Does the thing.
     *
     * {@see \App\Base::label()}
     */
    public function other(): void {}
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      class            keyword    
            2:6      Child            class      declaration
            2:12     extends          keyword    
            2:20     Base             class      
            4:4      /**              comment    
            4:8      {@inheritdoc     decorator  
            4:20     }                decorator  
            4:21      */              comment    
            5:4      public           keyword    
            5:11     function         keyword    
            5:20     label            method     declaration
            5:29     string           type       
            7:4      /**              comment    
            8:0           * Does the thing. comment    
            9:0           *           comment    
            10:0          *           comment    
            10:7     {@see            decorator  
            10:12                     comment    
            10:13    \App\Base        type       
            10:22    ::label()        comment    
            10:31    }                decorator  
            11:0          */          comment    
            12:4     public           keyword    
            12:11    function         keyword    
            12:20    other            method     declaration
            12:29    void             type       
        "#]],
    );
}

/// Drupal's older plugin syntax, which to every grammar is one long grey run -- holding, among
/// other things, the id a whole subsystem is keyed by.
///
/// The tag's own name was already a decorator. What is new is the inside of the parentheses: the
/// key of each entry, every quoted value, and a nested annotation's name. The punctuation between
/// them keeps the comment's colour, which is what it is.
#[test]
fn an_annotation_shows_its_keys_its_strings_and_the_annotations_inside_it() {
    check(
        r#"<?php
/**
 * Provides REST resource for the region notice.
 *
 * @RestResource(
 *   id = "app_region_notice",
 *   label = @Translation("Region Notice."),
 *   uri_paths = {
 *     "canonical" = "/api/v1/region-notice"
 *   }
 * )
 */
class RegionNotice {}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      /**              comment    
            3:0       * Provides REST resource for the region notice. comment    
            4:0       *               comment    
            5:0       *               comment    
            5:3      @RestResource    decorator  
            5:16     (                comment    
            6:0       *               comment    
            6:5      id               property   
            6:7       =               comment    
            6:10     "app_region_notice" string     
            6:29     ,                comment    
            7:0       *               comment    
            7:5      label            property   
            7:10      =               comment    
            7:13     @Translation     decorator  
            7:25     (                comment    
            7:26     "Region Notice." string     
            7:42     ),               comment    
            8:0       *               comment    
            8:5      uri_paths        property   
            8:14      = {             comment    
            9:0       *               comment    
            9:7      "canonical"      string     
            9:18      =               comment    
            9:21     "/api/v1/region-notice" string     
            10:0      *   }           comment    
            11:0      * )             comment    
            12:0      */              comment    
            13:0     class            keyword    
            13:6     RegionNotice     class      declaration
        "#]],
    );
}

#[test]
fn a_type_alias_declared_and_then_imported() {
    check(
        r#"<?php
/**
 * @phpstan-type AddressComparisonResult array{
 *    original_address1: string,
 *    exact_match: bool,
 *    similarity_percentage: float,
 *  }
 */
class AddressMatcher
{
    /**
     * @return AddressComparisonResult
     */
    public function compare(): array {}
}

/**
 * @phpstan-import-type AddressComparisonResult from \App\AddressMatcher as Comparison
 */
class Report {}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      /**              comment    
            3:0       *               comment    
            3:3      @phpstan-type    decorator  
            3:16                      comment    
            3:17     AddressComparisonResult type       
            3:40                      comment    
            3:41     array{           type       
            4:0       *               comment    
            4:6      original_address1 property   
            4:23     : string,        type       
            5:0       *               comment    
            5:6      exact_match      property   
            5:17     : bool,          type       
            6:0       *               comment    
            6:6      similarity_percentage property   
            6:27     : float,         type       
            7:0       *               comment    
            7:4      }                type       
            8:0       */              comment    
            9:0      class            keyword    
            9:6      AddressMatcher   class      declaration
            11:4     /**              comment    
            12:0          *           comment    
            12:7     @return          decorator  
            12:14                     comment    
            12:15    AddressComparisonResult type       
            13:0          */          comment    
            14:4     public           keyword    
            14:11    function         keyword    
            14:20    compare          method     declaration
            14:31    array            type       
            17:0     /**              comment    
            18:0      *               comment    
            18:3     @phpstan-import-type decorator  
            18:23                     comment    
            18:24    AddressComparisonResult type       
            18:47     from            comment    
            18:53    \App\AddressMatcher type       
            18:72     as              comment    
            18:76    Comparison       type       
            19:0      */              comment    
            20:0     class            keyword    
            20:6     Report           class      declaration
        "#]],
    );
}

#[test]
fn the_tags_that_name_a_type_without_being_anything_s_type() {
    check(
        r#"<?php
/**
 * @template TValue of \App\Item = \App\Item
 * @where TValue is \App\Item
 * @sealed \App\Item
 * @inheritors \App\Special
 */
class Holder
{
    /**
     * @param-closure-this \App\Scope $run
     * @param-later-invoked-callable $run
     */
    public function each(callable $run) {}
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            2:0      /**              comment    
            3:0       *               comment    
            3:3      @template        decorator  
            3:12                      comment    
            3:13     TValue           type       
            3:19      of              comment    
            3:23     \App\Item        type       
            3:32      =               comment    
            3:35     \App\Item        type       
            4:0       *               comment    
            4:3      @where           decorator  
            4:9                       comment    
            4:10     TValue           type       
            4:16      is              comment    
            4:20     \App\Item        type       
            5:0       *               comment    
            5:3      @sealed          decorator  
            5:10                      comment    
            5:11     \App\Item        type       
            6:0       *               comment    
            6:3      @inheritors      decorator  
            6:14                      comment    
            6:15     \App\Special     type       
            7:0       */              comment    
            8:0      class            keyword    
            8:6      Holder           class      declaration
            10:4     /**              comment    
            11:0          *           comment    
            11:7     @param-closure-this decorator  
            11:26                     comment    
            11:27    \App\Scope       type       
            11:37                     comment    
            11:38    $run             parameter  
            12:0          *           comment    
            12:7     @param-later-invoked-callable decorator  
            12:36                     comment    
            12:37    $run             parameter  
            13:0          */          comment    
            14:4     public           keyword    
            14:11    function         keyword    
            14:20    each             method     declaration
            14:25    callable         type       
            14:34    $run             parameter  declaration
        "#]],
    );
}
