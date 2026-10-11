# Python scripts in Forma

Forma runs Python 3 scripts inside the app; nothing needs installing. A script drives
Forma through its commands, exactly as if you typed them, so everything you know from the
command line works in a script, and one **Undo** removes all a script did.

## Running a script

- **Tools ▸ Scripts (Python) ▸ Run Python Script…**, or type `RunPythonScript` (a file
  dialog) or `RunPythonScript C:\path\to\script.py`.
- **Scripts as commands**: every `.py` file in the scripts folder
  (`%APPDATA%\Forma\scripts`, **Tools ▸ Scripts ▸ Open Scripts Folder**) is a command
  named after it: `stanza.py` → `Stanza`, `pannelli_muro.py` → `PannelliMuro`. Type the
  name like any command; the scripts are also listed in the Scripts menu.
- Without the app: `forma-cli python script.py [--open model.3dm] [--save out.3dm]`.
- From an AI assistant: the MCP tool `forma_python` (see [AGENTS.md](AGENTS.md)).

What a script prints goes to the command history; an error stops it and shows the
Python traceback with the line number.

## The `forma` module

| Call | What it does |
|---|---|
| `forma.run("Box 0,0,0 100,50,0 30")` | Run one command line; returns its message. Raises `RuntimeError` when the command fails. |
| `forma.objects()` | Every object as a dict: `id`, `kind` (`polysurface`, `curve`, `mesh`…), `layer`, `name`, `min`, `max` (bounding box corners). |
| `forma.selection()` | Ids of the selected objects. |
| `forma.select(ids)` | Select exactly these objects (a list of ids, or several ids). |
| `forma.units()` | Model units: `mm`, `cm`, `m`, `in` or `ft`. |

Refer to objects in commands as `#id`: `forma.run(f"Move #{i} 0,0 0,100")`.
The standard library is there (`math`, `random`, `json`, `itertools`…); C extensions such
as numpy are not.

## Example: a room with a door and cabinets

```python
import forma

L, W, T, H, DOOR = 500, 400, 15, 270, 90     # cm
forma.run("Layer Muri")
forma.run(f"Polyline 0,0 {L},0 {L},{W} 0,{W} c")
forma.run("SelLast")
forma.run(f"Slab {T} {H}")                     # walls on the outline
forma.run(f"Box {L/2-DOOR/2},{-T},0 {L/2+DOOR/2},{T},0 210")
walls, door = [o["id"] for o in forma.objects() if o["kind"] == "polysurface"]
forma.run(f"BooleanDifference #{walls} #{door}")   # door opening

forma.run("Layer Arredi")
for i in range(3):
    forma.run(f"Box {60 + i*140},{W-80},0 {160 + i*140},{W-20},0 75")
print("objects:", len(forma.objects()))
```

Save it as `stanza.py` in the scripts folder and type `Stanza`.
