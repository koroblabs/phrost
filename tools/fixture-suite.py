#!/usr/bin/env python3
"""Runs another project's fixture corpus against our server and scores it.

    python3 tools/fixture-suite.py ../phpantom_lsp/tests/fixtures
    python3 tools/fixture-suite.py <dir> --server phpantom     # what their own suite scores

The corpus is PHPantom's `.fixture` format (MIT), much of it ported in turn from Phpactor's
inference tests. It is worth borrowing for one reason: it was written by somebody else, about a
server that is not ours, so it asks the questions we did not think to ask. Our own fixtures can
only test what we already believed was worth testing.

A score against it is not a verdict. Their fixtures encode their feature set -- `@template`
inference, Laravel builder chains -- so a fixture we fail may be a feature we have not built rather
than something broken. What the report is for is the *ranked list*: which categories we lose whole,
and which we lose by one assertion.

Nothing here links our index. Each fixture becomes a throwaway project on disk, a server is started
over it, every file is opened, and one request is asked -- so a pass is an answer an editor would
have got. One server per fixture, because fixtures reuse class names and a shared project would
have `Foo` mean four things at once.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import Closed, Server, Timeout, path_of, uri_of  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SIBLINGS = os.path.dirname(REPO)

PRESETS = {
    "phrost": [os.path.join(REPO, "target/release/phrost"), "serve"],
    "phpantom": [os.path.join(SIBLINGS, "phpantom_lsp/target/release/phpantom_lsp"), "--stdio"],
    "intelephense": ["intelephense", "--stdio"],
}

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True, "symbol": {"dynamicRegistration": False}},
    "textDocument": {
        "synchronization": {"didSave": True, "willSave": False},
        "definition": {"linkSupport": True},
        "hover": {"contentFormat": ["markdown", "plaintext"]},
        "completion": {
            "completionItem": {"snippetSupport": True, "documentationFormat": ["markdown"]},
            "contextSupport": True,
        },
        "signatureHelp": {"signatureInformation": {"parameterInformation": {"labelOffsetSupport": True}}},
    },
}

STILL_INDEXING = -32801


# -- reading a fixture ---------------------------------------------------------------------------


class Fixture:
    """One scenario: some files, a cursor in one of them, and what the answer has to contain."""

    def __init__(self, path, category, relative):
        self.path = path
        self.category = category
        # Where it lives and what it is called are both needed: the path is how a failure is
        # reopened, and the corpus's own `// test:` line is what makes a list of them readable.
        self.relative = relative
        self.name = relative
        self.feature = None
        self.ignore = None
        self.expect = []
        self.expect_absent = []
        self.expect_hover = []
        self.expect_definition = []
        self.sig_label = None
        self.sig_active = None
        self.sig_params = []
        # A named harness setup -- installed stubs, an extra service hint. A black-box runner over
        # stdio cannot supply one, so a fixture asking for one is skipped rather than failed.
        self.setup = None
        # `expect_definition_line: 3` -- zero-based, which is how the protocol counts and how the
        # corpus that uses this key wrote it.
        self.definition_lines = []
        self.reference_lines = []
        self.files = []
        self.cursor_file = None
        self.cursor_line = 0
        self.cursor_character = 0


DIRECTIVES = {
    "test": "name",
    # The other corpus calls the same thing `name:`. One key each, because a corpus is somebody
    # else's file and renaming their header is not this tool's business.
    "name": "name",
    "feature": "feature",
    "ignore": "ignore",
    "setup": "setup",
    "expect_sig_label": "sig_label",
    "expect_sig_active": "sig_active",
}

# Features that ask about a position, so a fixture for one has to mark it.
POSITIONAL = {"completion", "hover", "definition", "goto_definition", "signature_help", "references"}

# The same feature under two corpora's names.
ALIASES = {"goto_definition": "definition"}


def parse(path, root):
    relative = os.path.relpath(path, root)
    fixture = Fixture(path, relative.split(os.sep)[0], relative)

    with open(path, encoding="utf-8") as handle:
        content = handle.read()
    if "\n---\n" not in content:
        raise ValueError("no `---` separating the header from the body")
    header, body = content.split("\n---\n", 1)

    for line in header.splitlines():
        # Written `// feature:` by one corpus and `feature:` by the other. Both are headers of the
        # same shape and the comment marker carries no meaning, so it is optional here.
        line = line.strip().removeprefix("//").strip()
        if not line:
            continue
        directive, _, value = line.partition(":")
        directive, value = directive.strip(), value.strip()
        if directive in DIRECTIVES:
            setattr(fixture, DIRECTIVES[directive], value)
        elif directive == "expect":
            fixture.expect.append(value)
        elif directive == "expect_absent":
            fixture.expect_absent.append(value)
        elif directive == "expect_hover" and "=>" in value:
            symbol, _, wanted = value.partition("=>")
            fixture.expect_hover.append((symbol.strip(), wanted.strip()))
        elif directive == "expect_definition":
            fixture.expect_definition.append(value)
        elif directive == "expect_sig_param":
            fixture.sig_params.append(value)
        elif directive == "expect_definition_line":
            fixture.definition_lines.append(int(value))
        elif directive == "expect_reference_line":
            fixture.reference_lines.append(int(value))

    fixture.files = split_files(body)
    for index, (_, text) in enumerate(fixture.files):
        if "<>" in text:
            fixture.cursor_file = index
            break

    if not fixture.feature:
        raise ValueError("no `feature:` directive")
    fixture.feature = ALIASES.get(fixture.feature, fixture.feature)

    if fixture.cursor_file is None:
        # A diagnostics fixture is about the whole file, so it marks no position -- and a feature
        # that asks about one cannot run without it.
        if fixture.feature in POSITIONAL:
            raise ValueError("no file holds a `<>` cursor")
        fixture.cursor_file = 0
        return fixture

    name, text = fixture.files[fixture.cursor_file]
    at = text.index("<>")
    before = text[:at]
    fixture.cursor_line = before.count("\n")
    fixture.cursor_character = len(before) - (before.rfind("\n") + 1)
    fixture.files[fixture.cursor_file] = (name, before + text[at + 2 :])
    return fixture


SPLIT = re.compile(r"^=== (.+?) ===$", re.MULTILINE)


def split_files(body):
    """The body's files, as `(relative path, text)`, single-file fixtures included."""
    parts = SPLIT.split(body)
    if len(parts) == 1:
        return [("fixture.php", body)]
    files = []
    for index in range(1, len(parts), 2):
        files.append((parts[index].strip(), parts[index + 1].lstrip("\n")))
    return files


# -- checking an answer --------------------------------------------------------------------------


def label_matches(label, expected):
    """Whether a completion item answers `// expect: name(`.

    Their runner matches the expectation as a prefix of the label, and their labels carry the
    brackets a callable is written with -- ours do not, and neither do intelephense's. So a trailing
    `(` or `()` is read as what it means: the item names a callable. Kept as its own rule rather
    than stripped, because a plain prefix would let `bar(` be answered by `barBaz`.
    """
    stem = expected
    for bracket in ("()", "("):
        if stem.endswith(bracket):
            stem = stem[: -len(bracket)]
            return label == stem or label.startswith(stem + "(")
    return label.startswith(expected)


# Sentences this corpus pins that we report in different words.
#
# The corpus's `diagnostics` directory came from the previous attempt and pins *its* prose, which this
# project deliberately rewrote -- so nine of its fixtures failed on wording while reporting the very
# finding they exist to check, and counting those as failures put a ceiling on the score that no
# amount of work could reach.
#
# Every pair below was read side by side once, and the tool prints both for any pair that is not --
# judging two sentences equivalent without seeing both is how a score gets prettier without anything
# getting better. Keyed by fixture as well as by sentence, so a rewrite elsewhere cannot quietly reuse
# an entry, and a fixture satisfied this way is reported apart from one satisfied outright.
SAME_FINDING = {
    (
        "diagnostics/class_constant_missing.fixture",
        "Constant `TestClass5::CON` is undefined.",
    ): "Undefined constant TestClass5::CON",
    (
        "diagnostics/assignment_var_annotation_mismatch.fixture",
        "Assignment to $file expects File, string given",
    ): "$file is declared File, and this is string",
    (
        "diagnostics/function_argument_count.fixture",
        "`needs_two` expects at least 2 arguments, 1 given.",
    ): "needs_two() needs 2 arguments, 1 given",
    (
        "diagnostics/instanceof_receiver_narrowing.fixture",
        "Method `any_method` not found",
    ): "Undefined method App\\BulkOrderApplication::any_method()",
    (
        "diagnostics/method_argument_count_variadic.fixture",
        "`Logger::write` expects at least 1 argument, 0 given.",
    ): "write() needs 1 argument, 0 given",
    (
        "diagnostics/function_argument_count.fixture",
        "`needs_two` expects at most 2 arguments, 3 given.",
    ): "needs_two() takes at most 2 arguments, 3 given",
    (
        "diagnostics/method_argument_count_variadic.fixture",
        "`Logger::write` expects at most 2 arguments, 3 given.",
    ): "write() takes at most 2 arguments, 3 given",
    (
        "diagnostics/nullable_property_reported.fixture",
        "Cannot access property 'name' on possibly null user",
    ): "Cannot access property $name on ?User",
    (
        "diagnostics/resolved_global_short_class_mismatch.fixture",
        "`App\\isTermsAccepted` parameter `$application` expects "
        "Drupal\\app_core\\Application\\Application, Application given.",
    ): "$application takes \\Drupal\\app_core\\Application\\Application, and this is Application",
    (
        "diagnostics/unpacked_array_element_mismatch_typed_variadic.fixture",
        "`App\\notify` parameter `$offers` expects Offer, Customer[] given.",
    ): "$offers takes \\App\\Offer, and this unpacks Customer",
    (
        "diagnostics/unused_local_variable_reported.fixture",
        "Unused variable $unused",
    ): "$unused is assigned and never read",
}


class Reworded:
    """A pass earned by the table above rather than by the corpus's own sentence."""

    def __bool__(self):
        return False


REWORDED = Reworded()


class Miss(str):
    """Why a fixture failed, and whether the server said anything at all while failing.

    A `str` subclass rather than a pair, so every existing use keeps working unchanged -- the
    `is None` test that decides pass from fail, the f-strings that print it, the JSON dump that
    serialises it. One fact is added, and it is the one the score was conflating.

    "We answered nothing" and "we answered something else" rank completely differently, and the
    Justfile says as much: a score is not a verdict, the ranked list of *what we lose whole* is what
    the corpus is for. Nine of this corpus's `diagnostics` fixtures come from the previous attempt and
    pin its exact sentences, which this project deliberately rewrote -- so they fail on wording while
    reporting the very finding they are about, and counting them beside a real absence made the score
    understate by eleven and put a ceiling on it that no amount of work could reach.
    """

    def __new__(cls, why, answered, said=()):
        miss = super().__new__(cls, why)
        miss.answered = answered
        # What the server did say, so a wording mismatch can be *read* rather than assumed. Judging
        # two sentences equivalent without seeing both is how a score gets prettier without anything
        # getting better.
        miss.said = list(said)
        return miss


def check_completion(fixture, result):
    items = (result or {}).get("items") if isinstance(result, dict) else result
    labels = [item.get("label", "") for item in (items or [])]
    for expected in fixture.expect:
        if not any(label_matches(label, expected) for label in labels):
            return Miss(
                f"no completion for `{expected}` among {len(labels)} items", len(labels), labels
            )
    for absent in fixture.expect_absent:
        if any(label_matches(label, absent) for label in labels):
            return Miss(f"`{absent}` was offered and should not have been", len(labels))
    return None


def hover_text(result):
    if not result:
        return None
    contents = result.get("contents")
    if isinstance(contents, dict):
        return contents.get("value", "")
    if isinstance(contents, list):
        return "\n".join(part.get("value", "") if isinstance(part, dict) else str(part) for part in contents)
    return str(contents)


BOUND = re.compile(r"^\$(\w+)\s*[=:]\s*(.+)$")
QUALIFIED = re.compile(r"^([A-Za-z_][\w\\]*)::\$?(\w+)")

# What one corpus writes when it means "there must be no hover at all".
NO_HOVER = "__ANY_HOVER_RESULT__"
# And the same for signature help: `$value = <>combine('demo')`, with the cursor before the callee.
NO_SIGNATURE = "__ANY_SIGNATURE_RESULT__"


def hover_says(text, expected):
    """Whether a hover says what a fixture wanted, in any of three house styles.

    A variable: one corpus writes `$k = int`, another `$k: int`, and ours `int $k`, which is how PHP
    spells a declaration. A member: they write `Owner::method(): T` on one line and ours renders the
    declaration as PHP inside a fence, which names the owner and the member in two places instead of
    one. All of it is layout, and the tool scores what a server *knows* -- an exact string here would
    mark every hover wrong for describing the same value differently.
    """
    if expected in text:
        return True
    bound = BOUND.match(expected.strip())
    if bound is not None and f"{bound.group(2)} ${bound.group(1)}" in text:
        return True
    qualified = QUALIFIED.match(expected.strip())
    return qualified is not None and all(
        part in text for part in (qualified.group(1).rsplit("\\", 1)[-1], qualified.group(2))
    )


def check_hover(fixture, text):
    # A fixture can want silence: hovering a comment must answer nothing rather than crash.
    if any(absent == NO_HOVER for absent in fixture.expect_absent):
        return None if text is None else f"hovered `{one_line(text)}` and should not have"
    if text is None:
        return None if not fixture.expect else "no hover"
    for expected in fixture.expect:
        if not hover_says(text, expected):
            return f"hover says `{one_line(text)}`, wanted `{expected}`"
    for absent in fixture.expect_absent:
        if absent in text:
            return f"hover says `{one_line(text)}`, which should not mention `{absent}`"
    return None


def check_definition(fixture, result, cursor_path):
    targets = []
    for hit in normalise(result):
        uri = hit.get("uri") or hit.get("targetUri") or ""
        span = hit.get("range") or hit.get("targetSelectionRange") or hit.get("targetRange") or {}
        line = (span.get("start") or {}).get("line")
        if line is not None:
            targets.append((os.path.basename(path_of(uri)), line + 1))
    if not targets:
        return "no definition"
    # `expect_definition_line` names the line and not the file, because the corpus that writes it
    # puts every declaration in the fixture's one file. Zero-based, as the protocol counts.
    for wanted in fixture.definition_lines:
        if not any(line == wanted + 1 for _, line in targets):
            return f"went to {targets}, wanted line {wanted + 1}"
    for expected in fixture.expect_definition:
        where, _, line = expected.rpartition(":")
        wanted_file = os.path.basename(cursor_path) if where == "self" else os.path.basename(where)
        if (wanted_file, int(line)) not in targets:
            return f"went to {targets}, wanted {wanted_file}:{line}"
    return None


def check_diagnostics(fixture, published):
    """Whether the diagnostics on a file say what a fixture wanted, by substring.

    Substrings, because that is what the corpus wrote: a message is prose and a fixture pins the part
    of it that names the problem. The absent half is the load-bearing one -- two thirds of these
    fixtures exist to say a *false* finding was fixed, and those are the ones a rewrite silently
    brings back.
    """
    messages = [one_line(found.get("message", ""), 200) for found in published]
    joined = "\n".join(messages)
    reworded = False
    for expected in fixture.expect:
        if expected in joined:
            continue
        # The same finding in our own words, judged equivalent once and recorded with both sentences.
        same = SAME_FINDING.get((fixture.relative, expected))
        if same is not None and same in joined:
            reworded = True
            continue
        return Miss(
            f"no diagnostic saying `{expected}` among {len(messages)}", len(messages), messages
        )
    for absent in fixture.expect_absent:
        if absent in joined:
            offending = next(message for message in messages if absent in message)
            return Miss(
                f"reported `{one_line(offending)}`, which should not mention `{absent}`",
                len(messages),
            )
    # Passed, and the caller is told how: a fixture satisfied only through the table above is a
    # behaviour match and not a text one, and the report keeps the two apart rather than letting the
    # number quietly absorb the difference.
    return REWORDED if reworded else None


def check_references(fixture, result):
    lines = sorted(
        (hit.get("range") or {}).get("start", {}).get("line")
        for hit in normalise(result)
        if hit.get("range")
    )
    for wanted in fixture.reference_lines:
        if wanted not in lines:
            return f"found references on lines {lines}, wanted {wanted}"
    if not fixture.reference_lines and not lines:
        return "no references"
    return None


def check_inlay_hints(fixture, result):
    labels = []
    for hint in result or []:
        label = hint.get("label")
        if isinstance(label, list):
            labels.append("".join(part.get("value", "") for part in label))
        else:
            labels.append(str(label))
    joined = " ".join(labels)
    for expected in fixture.expect:
        if expected not in joined:
            return Miss(f"no hint saying `{expected}` among {len(labels)}", len(labels), labels)
    for absent in fixture.expect_absent:
        if absent in joined:
            return Miss(f"hinted `{absent}`, which should not have been", len(labels))
    return None


def check_signature(fixture, result):
    signatures = (result or {}).get("signatures") or []
    # The negative fixture first, because for it an empty answer is the pass rather than the failure
    # -- read the other way round, a server that correctly stays quiet outside a call scored a miss.
    if any(absent == NO_SIGNATURE for absent in fixture.expect_absent):
        if signatures:
            label = signatures[0].get("label", "")
            return f"signature help says `{label}`, and there should be none here"
        return None
    if not signatures:
        return "no signature help"
    active = (result or {}).get("activeSignature") or 0
    signature = signatures[min(active, len(signatures) - 1)]
    label = signature.get("label", "")

    if fixture.sig_label is not None:
        mine, wanted = signature_shape(label), signature_shape(fixture.sig_label)
        # The parameter list always, the return type only when both sides render one. An
        # `expect_sig_label` is written against one server's house style -- theirs omits the
        # callable's name and ours omits the return type -- and comparing the strings whole would
        # score every signature as wrong for describing the same call differently.
        if mine[0] != wanted[0] or (mine[1] and wanted[1] and mine[1] != wanted[1]):
            return f"signature is `{label}`, wanted `{fixture.sig_label}`"
    if fixture.sig_active is not None:
        got = signature.get("activeParameter", (result or {}).get("activeParameter"))
        if got != int(fixture.sig_active):
            return f"parameter {got} is active, wanted {fixture.sig_active}"
    if fixture.sig_params:
        got = [parameter_label(parameter, label) for parameter in signature.get("parameters") or []]
        for index, expected in enumerate(fixture.sig_params):
            if index >= len(got):
                return f"signature has {len(got)} parameters, wanted at least {index + 1}"
            if got[index] != expected:
                return f"parameter {index} is `{got[index]}`, wanted `{expected}`"
    return None


def signature_shape(label):
    """A signature label as `(parameter list, return type)`, however it was written.

    The parameter list is taken by matching brackets rather than by the last `)`, because a default
    value can hold brackets of its own -- `array $rows = []` is fine, `int $at = strlen($s)` is not.
    """
    start = label.find("(")
    if start < 0:
        return label.strip(), ""
    depth = 0
    for index in range(start, len(label)):
        depth += (label[index] == "(") - (label[index] == ")")
        if depth == 0:
            returns = label[index + 1 :].lstrip(" :")
            return label[start + 1 : index].strip(), returns.strip()
    return label[start + 1 :].strip(), ""


def parameter_label(parameter, signature_label):
    label = parameter.get("label")
    if isinstance(label, list) and len(label) == 2:
        return signature_label[label[0] : label[1]]
    return label or ""


def normalise(result):
    if result is None:
        return []
    return result if isinstance(result, list) else [result]


def one_line(text, limit=90):
    flat = " ".join((text or "").split())
    return flat[:limit] + ("…" if len(flat) > limit else "")


# -- running one fixture -------------------------------------------------------------------------


class Runner:
    """A server over one throwaway project, asked the one question a fixture is about."""

    def __init__(self, name, argv, root, warmup):
        self.warmup = warmup
        self.working = set()
        self.ready_signal = None
        self.published = {}
        self.server = Server(name, argv, root, log=os.path.join(root, "server.log"))
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            # Keyed by uri and replaced rather than appended: a server republishes the whole list for
            # a file, and keeping both versions would score a finding that was withdrawn.
            self.published[params.get("uri", "")] = params.get("diagnostics") or []
            return
        if method != "$/progress":
            return
        token = json.dumps(params.get("token"))
        kind = (params.get("value") or {}).get("kind")
        if kind == "begin":
            self.working.add(token)
        elif kind == "end":
            self.working.discard(token)
            self.ready_signal = self.ready_signal or "progress"

    def settle(self):
        """Waits for the server to stop indexing, or for it to go quiet if it never says.

        Short, and it can expire without failing anything: every file is opened before the question
        is asked, so a server that answers from its buffers is entitled to answer straight away.
        Servers that reject requests while indexing say so with a code, and `ask` retries those.
        """
        deadline = time.monotonic() + self.warmup
        while time.monotonic() < deadline:
            heard = self.server.poll(0.2)
            if self.ready_signal and not self.working:
                return
            if not heard and self.ready_signal:
                return

    def ask(self, method, params):
        for _ in range(20):
            try:
                answer = self.server.request(method, params, timeout=20.0)
            except (Timeout, Closed) as failed:
                return None, str(failed)
            if (answer.get("error") or {}).get("code") == STILL_INDEXING:
                time.sleep(0.2)
                continue
            if answer.get("error"):
                return None, answer["error"].get("message", "error")
            return answer.get("result"), None
        return None, "still indexing"

    def stop(self):
        self.server.stop()


def run(fixture, argv, name, warmup):
    """Materialises the fixture, asks its question, and returns why it failed -- or None."""
    with tempfile.TemporaryDirectory(prefix="phrost-fixture-") as root:
        paths = []
        for relative, text in fixture.files:
            path = os.path.join(root, relative)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(text)
            paths.append(path)

        runner = Runner(name, argv, root, warmup)
        try:
            runner.server.initialize(CAPABILITIES)
            runner.settle()
            for path, (_, text) in zip(paths, fixture.files):
                runner.server.open(path, text)
            runner.settle()

            cursor = paths[fixture.cursor_file]
            where = {
                "textDocument": {"uri": uri_of(cursor)},
                "position": {"line": fixture.cursor_line, "character": fixture.cursor_character},
            }
            return ask_about(fixture, runner, where, cursor)
        except (Timeout, Closed) as failed:
            return f"server: {failed}"
        finally:
            runner.stop()


def ask_about(fixture, runner, where, cursor):
    if fixture.feature == "completion":
        result, failed = runner.ask(
            "textDocument/completion", dict(where, context={"triggerKind": 1})
        )
        return failed or check_completion(fixture, result)

    if fixture.feature == "hover":
        if fixture.expect_hover:
            text = fixture.files[fixture.cursor_file][1]
            for symbol, wanted in fixture.expect_hover:
                at = text.find(symbol)
                if at < 0:
                    return f"`{symbol}` is not in the fixture"
                before = text[:at]
                position = {
                    "line": before.count("\n"),
                    "character": len(before) - (before.rfind("\n") + 1),
                }
                result, failed = runner.ask(
                    "textDocument/hover", dict(where, position=position)
                )
                if failed:
                    return failed
                said = hover_text(result)
                if said is None:
                    return f"no hover on `{symbol}`"
                if not hover_says(said, wanted):
                    return f"hover on `{symbol}` says `{one_line(said)}`, wanted `{wanted}`"
                for absent in fixture.expect_absent:
                    if absent in said:
                        return f"hover on `{symbol}` should not mention `{absent}`"
            return None
        result, failed = runner.ask("textDocument/hover", where)
        return failed or check_hover(fixture, hover_text(result))

    if fixture.feature == "definition":
        result, failed = runner.ask("textDocument/definition", where)
        return failed or check_definition(fixture, result, cursor)

    if fixture.feature == "signature_help":
        result, failed = runner.ask(
            "textDocument/signatureHelp", dict(where, context={"triggerKind": 1, "isRetrigger": False})
        )
        return failed or check_signature(fixture, result)

    if fixture.feature == "references":
        result, failed = runner.ask(
            "textDocument/references", dict(where, context={"includeDeclaration": False})
        )
        return failed or check_references(fixture, result)

    if fixture.feature == "inlay_hints":
        text = fixture.files[fixture.cursor_file][1]
        whole = {
            "textDocument": where["textDocument"],
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": text.count("\n") + 1, "character": 0},
            },
        }
        result, failed = runner.ask("textDocument/inlayHint", whole)
        return failed or check_inlay_hints(fixture, result)

    if fixture.feature == "diagnostics":
        # Pushed rather than requested, so there is nothing to ask for: the notification arrives when
        # the server has an answer, and the wait is the same settle every other feature does.
        runner.settle()
        published = runner.published.get(uri_of(cursor), [])
        return check_diagnostics(fixture, published)

    return f"unknown feature `{fixture.feature}`"


# -- the report ----------------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixtures", help="directory of .fixture files")
    parser.add_argument("--server", default="phrost", help=f"one of {sorted(PRESETS)}, or --cmd")
    parser.add_argument("--cmd", help="server command line, when it is not a preset")
    parser.add_argument("--category", help="only fixtures under this subdirectory")
    parser.add_argument("--name", help="only fixtures whose path contains this")
    parser.add_argument("--warmup", type=float, default=6.0, help="seconds to wait for indexing")
    parser.add_argument("--failures", action="store_true", help="print every failure, not a sample")
    parser.add_argument("--json", help="also write every outcome here, for slicing afterwards")
    args = parser.parse_args()

    argv = args.cmd.split() if args.cmd else PRESETS.get(args.server)
    if not argv:
        parser.error(f"unknown server `{args.server}`; pass --cmd")
    if not os.path.exists(argv[0]) and os.sep in argv[0]:
        parser.error(f"{argv[0]} is not built")

    root = os.path.abspath(args.fixtures)
    paths = []
    for directory, subdirs, names in os.walk(root):
        subdirs[:] = sorted(subdirs)
        for name in sorted(names):
            if name.endswith(".fixture"):
                paths.append(os.path.join(directory, name))
    if args.category:
        paths = [p for p in paths if os.path.relpath(p, root).split(os.sep)[0] == args.category]
    if args.name:
        paths = [p for p in paths if args.name in p]

    # An instrument that reports `0/0 passed` on an empty directory is one that says "held" when it
    # is not running -- the shape `docs/08` records for the ratchet and for the capability audit.
    # The three corpora this is pointed at are sibling checkouts rather than anything this repo
    # ships, so the ordinary way to get here is that the checkout is simply not on the machine.
    if not paths:
        if not os.path.isdir(root):
            parser.error(f"{root} does not exist -- the corpus is a separate checkout, not part of this repo")
        parser.error(f"{root} holds no .fixture files")

    print(f"{len(paths)} fixtures from {root}")
    print(f"asking {args.server}: {' '.join(argv)}\n", flush=True)

    passed, failed, ignored, broken = [], [], [], []

    reworded = []
    by_category = defaultdict(Counter)
    by_feature = defaultdict(Counter)
    started = time.monotonic()

    for index, path in enumerate(paths):
        try:
            fixture = parse(path, root)
        except (ValueError, OSError) as bad:
            broken.append((os.path.relpath(path, root), str(bad)))
            continue
        if fixture.ignore:
            ignored.append((fixture.relative, fixture.ignore))
            by_category[fixture.category]["ignored"] += 1
            continue
        if fixture.setup:
            # A named harness setup: installed stubs, an extra service hint, a second definition of a
            # class. Nothing over stdio can supply one, so the fixture is skipped and said to be --
            # counting it either way would be a claim about a question that was never asked.
            ignored.append((fixture.relative, f"needs the `{fixture.setup}` setup"))
            by_category[fixture.category]["ignored"] += 1
            continue

        why = run(fixture, argv, args.server, args.warmup)
        # `REWORDED` is a pass the corpus's own sentence did not give us, so it counts as one and is
        # still countable apart from the rest -- see `SAME_FINDING`.
        won = why is None or why is REWORDED
        if why is REWORDED:
            reworded.append(fixture)
            why = None
        outcome = "passed" if won else "failed"
        by_category[fixture.category][outcome] += 1
        by_feature[fixture.feature][outcome] += 1
        (passed if won else failed).append((fixture, why))

        done = index + 1
        if done % 25 == 0 or done == len(paths):
            rate = (time.monotonic() - started) / done
            print(
                f"  {done}/{len(paths)} — {len(passed)} passed, {len(failed)} failed"
                f" ({rate:.1f}s each)",
                flush=True,
            )

    if args.json:
        with open(args.json, "w", encoding="utf-8") as handle:
            json.dump(
                [
                    {
                        "path": fixture.relative,
                        "test": fixture.name,
                        "feature": fixture.feature,
                        "category": fixture.category,
                        "why": why,
                    }
                    for fixture, why in passed + failed
                ],
                handle,
                indent=1,
            )

    report(args, passed, failed, ignored, broken, by_category, by_feature, reworded)


def report(args, passed, failed, ignored, broken, by_category, by_feature, reworded=()):
    answered = len(passed) + len(failed)
    print(f"\n{args.server}: {len(passed)}/{answered} passed", end="")
    if answered:
        print(f" ({100 * len(passed) / answered:.0f}%)", end="")
    print(f", {len(ignored)} ignored by the corpus, {len(broken)} unreadable")
    if reworded:
        print(
            f"  of those, {len(reworded)} report the finding in our own words rather than the"
            " corpus's -- see SAME_FINDING, every pair read side by side once"
        )
        for fixture in reworded:
            print(f"    {fixture.relative}")

    print("\nby feature")
    for feature, counts in sorted(by_feature.items(), key=lambda kv: -sum(kv[1].values())):
        total = counts["passed"] + counts["failed"]
        share = f"{100 * counts['passed'] / total:.0f}%" if total else "-"
        print(f"  {feature:<16} {counts['passed']:>3}/{total:<3} {share:>5}")

    print("\nby category, worst first")
    rows = []
    for category, counts in by_category.items():
        total = counts["passed"] + counts["failed"]
        if total:
            rows.append((counts["passed"] / total, counts["passed"], total, category))
    for share, won, total, category in sorted(rows):
        print(f"  {category:<22} {won:>3}/{total:<3} {100 * share:>3.0f}%")

    # The split the score used to hide. A fixture the server answered *nothing* for is a feature
    # missing whole; one it answered differently for is either a position taken on purpose or a
    # sentence somebody rewrote, and neither is the same kind of debt.
    silent = [pair for pair in failed if getattr(pair[1], "answered", 1) == 0]
    different = [pair for pair in failed if getattr(pair[1], "answered", 1) != 0]
    if failed:
        print(
            f"\nof the {len(failed)} failures: {len(silent)} answered nothing,"
            f" {len(different)} answered something else"
        )
        if different:
            print("  answered something else, so the gap is wording or a position, not a feature:")
            for fixture, why in different:
                print(f"    {fixture.relative}")
                print(f"      wanted: {why}")
                for said in getattr(why, "said", [])[:4]:
                    print(f"      got:    {said}")

    if failed:
        print(f"\nfailures ({len(failed)})")
        shown = failed if args.failures else failed[:25]
        for fixture, why in shown:
            print(f"  {fixture.relative}\n      {fixture.name}\n      {why}")
        if len(shown) < len(failed):
            print(f"  ... and {len(failed) - len(shown)} more (--failures for all)")

    if broken:
        print(f"\nunreadable ({len(broken)})")
        for name, why in broken:
            print(f"  {name}: {why}")


if __name__ == "__main__":
    main()
