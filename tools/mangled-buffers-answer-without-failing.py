#!/usr/bin/env python3
"""Whether a buffer cut, holed or half-typed is still answered, without an error and in time.

    python3 tools/mangled-buffers-answer-without-failing.py <project-root> [--files 40] [--seed 7]
                                                               [--slow-ms 200]

An editor sends the server what a person is in the middle of typing: a call with no closing
parenthesis, a `->` with nothing after it, a file with its second half deleted. Every other tool
here asks about whole files, so none of them reaches the code paths that only a broken buffer
does. This one takes each sampled file, mangles it four times -- truncated at a random offset, a
span deleted, or one of the openers a person types first inserted -- and asks every capability
that takes a position at eight random positions of each version, plus the whole-file ones.

Two things are reported, and nothing needs an expectation written down for either:

* **an error the server answered with** -- an internal failure is a panic caught by the request
  loop, and it is the client's request that was lost. Being refused before bootstrap (-32801), a
  request cancelled (-32800) and an invalid one (-32600) are not failures.
* **a request slower than `--slow-ms`** -- a mangled buffer reaches code that a whole one does not,
  and a walk that is fine over a finished file can be the one that runs away over a half-written one.

It has found, so far: inlay hints slicing the file with a range that the parse's own repair had
written past the end of it, and `prepareRename` walking every test in Symfony to refuse a rename of
`setUp()` -- and flattening every Laravel model to learn whether one line was in each.

Deterministic for a seed, so a failure can be asked about again; run several seeds to sample more.
"""
import argparse
import collections
import os
import pathlib
import random
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lspclient  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
OPENERS = ["->", "::", "(", "'", '"', "{", "}", "$", "#[", "/**", "<<<EOT\n", "fn(", "match ("]
NOT_FAILURES = (-32801, -32800, -32600)


def position(text, offset):
    line = text.count("\n", 0, offset)
    return {"line": line, "character": offset - (text.rfind("\n", 0, offset) + 1)}


def mangled(full, rnd):
    cut = rnd.randrange(1, len(full) + 1)
    kind = rnd.choice(["truncate", "delete", "insert"])
    if kind == "truncate":
        return f"truncate@{cut}", full[:cut]
    if kind == "delete":
        return f"delete@{cut}", full[:cut] + full[cut + rnd.randrange(1, 40):]
    return f"insert@{cut}", full[:cut] + rnd.choice(OPENERS) + full[cut:]


def requests_for(uri, text, rnd):
    whole = {"textDocument": {"uri": uri}}
    asked = [
        ("textDocument/documentSymbol", whole),
        ("textDocument/foldingRange", whole),
        ("textDocument/semanticTokens/full", whole),
        ("textDocument/inlayHint",
         dict(whole, range={"start": {"line": 0, "character": 0}, "end": position(text, len(text))})),
        ("textDocument/formatting", dict(whole, options={"tabSize": 2, "insertSpaces": True})),
        ("textDocument/codeLens", whole),
    ]
    start = position(text, rnd.randrange(0, len(text) + 1))
    end = position(text, rnd.randrange(0, len(text) + 1))
    if (end["line"], end["character"]) < (start["line"], start["character"]):
        start, end = end, start
    asked += [
        ("textDocument/semanticTokens/range", dict(whole, range={"start": start, "end": end})),
        ("textDocument/rangeFormatting",
         dict(whole, range={"start": start, "end": end},
              options={"tabSize": 2, "insertSpaces": True})),
    ]
    for _ in range(8):
        at = dict(whole, position=position(text, rnd.randrange(0, len(text) + 1)))
        asked += [(method, at) for method in (
            "textDocument/hover", "textDocument/completion", "textDocument/definition",
            "textDocument/declaration", "textDocument/typeDefinition",
            "textDocument/implementation", "textDocument/documentHighlight",
            "textDocument/signatureHelp", "textDocument/prepareRename",
            "textDocument/prepareCallHierarchy", "textDocument/prepareTypeHierarchy")]
        asked.append(("textDocument/references", dict(at, context={"includeDeclaration": True})))
        asked.append(("textDocument/rename", dict(at, newName="renamedByTheFuzzer")))
        asked.append(("textDocument/selectionRange", dict(whole, positions=[at["position"]])))
        asked.append(("textDocument/codeAction", dict(
            whole, range={"start": at["position"], "end": at["position"]},
            context={"diagnostics": []})))
    return asked


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--slow-ms", type=float, default=200.0)
    parser.add_argument("--phrost", default=str(REPO / "target" / "release" / "phrost"))
    args = parser.parse_args()
    root = os.path.abspath(args.root)

    files = subprocess.run(["rg", "--files", "--sort", "path", "-g", "*.php"], cwd=root,
                           capture_output=True, text=True).stdout.split()
    files = [f for f in files
             if not lspclient.is_dependency_dir(os.path.dirname(os.path.join(root, f)), root)]
    rnd = random.Random(args.seed)
    rnd.shuffle(files)
    files = files[:args.files]

    logs = []
    server = lspclient.Server("phrost", [args.phrost, "serve"], root)
    server.on_notification(lambda method, params: method == "window/logMessage"
                           and logs.append(params["message"]))
    server.initialize({})
    server.wait_until(lambda: any(m.startswith("phrost indexed") for m in logs), timeout=300)

    failures = collections.Counter()
    failed, slow, asked = [], [], 0
    for rel in files:
        path = os.path.join(root, rel)
        full = open(path, encoding="utf-8", errors="replace").read()
        if not full:
            continue
        uri = lspclient.uri_of(path)
        server.open(path, full)
        for version in range(2, 6):
            how, text = mangled(full, rnd)
            server.notify("textDocument/didChange", {
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": text}]})
            pending = requests_for(uri, text, rnd)
            while pending:
                method, params = pending.pop(0)
                asked += 1
                started = time.perf_counter()
                answer = server.request(method, params, timeout=30)
                took = (time.perf_counter() - started) * 1000
                if took > args.slow_ms:
                    slow.append(f"{took:.0f} ms {method} {rel} {how}")
                error = answer.get("error")
                if error and error.get("code") not in NOT_FAILURES:
                    failures[error.get("code")] += 1
                    failed.append(f"{rel} {how} {method}: {error.get('message', '')[:300]}")
                # A hierarchy is asked a second time with what the first answered, which is what a
                # client does next -- and the item it hands back is the server's own, round-tripped.
                for item in (answer.get("result") or [])[:1] if isinstance(answer.get("result"), list) else []:
                    if method == "textDocument/prepareCallHierarchy":
                        pending += [("callHierarchy/incomingCalls", {"item": item}),
                                    ("callHierarchy/outgoingCalls", {"item": item})]
                    elif method == "textDocument/prepareTypeHierarchy":
                        pending += [("typeHierarchy/supertypes", {"item": item}),
                                    ("typeHierarchy/subtypes", {"item": item})]
        server.close(path)
    server.stop()

    print(f"asked {asked} over {len(files)} files; failed {sum(failures.values())}; "
          f"slower than {args.slow_ms:.0f} ms {len(slow)}")
    for line in failed[:20]:
        print("  failed", line)
    for line in slow[:10]:
        print("  slow", line)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
