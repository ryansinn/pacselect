# pacSelect

> **Smart app updates. Stable system.**

pacSelect is a selective, session‑safe updater for Arch‑based systems including CachyOS, EndeavourOS, Manjaro, and plain Arch. It updates everyday applications like browsers, terminals, media players, and development tools while skipping kernel, KDE, and other system‑critical packages that would require a reboot or log‑out to apply safely.

Normal `pacman -Syu` is great for intentional full upgrades. pacSelect is for everything in between: staying current on applications daily without touching the kernel, drivers, Plasma session, or systemd.

---

## Why

A typical update batch on a KDE / CachyOS system might include:

- `linux-cachyos-bore` (kernel bump → needs reboot)
- `kwin 6.23 → 6.24` (KDE minor release → needs log-out)
- `plasma-workspace` (running session → unstable if hot-swapped)
- `pipewire` (live audio server → disruptive)
- `firefox`, `ghostty`, `zen-browser-bin` (safe to update now)

pacSelect separates these automatically. You get the app updates immediately; the system updates wait for when you next choose to do a full upgrade.

---

## Features

| Feature | Detail |
|---|---|
| **Safe-app detection** | Uses `checkupdates` (read-only, never touches pacman state) to find pending updates, then classifies each package |
| **System/core filter** | Blocks kernel, initramfs, systemd, glibc, audio pipeline, and ~80 other system packages |
| **Graphics filter** | Separate category covering the full graphics stack: Mesa (all sub-packages), NVIDIA/AMD/Intel drivers, Vulkan/GL dispatch libs, Xorg, Wayland |
| **KDE core filter** | Unconditionally blocks session-critical packages: `kwin`, `plasma-*`, `sddm`, `kscreenlocker`, etc. |
| **KDE version-bump detection** | Detects the installed KDE Frameworks version at runtime and skips *any* KDE ecosystem package moving to a new minor release line (e.g. 6.23 → 6.24) |
| **Group-based safety net** | After name-pattern classification, runs a single `pacman -Si` over all safe packages and demotes any whose pacman group (e.g. `xorg`, `plasma`, `base`) implies system membership |
| **Package descriptions** | Shows a short description from `pacman -Si` below each safe package — no extra process calls |
| **AUR updates** | Queries `paru` or `yay` (if installed) for AUR-only pending updates; displayed in a separate section in the update list |
| **AUR filtering** | AUR-installed packages (including an AUR-built `mesa`, GPU driver, or any other foreign package) pass through all the same name-pattern, group, and dependency filters as official packages — filtering is based on package name and group, not package origin |
| **Self-update check** | At startup, warns if pacselect itself has a pending AUR update and offers to install it via your AUR helper before proceeding |
| **Dependency safety check** | Reuses the same `pacman -Si` batch; any safe package that depends on a skipped package is **blocked** to prevent partial upgrades |
| **Soname-bump detection** | Detects when a library's shared-object version changes and blocks the update if any installed reverse-dependent has no pending rebuild in the repos |
| **Exact-version-pin detection** | Detects when an installed package pins a safe package at a specific version (e.g. `vim-runtime=9.2.0357`); if the pinner is being skipped, updating the pinned package would abort the transaction — so it is blocked too |
| **Grouped skipped display** | Deferred packages are shown grouped by category (`system`, `graphics`, `kde`, `user`, `partial`, `soname`, `pinned`) so you can see at a glance what part of the system is held back |
| **History log** | Appends every run to `~/.local/share/pacselect/history.log` |
| **JSON output** | `--json` emits machine-readable output — designed as the backend for a future KDE system-tray app |
| **User filter patterns** | Config file or `--skip` flag to permanently exclude extra packages, with glob support |
| **Config upgrade** | `--upgrade-config` merges new default keys into an existing config file without overwriting any values you've already set |
| **Full pacman output** | pacman's stdout and stderr are passed through directly — post-install notes, `.pacnew` warnings, and configuration change notices are never suppressed |

---

## Installation

### Prerequisites

```bash
sudo pacman -S pacman-contrib   # provides checkupdates
sudo pacman -S rust              # only needed to build from source
```

### From source

```bash
git clone https://github.com/ryansinn/pacselect.git
cd pacselect
cargo build --release
sudo cp target/release/pacselect /usr/local/bin/
```

### Using the PKGBUILD (local)

```bash
git clone https://github.com/ryansinn/pacselect.git
cd pacselect
makepkg -si
```

This builds, runs tests, and installs via pacman so it is tracked and removable with `sudo pacman -R pacselect`.

### Pre-built binary

Download the `pacselect` binary from the [Releases](https://github.com/ryansinn/pacselect/releases) page and copy it to `/usr/local/bin/`.

---

## Usage

```
pacselect [OPTIONS]
```

### Common invocations

```bash
# Check, show what would change, ask to confirm, then install
pacselect

# See exactly why each package is SAFE or SKIPped
pacselect --verbose

# Preview only — nothing is installed
pacselect --dry-run

# No confirmation prompt
pacselect --yes

# Skip an extra package (glob supported)
pacselect --skip "spotify" --skip "proprietary-*"

# Machine-readable JSON output (implies dry-run)
pacselect --json

# Full system upgrade — includes deferred system/core and KDE packages
pacselect --full-upgrade

# Print all built-in filter patterns
pacselect --list-filters

# Dump a sample config file
pacselect --gen-config > ~/.config/pacselect/config.toml
```

### All options

| Flag | Short | Description |
|---|---|---|
| `--dry-run` | `-n` | Show what would happen; install nothing |
| `--yes` | `-y` | Skip the confirmation prompt |
| `--verbose` | `-v` | Per-package SAFE/SKIP classification with reasons |
| `--no-descriptions` | | Hide the description line shown below each safe package |
| `--json` | | Machine-readable JSON output (implies dry-run) |
| `--skip PATTERN` | | Extra glob pattern to always skip; repeatable |
| `--no-system-filter` | | Disable the system/core filter *(dangerous)* |
| `--no-kde-filter` | | Disable KDE core + version-bump filters |
| `--full-upgrade` | | Run `pacman -Syu` — full upgrade, all filters bypassed |
| `--no-self-update` | | Don't check for a pacselect update on startup |
| `--config PATH` | | Use an alternate config file |
| `--list-filters` | | Print all built-in blocked patterns and exit |
| `--gen-config` | | Print a sample config file to stdout and exit |
| `--upgrade-config` | | Add missing keys to your config file, preserving existing values |

---

## Configuration

pacSelect reads `~/.config/pacselect/config.toml` on startup. Generate a commented template with:

```bash
pacselect --gen-config > ~/.config/pacselect/config.toml
```

If you already have a config from an older version, bring it up to date without losing your settings:

```bash
pacselect --upgrade-config
```

```toml
# ~/.config/pacselect/config.toml

[filter_sets]
# Block kernel, systemd, glibc, audio pipeline, storage, and other core packages
system_core = true

# Block KDE core session packages and version-line bumps
kde_core = true

[filters]
# Extra package patterns to ALWAYS skip, on top of the built-in lists.
extra_skip = [
    # "spotify",
    # "proprietary-*",
]

[display]
# Show a short description below each safe package in the update list
descriptions = true

# Show per-package SAFE/SKIP classification with reasons (same as --verbose)
verbose = false

[behavior]
# Skip the confirmation prompt (same as --yes)
auto_confirm = false

# Never install, only show what would happen (same as --dry-run)
dry_run = false

# Check for a pacselect update on startup and offer to install it first.
# Disable with --no-self-update.
self_update_check = true
```

All CLI flags take precedence over config values when both are set.

---

## How classification works

Each pending update passes through filter layers in order:

```
1. User filters    → extra_skip in config / --skip flags
2. Graphics        → Mesa (all sub-packages), NVIDIA/AMD/Intel drivers,
                     Vulkan/GL dispatch, Xorg, Wayland, libpciaccess
3. System/core     → kernel, boot, glibc, systemd, audio, network, storage, …
4. KDE core        → session-critical: kwin, plasma-*, sddm, kscreenlocker, …
5. KDE version bump → any KDE ecosystem package (k*, attica, solid, …)
                      moving to a new minor release line vs. what's installed
```

After initial classification, further passes over the safe set:

1. Query `pacman -Si` (one batch call) and demote packages whose pacman **group** (e.g. `xorg`, `plasma`, `base`) implies system membership → shown as `partial`
2. Demote packages whose runtime **dependencies** include a skipped package (partial-upgrade prevention) → shown as `partial`
3. Detect **soname bumps**: if a library's `.so` version changes and an installed reverse-dependent has no pending rebuild available, block the library → shown as `soname`
4. Detect **exact-version pins**: if an installed package that is being skipped this run pins a safe package at a specific version (e.g. `vim` requires `vim-runtime=9.2.0357-1.1`), block the pinned package — pacman enforces exact-version constraints strictly and would abort the transaction → shown as `pinned`

A package that clears all layers and all post-classification checks is **safe** and will be installed.

### KDE version-bump detection

At startup pacSelect runs `pacman -Q kcoreaddons` (falling back to `karchive`, `kconfig`, then the `kf6-` prefixed variants) to determine the installed KDE Frameworks minor version (e.g. `6.23`).

Any package in the KDE ecosystem whose update moves from that minor line to a new one (e.g. `6.24`) is deferred. Patch-level updates within the same minor line (e.g. `6.23.0 → 6.23.1`) are **not** blocked. The header at startup shows the detected version:

```
  Desktop: KDE Plasma  ·  KDE Frameworks 6.23
```

---

## Dependency safety check

Before displaying the install list, pacSelect syncs the pacman sync database once (`sudo pacman -Sy`) and then queries `pacman -Si` for all safe packages to check their runtime dependencies. The sync happens **before** both the dependency query and the install step — the install command itself does not re-sync the database. This matches the [Arch Linux guidance](https://wiki.archlinux.org/title/System_maintenance#Partial_upgrades_are_unsupported) to never run `pacman -Sy <packages>` without a full `-u`, since a mid-flight sync can expose newer library versions and create partial-upgrade breakage for deferred packages.

If a safe package depends on a skipped package, it is **blocked** and moved into the skipped list rather than installed:

```
  SKIP  firefox                        120.0-1 → 121.0-1
        (partial upgrade risk — needs skipped: nss)
```

This prevents partial upgrades entirely. The summary bar reflects the count:

```
  Safe to install: 4    17 skipped  (system: 8  graphics: 5  kde: 4)
```

The skipped section groups packages by category:

```
Skipped (17) — use --verbose for details:
  system:    systemd  glibc  openssl  pipewire
  graphics:  opencl-mesa  vulkan-mesa-implicit-layers  nvidia-utils  wayland
  kde:       kwin  plasma-workspace  karchive
  partial:   firefox                 (depends on skipped: nss)
  soname:    libvpx                  (soname bump; waiting for repo rebuild)
  pinned:    vim-runtime             (exact version pinned by skipped: vim)
```

| Category | Reason |
|---|---|
| `system` | Kernel, boot, glibc, systemd, audio pipeline, network, storage, and other core packages |
| `graphics` | Full GPU/display stack: Mesa, NVIDIA/AMD/Intel drivers, Vulkan/GL dispatch, Xorg, Wayland |
| `kde` | KDE session-critical packages (`kwin`, `plasma-*`, `sddm`, …) or a KDE minor-version bump |
| `user` | Matched a pattern from `extra_skip` in your config or a `--skip` flag |
| `partial` | Updating this package alone would be a partial upgrade — it depends on (or belongs to a group with) a package that is being skipped |
| `soname` | This library's shared-object version bumped but one or more installed reverse-dependents have no pending rebuild in the repos yet — updating now would break them |
| `pinned` | An installed package that is being skipped this run depends on this package at an exact version; pacman would abort the transaction if this package were updated alone |

---

## JSON output

`pacselect --dry-run --json` (or just `--json`) emits a JSON object designed to be consumed by automation, scripts, or a future KDE system-tray app:

```json
{
  "desktop": "KDE Plasma",
  "kde_frameworks": "6.23",
  "safe": [
    {
      "name": "firefox",
      "old": "120.0-1",
      "new": "121.0-1",
      "aur": false
    }
  ],
  "skipped": [
    {
      "name": "linux-cachyos-bore",
      "old": "6.12.1-1",
      "new": "6.12.2-1",
      "reason": "system/core",
      "aur": false
    },
    {
      "name": "karchive",
      "old": "6.23.0-1.1",
      "new": "6.24.0-1.1",
      "reason": "KDE version bump 6.23 → 6.24",
      "aur": false
    },
    {
      "name": "firefox",
      "old": "120.0-1",
      "new": "121.0-1",
      "reason": "partial upgrade risk — needs skipped: nss",
      "aur": false
    }
  ]
}
```

The tray app can poll this on a timer and badge when `safe` is non-empty.

---

## History log

Every run is appended to `~/.local/share/pacselect/history.log`:

```
[2026-03-16 14:32:05]
UPDATED (3): firefox ghostty zen-browser-bin
SKIPPED (17): linux-cachyos-bore plasma-workspace kwin ...

[2026-03-16 18:10:44] [aborted]
UPDATED (0):
SKIPPED (12): linux-cachyos-bore kwin ...
```

Modes: normal, `[dry-run]`, `[aborted]`.

---

## What pacSelect does NOT do

- It does **not** modify pacman, its hooks, or its configuration in any way
- It does **not** prevent you from running `sudo pacman -Syu` at any time
- It does **not** hold packages back in the pacman database
- It does **not** replace `pacman -Syu` — use that for intentional full system upgrades

pacSelect is entirely self-contained. Uninstalling it has zero effect on your system's package state.

---

## Roadmap

- [ ] `--upgrade-kde` flag — explicitly include KDE core + version bumps (for intentional log-out upgrades)
- [ ] Named filter presets in config (e.g. `[presets.gaming]` to also hold Mesa/Vulkan)
- [ ] KDE system-tray app consuming `--json` output
- [ ] AUR helper integration (`paru`, `yay`) for AUR package updates

---

## License

GNU General Public License v3.0 — see [LICENSE](LICENSE).
