#!/usr/bin/env python3
"""Whether `references` finds every site `definition` sent to the same declaration.

    python3 tools/navigation-roundtrip.py <project-root> [--limit 400]

The invariant needs no expected output written by hand, which is the point: if goto at a site
answers declaration D, then asking `references` at D must list that site. One direction is the
index's forward edge and the other is its reverse edge, and the two are built by different code --
so a disagreement is a bug in whichever of them is smaller, and nothing else has to be known about
the project to say one exists.

It is asked the way an editor would, over stdio against a driven `serve`, so a warm-start or
open/closed difference is inside what it measures. Sites come from a text scan of the project's own
code rather than from the index, for the reason `compare-lsp` gives: an instrument that asks the
index which sites exist cannot find a site the index has forgotten.

Reported by shape rather than by count: one missed `$this->method()` and one missed `Foo::CONST`
are two different bugs, and a total says neither.
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
    """Imports a tool whose filename holds a dash, so its scanners are not written twice."""
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
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "references": {"dynamicRegistration": False},
    },
}

# The kinds a reverse edge is expected to exist for. A local variable is deliberately left out:
# `references` on one is a within-file question that `documentHighlight` owns, and including it
# would drown the report in the commonest site there is.
KINDS = ("member", "static", "new", "name", "function", "constant")

# `self`, `static` and `parent` are not spellings of the class's name: goto answers the class, and
# references deliberately does not list them, because renaming the class must not rewrite them.
# A disagreement here is a position rather than a missing edge.
SKIP_NAMES = {"self", "static", "parent", "Self", "Static", "Parent", "SELF", "STATIC", "PARENT"}
# `new class {...}` is an anonymous class: goto lands on the expression that declares it and there
# is no name for anything to reference.
SKIP_NAMES.add("class")


class Probe:
    def __init__(self, root, limit, verbose):
        self.root = os.path.abspath(root)
        self.limit = limit
        self.verbose = verbose
        self.ready = None
        self.working = set()
        self.opened = {}
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "roundtrip.stderr.log"),
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
        deadline = time.monotonic() + 600
        text = read(probe_path)
        lines = compare.Lines(text)
        at = compare.mask(text).find(probe_name)
        self.open(probe_path)
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            answer = self.ask("textDocument/definition", probe_path, lines.position(at))
            if answer:
                return True
        return False

    def open(self, path):
        if path in self.opened:
            return self.opened[path]
        text = read(path)
        self.opened[path] = text
        self.server.open(path, text)
        return text

    def ask(self, method, path, position, extra=None):
        params = {
            "textDocument": {"uri": uri_of(path)},
            "position": position,
        }
        if extra:
            params.update(extra)
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


IMPORT_ALIAS = re.compile(r"^\s*use\b.*\bas\s+\w*$", re.IGNORECASE)


def aliased_import(masked, offset):
    """Whether the cursor is on the name an import binds: the `X` of `use A\\B as X;`.

    Goto there answers the class, and the class's reference list carries the *path* on that line
    rather than the alias token beside it -- two ranges on one line, and which of them a request
    hands back is a position rather than a missing edge. The alias's own occurrences are a separate
    target, reached from a cursor on a use of it.
    """
    start = masked.rfind("\n", 0, offset) + 1
    return IMPORT_ALIAS.search(masked[start:offset]) is not None


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def span(location):
    """One location as (path, start, end), whichever of the two shapes it arrived in."""
    if not location:
        return None
    if isinstance(location, list):
        location = location[0] if location else None
        if location is None:
            return None
    uri = location.get("uri") or location.get("targetUri")
    rng = location.get("range") or location.get("targetSelectionRange") or location.get("targetRange")
    if not uri or not rng:
        return None
    return (path_of(uri), rng["start"], rng["end"])


def holds(one, path, position):
    """Whether a reference covers the cursor the forward question was asked at.

    Not an equality on the start: a site is asked about from *inside* its identifier, because
    servers disagree about whether the first column is on the name. Comparing starts scored 145 of
    150 sites as missing on a server that had answered all of them.
    """
    if one is None or one[0] != path:
        return False
    _, start, end = one
    if position["line"] < start["line"] or position["line"] > end["line"]:
        return False
    if position["line"] == start["line"] and position["character"] < start["character"]:
        return False
    if position["line"] == end["line"] and position["character"] > end["character"]:
        return False
    return True


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--limit", type=int, default=400, help="how many sites to ask about")
    parser.add_argument("--files", type=int, default=40, help="how many files to sample")
    parser.add_argument("--needle", default="", help="only files whose path holds this")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    if not paths:
        print("no files sampled")
        return 1

    plan = {}
    for path in paths:
        text = read(path)
        plan[path] = (text, compare.sites_of(compare.mask(text)))

    probe = compare.probe_of({path: (text, None) for path, (text, _) in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    session = Probe(args.root, args.limit, args.verbose)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered about the project")
        return 1
    print(f"ready; {len(paths)} files sampled", flush=True)

    # Every site, strided so the sample is spread rather than taken off the first file.
    sites = []
    for path, (text, found) in plan.items():
        lines = compare.Lines(text)
        masked = compare.mask(text)
        for site in found:
            if site.kind not in KINDS or site.name in SKIP_NAMES:
                continue
            if aliased_import(masked, site.offset):
                continue
            sites.append((path, lines.position(site.offset), site))
    sites = compare.stride(sites, args.limit)

    for path, _, _ in sites:
        session.open(path)

    # Forward edge: where does each site say its declaration is.
    forward = defaultdict(list)
    unanswered = Counter()
    for path, position, site in sites:
        target = span(session.ask("textDocument/definition", path, position))
        if target is None:
            unanswered[site.kind] += 1
            continue
        forward[(target[0], target[1]["line"], target[1]["character"])].append(
            (path, position, site)
        )

    print(f"asked {len(sites)} sites; {len(forward)} distinct declarations", flush=True)

    # Reverse edge: one `references` per declaration, and every site that pointed here must be in it.
    missing = []
    refused = []
    for target, origins in sorted(forward.items()):
        path, line, character = target
        session.open(path)
        answer = session.ask(
            "textDocument/references",
            path,
            {"line": line, "character": character},
            {"context": {"includeDeclaration": False}},
        )
        if answer is None:
            refused.append((target, len(origins)))
            continue
        found = [span(one) for one in answer]
        for origin_path, position, site in origins:
            if not any(holds(one, origin_path, position) for one in found):
                missing.append((site, origin_path, position, target, len(answer)))

    session.server.stop()

    total = sum(len(origins) for origins in forward.values())
    print()
    print(f"round trip          {total - len(missing)} of {total} sites found again")
    for kind, count in sorted(unanswered.items()):
        print(f"  no definition     {count:5}  {kind}")
    if refused:
        print(f"  no references     {sum(count for _, count in refused):5} sites"
              f" over {len(refused)} declarations")

    if missing:
        print()
        print("missed, by shape")
        by_shape = Counter(site.kind for site, *_ in missing)
        for kind, count in by_shape.most_common():
            print(f"  {count:5}  {kind}")
        print()
        print("first of each shape")
        seen = set()
        for site, origin_path, position, target, count in missing:
            if site.kind in seen:
                continue
            seen.add(site.kind)
            rel = os.path.relpath(origin_path, args.root)
            decl = os.path.relpath(target[0], args.root)
            print(f"  {site.kind:7} {site.shape}")
            print(f"          site {rel}:{position['line'] + 1}:{position['character'] + 1}")
            print(f"          decl {decl}:{target[1] + 1}:{target[2] + 1} ({count} references)")
        if args.verbose:
            print()
            print("every miss")
            for site, origin_path, position, target, count in missing:
                rel = os.path.relpath(origin_path, args.root)
                decl = os.path.relpath(target[0], args.root)
                print(f"  {site.kind:7} {site.shape:50} {rel}:{position['line'] + 1}"
                      f" -> {decl}:{target[1] + 1}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
