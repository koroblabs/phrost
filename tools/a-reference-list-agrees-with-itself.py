#!/usr/bin/env python3
"""The three ways a server counts references say the same number.

    python3 tools/a-reference-list-agrees-with-itself.py <project-root> [--files 25]

One question -- what else means this -- reached three ways, each through different machinery:

* **the lens** over a declaration, which sweeps the project and then subtracts the declaration
  itself;
* **find references** at the same position, which is the sweep without the subtraction;
* **document highlight** at a use, which never leaves the buffer and so runs over the *open*
  analysis where the sweep runs over a scratch one.

The first two must differ by exactly the declaration. The third must be what the second says about
this file and nothing else, which is `open == closed` asked through the protocol rather than through
the harness. A lens that says three over a list that opens with four is the shape this is after --
the code that computes it says so in a comment, and nothing checked it.
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

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "codeLens": {"dynamicRegistration": False},
        "references": {"dynamicRegistration": False},
        "documentHighlight": {"dynamicRegistration": False},
        "definition": {"linkSupport": False},
    },
}

COUNTED = re.compile(r"^(\d+) references?$")


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def span(one):
    where = one.get("range") or {}
    start, end = where.get("start") or {}, where.get("end") or {}
    return (
        os.path.realpath(path_of(one.get("uri", ""))) if "uri" in one else None,
        start.get("line"),
        start.get("character"),
        end.get("line"),
        end.get("character"),
    )


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
            log=os.path.join(REPO, ".phrost-cache", "reference-list.stderr.log"),
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
        deadline = time.monotonic() + 900
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

    def lens_counts(self, path):
        """Every lens in the file, resolved, as {the declaration's own range: count}.

        A lens is drawn over the declaration's *name*, which is the reference the count subtracts,
        so its range is the whole of what is needed to find that one in the list.
        """
        found = {}
        for lens in self.ask("textDocument/codeLens", {
            "textDocument": {"uri": uri_of(path)}
        }) or []:
            resolved = self.ask("codeLens/resolve", lens)
            title = ((resolved or {}).get("command") or {}).get("title") or ""
            counted = COUNTED.match(title.strip())
            if counted:
                found[span({"uri": uri_of(path), "range": lens.get("range")})] = int(
                    counted.group(1)
                )
        return found

    def references(self, path, position, declaration=True):
        found = self.ask("textDocument/references", {
            "textDocument": {"uri": uri_of(path)},
            "position": position,
            "context": {"includeDeclaration": declaration},
        })
        return [span(one) for one in found or []]

    def highlights(self, path, position):
        found = self.ask("textDocument/documentHighlight", {
            "textDocument": {"uri": uri_of(path)},
            "position": position,
        })
        return [span(one)[1:] for one in found or []]


NAMED = re.compile(r"(?:->|::|\bnew\s+)\s*\$?([A-Za-z_]\w*)")


def used_at(text, limit):
    """A few positions per file where a name is written, spread over the whole file."""
    masked = compare.mask(text)
    return compare.stride([one.start(1) for one in NAMED.finditer(masked)], limit)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=25)
    parser.add_argument("--sites", type=int, default=4)
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

    asked = {"lens": 0, "highlight": 0}
    wrong = []
    for path in paths:
        real = os.path.realpath(path)
        text = read(path)
        lines = compare.Lines(text)
        probe.server.open(real, text)

        # A lens sits over a declaration and says how many places reach it. Find-references at the
        # same position is the same sweep with the declaration still in it.
        for declared, count in sorted(probe.lens_counts(real).items())[: args.sites]:
            line, character = declared[1], declared[2]
            if line is None:
                continue
            asked["lens"] += 1
            position = {"line": line, "character": character}
            # The declaration is in the list and is not a use of itself, so it is the one the count
            # leaves out. Any *other* declaration a sweep found -- an override in a subclass -- is
            # not a use either, and is counted, which is what the lens does.
            uses = [one for one in probe.references(real, position) if one != declared]
            if len(uses) != count:
                wrong.append(("lens", real, line, f"lens says {count}, the list has {len(uses)}"))

        for at in used_at(text, args.sites):
            position = lines.position(at)
            asked["highlight"] += 1
            here = sorted(
                one[1:] for one in probe.references(real, position) if one[0] == real
            )
            drawn = sorted(probe.highlights(real, position))
            if here != drawn:
                wrong.append((
                    "highlight",
                    real,
                    position["line"],
                    f"{len(here)} in the list against {len(drawn)} drawn",
                ))
        probe.server.close(real)
    probe.server.stop()

    total = sum(asked.values())
    print()
    print(f"a reference list agrees with itself  {total - len(wrong)} of {total}  "
          f"({asked['lens']} lenses, {asked['highlight']} highlights)")
    for kind, path, line, why in wrong[:20]:
        print(f"  [{kind}] {os.path.relpath(path, args.root)}:{line + 1}  {why}")
    return 1 if wrong else 0


if __name__ == "__main__":
    sys.exit(main())
