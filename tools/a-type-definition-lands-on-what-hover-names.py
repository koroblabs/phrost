#!/usr/bin/env python3
"""Go-to-type-definition lands on the class hover says the value is.

    python3 tools/a-type-definition-lands-on-what-hover-names.py <project-root> [--files 30]

Two answers to one question, reached by different code: hover renders what a value *is* and
type-definition resolves where that is declared. They are the shape CLAUDE.md §8.1 calls two
constructors of one rule, and nothing compared them -- the capability audit asks whether
type-definition answers and the perf sweep does not time it at all.

Asked of every site where hover names a class: the type-definition must land on a declaration whose
name is that class. A value hover renders as a builtin or a union of them is skipped -- there is no
declaration to go to and answering nothing is right.

So is a call whose return is `static`, `$this` or `self`, where the two answers are about different
things on purpose: hover renders the signature of the method as *declared*, so it names the class
declaring it, while the value's type is the receiver's. `$this->state(...)` in a Laravel factory is
declared on `Factory` and worth `UserFactory`, and both answers are right.
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
KINDS = ("var", "member", "static")

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "hover": {"contentFormat": ["markdown"]},
        "typeDefinition": {"linkSupport": False},
        "definition": {"linkSupport": False},
    },
}

FENCE = re.compile(r"```(?:php)?\n(.*?)```", re.DOTALL)
# A return that hands back the receiver rather than anything the card names.
#
# The `?` is not optional decoration: `EntityBase::load()` inherits `@return static|null`, so the
# card reads `: ?static` and a pattern anchored on the keyword alone missed every nullable one --
# which is most of them, because a loader that may find nothing is how the shape is written.
RELATIVE = re.compile(r":\s*\??(?:static|self|\$this)\s*$", re.M)
# What PHP writes that names no declaration: a scalar, a pseudo-type, a keyword.
BUILTIN = {
    "int", "float", "string", "bool", "array", "object", "mixed", "void", "never", "null",
    "false", "true", "callable", "iterable", "self", "static", "parent", "resource",
}
DECLARED = re.compile(
    r"^(?:final\s+|abstract\s+|readonly\s+)*(?:class|interface|trait|enum)\s+([A-Za-z_]\w*)"
)


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def hovered_classes(result):
    """The class-like names a hover card's first code block holds, short and lowercased.

    Empty when the card is about a member returning the receiver, because then it names the class
    the member is *declared* on and the value is worth the receiver's class instead.
    """
    if not result:
        return set()
    contents = result.get("contents")
    text = contents.get("value", "") if isinstance(contents, dict) else str(contents)
    found = FENCE.search(text)
    if not found:
        return set()
    card = found.group(1)
    if RELATIVE.search(card.rstrip()):
        return set()
    # The type side of `T $name`, a declaration's own header, or a bare class name. Every
    # identifier-shaped run is taken and the builtins dropped: which one is the type is a question
    # this does not need to answer, because the check is that the landing is *among* them.
    words = re.findall(r"[A-Za-z_][\w\\]*", card)
    return {
        word.rsplit("\\", 1)[-1].lower()
        for word in words
        if word.rsplit("\\", 1)[-1].lower() not in BUILTIN
    }


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
            log=os.path.join(REPO, ".phrost-cache", "type-definition.stderr.log"),
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

    def ask(self, method, params, timeout=30.0):
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


def declared_at(path, line):
    """The class-like name declared on a line of a file, if one is."""
    try:
        rows = read(path).split("\n")
    except OSError:
        return None
    if not 0 <= line < len(rows):
        return None
    found = DECLARED.search(rows[line].strip())
    return found.group(1).lower() if found else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=30)
    parser.add_argument("--sites", type=int, default=40)
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

    asked = answered = 0
    wrong, unnamed = [], []
    for path in paths:
        text = read(path)
        lines = compare.Lines(text)
        probe.server.open(path, text)
        chosen = compare.stride(
            [site for site in compare.sites_of(compare.mask(text)) if site.kind in KINDS],
            args.sites,
        )
        for site in chosen:
            position = lines.position(site.offset)
            named = hovered_classes(probe.ask("textDocument/hover", {
                "textDocument": {"uri": uri_of(path)}, "position": position
            }))
            if not named:
                continue
            asked += 1
            found = probe.ask("textDocument/typeDefinition", {
                "textDocument": {"uri": uri_of(path)}, "position": position
            })
            found = found if isinstance(found, list) else ([found] if found else [])
            if not found:
                continue
            answered += 1
            where = f"{os.path.relpath(path, args.root)}:{position['line'] + 1}"
            landed = set()
            for one in found:
                if not isinstance(one, dict) or "uri" not in one:
                    continue
                line = (one.get("range") or {}).get("start", {}).get("line", -1)
                name = declared_at(path_of(one["uri"]), line)
                if name:
                    landed.add(name)
            if not landed:
                unnamed.append((where, site.shape))
            elif not landed & named:
                wrong.append((where, site.shape, sorted(landed), sorted(named)[:6]))
        probe.server.close(path)
    probe.server.stop()

    print()
    print(f"a type definition lands on what hover names  "
          f"{answered - len(wrong) - len(unnamed)} of {answered}"
          f"  ({asked} sites hover named a class at, {asked - answered} not answered)")
    for where, label, landed, named in wrong[:15]:
        print(f"  {where}  {label}")
        print(f"     landed on {landed}, hover names {named}")
    for where, label in unnamed[:10]:
        print(f"  {where}  {label}: landed where no class-like is declared")
    return 1 if wrong or unnamed else 0


if __name__ == "__main__":
    sys.exit(main())
