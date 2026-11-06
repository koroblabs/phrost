# phrost for Claude Code

Gives Claude Code's `LSP` tool a PHP server — goto, references, hover, document and workspace
symbols, implementations and call hierarchy — and puts phrost's diagnostics into Claude's context
instead of whatever the editor happens to be running.

## Install

The `command` field takes a binary name and **not** a path, so `phrost` has to be on `$PATH`:

```sh
cargo build --release -p phrost-cli
ln -sf "$PWD/target/release/phrost" /usr/local/bin/phrost
```

Then point Claude Code at the plugin directory:

```sh
claude --plugin-dir /path/to/phrost/editors/claude-code
```

`/reload-plugins` picks up edits to these files without restarting the session.

## The extensions, and why there are seven

`.module`, `.inc`, `.install`, `.theme`, `.profile` and `.engine` are all PHP, and a Drupal project
keeps a great deal of code in them — `app_core.module` in the checkout this was measured on is one
of the larger files in the project. A mapping of `.php` alone would leave the server silent in
exactly the files a Drupal developer edits most.

`.inc` carries a caveat worth knowing rather than avoiding: a dependency's test fixtures are also
`.inc`, and they declare deliberately broken PHP. The index ranks a dependency's test data below
everything, so those cannot shadow a real declaration — see `docs/08-index.md`.

## Configuration

`initializationOptions` reaches the server at `initialize` and overrides `phrost.toml`, per setting.
To turn on the house-style checks without a file in the project root:

```json
{
  "php": {
    "command": "phrost",
    "args": ["serve"],
    "extensionToLanguage": { ".php": "php" },
    "initializationOptions": {
      "lint": {
        "categories": ["safety", "security", "correctness", "deprecation"],
        "off": ["strict-types"],
        "on": ["doc-missing-tags", "doc-param-name"]
      }
    }
  }
}
```

Left out of the shipped file deliberately: a lint policy is a project's decision, and baking one
into the plugin would apply it to every project the plugin is loaded in.

## Two things that are not documented, and what we assume

**Whether the tool waits for indexing.** The server sends `window/workDoneProgress/create` and
reports `$/progress` while it indexes, which takes about three seconds on a 30 000-file checkout.
Whether the `LSP` tool honours that or sends the first request immediately is not documented.
`startupTimeout` covers the process starting, not the index being ready. If early requests come back
empty, that is the reason.

**How `workspaceFolder` reaches the protocol.** The server takes its root from `workspace_folders`
or `root_uri` at `initialize`; which of the two this field becomes is not documented. If the server
indexes nothing, that is the first thing to check.
