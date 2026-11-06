#!/usr/bin/env python3
"""Signature help highlights the argument the cursor is in.

    python3 tools/a-signature-points-at-the-argument.py <project-root> [--files 25]

An editor draws the active parameter in bold, and a server that names the wrong one is worse than
one that names none: the reader is told which value they are typing and it is not that value.
Nothing here measured it -- the perf sweep times the request and the capability audit asks whether
it answers, and neither reads what came back.

The expected answer is read off the *text*, not off the server: the arguments of a call are split
on the commas that are not inside brackets or a string, so the slot a cursor sits in is arithmetic.
Three things are then required of every answer:

* `activeSignature` and `activeParameter` index something that is there;
* a positional argument in slot k highlights parameter k -- unless the signature's last parameter is
  variadic and k has run past it, where PHP keeps feeding the same one;
* a named argument `name:` highlights the parameter called `name`, wherever it sits.

The second is skipped where the callee could not be resolved at all: a signature with no parameters
says nothing about slots.
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
        "signatureHelp": {
            "signatureInformation": {
                "parameterInformation": {"labelOffsetSupport": True},
                "activeParameterSupport": True,
            }
        },
        "definition": {"linkSupport": False},
    },
}

CALL = re.compile(r"(?<![\w$>:\\])([A-Za-z_]\w*)\s*\(")
NAMED = re.compile(r"^\s*([A-Za-z_]\w*)\s*:(?!:)")
# Words PHP spells like a call and does not resolve like one.
NOT_A_CALL = compare.NOT_A_CALL


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def argument_slots(masked, open_paren):
    """Where each argument of the call starting at `open_paren` begins, and where the list ends.

    Read off the masked text, so a comma inside a string or a comment is not a separator. Nested
    brackets are counted, so `f(g(1, 2), 3)` has two slots and not three.
    """
    depth = 0
    starts = [open_paren + 1]
    at = open_paren
    while at < len(masked):
        char = masked[at]
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth == 0:
                return starts, at
        elif char == "," and depth == 1:
            starts.append(at + 1)
        at += 1
    return starts, None


def parameter_names(signature):
    """The name of each parameter of a signature, as the label spells it."""
    label = signature.get("label") or ""
    out = []
    for parameter in signature.get("parameters") or []:
        written = parameter.get("label")
        if isinstance(written, list) and len(written) == 2:
            written = label[written[0] : written[1]]
        out.append((written or "").strip())
    return out


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
            log=os.path.join(REPO, ".phrost-cache", "signature.stderr.log"),
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=25)
    parser.add_argument("--calls", type=int, default=12, help="how many calls per file")
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

    asked = 0
    answered = 0
    wrong = []
    silent = []
    for path in paths:
        text = read(path)
        masked = compare.mask(text)
        lines = compare.Lines(text)
        probe.server.open(path, text)

        # A *declaration*'s parameter list is not a call, and signature help is right to say
        # nothing inside one. Without this the probe asked about 41 `__construct(` headers on one
        # project and read the silence as a third of its questions going unanswered.
        calls = [
            one
            for one in CALL.finditer(masked)
            if one.group(1) not in NOT_A_CALL
            and not compare.DECLARES.search(masked[max(0, one.start() - 12) : one.start()])
        ]
        for call in compare.stride(calls, args.calls):
            starts, end = argument_slots(masked, call.end() - 1)
            if end is None or end == call.end():
                continue
            for slot, start in enumerate(starts):
                written = masked[start : starts[slot + 1] - 1 if slot + 1 < len(starts) else end]
                if not written.strip():
                    continue
                at = start + (len(written) - len(written.lstrip()))
                asked += 1
                help_ = probe.ask("textDocument/signatureHelp", {
                    "textDocument": {"uri": uri_of(path)},
                    "position": lines.position(at + 1),
                    "context": {"triggerKind": 1, "isRetrigger": False},
                })
                signatures = (help_ or {}).get("signatures") or []
                if not signatures:
                    silent.append((
                        f"{os.path.relpath(path, args.root)}:{lines.position(at)['line'] + 1}",
                        f"{call.group(1)}(",
                    ))
                    continue
                answered += 1
                where = f"{os.path.relpath(path, args.root)}:{lines.position(at)['line'] + 1}"
                chosen = (help_ or {}).get("activeSignature") or 0
                if not 0 <= chosen < len(signatures):
                    wrong.append((where, f"activeSignature {chosen} of {len(signatures)}"))
                    continue
                signature = signatures[chosen]
                names = parameter_names(signature)
                active = signature.get("activeParameter")
                if active is None:
                    active = (help_ or {}).get("activeParameter")
                if active is None or not names:
                    continue
                if not 0 <= active < len(names):
                    wrong.append((where, f"activeParameter {active} of {len(names)}"))
                    continue
                named = NAMED.match(written)
                if named:
                    wanted = named.group(1)
                    if wanted not in names[active] and f"${wanted}" not in names[active]:
                        wrong.append((
                            where,
                            f"`{wanted}:` highlights {names[active]!r}, not the parameter it names",
                        ))
                    continue
                # A trailing variadic keeps taking values, so every slot past it is that one.
                last = len(names) - 1
                expected = min(slot, last) if "..." in (names[last] or "") else slot
                if expected < len(names) and active != expected:
                    wrong.append((
                        where,
                        f"slot {slot} highlights {active} ({names[active]!r}), not {expected}",
                    ))
        probe.server.close(path)
    probe.server.stop()

    print()
    print(f"a signature points at the argument  {answered - len(wrong)} of {answered}"
          f"  ({asked} slots asked, {asked - answered} had no signature)")
    for where, why in wrong[:20]:
        print(f"  {where}  {why}")
    if silent:
        from collections import Counter

        shapes = Counter(call for _, call in silent)
        print("  no signature, by callee:")
        for call, count in shapes.most_common(10):
            first = next(where for where, one in silent if one == call)
            print(f"    {count:>4}  {call:<28} first at {first}")
    return 1 if wrong else 0


if __name__ == "__main__":
    sys.exit(main())
