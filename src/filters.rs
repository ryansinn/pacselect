/// Built-in filter patterns for packages that should not be updated
/// during a live session without a full system upgrade + restart.
///
/// Pattern syntax (matched case-insensitively against package names):
///   "foo*"  → any name starting with "foo"
///   "*foo"  → any name ending with "foo"
///   "foo"   → exact match only

// ─────────────────────────────────────────────────────────────────────────────
// SYSTEM / CORE packages
// Updating any of these requires a reboot, session restart, or risks
// partial-upgrade breakage (glibc, mesa, Qt base, etc.)
// ─────────────────────────────────────────────────────────────────────────────
pub const SYSTEM_CORE_PATTERNS: &[&str] = &[
    // ── Kernel ──────────────────────────────────────────────────────────────
    "linux",
    "linux-lts",
    "linux-zen",
    "linux-hardened",
    "linux-cachyos",
    "linux-cachyos-*",          // bore, bmq, lto, server, tt, rc variants
    "linux-*-headers",
    "linux-headers",
    "linux-firmware",
    "linux-firmware-whence",
    "linux-api-headers",
    "kernel-modules-hook",

    // ── Boot / initramfs ────────────────────────────────────────────────────
    "grub",
    "efibootmgr",
    "refind",
    "dracut",
    "mkinitcpio",
    "mkinitcpio-*",
    "systemd-boot-manager",
    "sbctl",

    // ── Core C runtime ──────────────────────────────────────────────────────
    "glibc",
    "lib32-glibc",
    "gcc-libs",
    "lib32-gcc-libs",
    "filesystem",               // base dir layout

    // ── Init system / service manager ───────────────────────────────────────
    "systemd",
    "systemd-libs",
    "systemd-sysvcompat",
    "systemd-ukify",
    "systemd-resolvconf",
    "dbus",
    "dbus-broker",
    "dbus-broker-units",
    "dbus-daemon",

    // ── Core utils / auth ───────────────────────────────────────────────────
    "bash",                     // risky to swap the running shell mid-session
    "util-linux",
    "util-linux-libs",
    "shadow",
    "pam",
    "linux-pam",
    "sudo",
    "polkit",
    "kmod",

    // ── Core widget toolkits ─────────────────────────────────────────────────
    // Updating these mid-session can affect Plasma and running Qt/GTK apps
    "qt6-base",
    "qt5-base",
    "glib2",
    "lib32-glib2",

    // ── Package manager itself ───────────────────────────────────────────────
    "pacman",
    "pacman-*",
    "libalpm",
    "archlinux-keyring",
    "cachyos-keyring",
    "cachyos-mirrorlist",
    "cachyos-rate-mirrors",

    // ── Security / TLS core ──────────────────────────────────────────────────
    "openssl",
    "ca-certificates",
    "ca-certificates-utils",
    "ca-certificates-mozilla",
    "gnupg",
    "gpgme",
    "libgpg-error",
    "libgcrypt",
    "nss",
    "nspr",
    "krb5",

    // ── Core compression libs ────────────────────────────────────────────────
    "zlib",
    "zstd",
    "xz",
    "lz4",

    // ── Network core ─────────────────────────────────────────────────────────
    "networkmanager",
    "wpa_supplicant",
    "iwd",
    "iproute2",
    "iptables",
    "iptables-nft",
    "nftables",
    "firewalld",
    "dnsmasq",
    "openssh",                  // restarting sshd mid-session can drop remote

    // ── Audio pipeline ───────────────────────────────────────────────────────
    // Replacing the running audio server mid-session is disruptive.
    // All pipewire sub-packages, libraries, and plugins are blocked together
    // because they share a single version and must be upgraded atomically.
    "pipewire",
    "pipewire-*",           // pipewire-alsa, pipewire-pulse, pipewire-libcamera, …
    "libpipewire",
    "lib32-pipewire",
    "lib32-libpipewire",
    "gst-plugin-pipewire",
    "alsa-card-profiles",   // ships with pipewire, tracks the same version
    "wireplumber",
    "alsa-lib",
    "lib32-alsa-lib",

    // ── Kernel userspace tools (track the kernel version exactly) ────────────
    // These are built from the same source tree as the running kernel.
    // Upgrading them without upgrading the kernel (or vice-versa) is safe
    // only within the same source release; a version bump should travel
    // with a full system upgrade.
    "bpf",
    "cpupower",
    "turbostat",
    "usbip",
    "x86_energy_perf_policy",
    "tmon",

    // ── Input ────────────────────────────────────────────────────────────────
    "libinput",
    "xf86-input-*",

    // ── Storage / block layer ────────────────────────────────────────────────
    "lvm2",
    "device-mapper",
    "e2fsprogs",
    "btrfs-progs",
    "ntfs-3g",
    "exfatprogs",
    "udisks2",

    // ── System locale / time ─────────────────────────────────────────────────
    "tzdata",
    "iana-etc",
    "icu",
];

// ─────────────────────────────────────────────────────────────────────────────
// GRAPHICS packages
// Everything in the graphics stack: GPU drivers (Mesa, NVIDIA, AMD, Intel),
// the OpenGL/Vulkan dispatch layer, and the display-server layer (Xorg,
// Wayland).  All of these require a display-server or session restart to take
// effect, and many packages within the stack must upgrade atomically.
//
// Layers covered:
//   Kernel GPU   — DRM/KMS is in-kernel; libpciaccess bridges userspace to it
//   Rendering    — Mesa (all sub-packages), vendor Vulkan ICDs
//   Display srvr — Xorg, Xwayland, Wayland protocol, DDX drivers
//   Dispatch     — libglvnd, vulkan-icd-loader (software, but ABI-coupled)
// ─────────────────────────────────────────────────────────────────────────────
pub const GRAPHICS_PATTERNS: &[&str] = &[
    // ── Mesa (rendering layer — must all upgrade atomically) ─────────────────
    "mesa",
    "lib32-mesa",
    "opencl-mesa",
    "lib32-opencl-mesa",
    "vulkan-mesa-*",            // vulkan-mesa-implicit-layers, vulkan-mesa-layers, …
    "lib32-vulkan-mesa-*",
    "libva-mesa-driver",
    "lib32-libva-mesa-driver",
    "mesa-vdpau",
    "lib32-mesa-vdpau",

    // ── NVIDIA (rendering layer) ─────────────────────────────────────────────
    "nvidia",
    "nvidia-dkms",
    "nvidia-utils",
    "lib32-nvidia-utils",
    "nvidia-settings",
    "nvidia-open",
    "nvidia-open-dkms",

    // ── AMD / Intel standalone drivers ──────────────────────────────────────
    "amdvlk",
    "lib32-amdvlk",
    "vulkan-radeon",
    "lib32-vulkan-radeon",
    "vulkan-intel",
    "lib32-vulkan-intel",
    "intel-media-driver",
    "libva-intel-driver",

    // ── GL / Vulkan dispatch (software, ABI-coupled to drivers) ─────────────
    "libglvnd",
    "lib32-libglvnd",
    "vulkan-icd-loader",
    "lib32-vulkan-icd-loader",

    // ── Display server layer ─────────────────────────────────────────────────
    "xorg-server",
    "xorg-server-*",
    "xorg-xwayland",
    "wayland",
    "wayland-protocols",
    "xf86-video-*",

    // ── Kernel GPU userspace bridge ──────────────────────────────────────────
    "libpciaccess",
    "lib32-libpciaccess",
];

/// Returns `true` if `name` belongs to the graphics stack.
pub fn is_graphics(name: &str) -> bool {
    let name = name.to_lowercase();
    GRAPHICS_PATTERNS
        .iter()
        .any(|&p| glob_match(&name, &p.to_lowercase()))
}

// ─────────────────────────────────────────────────────────────────────────────
// KDE ECOSYSTEM patterns
// Used for *version-bump detection* only (not unconditional blocking).
// When a package's installed version matches the current KDE minor line
// (e.g. 6.23) and the new version bumps to a different minor (e.g. 6.24),
// the update is deferred so the entire ecosystem moves together.
//
// Session-critical packages (kwin, plasma-*, …) are covered by
// KDE_CORE_PATTERNS above and are unconditionally blocked.
// System packages that happen to start with "k" (kmod, krb5, …) are caught
// by SYSTEM_CORE_PATTERNS before this check runs.
// ─────────────────────────────────────────────────────────────────────────────
pub const KDE_ECOSYSTEM_PATTERNS: &[&str] = &[
    // KDE Frameworks — all named k* and versioned on the KDE release cycle
    "k*",
    // Qt-binding libraries maintained by KDE
    "*-qt",
    // Well-known KDE-cadence packages without a k- prefix
    "attica",
    "baloo",
    "extra-cmake-modules",
    "frameworkintegration",
    "prison",
    "purpose",
    "qqc2-desktop-style",
    "qqc2-breeze-style",
    "solid",
    "sonnet",
    "syndication",
    "syntax-highlighting",
    // Plasma workspace sub-packages
    "plasma-*",
    "kdecoration",
    "breeze",
    "breeze-gtk",
    "breeze-icons",
];

/// Shared glob matcher used by both classify and ecosystem detection.
///
/// `"foo*"` → name starts with "foo"
/// `"*foo"` → name ends with "foo"
/// `"foo"`  → exact match
pub fn glob_match(name: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else {
        name == pattern
    }
}

/// Returns true if `name` belongs to the KDE software ecosystem
/// (checked against KDE_ECOSYSTEM_PATTERNS).
pub fn is_kde_ecosystem(name: &str) -> bool {
    let name = name.to_lowercase();
    KDE_ECOSYSTEM_PATTERNS
        .iter()
        .any(|&p| glob_match(&name, &p.to_lowercase()))
}

// ─────────────────────────────────────────────────────────────────────────────
// KDE CORE packages
// These are part of the active Plasma session. Updating them without
// logging out can leave the session in a broken/inconsistent state.
// ─────────────────────────────────────────────────────────────────────────────
pub const KDE_CORE_PATTERNS: &[&str] = &[
    // ── Plasma workspace ─────────────────────────────────────────────────────
    "plasma-workspace",
    "plasma-desktop",
    "plasma-shell",
    "plasma-framework",
    "plasma-pa",
    "plasma-nm",
    "plasma-systemmonitor",
    "plasma-firewall",
    "plasma-vault",
    "plasma-integration",
    "plasma-browser-integration",
    "plasma-wayland-protocols",
    "plasma-*",                 // catch any other plasma- sub-packages

    // ── Compositor / screen locker ───────────────────────────────────────────
    "kwin",
    "kscreenlocker",

    // ── KDE Frameworks (session-level libs) ──────────────────────────────────
    "kf6-*",
    "kf5-*",
    "kframeworkintegration",

    // ── Session / display management ─────────────────────────────────────────
    "kscreen",
    "powerdevil",
    "bluedevil",
    "sddm",
    "sddm-kcm",

    // ── Theming / decoration ─────────────────────────────────────────────────
    "kdecoration",
    "breeze",
    "breeze-gtk",
    "breeze-icons",

    // ── Core KDE system plumbing ─────────────────────────────────────────────
    "kde-cli-tools",
    "kde-gtk-config",
    "kglobalaccel",
    "khotkeys",
    "kwallet",
    "kwalletmanager",
    "polkit-kde-agent",
    "xdg-desktop-portal-kde",
    "ksshaskpass",

    // ── System settings ──────────────────────────────────────────────────────
    "systemsettings",
    "kcmutils",
    "kinfocenter",
];
