# Publishing checklist (release day)

Steps for Filippo to make Forma public. Prepared on 2026-10-10; nothing below has been done
automatically (no push, no visibility change, no release edits).

## Before you start: what was found

- **The repository already reports as public.** `gh api repos/filippovisentin/forma` returned
  `"visibility": "public"` on 2026-10-10. If you meant to keep it private until release day,
  check Settings now. Everything already pushed (v0.1.0–v0.6.0 and their release assets) may
  already be visible.
- **All six releases are marked as pre-releases.** Because of that,
  `https://github.com/filippovisentin/forma/releases/latest` returns *Not Found*, and the
  **Download** buttons in the README and on the site (which point there) will not work until
  step 3 below. `release.yml` publishes with `prerelease: true`, so future releases will be
  pre-releases too unless the workflow is changed.
- **Private model files are not in git history** (details in the checklist at the end).
- Local `main` has commits that are **not pushed yet** (the UI review plus these docs).

## 1. Push

```sh
git push origin main
```

Wait for the `ci` workflow to be green on both Linux and Windows.

## 2. Repository visibility

Settings → General → scroll to **Danger Zone** → **Change visibility** → **Make public** →
confirm with the repository name. (Skip if it already shows as public; see above.)

## 3. Make v0.6.0 the latest release

Releases → **Forma v0.6.0** → **Edit** (pencil) →

- untick **Set as a pre-release**
- tick **Set as the latest release**
- optionally replace the body with the `0.6.0` section of [CHANGELOG.md](../CHANGELOG.md),
  keeping the download and SmartScreen lines
- **Update release**

Then check that both of these download the zip:

- https://github.com/filippovisentin/forma/releases/latest
- https://github.com/filippovisentin/forma/releases/latest/download/Forma-windows.zip

GitHub has no "pin a release" button: the release marked **Latest** is the one shown in the
repository sidebar and served by `/releases/latest`. To keep it that way for future versions,
either untick "pre-release" on each new release or change `prerelease: true` to `false` in
`.github/workflows/release.yml` (a workflow change; not done here).

## 4. GitHub Pages

GitHub Pages can publish only from the repository root or from `/docs`, not from `/docs/site`.
`docs/index.html` therefore redirects to `docs/site/`, and `docs/.nojekyll` turns off Jekyll.

Settings → **Pages** → Build and deployment → Source: **Deploy from a branch** →
Branch: **main**, folder: **/docs** → **Save**.

After a minute or two the page is at **https://filippovisentin.github.io/forma/** (it redirects to
`/forma/site/`). Open it and check the screenshots and the Download button.

(Alternative for later: a small Pages workflow that uploads only `docs/site`; that is a new
workflow and was not added.)

## 5. Description, website, topics

On the repository front page, click the gear next to **About**:

- **Description**: `Personal Rhino-style 3D modeller for interior design, in Rust, built with AI agents`
- **Website**: `https://filippovisentin.github.io/forma/` (tick "Use your GitHub Pages website")
- **Topics**: `cad`, `3d-modeling`, `nurbs`, `interior-design`, `rust`, `egui`, `wgpu`,
  `opennurbs`, `3dm`, `ai-agents`, `claude-code`
  (suggestion: no `rhino` topic, to keep clear of implying affiliation)
- Leave **Releases** ticked; untick **Packages** and **Deployments** if you prefer a cleaner sidebar.

Settings → General → **Social preview** → **Edit** → upload
[`docs/images/social-preview.jpg`](images/social-preview.jpg) (1280 × 640).

## 6. Profile and portfolio

- Your profile → **Customize your pins** → pin `forma`.
- Add a link from filippovisentin.github.io to the repository or the Pages site.

## 7. Final checklist

- [x] **No private files in history.** `git log --all -- tests/data/private` printed nothing
      (no commit has ever touched that path). No `.3dm` file has ever been added in any
      commit, no blob larger than 500 KB exists in history, and `git ls-tree -r origin/main`
      contains no `.3dm`. `tests/data/private/` (holding `binario.3dm` and `gggg.3dm`
      locally) is ignored by `.gitignore` line 2 (`/tests/data/private/`).
- [x] Simple secret scan of tracked files (API keys, tokens, private keys): nothing found.
- [ ] Commit author e-mail: every commit shows `filippovise2003@gmail.com`. That is normal on
      GitHub, but if you prefer, use your GitHub `noreply` address for future commits
      (Settings → Emails → "Keep my email addresses private").
- [ ] `git push origin main` done and CI green on Linux and Windows.
- [ ] v0.6.0 is **Latest**, not pre-release; `/releases/latest` works.
- [ ] Pages enabled from `main` `/docs`; the site loads and the Download button works.
- [ ] README renders on GitHub: screenshots visible, links to `docs/INSTALL.md`,
      `docs/QUICKSTART.md`, `CHANGELOG.md`, `CONTRIBUTING.md` work.
- [ ] Open a `.3dm` saved by Forma in **your Rhino 8** (for example `stanza.3dm` from the
      tutorial). The roadmap item "a file written by Forma opens correctly in Rhino" is still
      unchecked, and the README and tutorial say it works.
- [ ] Issues are enabled (Settings → General → Features); the bug / feature templates appear on
      **New issue**.
- [ ] Optional: Discussions on, for questions that are not bugs.
