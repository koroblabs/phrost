"""Speaks LSP over stdio to a server subprocess.

A module of its own because a second tool needs a second server. Comparing our answers against
another language server's only means something if both are black boxes reached through the
protocol -- the moment one side is asked in-process, the comparison stops being about what an
editor would see.
"""
import json
import os
import select
import subprocess
import time
import urllib.parse
import urllib.request


def uri_of(path):
    """A `file:` URI for an absolute path, escaped the way a client would send it."""
    return "file://" + urllib.parse.quote(os.path.abspath(path))


def path_of(uri):
    """The absolute path a `file:` URI names, or the URI itself if it names something else."""
    if not uri.startswith("file://"):
        return uri
    return urllib.request.url2pathname(urllib.parse.urlparse(uri).path)


class Timeout(Exception):
    """A server did not answer inside the time allowed."""


class Closed(Exception):
    """A server closed the connection, usually because it crashed."""


class Server:
    """One language server, spoken to over its stdin and stdout."""

    def __init__(self, name, argv, root, log=None, init_options=None):
        self.name = name
        self.root = os.path.abspath(root)
        self.log = open(log, "wb") if log else subprocess.DEVNULL
        self.process = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.log,
            cwd=self.root,
            bufsize=0,
        )
        # Reads go through a buffer of our own. Asking `select` about the pipe while a
        # `BufferedReader` holds already-read bytes is a deadlock: a server that writes a
        # notification and a response in one go leaves the response in the buffer, the pipe empty,
        # and `select` waiting for data that has already arrived.
        self.pending = bytearray()
        self.stdout = self.process.stdout.fileno()
        self.next_id = 1
        self.handlers = []
        self.init_options = init_options

    def rss_mb(self):
        """Resident memory of the running server, in MB, or None if it cannot be read.

        Asked of the operating system rather than of the server, because that is the only form of
        the question every server answers the same way: what a session costs the machine it runs
        on, whatever the server is written in and whatever it says about itself.
        """
        try:
            out = subprocess.run(
                ["ps", "-o", "rss=", "-p", str(self.process.pid)],
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (OSError, subprocess.SubprocessError):
            return None
        value = out.stdout.strip().split("\n")[0].strip()
        return round(int(value) / 1024, 1) if value.isdigit() else None

    def cpu_seconds(self):
        """Processor time the server has consumed since it started, or None if unreadable.

        Beside [`rss_mb`](Server.rss_mb) because the two answer different halves of what a session
        costs: memory is what it holds and this is what it burned, and a server that indexes on
        eight threads finishes sooner while spending more.
        """
        try:
            out = subprocess.run(
                ["ps", "-o", "time=", "-p", str(self.process.pid)],
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (OSError, subprocess.SubprocessError):
            return None
        parts = out.stdout.strip().split("\n")[0].strip().split(":")
        if not parts or not parts[-1]:
            return None
        try:
            seconds = float(parts[-1])
            for index, part in enumerate(reversed(parts[:-1]), start=1):
                seconds += float(part) * (60**index)
        except ValueError:
            return None
        return round(seconds, 1)

    def on_notification(self, handler):
        """Registers a `handler(method, params)` called for every notification that arrives."""
        self.handlers.append(handler)

    # -- transport ------------------------------------------------------------------------------

    def _send(self, message):
        body = json.dumps(message).encode()
        try:
            self.process.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
            self.process.stdin.flush()
        except BrokenPipeError as broken:
            raise Closed(f"{self.name} closed its input") from broken

    def _fill(self, deadline):
        """Waits for more bytes. `select` rather than a blocking read, so a hung server times out."""
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise Timeout(f"{self.name} went quiet")
        ready, _, _ = select.select([self.stdout], [], [], remaining)
        if not ready:
            raise Timeout(f"{self.name} went quiet")
        chunk = os.read(self.stdout, 1 << 16)
        if not chunk:
            raise Closed(f"{self.name} closed the connection")
        self.pending += chunk

    def _take(self):
        """One whole message out of the buffer, or None while part of it is still in flight.

        Nothing is consumed until the entire message is present, which is what makes a read safe to
        abandon. Taking the header first and then waiting for the body means a timeout in between
        swallows the header and leaves the stream parsing a body as if it were a header -- and the
        symptom is a confident "sent a header with no length" about a server that sent no such
        thing.
        """
        end = self.pending.find(b"\r\n\r\n")
        gap = 4
        if end < 0:
            end = self.pending.find(b"\n\n")
            gap = 2
            if end < 0:
                return None

        length = None
        for line in bytes(self.pending[:end]).decode("ascii", "replace").splitlines():
            if line.lower().startswith("content-length:"):
                length = int(line.split(":")[1].strip())
        if length is None:
            raise Closed(f"{self.name} sent a header with no length")

        start = end + gap
        if len(self.pending) < start + length:
            return None
        body = bytes(self.pending[start : start + length])
        del self.pending[: start + length]
        return json.loads(body)

    def _read(self, deadline):
        while True:
            message = self._take()
            if message is not None:
                return message
            self._fill(deadline)

    def _dispatch(self, message):
        """Handles anything that is not the response being waited for. Returns it if it is one."""
        if "id" in message and ("result" in message or "error" in message):
            return message
        if "id" in message:
            # A server-to-client *request* is awaited on the server side, so leaving one
            # unanswered hangs whatever sent it. `window/workDoneProgress/create` is sent before
            # indexing starts, and a server blocked there looks exactly like one that is slow.
            self._send({"jsonrpc": "2.0", "id": message["id"], "result": None})
            return None
        for handler in self.handlers:
            handler(message.get("method"), message.get("params") or {})
        return None

    # -- protocol -------------------------------------------------------------------------------

    def notify(self, method, params=None):
        self._send({"jsonrpc": "2.0", "method": method, "params": params or {}})

    def request(self, method, params=None, timeout=30.0):
        """Sends a request and pumps everything else until its response arrives."""
        want = self.next_id
        self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": want, "method": method, "params": params or {}})
        deadline = time.monotonic() + timeout
        while True:
            answer = self._dispatch(self._read(deadline))
            if answer is not None and answer["id"] == want:
                return answer

    def pump(self, seconds):
        """Reads whatever arrives for a while. Used to collect notifications nobody asked for."""
        deadline = time.monotonic() + seconds
        while True:
            try:
                self._dispatch(self._read(deadline))
            except (Timeout, Closed):
                return

    def poll(self, seconds):
        """Handles messages for up to `seconds`, returning whether any arrived.

        Separate from `pump` because quiet is information: a caller waiting for a server to settle
        needs to know that a whole second passed with nothing said.
        """
        deadline = time.monotonic() + seconds
        heard = False
        while True:
            try:
                self._dispatch(self._read(deadline))
            except (Timeout, Closed):
                return heard
            heard = True

    def wait_until(self, ready, timeout, settle=1.0):
        """Waits for `ready()` and for the server to go quiet, whichever takes longer.

        Both halves are needed. `ready()` is fed by notifications, and a server that reports its
        indexing in phases satisfies any single one of them long before it can answer anything --
        so what it says about its state is only believed once it has stopped talking about it.

        Returns whether it became ready inside `timeout`.
        """
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            heard = self.poll(min(settle, max(0.05, deadline - time.monotonic())))
            if ready() and not heard:
                return True
        return False

    def initialize(self, capabilities):
        params = {
            "processId": os.getpid(),
            "rootUri": uri_of(self.root),
            "rootPath": self.root,
            "capabilities": capabilities,
            "workspaceFolders": [{"uri": uri_of(self.root), "name": os.path.basename(self.root)}],
        }
        if self.init_options is not None:
            params["initializationOptions"] = self.init_options
        answer = self.request("initialize", params, timeout=120.0)
        self.notify("initialized", {})
        return answer.get("result") or {}

    def open(self, path, text, version=1):
        self.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": uri_of(path),
                    "languageId": "php",
                    "version": version,
                    "text": text,
                }
            },
        )

    def close(self, path):
        self.notify("textDocument/didClose", {"textDocument": {"uri": uri_of(path)}})

    def stop(self):
        try:
            self.request("shutdown", None, timeout=10.0)
            self.notify("exit")
        except (Timeout, Closed, OSError):
            pass
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
        if self.log is not subprocess.DEVNULL:
            self.log.close()


# A package manager's directory is not always `vendor/`, and the tools here walk a checkout to pick
# files to measure. Drupal installs its core at `<docroot>/core` and its contributed modules at
# `<docroot>/{modules,themes,profiles}/contrib` -- composer-installed, gitignored, and outside
# `vendor/` entirely, which is 11 256 files on one real checkout.
#
# Sampling them makes an instrument measure the wrong project: `capability-audit` asked for
# references at core declarations nothing in the project's own code writes and scored the capability
# 0/20 for correctly answering nothing, and `compare-lsp` drew four of its seven remaining goto gaps
# from `docroot/core` and `docroot/modules/contrib`.
#
# Mirrors `FrameworkProject::dependency_paths` and, for the docroot, `providers/drupal/layout.rs`'s
# marker file. Duplicated rather than asked for, because a tool reaches the server over stdio and
# the server has no request that answers "whose code is this".
CORE_MARKER = os.path.join("lib", "Drupal.php")


def is_dependency_dir(path, root):
    """Whether a directory holds code the project did not write.

    Two kinds, and the second was added after it cost a whole comparison run. Code the project
    *reads and does not own* -- a vendored dependency, Drupal's core and contrib -- which a sampler
    should reach into, because an editor's user reaches there too. And code nothing wrote at all:
    a Drupal public files directory holds compiled Twig templates, one `__TwigTemplate_<hash>` class
    per rendered template, and a server is right not to index them.

    What that cost: `compare-lsp` takes its readiness probe from the *last* file of the sample, so
    the probe was a Twig hash no server has ever heard of. The settle loop then waited out its whole
    600-second warmup and recorded it as bootstrap -- `phrost bootstrap 601.1s` against
    intelephense's 2.4, for a server the perf sweep clocks at 1.6 to 2.3 on the same checkout.
    """
    name = os.path.basename(path)
    if name in ("vendor", "node_modules"):
        return True
    # `sites/<site>/files` is Drupal's public files directory: uploads, aggregated CSS and JS, and
    # the compiled Twig above. Matched by shape rather than by name, because `files` on its own is
    # a name anybody may use for source.
    parent, site = os.path.split(os.path.dirname(path))
    if name == "files" and site and os.path.basename(parent) == "sites":
        return True
    # `contrib` is the name composer's installer paths use and every Drupal project keeps; `custom`
    # beside it is the project's own.
    if name == "contrib":
        return True
    # `sites/simpletest` is where Drupal's test runner materialises throwaway sites.
    if name == "simpletest" and os.path.basename(os.path.dirname(path)) == "sites":
        return True
    # `core` only where it *is* Drupal's, which its own marker file settles -- a directory called
    # `core` is otherwise an ordinary name a project may use for its own code.
    return name == "core" and os.path.isfile(os.path.join(path, CORE_MARKER))


def is_configuration_file(path):
    """Whether a `.php` file is one a server reads as configuration and does not index.

    Drupal's `settings.php`, `default.settings.php`, `example.settings.local.php`, `sites.php` and
    `example.sites.php`, under `sites/` or one site's directory in it: arrays of credentials and
    `$settings` keys with no declaration in them. phrost leaves them out by name -- see
    `providers/drupal/sites.rs`, which this mirrors -- and a sampler that drew one as its readiness
    probe asked about a file the server had never heard of: `a-selection-grows-...` took its anchor
    from the last file of the sample, which on Drupal core's own checkout is
    `sites/example.settings.local.php`, and waited out fifteen minutes for an answer.
    """
    directory, name = os.path.split(path)
    if not name.endswith(".php"):
        return False
    if not any(part in ("settings", "sites") for part in name[: -len(".php")].split(".")):
        return False
    return "sites" in (
        os.path.basename(directory),
        os.path.basename(os.path.dirname(directory)),
    )
