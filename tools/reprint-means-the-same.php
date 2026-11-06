<?php
/**
 * Whether a reprinted tree means what the written one did.
 *
 *     target/release/phrost reprint <dir> <out>
 *     php tools/reprint-means-the-same.php <project with nikic/php-parser> <out>/before <out>/after
 *
 * `formatting-preserves-the-code.py` asks whether a reprint still *parses* and whether it settles;
 * this asks whether it computes the same thing. Each file is parsed by nikic/php-parser -- which
 * any project with PHPStan or PHPUnit vendors, and which applies PHP's own rules to a heredoc's
 * indentation and a string's escapes -- with positions and comments dropped, and the two trees
 * compared. The respellings the formatter makes on purpose are normalised first, and each is one
 * that changes nothing PHP computes:
 *
 * - the order of `use` imports, and a group `use A\{B, C}` written as two lines;
 * - `X|null` written `?X`, and a union's operands in any order;
 * - `TRUE` written `true`;
 * - `use A, B;` in a class split into one `use` per trait, where neither adapts anything;
 * - an empty slot at the end of `list()`, and a stray `;`;
 * - parentheses `&&`, `||`, `and`, `or` and `.` make redundant -- the same values, evaluated in
 *   the same order.
 *
 * Anything else that differs is a change of meaning, and so is a file that no longer parses. Run
 * first over five corpora on 2026-10-07, it found five: an `instanceof` right-hand side whose
 * parentheses PHP needs, a heredoc indented with tabs and spaces together, a trailing blank line of
 * template output, a closing tag whose removal turned output into whitespace, and an empty file
 * given a newline. Every one was invisible to `phpcs` and four of them to `php -l`. A sixth came
 * from Laravel's own vendor tree the same day: `$a + ($b - $c) + $d` reprinted without its
 * parentheses, which for a float is a different number.
 *
 * Exits 1 when any file differs.
 */
require $argv[1] . '/vendor/autoload.php';
use PhpParser\ParserFactory;
use PhpParser\NodeTraverser;
use PhpParser\NodeVisitorAbstract;
use PhpParser\Node;

$parser = (new ParserFactory())->createForNewestSupportedVersion();
$strip = new class extends NodeVisitorAbstract {
    public function enterNode(Node $node) { $node->setAttributes([]); return null; }
    // `X|null` and `?X` are one type; the formatter writes the second.
    public function leaveNode(Node $node) {
        // `TRUE` and `true` are one constant; the formatter lowercases them.
        if ($node instanceof PhpParser\Node\Expr\ConstFetch && in_array(strtolower($node->name->toString()), ['true', 'false', 'null'], true)) {
            $node->name = new PhpParser\Node\Name(strtolower($node->name->toString()));
        }
        // An empty slot at the end of `list()` assigns nothing.
        if ($node instanceof PhpParser\Node\Expr\List_) {
            while ($node->items !== [] && end($node->items) === null) { array_pop($node->items); }
        }
        // A stray `;` is a statement that does nothing, wherever it sits.
        foreach (['stmts'] as $field) {
            if (property_exists($node, $field) && is_array($node->$field)) {
                $node->$field = array_values(array_filter($node->$field, fn ($s) => !($s instanceof PhpParser\Node\Stmt\Nop)));
            }
        }
        // `use A, B;` in a class is `use A; use B;` when neither adapts anything: one set of traits.
        if ($node instanceof PhpParser\Node\Stmt\ClassLike) {
            $traits = []; $rest = [];
            foreach ($node->stmts as $stmt) {
                if ($stmt instanceof PhpParser\Node\Stmt\TraitUse && $stmt->adaptations === []) {
                    foreach ($stmt->traits as $trait) { $traits[] = $trait->toString(); }
                } else { $rest[] = $stmt; }
            }
            sort($traits);
            $node->stmts = $rest;
            $node->setAttribute('traits', $traits);
        }
        // A union's operands in any order are one type.
        if ($node instanceof PhpParser\Node\UnionType) {
            usort($node->types, fn ($a, $b) => strcmp(json_encode($a), json_encode($b)));
        }
        if ($node instanceof PhpParser\Node\UnionType && count($node->types) === 2) {
            foreach ($node->types as $i => $type) {
                if ($type instanceof PhpParser\Node\Identifier && strtolower($type->name) === 'null') {
                    return new PhpParser\Node\NullableType($node->types[1 - $i]);
                }
            }
        }
        return null;
    }
};
$dump = function (string $code) use ($parser, $strip): ?string {
    try { $ast = $parser->parse($code); } catch (Throwable $e) { return 'PARSE ERROR: ' . $e->getMessage(); }
    $t = new NodeTraverser(); $t->addVisitor($strip); $ast = $t->traverse($ast);
    $ast = array_values(array_filter($ast, fn ($s) => !($s instanceof PhpParser\Node\Stmt\Nop)));
    // The order of `use` statements decides nothing, and the formatter sorts them.
    $sortUses = function (array $stmts) use (&$sortUses): array {
        $uses = []; $rest = [];
        foreach ($stmts as $stmt) {
            // One entry per imported name, so `use A\{B, C as D};` and two `use` lines read alike.
            if ($stmt instanceof PhpParser\Node\Stmt\Use_) {
                foreach ($stmt->uses as $use) { $uses[] = ($stmt->type ?: $use->type) . '|' . $use->name->toString() . '|' . ($use->alias?->toString() ?? ''); }
            } elseif ($stmt instanceof PhpParser\Node\Stmt\GroupUse) {
                foreach ($stmt->uses as $use) { $uses[] = ($stmt->type ?: $use->type) . '|' . $stmt->prefix->toString() . '\\' . $use->name->toString() . '|' . ($use->alias?->toString() ?? ''); }
            }
            else {
                if ($stmt instanceof PhpParser\Node\Stmt\Namespace_) { $stmt->stmts = $sortUses($stmt->stmts ?? []); }
                $rest[] = $stmt;
            }
        }
        sort($uses);
        return [$uses, $rest];
    };
    // `a && (b && c)` is `a && b && c`: the same values, evaluated in the same order. Flattened for
    // the operators where that holds, so a formatter dropping such parentheses reads as nothing.
    $flat = function ($value) use (&$flat) {
        if (!is_array($value)) { return $value; }
        $assoc = ['Expr_BinaryOp_BooleanAnd', 'Expr_BinaryOp_BooleanOr', 'Expr_BinaryOp_LogicalAnd', 'Expr_BinaryOp_LogicalOr', 'Expr_BinaryOp_Concat'];
        if (isset($value['nodeType']) && in_array($value['nodeType'], $assoc, true)) {
            $kind = $value['nodeType'];
            $operands = [];
            $collect = function ($side) use (&$collect, &$operands, $kind, $flat) {
                if (is_array($side) && ($side['nodeType'] ?? null) === $kind) { $collect($side['left']); $collect($side['right']); }
                else { $operands[] = $flat($side); }
            };
            $collect($value['left']); $collect($value['right']);
            return ['nodeType' => $kind, 'operands' => $operands];
        }
        return array_map($flat, $value);
    };
    return json_encode($flat(json_decode(json_encode($sortUses($ast)), true)));
};
[$_, $root, $before, $after] = $argv;
$it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($before, FilesystemIterator::SKIP_DOTS));
$checked = 0; $differ = [];
foreach ($it as $file) {
    $path = $file->getPathname();
    $rel = substr($path, strlen($before));
    $other = $after . $rel;
    if (!is_file($other)) continue;
    $a = file_get_contents($path); $b = file_get_contents($other);
    if ($a === $b) continue;
    $checked++;
    if ($dump($a) !== $dump($b)) $differ[] = $rel;
}
echo "reprinted files compared: $checked, semantically different: " . count($differ) . "\n";
foreach (array_slice($differ, 0, 20) as $d) echo "  $d\n";
exit($differ === [] ? 0 : 1);
