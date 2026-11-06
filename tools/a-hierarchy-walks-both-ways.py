#!/usr/bin/env python3
"""Walking a hierarchy one way and back arrives where it started.

    python3 tools/a-hierarchy-walks-both-ways.py <project-root> [--files 40] [--branch 8]

Two trees an editor draws, each answered by a pair of requests that are two readings of one
relation:

* **types** -- `S` is a supertype of `X` exactly when `X` is a subtype of `S`. Both directions are
  exact, so both are checked.
* **calls** -- if walking *down* from a caller reaches a callee, then walking *up* from that callee
  has to reach the caller. Only that direction: down is a resolution of what the body actually
  calls, up is a project-wide search by name, and a search may honestly over-report where a
  resolution would not. A callee the resolver found and the search cannot is a name the search does
  not look for, which is the bug this is after.

No expected output is written down anywhere. The two halves of each pair have to agree with each
other, and where they do not, one of them is wrong.
"""
import argparse
import importlib.util
import json
import os
import re
import sys
import time

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
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "typeHierarchy": {"dynamicRegistration": True},
        "callHierarchy": {"dynamicRegistration": True},
        "definition": {"linkSupport": False},
    },
}

DECLARES_A_TYPE = re.compile(
    r"\b(?:class|interface|trait|enum)\s+([A-Za-z_]\w*)", re.MULTILINE
)
DECLARES_A_CALLABLE = re.compile(r"\bfunction\s+&?\s*([A-Za-z_]\w*)\s*\(")


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "hierarchy.stderr.log"),
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

    def start(self, anchor, anchor_name):
        self.server.initialize(CAPABILITIES)
        text = read(anchor)
        at = compare.mask(text).find(anchor_name)
        self.server.open(anchor, text)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", {
                "textDocument": {"uri": uri_of(anchor)},
                "position": compare.Lines(text).position(at),
            }):
                return True
        return False

    def ask(self, method, params, timeout=60.0):
        for _ in range(60):
            try:
                answer = self.server.request(method, params, timeout=timeout)
            except (Timeout, Closed):
                return None
            if answer.get("error", {}).get("code") == STILL_INDEXING:
                self.server.pump(0.5)
                continue
            return answer.get("result")
        return None

    def head(self, method, path, position):
        found = self.ask(method, {
            "textDocument": {"uri": uri_of(path)},
            "position": position,
        }) or []
        return found[0] if found else None


def key_of(item):
    return (item.get("data") if isinstance(item, dict) else None) or ""


def sites(text, pattern, limit):
    """Where each declaration's *name* starts, at most `limit` of them, spread over the file."""
    masked = compare.mask(text)
    found = [(one.start(1), one.group(1)) for one in pattern.finditer(masked)]
    return compare.stride(found, limit)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--branch", type=int, default=8,
                        help="how many of a node's neighbours to walk back from")
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    anchor = None
    for path in reversed(paths):
        found = compare.DECLARED_CLASS.search(compare.mask(read(path)))
        if found:
            anchor = (path, found.group(1))
            break
    if anchor is None:
        print("no file in the sample declares a class")
        return 1

    probe = Probe(args.root)
    if not probe.start(*anchor):
        print("the server never answered about the project")
        return 1

    asked = {"supertype": 0, "subtype": 0, "call": 0}
    broken = []
    for path in paths:
        text = read(path)
        lines = compare.Lines(text)
        probe.server.open(path, text)

        for at, name in sites(text, DECLARES_A_TYPE, 6):
            item = probe.head("textDocument/prepareTypeHierarchy", path, lines.position(at))
            if item is None:
                continue
            mine = key_of(item)
            for direction, back in (
                ("typeHierarchy/supertypes", "typeHierarchy/subtypes"),
                ("typeHierarchy/subtypes", "typeHierarchy/supertypes"),
            ):
                counted = "supertype" if direction.endswith("supertypes") else "subtype"
                step = probe.ask(direction, {"item": item}) or []
                for neighbour in compare.stride(step, args.branch):
                    asked[counted] += 1
                    returned = probe.ask(back, {"item": neighbour}) or []
                    if mine not in {key_of(one) for one in returned}:
                        broken.append((
                            path, at, counted,
                            f"{mine} -> {key_of(neighbour)}",
                            f"{key_of(neighbour)} does not lead back",
                        ))

        for at, name in sites(text, DECLARES_A_CALLABLE, 6):
            item = probe.head("textDocument/prepareCallHierarchy", path, lines.position(at))
            if item is None:
                continue
            mine = key_of(item)
            out = probe.ask("callHierarchy/outgoingCalls", {"item": item}) or []
            for call in compare.stride(out, args.branch):
                callee = call.get("to") or {}
                asked["call"] += 1
                back = probe.ask("callHierarchy/incomingCalls", {"item": callee}) or []
                if mine not in {key_of(one.get("from") or {}) for one in back}:
                    broken.append((
                        path, at, "call",
                        f"{mine} -> {key_of(callee)}",
                        f"{key_of(callee)} does not list its caller",
                    ))
        probe.server.close(path)
    probe.server.stop()

    total = sum(asked.values())
    print()
    print(f"a hierarchy walks both ways  {total - len(broken)} of {total}  "
          f"({asked['supertype']} up, {asked['subtype']} down, {asked['call']} calls)")
    for path, at, kind, edge, why in broken[:30]:
        where = f"{os.path.relpath(path, args.root)}:{compare.Lines(read(path)).position(at)['line'] + 1}"
        print(f"  {where} [{kind}] {edge}")
        print(f"     {why}")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
