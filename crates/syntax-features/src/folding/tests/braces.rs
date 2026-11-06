//! Folds that come from a delimiter pair.

use expect_test::expect;

use super::check;

#[test]
fn a_class_its_methods_and_the_blocks_inside_them() {
    check(
        r#"<?php

class Repo
{
    public function find(int $id): void
    {
        if ($id > 0) {
            return;
        }
    }
}
"#,
        expect![[r#"
            block   4:0-11:1  {
            block   6:4-10:5  {
            block   7:21-9:9  if ($id > 0) {
        "#]],
    );
}

#[test]
fn a_pair_that_fits_on_one_line_is_not_offered() {
    // Nothing to collapse, and an editor draws a fold arrow for every range it is given.
    check(
        "<?php\nclass Repo { public function find(): void {} }\n",
        expect![""],
    );
}

#[test]
fn every_class_like_declaration_folds() {
    check(
        r#"<?php

interface Reads
{
}

trait Counts
{
}

enum Suit: string
{
    case Hearts = 'h';
}

final class Repo implements Reads
{
    use Counts;
}
"#,
        expect![[r#"
            block   4:0-5:1  {
            block   8:0-9:1  {
            block   12:0-14:1  {
            block   17:0-19:1  {
        "#]],
    );
}

#[test]
fn arrays_in_all_three_spellings() {
    check(
        r#"<?php

$build = [
    '#type' => 'markup',
];
$legacy = array(
    'a' => 1,
);
list(
    $first,
    $second,
) = $pair;
"#,
        expect![[r#"
            block   3:9-5:1  $build = [
            block   6:15-8:1  $legacy = array(
            block   9:4-12:1  list(
        "#]],
    );
}

#[test]
fn match_and_both_spellings_of_switch() {
    check(
        r#"<?php

$name = match ($suit) {
    Suit::Hearts => 'hearts',
    default => 'other',
};

switch ($suit) {
    case 'h':
        break;
}

switch ($suit):
    case 'h':
        break;
endswitch;
"#,
        expect![[r#"
            block   3:22-6:1  $name = match ($suit) {
            block   8:15-11:1  switch ($suit) {
            block   13:14-16:10  switch ($suit):
        "#]],
    );
}

#[test]
fn the_alternative_syntax_folds_from_the_colon_to_the_closing_keyword() {
    // Common in template files, and it has no delimiter pair at all -- the whole clause is the
    // region, which is why these arms take the node's own span.
    check(
        r#"<?php

foreach ($items as $item):
    echo $item;
endforeach;

if ($ok):
    echo 'yes';
endif;

while ($ok):
    $ok = false;
endwhile;

for ($i = 0; $i < 2; $i++):
    echo $i;
endfor;
"#,
        expect![[r#"
            block   3:25-5:11  foreach ($items as $item):
            block   7:8-9:6  if ($ok):
            block   11:11-13:9  while ($ok):
            block   15:26-17:7  for ($i = 0; $i < 2; $i++):
        "#]],
    );
}

#[test]
fn a_wrapped_argument_or_parameter_list_folds_on_its_own() {
    check(
        r#"<?php

function build(
    string $type,
    array $options,
): void {
    render(
        $type,
        $options,
    );
}
"#,
        expect![[r#"
            block   3:14-6:1  function build(
            block   6:8-11:1  ): void {
            block   7:10-10:5  render(
        "#]],
    );
}

#[test]
fn closures_anonymous_classes_and_property_hooks() {
    check(
        r#"<?php

$make = function (int $id) {
    return $id;
};

$reader = new class {
    public int $count = 0 {
        get => $this->count;
    }
};
"#,
        expect![[r#"
            block   3:27-5:1  $make = function (int $id) {
            block   7:20-11:1  $reader = new class {
            block   8:26-10:5  public int $count = 0 {
        "#]],
    );
}

#[test]
fn a_trait_use_adaptation_block_folds() {
    check(
        r#"<?php

class Repo
{
    use Counts, Reads {
        Counts::total insteadof Reads;
    }
}
"#,
        expect![[r#"
            block   4:0-8:1  {
            block   5:22-7:5  use Counts, Reads {
        "#]],
    );
}

#[test]
fn a_heredoc_folds() {
    check(
        r#"<?php

$sql = <<<SQL
    SELECT 1
    SQL;
"#,
        expect![[r#"
            block   3:7-5:7  $sql = <<<SQL
        "#]],
    );
}

#[test]
fn a_buffer_that_is_briefly_invalid_still_folds_what_parsed() {
    // The moment an editor asks most. Half a declaration must not cost the folds of the rest of
    // the file.
    check(
        r#"<?php

class Repo
{
    public function find(
}

$build = [
    'a' => 1,
];
"#,
        expect![[r#"
            block   4:0-6:1  {
            block   8:9-10:1  $build = [
        "#]],
    );
}
