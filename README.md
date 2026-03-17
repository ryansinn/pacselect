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
| **System/core filter** | Blocks kernel, initramfs, systemd, glibc, Mesa, GPU drivers, audio pipeline, and ~100 other system packages |
| **KDE core filter** | Unconditionally blocks session-critical packages: `kwin`, `plasma-*`, `sddm`, `kscreenlocker`, etc. |
| **KDE version-bump detection** | Detects the installed KDE Frameworks version at runtime (`pacman -Q kcoreaddons`) and skips *any* KDE ecosystem package moving to a new minor release line (e.g. 6.23 → 6.24) |
| **AUR labelling** | Runs `pacman -Qm` to flag foreign/AUR packages in output and JSON |
| **Dependency safety check** | After classifying, runs `pacman -Si` against safe packages; any that depend on a skipped package are **blocked** (moved to skipped) to prevent partial upgrades |
| **History log** | Appends every run to `~/.local/share/pacselect/history.log` |
| **JSON output** | `--json` emits machine-readable output — designed as the backend for a future KDE system-tray app |
| **User filter patterns** | Config file or `--skip` flag to permanently exclude extra packages, with glob support |

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
| `--json` | | Machine-readable JSON output (implies dry-run) |
| `--skip PATTERN` | | Extra glob pattern to always skip; repeatable |
| `--no-system-filter` | | Disable the system/core filter *(dangerous)* |
| `--no-kde-filter` | | Disable KDE core + version-bump filters |
| `--full-upgrade` | | Run `pacman -Syu` — full upgrade, all filters bypassed |
| `--config PATH` | | Use an alternate config file |
| `--list-filters` | | Print all built-in blocked patterns and exit |
| `--gen-config` | | Print a sample config file to stdout and exit |

---

## Configuration

pacSelect reads `~/.config/pacselect/config.toml` on startup (created automatically on first run if you use `--gen-config`).

```toml
# ~/.config/pacselect/config.toml

[filter_sets]
# Block system/core packages (kernel, systemd, glibc, Mesa, GPU drivers, …)
system_core = true

# Block KDE core session packages and version-line bumps
kde_core = true

[filters]
# Extra package patterns to ALWAYS skip, on top of the built-in lists.
# Supports prefix globs ("myapp*") and suffix globs ("*-git").
extra_skip = [
    # "spotify",
    # "discord",
    # "proprietary-*",
]
```

---

## How classification works

Each pending update passes through four filter layers in order:

```
1. User filters    → extra_skip in config / --skip flags
2. System/core     → ~100 kernel/systemd/glibc/Mesa/driver patterns
3. KDE core        → session-critical: kwin, plasma-*, sddm, kscreenlocker, …
4. KDE version bump → any KDE ecosystem package (k*, attica, solid, …)
                      moving to a new minor release line vs. what's installed
```

A package that passes all four layers is **safe** and will be installed. The first layer that matches determines the skip reason shown in `--verbose` output.

### System/core packages (examples)

`linux*`, `linux-cachyos-*`, `linux-firmware`, `systemd`, `glibc`, `mesa`, `lib32-mesa`, `nvidia`, `nvidia-dkms`, `pipewire`, `pipewire-*`, `libpipewire`, `wireplumber`, `kmod`, `dbus`, `openssl`, `bpf`, `cpupower`, `xorg-server`, `xorg-xwayland`, `wayland`, `qt6-base`, `pacman`, …

Run `pacselect --list-filters` to see the full list.

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
  Safe to install: 4    17 skipped  (system: 12  kde: 4  user: 0  partial: 1)
```

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
