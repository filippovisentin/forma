# ADR 0008 — Python scripting inside the app (RustPython)

- Status: accepted
- Date: 2026-10-11

## Context
Filippo wants Rhino-style Python scripts: loops and arithmetic over commands (a row of
panels, a room from its sizes), saved and reused as commands, and later written by an AI
model. He wants Python inside the app, with nothing else to install.

## Options
1. The user's own Python talking to the app over the agent connection: no new
   dependency, but needs a Python installation and a running app.
2. CPython embedded with `pyo3`: the real interpreter and its whole ecosystem, but
   `python3XX.dll` and its standard library must ship next to `forma.exe` and match
   the version linked at build time (fragile on CI and for users).
3. RustPython: a Python 3 interpreter written in Rust, compiled into the binary with its
   standard library (`freeze-stdlib`). No DLLs, same build on Linux and Windows; slower
   than CPython and no C extensions (numpy), which modelling scripts do not need.

## Decision
RustPython 0.4 (`rustpython`, `rustpython-vm`) in a new crate `forma-script` (layer 4).
0.5/0.6 do not build today against their own parser fork (`rustpython-ruff_*` resolve to
incompatible versions). The crate exposes a `forma` module (`run`, `objects`,
`selection`, `select`, `units`) whose calls go through command lines (ground rule 1);
front ends plug in through the `Host` trait. A whole script is one undo step
(`Document::group_undo_since`).

Layers move up by one to make room: `forma-script` 4, `forma-ui` and `forma-mcp` 5, apps 6.

## Consequences
- `RunPythonScript`, scripts folder as commands, `forma-cli python`, MCP `forma_python`.
- The release binary grows by about 25 MB (interpreter and frozen standard library).
- Scripts can do anything the user can (files, network): the MCP server refuses Python
  when it runs with `--root`, and AI-written scripts are always shown before they run.

## Addendum: `Ask` (AI-written scripts)
`Ask` sends a request in plain words to a language model and shows the Python script it
writes; the user runs it, asks for a fix (the traceback goes back to the model) or saves
it in the scripts folder as a command. The model is **local by default** (Ollama at
`http://localhost:11434`; LM Studio and other OpenAI-style servers at `…/v1`), set by
URL in the Ask window, so models and requests stay on the computer. An assistant on
another account or machine (e.g. Claude) uses the existing agent connection instead
(`forma-cli mcp --live`, ADR 0007).

- The HTTP client is a few dozen lines over `std::net` (plain `http://`, one request per
  connection, `Content-Length` and chunked answers): enough for a local server, and no
  TLS stack or HTTP crate in the build. `https://` URLs are refused with a message.
- `serde_json` (already used by `forma-mcp`, ADR 0007) is added to `forma-script` to
  build requests and read answers.
- The prompt carries the `forma` module's API and every command's one-line syntax
  (`Engine::command_list`), well under a 16k-token context.
- Nothing the model writes runs without the user pressing Run; one Undo reverts it.
