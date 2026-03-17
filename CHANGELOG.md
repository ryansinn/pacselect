# Changelog

All notable changes to pacSelect are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [0.3.0] — 2026-03-16

### Fixed

- **Eliminated `pacman -Sy <packages>` partial-upgrade antipattern** — `install_packages`
  previously passed `-y` to pacman, which re-synced the live database just before installing
  the selected subset of packages. This is exactly the scenario the
  [Arch wiki §3.3](https://wiki.archlinux.org/title/System_maintenance#Partial_upgrades_are_unsupported)
  warns against: a mid-flight DB sync can expose newer shared library versions while leaving
  unselected packages on the old versions, silently creating a partially-upgraded system.
  The `-y` flag has been removed from `install_packages`; the single authoritative DB sync
  now happens earlier in the main flow via `depcheck::sync_db()`, before both the
  `pacman -Si` dependency query and the install step.

### Added

- **`--full-upgrade` flag** — runs `sudo pacman -Syu`, the Arch-recommended full upgrade path.
  Use this periodically to apply deferred system/core and KDE packages and prevent
  partial-upgrade drift from accumulating. Respects `--yes` to skip the confirmation prompt.
- **Post-install nudge** — after a successful selective install, if any system/core or KDE
  packages remain deferred, pacSelect prints a reminder to run `--full-upgrade` or
  `sudo pacman -Syu` periodically.
- **`depcheck::sync_db` is now `pub`** — allows callers to control the sync lifecycle
  explicitly rather than having it buried inside `check()`.

---

## [0.2.0] — 2026-03-16

### Changed

- **Dependency safety check is now a hard block** — packages that depend on a
  skipped package are moved into the skipped list and never installed, instead of
  showing an advisory warning and proceeding. This eliminates the entire class of
  partial-upgrade problems that the previous warn-only behaviour could allow.
  - New `SkipReason::PartialUpgrade { needs }` variant, displayed as
    `partial upgrade risk — needs skipped: <pkg>, …`
  - Summary bar now includes a `partial: N` counter when any packages are
    blocked by this rule.

### Removed

- Inline `⚠ needs skipped: …` warning annotations on safe packages (no longer
  applicable — those packages are now blocked outright).
- "Partial upgrade warnings" summary block shown before the confirmation prompt.
- `dep_warning` field from JSON safe-package objects.
- `dep_warnings` top-level array from JSON output.

---

## [0.1.0] — 2026-03-16

Initial release.

### Added

- `checkupdates`-based pending-update detection (read-only, never touches pacman state)
- System/core filter with ~100 built-in patterns (kernel, initramfs, systemd,
  glibc, Mesa, GPU drivers, pipewire, wireplumber, and more)
- KDE core filter — unconditionally blocks session-critical packages (`kwin`,
  `plasma-*`, `sddm`, `kscreenlocker`, etc.)
- KDE Frameworks version-bump detection — probes the installed Frameworks
  minor version at runtime (`kcoreaddons` / `kf6-kcoreaddons` with fallbacks)
  and defers any KDE ecosystem package moving to a new minor line
- AUR / foreign-package labelling via `pacman -Qm`
- Dependency safety check — syncs pacman db and queries `pacman -Si` to detect
  when a safe package depends on a skipped one
- Append-only history log at `~/.local/share/pacselect/history.log`
- `--json` machine-readable output (designed as backend for a future tray app)
- TOML config at `~/.config/pacselect/config.toml` with `filter_sets` and
  `extra_skip` glob patterns
- CLI flags: `--dry-run`, `--yes`, `--verbose`, `--json`, `--skip`,
  `--no-system-filter`, `--no-kde-filter`, `--config`, `--list-filters`,
  `--gen-config`
- ASCII "pacSelect" logo (pac in white, Select in red)
- PKGBUILD for local `makepkg -si` installation
