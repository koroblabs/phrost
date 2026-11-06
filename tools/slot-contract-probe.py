"""Completion on a receiver whose type comes only from an `@assert` contract.

Three receivers on one running server: a bare local, a hinted parameter, and an array element. All
three answer, and the third is the one worth checking -- it was 0 items until
`written_path` stopped looking a literal element key up in the *project's* interner
(`phrost_body::element_key_name`).

Kept as a driven server rather than folded into the test suite, because the suite has the same
check at the flow layer and this is the instrument that found the bug: a real `serve` over a
throwaway project, which is what neither `phrost query` nor the in-process harnesses could
reproduce.

    python3 tools/slot-contract-probe.py

Expected:

    $bare->                ->   1 items ['getImage']
    $node->                ->   1 items ['getImage']
    $constants['C']->      ->   1 items ['getImage']
"""


import sys, os, time, tempfile, shutil
sys.path.insert(0, os.path.join(os.getcwd(), "tools"))
from lspclient import Server, uri_of

BODY = """<?php

class ASTNode {
    public function getImage(): string { return ''; }
}

class BaseAssert
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, object $actual): void
    {
    }
}

class MyTestCase extends BaseAssert
{
    /**
     * @param array<string, mixed> $constants
     * @param mixed $bare
     */
    public function localCase($bare): void
    {
        static::assertInstanceOf(ASTNode::class, $bare);
        $bare->
    }

    public function nodeCase(ASTNode $node): void
    {
        $node->
    }

    /**
     * @param array<string, mixed> $constants
     */
    public function slotCase(array $constants): void
    {
        static::assertInstanceOf(ASTNode::class, $constants['C']);
        $constants['C']->
    }
}
"""
root = tempfile.mkdtemp(prefix="drive4-")
path = os.path.join(root, "fixture.php")
open(path, "w").write(BODY)
srv = Server("phrost", [os.path.abspath("target/release/phrost"), "serve"], root)
srv.initialize({"textDocument": {"completion": {"completionItem": {"snippetSupport": True}}}})
srv.open(path, BODY)
time.sleep(2.0)
lines = BODY.split("\n")
for i, l in enumerate(lines):
    if l.rstrip().endswith("->"):
        for _ in range(8):
            r = srv.request("textDocument/completion", {
                "textDocument": {"uri": uri_of(path)},
                "position": {"line": i, "character": len(l)}})
            if (r.get("error") or {}).get("code") == -32801:
                time.sleep(0.5); continue
            items = r.get("result") or []
            if isinstance(items, dict): items = items.get("items", [])
            print(f"{l.strip():<22} -> {len(items):3} items {[x['label'] for x in items][:5]}")
            break
srv.stop(); shutil.rmtree(root, ignore_errors=True)
