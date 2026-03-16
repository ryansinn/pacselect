use crate::filters::{glob_match, is_kde_ecosystem, KDE_CORE_PATTERNS, SYSTEM_CORE_PATTERNS};
use crate::environment::version_minor;

/// Why a package update is being skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    SystemCore,
    KdeCore,
    /// The package tracks the KDE release cycle and this update bumps the
    /// minor version line (e.g. 6.23 → 6.24). Defer until a full KDE upgrade.
    KdeVersionBump { from: String, to: String },
    UserFilter(String),
    /// Installing this package would cause a partial upgrade: it depends on
    /// one or more packages that are being skipped this run.
    PartialUpgrade { needs: Vec<String> },
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipReason::SystemCore => write!(f, "system/core"),
            SkipReason::KdeCore => write!(f, "KDE core (session-critical)"),
            SkipReason::KdeVersionBump { from, to } => {
                write!(f, "KDE version bump {} → {}", from, to)
            }
            SkipReason::UserFilter(pat) => write!(f, "user filter: {}", pat),
            SkipReason::PartialUpgrade { needs } => {
                write!(f, "partial upgrade risk — needs skipped: {}", needs.join(", "))
            }
        }
    }
}

/// Classify a package update.
///
/// Returns `Some(reason)` if the update should be skipped, `None` if safe.
///
/// Precedence:
///   1. User filters  (always honoured)
///   2. System/core   (unconditional)
///   3. KDE core      (session-critical, unconditional)
///   4. KDE version bump  (package tracks KDE cycle and minor line changes)
pub fn classify(
    package: &str,
    old_version: &str,
    new_version: &str,
    extra_skip: &[String],
    filter_system: bool,
    filter_kde: bool,
    kde_minor: Option<&str>,
) -> Option<SkipReason> {
    let pkg = package.to_lowercase();

    // 1. User-defined patterns
    for pattern in extra_skip {
        if glob_match(&pkg, &pattern.to_lowercase()) {
            return Some(SkipReason::UserFilter(pattern.clone()));
        }
    }

    // 2. System / core
    if filter_system {
        for &pattern in SYSTEM_CORE_PATTERNS {
            if glob_match(&pkg, &pattern.to_lowercase()) {
                return Some(SkipReason::SystemCore);
            }
        }
    }

    // 3. KDE core (session-critical — always skip regardless of version)
    if filter_kde {
        for &pattern in KDE_CORE_PATTERNS {
            if glob_match(&pkg, &pattern.to_lowercase()) {
                return Some(SkipReason::KdeCore);
            }
        }
    }

    // 4. KDE version-bump detection
    //
    // If the package belongs to the KDE ecosystem AND its installed version
    // sits on the same minor line as the running KDE (e.g. 6.23.x), but the
    // new version moves to a different minor line (e.g. 6.24.x), defer it.
    // This catches all KDE Frameworks (karchive, kauth, …) automatically
    // without needing to enumerate every package name.
    if filter_kde {
        if let Some(installed_minor) = kde_minor {
            if is_kde_ecosystem(&pkg) {
                let old_minor = version_minor(old_version);
                let new_minor = version_minor(new_version);
                if old_minor.as_deref() == Some(installed_minor)
                    && new_minor.as_deref() != old_minor.as_deref()
                {
                    return Some(SkipReason::KdeVersionBump {
                        from: old_minor.unwrap(),
                        to: new_minor.unwrap_or_else(|| new_version.to_string()),
                    });
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::glob_match;

    #[test]
    fn exact_match() {
        assert!(glob_match("linux", "linux"));
        assert!(!glob_match("linux-firmware", "linux"));
    }

    #[test]
    fn prefix_wildcard() {
        assert!(glob_match("linux-cachyos-bore", "linux-cachyos-*"));
        assert!(glob_match("plasma-workspace", "plasma-*"));
        assert!(!glob_match("blender", "plasma-*"));
    }

    #[test]
    fn suffix_wildcard() {
        assert!(glob_match("lib32-mesa", "*-mesa"));
        assert!(!glob_match("mesa", "*-mesa"));
    }

    #[test]
    fn classify_system_core() {
        let r = classify("linux", "6.1-1", "6.2-1", &[], true, false, None);
        assert_eq!(r, Some(SkipReason::SystemCore));
    }

    #[test]
    fn classify_kde_core() {
        let r = classify("kwin", "6.23.0-1", "6.23.1-1", &[], false, true, None);
        assert_eq!(r, Some(SkipReason::KdeCore));
    }

    #[test]
    fn classify_kde_version_bump() {
        let r = classify(
            "karchive",
            "6.23.0-1.1",
            "6.24.0-1.1",
            &[],
            false,
            true,
            Some("6.23"),
        );
        assert_eq!(
            r,
            Some(SkipReason::KdeVersionBump {
                from: "6.23".to_string(),
                to: "6.24".to_string(),
            })
        );
    }

    #[test]
    fn classify_kde_patch_release_is_safe() {
        // Same minor line (6.23.0 → 6.23.1): only a patch bump, not a version-line bump.
        // karchive is in KDE ecosystem but NOT in KDE_CORE_PATTERNS, so this
        // should be allowed through when no minor-line change occurs.
        let r = classify(
            "karchive",
            "6.23.0-1.1",
            "6.23.1-1.1",
            &[],
            false,
            true,
            Some("6.23"),
        );
        assert!(r.is_none());
    }

    #[test]
    fn classify_safe() {
        let r = classify("firefox", "120.0-1", "121.0-1", &[], true, true, Some("6.23"));
        assert!(r.is_none());
    }

    #[test]
    fn classify_user_filter() {
        let skip = vec!["my-app".to_string()];
        let r = classify("my-app", "1.0-1", "1.1-1", &skip, false, false, None);
        assert_eq!(r, Some(SkipReason::UserFilter("my-app".to_string())));
    }

    #[test]
    fn non_kde_k_package_not_caught_by_version_bump() {
        // kbd is a system keyboard utility — its version won't match KDE minor.
        // Even if is_kde_ecosystem matched it (via k*), old_minor "2.7" != "6.23"
        // so the version bump check shouldn't fire.
        let r = classify("kbd", "2.7.0-1", "2.8.0-1", &[], false, true, Some("6.23"));
        // kbd is not in KDE_CORE_PATTERNS so it should be safe
        assert!(r.is_none());
    }
}
