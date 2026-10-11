# Letting an AI assistant work in Forma

Forma includes an MCP server (Model Context Protocol), so assistants such as Claude
Desktop or Claude Code can build, edit and delete objects, inspect the model and look at
renders. Every change goes through Forma's own commands, the same ones you type, so
**Undo** works and the command history shows what was done.

The server is `forma-cli.exe` (it ships next to `forma.exe` in `Forma-windows.zip`).
It has two modes.

## Live: work on the model open in the app

1. In Forma, choose **Tools ▸ Agent Connection (AI assistants)** (or type
   `AgentConnection`). The command history says it is on.
2. Point the assistant at `forma-cli.exe mcp --live` (see below).
3. Ask, for example: *"Draw a 5 × 4 m room with 15 cm walls, 2.7 m high, and put a
   table in the middle."* The objects appear in the open window; Undo removes them.

The connection is off every time Forma starts, and only works from the same computer:
Forma listens on `127.0.0.1` and writes a random key to `agent.txt` in its settings
folder (`%APPDATA%\Forma`); requests without the key are refused. Turn it off with the
same menu item; it also stops when Forma closes.

While Forma shows a dialog (Save changes?, New model, Recover) the assistant's commands
wait for you to answer it.

## Files: work on `.3dm` files without the app

`forma-cli.exe mcp` keeps a model of its own. The assistant opens and saves files with
the `Open` and `Save` commands. Add `--root <folder>` to allow only files inside one
folder, and `--open <file.3dm>` to start from a model.

## Setting up the assistant

### Claude Desktop

Edit `%APPDATA%\Claude\claude_desktop_config.json` (Settings ▸ Developer ▸ Edit Config)
and restart Claude Desktop:

```json
{
  "mcpServers": {
    "forma": {
      "command": "C:\\Programs\\Forma\\forma-cli.exe",
      "args": ["mcp", "--live"]
    }
  }
}
```

For the file mode use `"args": ["mcp", "--root", "C:\\Users\\Filippo\\Progetti"]`.

### Claude Code

```text
claude mcp add forma -- "C:\Programs\Forma\forma-cli.exe" mcp --live
```

(Use the real path of `forma-cli.exe`.)

## What the assistant can do

| Tool | What it does |
|---|---|
| `forma_run` | Runs command lines, one per line: `Box 0,0,0 200,100,0 75`, `SelLast`, `Extrude 30`, `Move 0,0 0,100`, `Delete #4`, `Undo`, `Layer Walls`, `Save C:/models/a.3dm`, and in the live app `ZEA`, `Top`, `Shaded`. Stops at the first error and says which line failed. |
| `forma_inspect` | Units, layers, every object (id, kind, layer, name, size and position) and the selection. |
| `forma_commands` | The list of commands with their syntax, filtered by a word (`extrude`, `mesh`…). |
| `forma_render` | A shaded picture of the model (perspective, top, front or right). |

## Trying it by hand

```text
forma-cli mcp --root C:\temp
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"forma_run","arguments":{"commands":"Box 0,0,0 10,10,0 10\nSave box.3dm"}}}
```

Each line in is one JSON-RPC message; each answer is one line out.
