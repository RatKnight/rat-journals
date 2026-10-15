# Rebrand Round 2: Legacy Identifiers Users Can See

**Date:** 2026-10-13
**Status:** Exploration (no code changes)
**Builds on:** [2026-09-08-rebranding-audit.md](2026-09-08-rebranding-audit.md), [2026-09-20-rebrand-name-shortlist.md](2026-09-20-rebrand-name-shortlist.md), and the plan `docs/plans/2026-09-20-mini-diarium-display-name-rebrand-plan.md`

## Question

The plan renames the display name to **Fenmark** but keeps every identifier. Some of those identifiers appear in install commands, for example `flatpak install flathub io.github.fjrevoredo.mini-diarium`. Telling users to install `mini-diarium` for an app called Fenmark looks wrong. Which identifiers can change, at what cost, and when?

## Summary

```
                     USERS TYPE OR SEE IT?
                 no ◄─────────────────────────► yes
             ┌──────────────────────┬──────────────────────────┐
  cheap to   │ crate / npm names    │ WinGet moniker (add)     │
  change     │ (internal sweep)     │ Homebrew cask token      │
             │                      │ Binary name              │
             │                      │ GitHub repo + tap repo   │
             ├──────────────────────┼──────────────────────────┤
  costly or  │ Tauri identifier     │ Flatpak app ID           │
  impossible │   com.minidiarium    │ WinGet PackageIdentifier │
             │ MSIX Identity Name   │ SourceForge slug (no)    │
             │ Store product ID     │                          │
             └──────────────────────┴──────────────────────────┘
   → keep the left column; rename the right column in a planned order.
```

Two new **Stage 1 blockers** came out of this round. They come from `productName` alone, not from any identifier. See "New Stage 1 findings".

## Identifier Inventory

| Identifier | Current value | Where users see it | Mechanism to rename | Cost | Recommendation |
|---|---|---|---|---|---|
| Flatpak app ID | `io.github.fjrevoredo.mini-diarium` | `flatpak install` / `run` / `update`, Flathub URL, `~/.var/app/` | Resubmit under new ID; old ID in `<provides>` + `<replaces>`; `flathub.json` with `end-of-life-rebase` on the old repo | Medium, one time | **Rename** to a `fenmark.app` domain ID (for example `app.fenmark.Fenmark`) |
| WinGet PackageIdentifier | `fjrevoredo.MiniDiarium` | `winget install` / `upgrade` | Move each version to the new ID (`YamlCreate.ps1` "Move package to a new identifier"), one remove PR and one add PR | Medium, breaks scripts with the old ID | **Rename** to `fjrevoredo.Fenmark` in the release that changes the installer name; add moniker `fenmark` |
| WinGet moniker | none | `winget install <moniker>` | Add `Moniker:` field to the manifest | Low | **Add** `fenmark` (works even before the ID move) |
| Homebrew cask token | `mini-diarium` | `brew install --cask mini-diarium` | Rename cask file + `cask_renames.json` in the tap; `brew upgrade` / `brew migrate` follow it | Low | **Rename** to `fenmark` |
| Homebrew tap repo | `fjrevoredo/homebrew-mini-diarium` | `brew tap fjrevoredo/mini-diarium` | GitHub repo rename (git remote redirects) | Low | **Rename** to `homebrew-fenmark`, after the cask rename |
| GitHub repo slug | `fjrevoredo/mini-diarium` | Release download URLs, issues, `nix run github:…`, clone URL | GitHub rename (redirects for web, git, API) | Medium: CI, SignPath, Codecov, SonarCloud, Flathub manifest, gh-pages, badges, workflow refs | **Rename last**, as one coordinated step |
| Binary name | `mini-diarium(.exe)` | Linux terminal command, `Exec=`, Task Manager details, nix `bin/` | Set `mainBinaryName: "fenmark"` | Low to medium: NSIS already tracks `OldMainBinaryName`; Flatpak `command:`, nix, `.desktop` change | **Rename** together with the package rename stage |
| Tauri identifier | `com.minidiarium` | Only in data-folder paths that docs print (plugins, backups) | None automatic; app must find or move `config.json`, plugins, backups, and the WebView profile (localStorage preferences) on 3 platforms; macOS sees a new app | **High**; `config.json` holds device-bound keys for local-only journals | **Keep**. Remove the paths from user docs instead (use "Open folder") |
| MSIX Identity Name | `fjrevoredo.MiniDiarium` | Only `%LOCALAPPDATA%\Packages\…` path | Fixed by the Partner Center reservation | Not possible for this product | **Keep** (DisplayName changes already in plan) |
| Store product ID | `9PJFTX44ZS43` | Store URL only | None | n/a | **Keep** |
| SourceForge slug | `mini-diarium` | Project URL | SourceForge projects cannot be renamed; only a new project | High for little value | **Keep**; title only (already in plan) |
| Crate / npm / repo-internal names | `mini-diarium*` | Nobody | Find and replace | Low but wide (about 390 files) | **Keep**; existing TODO for an internal sweep |

## New Stage 1 Findings (affect the current plan)

These follow from changing `productName`. They were open questions in the plan. Source: the Tauri bundler at `@tauri-apps/cli-v2.11.5` (the version in `package.json`).

### 1. NSIS: two installs side by side

```
 installer.nsi:66   UNINSTKEY = …\Uninstall\${PRODUCTNAME}
 installer.nsi:514  INSTDIR   = $LOCALAPPDATA\${PRODUCTNAME}
```

The Fenmark NSIS installer does not find the "Mini Diarium" uninstall key. Result: two entries in Apps & features, two install folders, two Start-menu shortcuts. The user data is safe, because the data folder uses `BUNDLEID` (`com.minidiarium`), which does not change.

Mitigation: an `NSIS_HOOK_PREINSTALL` hook (Tauri `bundle.windows.nsis.installerHooks`). The hook reads the old `UninstallString` and runs it silently. The old uninstaller deletes app data only when its checkbox is set (`installer.nsi:864-883`), so a silent run keeps the data. Verify this on a VM before release.

### 2. Linux .deb / .rpm: file conflict

```
 debian.rs:172  Package: kebab(productName)  → "fenmark"  (was "mini-diarium")
 rpm.rs:62      name    = kebab(productName) → "fenmark"
 both install   /usr/bin/<mainBinaryName>    → same path while the binary stays "mini-diarium"
```

`dpkg` refuses to install `fenmark` because `/usr/bin/mini-diarium` belongs to the `mini-diarium` package. Mitigation: set `bundle.linux.deb.conflicts` + `replaces` to `["mini-diarium"]` and `bundle.linux.rpm.obsoletes` + `conflicts` to `["mini-diarium"]`. Both fields exist in this bundler version (`debian.rs:233-242`, `rpm.rs:118-137`). Then the package manager replaces the old package in one step.

### 3. macOS DMG: two app bundles

A manual DMG install puts `Fenmark.app` next to `Mini Diarium.app` (same bundle ID). The cask rename handles Homebrew users. DMG users need one line in the release notes: "Delete Mini Diarium.app after you install Fenmark".

## Proposed Staging

```
 STAGE 1  display name (current plan + 3 fixes above)
   productName = Fenmark, upgradeCode pinned, NSIS pre-install hook,
   deb/rpm replaces+conflicts, release note for DMG,
   WinGet moniker "fenmark" ← cheap win, no ID change yet
        │
        ▼
 STAGE 2  install identifiers (one coordinated release)
   binary         mini-diarium        → fenmark
   Homebrew cask  mini-diarium        → fenmark   (cask_renames.json)
   WinGet ID      fjrevoredo.MiniDiarium → fjrevoredo.Fenmark (move PRs)
   Flatpak ID     io.github.fjrevoredo.mini-diarium → app.fenmark.Fenmark
                  (resubmit, verify via fenmark.app/.well-known, then EOL-rebase)
        │
        ▼
 STAGE 3  repositories
   GitHub repo    fjrevoredo/mini-diarium → fjrevoredo/fenmark
   tap repo       homebrew-mini-diarium   → homebrew-fenmark
   then the internal-name sweep TODO
        │
        ▼
 NEVER   com.minidiarium · MSIX Identity Name · Store ID · SourceForge slug
```

Why the Flatpak ID waits for Stage 2: Flathub accepts a rename only when the domain part changes and the new domain is verified. `fenmark.app` must serve the site and the verification file first (plan Task 4.3 and the domain setup). Flathub also rejects frequent renames, so do it once, with the final name.

Why the Tauri identifier stays: it is the only rename with a data-loss path. `config.json` holds the device-bound key of local-only journals. The WebView profile (preferences in localStorage) sits under the identifier on Windows and Linux and under the bundle ID on macOS. The v0.3.0 precedent (`src-tauri/src/lib.rs:50-69`, `resolve_app_data_dir`) only reads the legacy folder; it never moves data and never moved the WebView profile. The folder name is invisible in normal use.

## Flatpak ID choice

| Candidate | Valid? | Note |
|---|---|---|
| `app.fenmark.Fenmark` | Yes: 3 components, reverse of `fenmark.app`, last part not generic | Recommended |
| `app.fenmark.fenmark` | Yes | Lower-case style; also fine |
| `io.github.fjrevoredo.fenmark` | Yes, after the GitHub repo rename | Flathub rejects a rename that keeps the same domain part, so this needs Stage 3 first and still ties the ID to GitHub |

## Decisions (2026-10-13)

| # | Question | Decision |
|---|---|---|
| 1 | Stage 2 timing | **Separate release** after Stage 1. Stage 2 gets its own plan (rebrand plan Task 7.1 step 3). |
| 2 | WinGet ID form | **`fjrevoredo.Fenmark`**: same publisher, simple "move" path. |
| 3 | GitHub repo rename | **Rename if easy.** It is easy; see the checklist below. Stage 3. |
| 4 | `mini-diarium` symlink on Linux | **No symlink.** See the reason below. |

The Stage 1 fixes (NSIS hook, deb/rpm replacement, DMG note, WinGet moniker) are now in the rebrand plan: Tasks 3.3, 4.4, 5.2 and 7.2.

### Why no Linux symlink

```
 who runs "mini-diarium" by name?
   Flatpak users  → flatpak run <id>       (binary name not used)
   AppImage users → the file name          (binary name not used)
   Nix users      → meta.mainProgram       (the package update sets it)
   .deb / .rpm    → the .desktop Exec line (the package regenerates it)
   custom scripts → only these break       (small group, one release note line)
```

A symlink needs an extra `bundle.linux.deb.files` / `rpm.files` entry and must be removed later. The bundler copies files; it does not create symlinks, so the link needs a post-install script or a committed link. That is more work and risk than one release-note line: "The Linux command is now `fenmark`".

### GitHub repo rename checklist

The rename itself is one click. GitHub redirects web, git, API and release-download URLs from the old slug. Measured 2026-10-13: 106 tracked files contain `fjrevoredo/mini-diarium`. Most are website pages and archived docs, which work through the redirect. These items need action:

| Item | Why | Action |
|---|---|---|
| GitHub Pages benchmarks `fjrevoredo.github.io/mini-diarium/benchmarks/` | Pages URLs **do not** redirect after a rename | Update `README.md:27,210`; accept the old link breaking |
| `flathub-publish.yml:86` | Hard-coded `vcs-browser` URL check; fails after the metainfo changes | Update together with the metainfo |
| `flatpak/rewrite-manifest.py:18` | Source URL (redirect works, but the Flathub reviewer sees the old name) | Update |
| `homebrew-cask.yml:46` | `gh release download --repo` (redirect works) | Update for clarity |
| SignPath project | The trusted-build origin check names the repository URL | Update the repository URL in the SignPath dashboard **before** the first signed release; verify with a test signing |
| Codecov, SonarCloud | Both link to the repo; usually follow renames | Verify the first CI run after the rename |
| In-app links (`AboutOverlay.tsx:80`, `ProjectSupportOverlay.tsx:17`, `feedback-links.ts:4`, test) | Redirect works for old installs | Update in source |
| Website generators (`generate-website-blog.mjs`, `generate-website-docs.mjs`), `Cargo.toml:7` | Redirect works | Update, rebuild the site |
| `nix run github:fjrevoredo/mini-diarium` | Flake refs follow the redirect | Update the docs |
| Never create a new repo named `mini-diarium` on this account | That breaks every redirect | Note it in the follow-up plan |

Effort: about one session of edits plus one verification release. The SignPath step is the only one that can block a release.

## Sources

- Tauri bundler `@tauri-apps/cli-v2.11.5`: `crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`, `linux/debian.rs`, `linux/rpm.rs`, `linux/freedesktop/mod.rs`
- [Flathub: App maintenance (renaming)](https://docs.flathub.org/docs/for-app-authors/maintenance)
- [Flathub: Requirements (app ID rules and renames)](https://docs.flathub.org/docs/for-app-authors/requirements)
- [Flathub Discourse: EOL rebase renames `~/.var/app` data](https://discourse.flathub.org/t/does-flathub-use-an-internal-id-in-case-packages-are-renamed/6169/2)
- [winget-pkgs discussion #236595: renaming published packages](https://github.com/microsoft/winget-pkgs/discussions/236595)
- [winget-pkgs issue #99791: how to change a package identifier](https://github.com/microsoft/winget-pkgs/issues/99791)
- [Homebrew: Rename a formula or cask](https://docs.brew.sh/Rename-A-Formula)
- Codebase: `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs:50-69`, `msix/Package.appxmanifest:27-35`, `data/linux/io.github.fjrevoredo.mini-diarium.desktop`, `docs/INSTALLATION.md:46-100`, `website/docs-src/06-plugins.md:22-24`, `website/docs-src/09-backups.md:36-38`
