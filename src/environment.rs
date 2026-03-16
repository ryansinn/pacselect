use std::process::Command;

#[derive(Debug, Default)]
pub struct SystemEnv {
    /// Detected desktop environment name (e.g. "KDE Plasma", "GNOME")
    pub desktop: Option<String>,
    /// Full installed version of the KDE Frameworks probe package (e.g. "6.23.0-1.1")
    #[allow(dead_code)]
    pub kde_version: Option<String>,
    /// The "major.minor" slice of the installed KDE Frameworks version (e.g. "6.23").
    /// Used to detect whether a pending update crosses a KDE minor-release line.
    pub kde_frameworks_minor: Option<String>,
}

pub fn detect() -> SystemEnv {
    let desktop = detect_desktop();
    let (kde_version, kde_frameworks_minor) = detect_kde_frameworks_version();
    SystemEnv {
        desktop,
        kde_version,
        kde_frameworks_minor,
    }
}

fn detect_desktop() -> Option<String> {
    for var in &["XDG_CURRENT_DESKTOP", "DESKTOP_SESSION"] {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                return Some(normalize_desktop_name(&val));
            }
        }
    }
    None
}

fn normalize_desktop_name(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("kde") || lower.contains("plasma") {
        "KDE Plasma".to_string()
    } else if lower.contains("gnome") {
        "GNOME".to_string()
    } else if lower.contains("hypr") {
        "Hyprland".to_string()
    } else if lower.contains("sway") {
        "Sway".to_string()
    } else if lower.contains("xfce") {
        "XFCE".to_string()
    } else if lower.contains("lxqt") {
        "LXQt".to_string()
    } else {
        raw.to_string()
    }
}

/// Query installed packages to detect the KDE **Frameworks** version.
///
/// KDE Frameworks and KDE Plasma ship on *separate* release cycles with
/// different version numbers:
///   - KDE Frameworks 6  → 6.23, 6.24, …  (monthly, e.g. kcoreaddons)
///   - KDE Plasma 6      → 6.3, 6.4, …    (bimonthly, e.g. plasma-workspace)
///
/// We specifically want the Frameworks version because that's what all the
/// k* library packages (karchive, kauth, kio, …) are versioned against.
///
/// CachyOS packages frameworks without the kf6- prefix (just "kcoreaddons").
/// Standard Arch / upstream uses "kf6-kcoreaddons".
fn detect_kde_frameworks_version() -> (Option<String>, Option<String>) {
    const PROBES: &[&str] = &[
        // CachyOS / AUR naming (no kf6- prefix)
        "kcoreaddons",
        "karchive",
        "kconfig",
        // Standard Arch upstream naming
        "kf6-kcoreaddons",
        "kf6-karchive",
    ];
    for pkg in PROBES {
        if let Some(ver) = query_package_version(pkg) {
            let minor = version_minor(&ver);
            return (Some(ver), minor);
        }
    }
    (None, None)
}

fn query_package_version(package: &str) -> Option<String> {
    let output = Command::new("pacman")
        .arg("-Q")
        .arg(package)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // Output format: "plasma-workspace 6.23.0-1.1\n"
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().nth(1).map(str::to_string)
}

/// Extract the "major.minor" string from a pacman version, stripping any epoch.
///
/// "6.23.0-1.1"  → Some("6.23")
/// "1:6.23.0-1"  → Some("6.23")
/// "1.19.3b-1.1" → Some("1.19")
/// "2"           → None
pub fn version_minor(version: &str) -> Option<String> {
    // Strip epoch (e.g. "1:")
    let v = match version.find(':') {
        Some(idx) => &version[idx + 1..],
        None => version,
    };
    // Strip pkgrel (everything after the first '-')
    let pkgver = v.split('-').next()?;
    let mut parts = pkgver.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    Some(format!("{}.{}", major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_epoch() {
        assert_eq!(version_minor("1:6.23.0-1"), Some("6.23".to_string()));
    }

    #[test]
    fn plain_version() {
        assert_eq!(version_minor("6.23.0-1.1"), Some("6.23".to_string()));
    }

    #[test]
    fn non_kde_version() {
        assert_eq!(version_minor("1.19.3b-1.1"), Some("1.19".to_string()));
    }

    #[test]
    fn too_short() {
        assert_eq!(version_minor("2"), None);
    }
}
