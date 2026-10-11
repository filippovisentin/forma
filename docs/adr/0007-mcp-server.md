# ADR 0007 — MCP server for agents (`serde_json`), and the live link to the app

- Status: accepted
- Date: 2026-10-11

## Context
Milestone M6: AI agents (Claude Desktop, Claude Code) should be able to build and edit
Forma models through the Model Context Protocol. MCP is JSON-RPC 2.0 over stdio, so
`forma-mcp` must read and write JSON. Filippo also wants the agent to work on the model
open in the desktop app, not only on files.

## Options
JSON:
1. A hand-written JSON parser: no dependency, but a parser for untrusted input is
   exactly the code we should not write ourselves.
2. `serde_json` (with `serde`, already in the tree through other crates): the standard,
   heavily fuzzed JSON crate.

Live link to the desktop app:
1. The app embeds the MCP server: the app would have to be started by the agent's
   client, which is not how Filippo works.
2. The MCP server runs as `forma-cli mcp --live` and forwards commands to the running
   app over a localhost TCP connection.

## Decision
`serde_json` 1.x in `forma-mcp` only (`serde_json::Value`, no derives). `forma-mcp`
also uses `forma-render` and `png` (ADR 0005) for the render tool.

Live link (option 2), off by default: Tools ▸ Agent Connection in the app listens on
`127.0.0.1` (random port) and writes the port and a random 128-bit token to
`agent.txt` in the settings folder; requests without the token are refused, the file
is deleted when the connection is turned off or the app exits. Every command runs
through the engine like a typed command (undo works, it shows in the history).

Files: `--root <folder>` limits Open / Save / Import / Export paths to one folder.

## Consequences
- Agents get four tools: run command lines, inspect the model, list commands, render a
  view to PNG. Everything they do goes through the command registry (ground rule 1).
- Any local process of the same user that can read `agent.txt` can drive the open app
  while the connection is on; that is the same trust level as the user's files.
- `serde_json` is one more small, widely used dependency.
