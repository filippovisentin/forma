# Forma in 10 minutes: model a room

*Italiano: [QUICKSTART.it.md](QUICKSTART.it.md)*

This tutorial models a 6 × 4 m room with walls, a floor, a table and a vase, then saves it
as a `.3dm` file you can open in Rhino. Everything is in **centimetres**, the default for new
documents in Forma. You need Forma installed ([INSTALL.md](INSTALL.md)).

![The finished room](images/overview.jpg)

## 0. Five things to know first

1. **Just type.** Keystrokes go to the command line at the top. Type a command name and press
   **Enter** (or **Space**). An autocomplete list helps you: `rec` → `Rectangle`.
2. **Coordinates** are typed as `x,y` or `x,y,z`; `@dx,dy` is relative to the last point.
   Separate them with spaces or Enter: `Rectangle 0,0 600,400` then Enter.
3. **Esc** cancels whatever is going on. **Ctrl+Z** undoes.
4. **Right-drag** orbits the Perspective view and pans the others; the **wheel** zooms.
   **Right click** = Enter, and on an empty command line it repeats the last command.
5. **Double-click a viewport title** (`Top ▾`, `Perspective ▾`) to maximize it, and again to get
   the four views back.

The status bar at the bottom should say **Centimeters**. If it does not, type `New` and Enter.

## 1. Layers for walls and floor (1 min)

Layers are created and made current with one command:

```text
Layer Muri
LayerColor 200,80,60
```

`LayerColor` without a layer name colours the current layer. The colour is also editable
from the **Layers** panel on the right (click the swatch).

## 2. The room outline (1 min)

Click in the **Top** view, then type:

```text
Rectangle 0,0 600,400
```

You can also draw it with the mouse: pick the rectangle tool on the left, click the first
corner, move, and watch the live **width × height** next to the cursor; type `600` and Enter
to fix the length.

## 3. Walls 15 cm thick (2 min)

Offset the outline inwards, turn the two outlines into a flat ring, and extrude it:

1. Click the rectangle to select it, type `Offset` and Enter.
2. The prompt says `Side to offset ( Distance=10 )`. Click **Distance=10** (or type
   `Distance=15`), enter `15`, then click **inside** the rectangle. A second outline appears.
3. Select both outlines (drag a window around them, or type `SelAll`), then type `PlanarSrf`.
   The inner outline becomes a hole: you get a flat ring.
4. Type `SelNone`, then `SelLast` to select the ring, then `ExtrudeSrf 280`.

You now have walls 280 cm high. Type `Volume` with the walls selected: 8,148,000 cm³, i.e.
(600 × 400 − 570 × 370) × 280. The outlines and the flat ring stay underneath; you can leave
them or select them and press **Delete**.

> Solids in Forma are meshes for now (exact solids come with the OpenCascade kernel).
> They display, snap, measure and save correctly, but there are no booleans yet, so door and
> window openings are made by modelling the wall pieces around them (see
> [examples/soggiorno.txt](examples/soggiorno.txt)).

## 4. Push / pull: make the room 3 m high (1 min)

1. In the Perspective view, hold **Ctrl+Shift** and click the **top face** of the walls.
   It is outlined in yellow and an orange arrow appears.
2. Drag the arrow upwards: the live label shows `Pull … cm`. Release to apply.
   Or **click** the arrow, type `20` and Enter for an exact value (negative values push in).
3. The walls are now 300 cm high, and the faces around the top stretched to follow.

![Push / pull with the live measurement](images/pushpull.jpg)

The same works on any flat face of a box or extrusion. From the command line:
`MoveFace #id <point on face> <distance>`, or `PushPull <direction> <distance>` on the
selected solids.

## 5. Floor and table (1 min)

```text
Layer Pavimento
LayerColor 196,178,150
Box 0,0 600,400 -10
Layer Arredi
LayerColor 150,105,70
Box 200,150 380,250 75
```

`Box` takes two corners of the base and a height; a negative height goes down, so the floor
slab sits under the walls. The second box is a 180 × 100 table block, 75 cm high.

## 6. The gumball (2 min)

Click the table. The **gumball** appears: arrows, arcs and small squares.

- **Drag an arrow** to move along that axis. Object snaps guide the drag.
- **Click an arrow**, type `50` and Enter to move exactly 50 cm.
- **Drag an arc** (or click it and type `90`) to rotate.
- **Drag a square** to move in a plane.
- The **dot on each arrow** extrudes: on a solid it pushes or pulls the face on that side.
  Click the blue dot of the table, type `-3`, Enter: the table is now 72 cm high.

Now a vase: type `Circle 520,320 30`, select the circle, and drag the **blue dot** upwards
(about 60 cm). A closed curve extrudes into a solid.

![Gumball and Properties panel](images/gumball.jpg)

## 7. Colours and layers (1 min)

- With the vase selected, open the **Properties** panel (F3): it shows the layer, the colour and
  the size. Click a colour swatch (or choose a custom colour and click **Apply**), or type
  `SetObjectColor 90,140,100`.
  `SetObjectColor ByLayer` goes back to the layer colour.
- Move objects to another layer: select them, type `ChangeLayer Arredi`, or make the layer
  current in the **Layers** panel and click **Move Selection Here**.
- In the **Layers** panel the bulb hides / shows a layer and the padlock locks it.
- Try the display modes from the viewport title menu (`Perspective ▾`): Wireframe, Shaded,
  Ghosted, X-Ray.

## 8. Save and open in Rhino (1 min)

1. **Ctrl+S** (or **File → Save**), choose a folder and a name such as `stanza.3dm`.
   The window title shows `*` while there are unsaved changes.
2. Open `stanza.3dm` in Rhino (any recent version reads it). You will find:
   - the layers `Muri`, `Pavimento`, `Arredi` with their colours, and units in centimetres;
   - the outlines and the vase's circle as exact curves;
   - the walls, floor, table and vase as **meshes**.
3. Going the other way, Forma opens Rhino files for viewing and light editing, but Rhino
   solids and surfaces become meshes and curves become polylines. When you save such a
   file, Forma proposes a new name (`name-forma.3dm`) so the original is never overwritten.

If something does not look right in Rhino, please
[open an issue](https://github.com/filippovisentin/forma/issues/new/choose) with the file.

## Where next

- `Help` (or F1) lists every command with its syntax.
- Curve tools on the **Curve Tools** tab: `Offset`, `Trim`, `Split`, `Extend`, `Fillet`,
  `FilletCorners`, `Chamfer`, `Join`.
- `Array`, `ArrayLinear`, `ArrayPolar` for chairs around a table.
- The sample room in the screenshots is a script: run
  `forma-cli run --file soggiorno.txt` with [examples/soggiorno.txt](examples/soggiorno.txt)
  and open the resulting `soggiorno.3dm`.
