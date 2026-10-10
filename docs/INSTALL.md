# Installing Forma on Windows

*Italiano: [INSTALL.it.md](INSTALL.it.md)*

Forma is a portable app: there is no installer, nothing is written to the registry, and
you can keep it in any folder.

## Requirements

- Windows 10 or Windows 11, 64-bit (x64)
- A graphics driver with DirectX 12 or Vulkan support (any reasonably recent PC or laptop)
- About 50 MB of disk space (the download is about 14 MB)

## Steps

1. Open the [latest release](https://github.com/filippovisentin/forma/releases/latest) page.
2. Under **Assets**, click **`Forma-windows.zip`** to download it.
   (The separate `forma.exe` asset is the same program without the extra files.)
3. In File Explorer, right-click the downloaded zip → **Extract All…** and choose a folder,
   for example `Documents\Forma`. Do not run Forma from inside the zip.
4. Open the extracted `Forma` folder. It contains:
   - `forma.exe` — the desktop app
   - `forma-cli.exe` — the command-line version (file info, scripts, PNG renders)
   - `LEGGIMI.txt` — a short user guide (in Italian)
   - `LICENSE-MIT`, `LICENSE-APACHE`, `ATTRIBUTION.md` — licences
5. Double-click **`forma.exe`**.
6. The first time, Windows may show a blue box saying **"Windows protected your PC"**.
   This is Microsoft SmartScreen: Forma is not code-signed, so Windows does not know it yet.
   Click **More info**, check that the app name is `forma.exe`, then click **Run anyway**.
   Windows remembers the choice for that file.

Optional: right-click `forma.exe` → **Show more options → Send to → Desktop (create shortcut)**,
or pin it to Start or the taskbar.

## Opening files

- **File → Open** or **Ctrl+O**, or drag a `.3dm` file onto the Forma window.
- To open `.3dm` files with Forma by double-clicking them, right-click a `.3dm` file →
  **Open with → Choose another app → More apps → Look for another app on this PC** and select
  `forma.exe`. Leave "Always use this app" unchecked if Rhino should stay the default.

## Updating

Download the new `Forma-windows.zip` and extract it over the old folder (or into a new one).
Your settings are stored separately and are kept.

## Where settings are stored

Forma remembers active object snaps, Grid Snap, Ortho, Planar, SmartTrack, Gumball, grid
visibility, the selection filter, each viewport's display mode, the toolbar tab, panel widths,
the last folder used and the 10 most recent files. They live in one text file:

```
%APPDATA%\Forma\settings.txt
```

(Paste `%APPDATA%\Forma` into the File Explorer address bar to open the folder.) Delete the file
to go back to the defaults.

Forma does not create any other files except the `.3dm` files you save.

## Uninstalling

1. Delete the folder where you extracted Forma.
2. Optionally delete `%APPDATA%\Forma` to remove the settings.

That is all: nothing else is installed.

## Troubleshooting

| Problem | What to do |
|---|---|
| "Windows protected your PC" with no **Run anyway** button | Click **More info** first. On a managed (school or work) PC, an administrator may have disabled unsigned apps. |
| "VCRUNTIME140.dll was not found" | Install the *Microsoft Visual C++ Redistributable (x64)* from Microsoft's website, then start Forma again. |
| The window opens but the viewports stay empty or the app closes at once | Update the graphics driver from the manufacturer's site (Intel, AMD, NVIDIA). Forma needs DirectX 12 or Vulkan. |
| A `.3dm` file opens but some objects are missing | Block instances, text and dimensions are not displayed yet. Run `forma-cli.exe info file.3dm` in a terminal to see what the file contains. |
| Anything else | Open an [issue](https://github.com/filippovisentin/forma/issues/new/choose) and attach, if you can, the `.3dm` file and a screenshot. |
