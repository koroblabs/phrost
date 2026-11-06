#!/usr/bin/env python3
"""Whether `implementation` lists every class that writes the name in its own header.

    python3 tools/implementers-are-what-is-written.py <project-root> [--files 60]

Direct inheritance is a fact a text scan can state exactly: a file writing `class C extends S`,
`implements S` or `use S;` inside a class body has said so, and asking `textDocument/implementation`
at S's own declaration must hand C back. No expected output is needed and nothing about the project
has to be known -- which is what makes this runnable against any checkout.

Only supertypes the project itself declares, because the question is asked *at* the declaration and
a vendor one is not somewhere this can put a cursor. And only names written unqualified or under an
import this scan can resolve, so a name it cannot be sure about is left out rather than guessed at.
A supertype whose answer is at the hundred-implementer cap is skipped: a truncated list is not a
missing one.
"""
import argparse
import importlib.util
import json
import os
import re
import sys
import time
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lspclient import Closed, Server, Timeout, path_of, uri_of  # noqa: E402


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
# `IMPLEMENTERS` in `phrost_analysis::navigation`. A list at the cap is truncated, not short.
CAP = 100

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "implementation": {"linkSupport": False},
    },
}

DECLARES = re.compile(
    r"^[ \t]*(?:(?:final|abstract|readonly)[ \t]+)*"
    r"(?:class|interface|trait|enum)[ \t]+(?P<name>[A-Za-z_]\w*)",
    re.MULTILINE,
)
# The header of a class-like, up to the `{` that opens its body. Everything between is where
# `extends` and `implements` are written, however many lines it takes.
HEADER = re.compile(
    r"^[ \t]*(?:(?:final|abstract|readonly)[ \t]+)*"
    r"(?:class|interface|trait|enum)[ \t]+(?P<name>[A-Za-z_]\w*)(?P<rest>[^{;]*)",
    re.MULTILINE,
)
USES = re.compile(r"^[ \t]+use[ \t]+(?P<names>[A-Za-z_][\w\\ \t,]*);", re.MULTILINE)
IMPORTS = re.compile(
    r"^[ \t]*use[ \t]+(?P<path>[A-Za-z_][\w\\]*)(?:[ \t]+as[ \t]+(?P<alias>[A-Za-z_]\w*))?[ \t]*;",
    re.MULTILINE,
)
NAMESPACE = re.compile(r"^[ \t]*namespace[ \t]+(?P<path>[A-Za-z_][\w\\]*)[ \t]*;", re.MULTILINE)


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
            log=os.path.join(REPO, ".phrost-cache", "implementers-probe.stderr.log"),
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
            if self.ask("textDocument/definition", probe_path, lines.position(at)):
                return True
        return False

    def open(self, path):
        if path not in self.opened:
            self.opened[path] = read(path)
            self.server.open(path, self.opened[path])
        return self.opened[path]

    def ask(self, method, path, position):
        params = {"textDocument": {"uri": uri_of(path)}, "position": position}
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


def qualified(masked, written):
    """The fully-qualified name a header wrote, as far as one file's imports can say."""
    written = written.strip()
    if written.startswith("\\"):
        return written[1:]
    head = written.split("\\", 1)[0]
    for match in IMPORTS.finditer(masked):
        alias = match.group("alias") or match.group("path").rsplit("\\", 1)[-1]
        if alias == head:
            rest = written.split("\\", 1)[1:]
            return "\\".join([match.group("path")] + rest)
    namespace = NAMESPACE.search(masked)
    return f"{namespace.group('path')}\\{written}" if namespace else written


def supertypes_in(masked):
    """Every (child fqn, supertype fqn, how) this file writes, as far as text can say."""
    found = []
    for match in HEADER.finditer(masked):
        child = qualified(masked, match.group("name"))
        rest = match.group("rest")
        for keyword, how in (("extends", "extends"), ("implements", "implements")):
            part = re.search(rf"\b{keyword}\b(?P<names>[^{{]*?)(?=\bimplements\b|$)", rest)
            if not part:
                continue
            for name in part.group("names").split(","):
                name = name.strip()
                if re.fullmatch(r"\\?[A-Za-z_][\w\\]*", name or ""):
                    found.append((child, qualified(masked, name), how))
    # A trait `use` is indented inside a class body, which is what separates it from an import.
    for match in USES.finditer(masked):
        if "(" in match.group(0) or "{" in match.group(0):
            continue
        for name in match.group("names").split(","):
            name = name.strip()
            if re.fullmatch(r"\\?[A-Za-z_][\w\\]*", name or ""):
                found.append((None, qualified(masked, name), "uses"))
    return found


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=60)
    parser.add_argument("--needle", default="")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {path: read(path) for path in paths}
    probe = compare.probe_of({path: (text, None) for path, text in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    # Where every declared class is, so a supertype can be asked about at its own declaration.
    declared = {}
    for path, text in plan.items():
        masked = compare.mask(text)
        for match in DECLARES.finditer(masked):
            declared[qualified(masked, match.group("name"))] = (path, match.start("name"))

    # Who names whom.
    wanted = defaultdict(list)
    for path, text in plan.items():
        for _, supertype, how in supertypes_in(compare.mask(text)):
            if supertype in declared:
                wanted[supertype].append((path, how))

    session = Probe(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered about the project")
        return 1

    asked = 0
    missing = []
    for supertype, writers in sorted(wanted.items()):
        path, offset = declared[supertype]
        text = session.open(path)
        answer = session.ask(
            "textDocument/implementation", path, compare.Lines(text).position(offset + 1)
        )
        answer = answer if isinstance(answer, list) else ([answer] if answer else [])
        if len(answer) >= CAP:
            continue
        found = {path_of(one.get("uri") or one.get("targetUri") or "") for one in answer}
        for writer, how in writers:
            asked += 1
            if writer not in found:
                missing.append((how, supertype, writer, len(answer)))

    session.server.stop()
    print()
    print(f"implementers        {asked - len(missing)} of {asked} written relationships listed")
    if missing:
        print()
        for how, count in Counter(how for how, *_ in missing).most_common():
            print(f"  {count:5}  {how}")
        print()
        seen = set()
        for how, supertype, writer, count in missing:
            if how in seen and not args.verbose:
                continue
            seen.add(how)
            print(f"  {how:11} {supertype}")
            print(f"              written in {os.path.relpath(writer, args.root)} ({count} listed)")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
