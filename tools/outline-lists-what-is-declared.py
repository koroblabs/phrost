#!/usr/bin/env python3
"""Whether the outline lists every declaration the file writes, and the project can find it.

    python3 tools/outline-lists-what-is-declared.py <project-root> [--files 40]

Two invariants, neither needing an expected output. A file's `documentSymbol` answer must hold every
declaration a text scan of that same file finds -- a class, an interface, a trait, an enum, a
function, a method, a constant, a case -- because the outline is what an editor's breadcrumb, its
symbol list and its "go to symbol in file" all read. And `workspace/symbol` asked for a class's own
short name must return the class, because that is the only way a person reaches a file they have
not opened.

The scan is deliberately narrow: only declarations whose keyword is unambiguous at the start of a
line, so a `class` inside a string or a `function` used as a type never enters the sample. A miss is
then the server's, and an outline entry the scan did not predict is ignored rather than reported --
a server may know about more than text can see, and this is not a comparison.
"""
import argparse
import importlib.util
import json
import os
import re
import sys
import time
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lspclient import Closed, Server, Timeout, uri_of  # noqa: E402


def _sibling(name):
    spec = importlib.util.spec_from_file_location(
        name.replace("-", "_"), os.path.join(HERE, f"{name}.py")
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


compare = _sibling("compare-lsp")

REPO = os.path.dirname(HERE)
STILL_INDEXING = -32801

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {
        "workspaceFolders": True,
        "symbol": {"symbolKind": {"valueSet": list(range(1, 27))}},
    },
    "textDocument": {
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "documentSymbol": {
            "hierarchicalDocumentSymbolSupport": True,
            "symbolKind": {"valueSet": list(range(1, 27))},
        },
    },
}

# One pattern per kind, each anchored at the start of a line so a keyword inside an expression is
# never taken for a declaration. `function` is matched with and without modifiers, which is what
# separates a method from a free function without the scan having to track braces.
DECLARED = re.compile(
    r"""^[ \t]*
    (?:(?:final|abstract|readonly)[ \t]+)*
    (?P<kind>class|interface|trait|enum)[ \t]+(?P<name>[A-Za-z_]\w*)
    """,
    re.MULTILINE | re.VERBOSE,
)
FUNCTIONS = re.compile(
    r"""^[ \t]*
    (?:(?:final|abstract|public|protected|private|static)[ \t]+)*
    function[ \t]*&?[ \t]*(?P<name>[A-Za-z_]\w*)[ \t]*\(
    """,
    re.MULTILINE | re.VERBOSE,
)
CASES = re.compile(r"^[ \t]*case[ \t]+(?P<name>[A-Za-z_]\w*)[ \t]*[=;]", re.MULTILINE)
CONSTANTS = re.compile(
    r"^[ \t]*(?:(?:final|public|protected|private)[ \t]+)*const[ \t]+"
    r"(?:[A-Za-z_][\w\\|]*[ \t]+)?(?P<name>[A-Za-z_]\w*)[ \t]*=",
    re.MULTILINE,
)


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        self.opened = {}
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "outline-probe.stderr.log"),
        )
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "$/progress":
            token = json.dumps(params.get("token"))
            kind = (params.get("value") or {}).get("kind")
            if kind == "begin":
                self.working.add(token)
            elif kind == "end":
                self.working.discard(token)
                self.ready = self.ready or "progress"

    def start(self, probe_path, probe_name):
        self.server.initialize(CAPABILITIES)
        text = read(probe_path)
        lines = compare.Lines(text)
        at = compare.mask(text).find(probe_name)
        self.open(probe_path)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", {
                "textDocument": {"uri": uri_of(probe_path)},
                "position": lines.position(at),
            }):
                return True
        return False

    def open(self, path):
        if path not in self.opened:
            self.opened[path] = read(path)
            self.server.open(path, self.opened[path])
        return self.opened[path]

    def ask(self, method, params):
        for _ in range(40):
            try:
                answer = self.server.request(method, params, timeout=30.0)
            except (Timeout, Closed):
                return None
            if answer.get("error", {}).get("code") == STILL_INDEXING:
                self.server.pump(0.5)
                continue
            return answer.get("result")
        return None


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def flatten(symbols):
    """Every name in a documentSymbol answer, whichever of the two shapes it arrived in."""
    out = []
    for symbol in symbols or []:
        out.append(symbol.get("name", ""))
        out.extend(flatten(symbol.get("children")))
    return out


INLINE = re.compile(r"\?>.*?(?:<\?php|<\?=|\Z)", re.DOTALL)


def php_only(masked):
    """Blanks everything between a `?>` and the next opening tag, keeping offsets and newlines.

    WordPress writes JavaScript there, and `function pickColor(color) {` inside a `<script>` block
    is not a declaration any PHP server should list. `mask` blanks strings and comments and knows
    nothing about leaving PHP mode, so the scan read three such functions as missing from an outline
    that was right to omit them.
    """
    out = list(masked)
    for match in INLINE.finditer(masked):
        for index in range(match.start(), match.end()):
            if out[index] != "\n":
                out[index] = " "
    return "".join(out)


def declared_in(masked):
    """Every declaration a text scan can be sure of, as (kind, name)."""
    found = []
    for match in DECLARED.finditer(masked):
        found.append((match.group("kind"), match.group("name")))
    for match in FUNCTIONS.finditer(masked):
        found.append(("function", match.group("name")))
    for match in CASES.finditer(masked):
        found.append(("case", match.group("name")))
    for match in CONSTANTS.finditer(masked):
        found.append(("const", match.group("name")))
    return found


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--needle", default="")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {path: read(path) for path in paths}
    probe = compare.probe_of({path: (text, None) for path, text in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    session = Probe(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered about the project")
        return 1

    asked = 0
    missing = []
    for path, text in plan.items():
        wanted = declared_in(php_only(compare.mask(text)))
        if not wanted:
            continue
        session.open(path)
        listed = flatten(session.ask(
            "textDocument/documentSymbol", {"textDocument": {"uri": uri_of(path)}}
        ))
        # By name alone: a server is free to render `m()` or `Foo::m`, and what this is about is
        # whether the declaration is in the answer at all.
        names = {name.split("(")[0].split("::")[-1].lstrip("$").strip() for name in listed}
        for kind, name in wanted:
            asked += 1
            if name not in names:
                missing.append((kind, name, path, len(listed)))

    print()
    print(f"outline             {asked - len(missing)} of {asked} declarations listed")
    if missing:
        print()
        for kind, count in Counter(kind for kind, *_ in missing).most_common():
            print(f"  {count:5}  {kind}")
        print()
        seen = set()
        for kind, name, path, listed in missing:
            if kind in seen and not args.verbose:
                continue
            seen.add(kind)
            print(f"  {kind:10} {name:30} {os.path.relpath(path, args.root)} ({listed} listed)")

    # And the project-wide half: a class the project declares has to be reachable by its own name.
    classes = []
    for path, text in plan.items():
        for kind, name in declared_in(php_only(compare.mask(text))):
            if kind in ("class", "interface", "trait", "enum"):
                classes.append((name, path))
    classes = compare.stride(classes, 60)
    unfound = []
    for name, path in classes:
        answer = session.ask("workspace/symbol", {"query": name})
        if not any(one.get("name", "").split("\\")[-1] == name for one in answer or []):
            unfound.append((name, path))
    session.server.stop()

    print()
    print(f"workspace symbol    {len(classes) - len(unfound)} of {len(classes)} classes found by name")
    for name, path in unfound[:15]:
        print(f"  {name:30} {os.path.relpath(path, args.root)}")
    return 1 if missing or unfound else 0


if __name__ == "__main__":
    sys.exit(main())
