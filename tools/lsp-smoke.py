#!/usr/bin/env python3
"""Drives the real server over stdio and reports what came back.

An end-to-end check of the one path nothing else covers: the protocol itself. The Rust tests reach
the analysis layer, but a request that never gets dispatched, a capability never advertised, or a
position converted the wrong way all look fine to them.

    python3 tools/lsp-smoke.py <project-root> <file> <line> <character>

Line and character are zero-based, as LSP counts them.

This is deliberately a script and not a Rust test. In-process flow tests -- driving the service with
a fake transport to cover version guards, cancellation, and requests arriving mid-bootstrap -- are
worth having and are a separate piece of work; this covers "does it answer at all" today.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import Server, uri_of  # noqa: E402

DEFAULT_ROOT = "."

ROOT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT)
FILE = os.path.join(ROOT, sys.argv[2] if len(sys.argv) > 2 else "SPL/SPL.php")
LINE = int(sys.argv[3]) if len(sys.argv) > 3 else 12
CHARACTER = int(sys.argv[4]) if len(sys.argv) > 4 else 31

if not os.path.isdir(ROOT):
    sys.exit(f"no such project root: {ROOT}")

# Absolute, because the server runs with the *project* as its working directory: a relative path
# only worked while this happened to be invoked from the repo root.
BINARY = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "target/release/phrost"
)

server = Server("phrost", [BINARY, "serve"], ROOT)


def heard(method, params):
    """Prints the two things bootstrap says about itself, and swallows the rest."""
    if method == "window/logMessage":
        message = params.get("message", "")
        if message.startswith("phrost indexed"):
            print("summary:", message)
    elif method == "$/progress":
        value = params.get("value") or {}
        kind = value.get("kind")
        if kind in ("begin", "end"):
            print(f"progress {kind}: {value.get('title', '')} {value.get('message', '')}".strip())
        elif value.get("percentage", 0) in (50, 100):
            print(f"progress report: {value.get('message')} ({value['percentage']}%)")


server.on_notification(heard)

bootstrap_started = time.monotonic()
result = server.initialize({"window": {"workDoneProgress": True}})
caps = result.get("capabilities", {})
print("initialize ok:", (result.get("serverInfo") or {}).get("name"),
      "definition:", caps.get("definitionProvider"), "hover:", caps.get("hoverProvider"))

with open(FILE, encoding="utf-8") as handle:
    server.open(FILE, handle.read())

position = {
    "textDocument": {"uri": uri_of(FILE)},
    "position": {"line": LINE, "character": CHARACTER},
}
# A cross-file request arriving before bootstrap finishes is refused on purpose, and the server
# answers `initialized` as a notification -- so a client that fires straight away races the index.
# A real editor retries as the user keeps typing; this waits, and says so if it never becomes ready.
STILL_INDEXING = -32801
bootstrap_reported = False


def report_ready():
    """Prints how long bootstrap took, once, when the first request is finally answered."""
    global bootstrap_reported
    if not bootstrap_reported:
        bootstrap_reported = True
        print(f"ready after: {(time.monotonic() - bootstrap_started) * 1000:.0f}ms")


def ask(method, params=None):
    if params is None:
        params = position
    for attempt in range(60):
        started = time.monotonic()
        response = server.request(method, params, timeout=60.0)
        elapsed = (time.monotonic() - started) * 1000
        if response.get("error", {}).get("code") != STILL_INDEXING:
            report_ready()
            print(f"{method}: {elapsed:.1f}ms" + (f" after {attempt} retries" if attempt else ""))
            return response
        time.sleep(0.5)
    sys.exit(f"{method}: still indexing after 30s")


definition = ask("textDocument/definition")
hover = ask("textDocument/hover")
completion = ask("textDocument/completion")
outline = ask("textDocument/documentSymbol", {"textDocument": {"uri": uri_of(FILE)}})
found = ask("workspace/symbol", {"query": "Applic"})
renamed = ask(
    "textDocument/rename",
    {
        "textDocument": {"uri": uri_of(FILE)},
        "position": {"line": LINE, "character": CHARACTER},
        "newName": "RenamedBySmokeTest",
    },
)
refs = ask(
    "textDocument/references",
    {
        "textDocument": {"uri": uri_of(FILE)},
        "position": {"line": LINE, "character": CHARACTER},
        "context": {"includeDeclaration": True},
    },
)
signature = ask(
    "textDocument/signatureHelp",
    {
        "textDocument": {"uri": uri_of(FILE)},
        "position": {"line": LINE, "character": CHARACTER},
    },
)
folds = ask("textDocument/foldingRange", {"textDocument": {"uri": uri_of(FILE)}})
names = ask("textDocument/semanticTokens/full", {"textDocument": {"uri": uri_of(FILE)}})
hints = ask(
    "textDocument/inlayHint",
    {
        "textDocument": {"uri": uri_of(FILE)},
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 100000, "character": 0},
        },
    },
)

ranges = folds.get("result") or []
kinds = {}
for fold in ranges:
    kinds[fold.get("kind") or "block"] = kinds.get(fold.get("kind") or "block", 0) + 1
print("folding:", len(ranges), "ranges:", ", ".join(f"{k} {n}" for k, n in sorted(kinds.items())))

# The server's legend, in its order -- a client is told the list once and then receives indices, so
# a short list here does not fail, it just reports most of the file as `?`.
TOKEN_TYPES = ["namespace", "class", "interface", "enum", "enumMember", "property",
               "method", "function", "parameter", "variable", "decorator", "type",
               "keyword", "comment", "string", "number"]
data = (names.get("result") or {}).get("data") or []
histogram = {}
for index in range(0, len(data), 5):
    kind = TOKEN_TYPES[data[index + 3]] if data[index + 3] < len(TOKEN_TYPES) else "?"
    histogram[kind] = histogram.get(kind, 0) + 1
print("tokens:", len(data) // 5, "names:",
      ", ".join(f"{k} {n}" for k, n in sorted(histogram.items(), key=lambda kv: -kv[1])))

drawn = hints.get("result") or []
kinds = {}
for hint in drawn:
    kinds[{1: "type", 2: "parameter"}.get(hint.get("kind"), "?")] = \
        kinds.get({1: "type", 2: "parameter"}.get(hint.get("kind"), "?"), 0) + 1
print("inlay:", len(drawn), "hints:", ", ".join(f"{k} {n}" for k, n in sorted(kinds.items())))

print("definition:", json.dumps(definition.get("result"))[:400])
contents = (hover.get("result") or {}).get("contents") or {}
print("hover:", json.dumps(contents.get("value"))[:800])
items = (completion.get("result") or {}).get("items", [])
print("completion:", len(items), "items:", ", ".join(item["label"] for item in items[:8]))
for item in items[:3]:
    doc = ((item.get("documentation") or {}).get("value") or "").replace("\n", " ⏎ ")
    print(f"   {item['label']}: detail={item.get('detail')!r} doc={doc!r}")
for item in items[:3]:
    edit = item.get("textEdit") or {}
    r = edit.get("range", {})
    print(
        "   edit:",
        f'{r.get("start", {}).get("line")}:{r.get("start", {}).get("character")}'
        f'-{r.get("end", {}).get("line")}:{r.get("end", {}).get("character")}',
        "->",
        json.dumps(edit.get("newText")),
    )

top = outline.get("result") or []
print("outline:", len(top), "top-level:", ", ".join(entry["name"] for entry in top[:6]))
hits = found.get("result") or []
print("symbol search:", len(hits), "hits:", ", ".join(hit["name"] for hit in hits[:5]))
changes = ((renamed.get("result") or {}).get("changes")) or {}
print(
    "rename: files",
    len(changes),
    "edits",
    sum(len(v) for v in changes.values()),
    ("error: " + renamed.get("error", {}).get("message", "")) if renamed.get("error") else "",
)
places = refs.get("result") or []
print(
    "references:",
    len(places),
    "first:",
    ", ".join(
        f'{p["uri"].rsplit("/", 1)[-1]}:{p["range"]["start"]["line"] + 1}' for p in places[:6]
    ),
)
help_ = signature.get("result") or {}
signatures = help_.get("signatures") or []
print(
    "signature help:",
    signatures[0]["label"] if signatures else "(none)",
    "active:",
    help_.get("activeParameter"),
)

drawn = hints.get("result") or []
print(
    "inlay hints:",
    len(drawn),
    "first:",
    ", ".join(f'{h["position"]["line"] + 1}:{h["label"]}' for h in drawn[:6]),
)

server.stop()
print("exit code:", server.process.returncode)
