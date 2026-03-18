# Changelog

All notable changes to pacSelect are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [0.4.0] — 2026-03-18

### Added

- **Graphics filter category** — the full graphics stack is now its own named
  filter tier, separate from system/core. Covers all Mesa sub-packages
  (`opencl-mesa`, `vulkan-mesa-*`, `libva-mesa-driver`, `mesa-vdpau`, …),
  NVIDIA/AMD/Intel GPU drivers, the Vulkan/GL dispatch layer (`vulkan-icd-loader`,
  `libglvnd`), Xorg, Wayland, and the kernel GPU userspace bridge (`libpciaccess`).
  Users now see `graphics: N` in the summary bar and deferred graphics packages
  grouped under `graphics:` in the skipped list — making it obvious when a full
  upgrade is needed to pick up GPU driver updates.
- **Group-based safety net** — after name-pattern classification, a single
  `pacman -Si` batch query checks the Groups field of every safe package.
  Any package whose group is `xorg`, `plasma`, `base`, or similar is demoted
  to skipped automatically, catching packages the name patterns miss.
- **Package descriptions** — the safe-to-install list now shows a short
  description from `pacman -Si` below each package name. Zero extra process
  calls: descriptions are extracted from the same batch query used for dep-check
  and group detection. Hide with `--no-descriptions` or `display.descriptions = false`
  in config.
- **Grouped skipped display** — deferred packages are now shown grouped by
  category (`system`, `graphics`, `kde`, `user`, `partial`) instead of a flat
  word-wrapped list, so users get an at-a-glance picture of what part of the
  system is being held back.
- **`--upgrade-config`** — merges any new default keys from the current schema
  into an existing config file, preserving all values the user has already set
  and their comments. Prints a list of added key paths. Safe to re-run; reports
  "Config is already up to date." when nothing is missing.
- **`[display]` config section** — `descriptions` (bool, default `true`) and
  `verbose` (bool, default `false`) can now be set persistently in config.
- **`[behavior]` config section** — `auto_confirm` (bool, default `false`) and
  `dry_run` (bool, default `false`) can now be set persistently in config.
  All CLI flags still override their config equivalents.
- **`src/config_upgrade.rs`** — self-contained, copy-portable TOML config
  upgrade module. No pacselect-specific logic; depends only on `toml_edit`
  and `anyhow`. Includes 4 unit tests.

### Changed

- `SkipReason` gains a `Graphics` variant (replaces ad-hoc graphics entries
  inside `SystemCore`). `GroupFilter(String)` variant added for packages
  demoted via their pacman group.
- `depcheck::check()` replaced by `depcheck::check_all()` which returns a
  single `SiResult` containing dep warnings, group demotions, and descriptions
  — one `pacman -Si` subprocess for all three.
- Summary bar breakdown now shows `system`, `graphics`, `kde`, `user`,
  `partial` counts individually.
- `--list-filters` output now includes the Graphics patterns section.

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
