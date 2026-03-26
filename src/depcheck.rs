use std::collections::{HashMap, HashSet};
use std::process::Command;

/// Pacman groups that indicate a package belongs to a system/graphics/session
/// category and should be deferred rather than installed mid-session.
/// This acts as a safety net on top of the name-pattern lists, catching
/// packages the patterns might miss.
const SYSTEM_GROUPS: &[&str] = &[
    // Display server
    "xorg",
    "xorg-drivers",
    // Core base
    "base",
    // Plasma session (display manager, compositor, etc.)
    "plasma",
    "plasma-wayland-session",
];

/// A safe package that depends on one or more skipped packages.
#[derive(Debug)]
pub struct DepWarning {
    pub package: String,
    pub depends_on_skipped: Vec<String>,
}

/// Refresh the pacman sync database.  Runs `sudo pacman -Sy` with no
/// packages to install — purely a database sync.
///
/// Must be called **once** from `main` before both the dependency check and
/// the install step.  Keeping it here (rather than inside `install_packages`)
/// ensures the DB is current for `pacman -Si` queries while avoiding the
/// partial-upgrade anti-pattern: we sync once, then install the exact set of
/// packages determined safe — we do NOT pass `-y` to the install command.
///
/// Errors are swallowed: if the sync fails (e.g. offline, no sudo) we still
/// proceed using whatever is already cached.  The dep check is advisory.
pub fn sync_db() {
    let _ = Command::new("sudo")
        .arg("pacman")
        .arg("-Sy")
        .arg("--noconfirm")
        .status();
}

/// An installed package that depends on a library being updated, but is
/// itself not in the current update set — meaning the update could break it.
#[derive(Debug)]
pub struct ReverseDepWarning {
    /// The library/package being updated.
    pub updated_pkg: String,
    /// Installed packages that depend on it but are NOT being updated.
    pub broken_by: Vec<String>,
}

/// The result of a single-pass `pacman -Si` query over all safe packages.
pub struct SiResult {
    /// Packages that depend on a skipped package (partial-upgrade risk).
    pub dep_warnings: Vec<DepWarning>,
    /// Packages whose pacman group implies they should be deferred.
    /// Maps package name → the offending group name.
    pub group_demotions: HashMap<String, String>,
    /// Short description for each package, from the Description field.
    pub descriptions: HashMap<String, String>,
    /// Libraries in the safe set whose installed reverse-dependents are NOT
    /// also being updated — risk of broken installed packages.
    pub reverse_dep_warnings: Vec<ReverseDepWarning>,
}

/// Run a single `pacman -Si` over all safe packages and return both dep
/// warnings and group-based demotions in one pass.
pub fn check_all(safe: &[&str], skipped_names: &HashSet<String>) -> SiResult {
    if safe.is_empty() {
        return SiResult {
            dep_warnings: Vec::new(),
            group_demotions: HashMap::new(),
            descriptions: HashMap::new(),
            reverse_dep_warnings: Vec::new(),
        };
    }

    let output = match Command::new("pacman")
        .arg("-Si")
        .args(safe)
        .output()
    {
        Ok(o) => o,
        Err(_) => return SiResult {
            dep_warnings: Vec::new(),
            group_demotions: HashMap::new(),
            descriptions: HashMap::new(),
            reverse_dep_warnings: Vec::new(),
        },
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let dep_warnings = if skipped_names.is_empty() {
        Vec::new()
    } else {
        parse_warnings(&stdout, skipped_names)
    };
    let group_demotions = parse_group_demotions(&stdout);
    let descriptions = parse_descriptions(&stdout);

    // Build the set of all update names for the reverse-dep check, plus the
    // new sonames provided by each package (from the -Si Provides field).
    let safe_set: HashSet<String> = safe.iter().map(|s| s.to_lowercase()).collect();
    let new_sonames = parse_new_sonames(&stdout);
    let reverse_dep_warnings = check_reverse_deps(safe, &safe_set, &new_sonames);

    SiResult { dep_warnings, group_demotions, descriptions, reverse_dep_warnings }
}

/// Parse `pacman -Si` output into a map of package → dep warnings.
///
/// pacman -Si output looks like:
///   Name            : firefox
///   Groups          : xorg
///   Depends On      : gtk3  libxt  mime-types  ...
///   ...
///   (blank line between packages)
fn parse_warnings(output: &str, skipped: &HashSet<String>) -> Vec<DepWarning> {
    let mut warnings = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_deps: Vec<String> = Vec::new();

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Name            : ") {
            // Flush previous package
            if let Some(name) = current_name.take() {
                if !current_deps.is_empty() {
                    warnings.push(DepWarning {
                        package: name,
                        depends_on_skipped: current_deps.drain(..).collect(),
                    });
                }
            }
            current_deps.clear();
            current_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Depends On      : ") {
            current_deps = parse_dep_names(rest, skipped);
        }
    }

    // Flush the last package
    if let Some(name) = current_name {
        if !current_deps.is_empty() {
            warnings.push(DepWarning {
                package: name,
                depends_on_skipped: current_deps,
            });
        }
    }

    warnings
}

/// Extract plain package names from a pacman dep string, keeping only those
/// that appear in `skipped`.
///
/// Dep strings look like: `gtk3>=3.24  libxt  mime-types  nss`
/// We strip version constraints (>=, <=, =, >, <) and the version itself.
fn parse_dep_names(deps_str: &str, skipped: &HashSet<String>) -> Vec<String> {
    deps_str
        .split_whitespace()
        .filter_map(|dep| {
            // Strip any version constraint suffix
            let name = dep
                .split(|c| c == '>' || c == '<' || c == '=')
                .next()
                .unwrap_or(dep)
                .to_lowercase();
            if skipped.contains(&name) {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

/// Scan `pacman -Si` output and collect the Description field for each package.
fn parse_descriptions(output: &str) -> HashMap<String, String> {
    let mut descriptions = HashMap::new();
    let mut current_name: Option<String> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Name            : ") {
            current_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Description     : ") {
            if let Some(ref name) = current_name {
                descriptions.insert(name.clone(), rest.trim().to_string());
            }
        }
    }

    descriptions
}

/// Scan `pacman -Si` output for packages whose Groups field contains one of
/// the entries in SYSTEM_GROUPS.  Returns a map of package name → group.
fn parse_group_demotions(output: &str) -> HashMap<String, String> {
    let mut demotions = HashMap::new();
    let mut current_name: Option<String> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Name            : ") {
            current_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Groups          : ") {
            if let Some(ref name) = current_name {
                for group in rest.split_whitespace() {
                    let g = group.to_lowercase();
                    if SYSTEM_GROUPS.iter().any(|&sg| sg == g) {
                        demotions.insert(name.clone(), group.to_string());
                        break;
                    }
                }
            }
        }
    }

    demotions
}

/// Extract `libfoo.so=X-ARCH` tokens from a `pacman -Si` Provides field,
/// keyed by package name.  Used to compare against the installed sonames.
fn parse_new_sonames(output: &str) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_name: Option<String> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Name            : ") {
            current_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Provides        : ") {
            if let Some(ref name) = current_name {
                let sonames: Vec<String> = rest
                    .split_whitespace()
                    .filter(|p| p.contains(".so="))
                    .map(|p| p.to_string())
                    .collect();
                if !sonames.is_empty() {
                    map.insert(name.clone(), sonames);
                }
            }
        }
    }

    map
}

/// For each package in `safe` that provides a shared library with a versioned
/// soname (`.so=X`), query `pacman -Qi` for installed reverse-dependents NOT
/// in `safe_set`, but only warn when the soname version actually changes
/// compared to what is currently installed.  Patch-level updates that keep the
/// same soname (e.g. gtk3 3.24.51→3.24.52 still provides libgtk-3.so=0-64)
/// are silently ignored.
fn check_reverse_deps(
    safe: &[&str],
    safe_set: &HashSet<String>,
    new_sonames: &HashMap<String, Vec<String>>,
) -> Vec<ReverseDepWarning> {
    if safe.is_empty() {
        return Vec::new();
    }

    // Only bother querying -Qi for packages that actually have a new soname
    // in the -Si output — skip pure application packages immediately.
    let lib_pkgs: Vec<&str> = safe
        .iter()
        .copied()
        .filter(|p| new_sonames.contains_key(*p))
        .collect();

    if lib_pkgs.is_empty() {
        return Vec::new();
    }

    let output = match Command::new("pacman")
        .arg("-Qi")
        .args(&lib_pkgs)
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_reverse_dep_warnings(&stdout, safe_set, new_sonames)
}

/// Parse `pacman -Qi` output to find library packages whose soname version
/// changed AND whose installed reverse-dependents are not all in `safe_set`.
///
/// A warning is only emitted when the installed `Provides` soname token
/// (e.g. `libvpx.so=12-64`) differs from the new version's soname token
/// (e.g. `libvpx.so=13-64`).  When sonames are identical the update is
/// ABI-compatible and no warning is needed.
fn parse_reverse_dep_warnings(
    output: &str,
    safe_set: &HashSet<String>,
    new_sonames: &HashMap<String, Vec<String>>,
) -> Vec<ReverseDepWarning> {
    let mut warnings = Vec::new();

    let mut current_name: Option<String> = None;
    let mut installed_sonames: Vec<String> = Vec::new();
    let mut required_by: Vec<String> = Vec::new();

    // Closure to evaluate and flush one package's data.
    let flush = |name: String,
                     installed: &mut Vec<String>,
                     req_by: &mut Vec<String>,
                     warnings: &mut Vec<ReverseDepWarning>| {
        let soname_bumped = new_sonames
            .get(&name)
            .map(|new| new != installed)
            .unwrap_or(false);

        if soname_bumped && !req_by.is_empty() {
            let broken_by: Vec<String> = req_by
                .drain(..)
                .filter(|r| !safe_set.contains(&r.to_lowercase()))
                .collect();
            if !broken_by.is_empty() {
                warnings.push(ReverseDepWarning { updated_pkg: name, broken_by });
            }
        } else {
            req_by.clear();
        }
        installed.clear();
    };

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Name            : ") {
            if let Some(name) = current_name.take() {
                flush(name, &mut installed_sonames, &mut required_by, &mut warnings);
            }
            installed_sonames.clear();
            required_by.clear();
            current_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Provides        : ") {
            installed_sonames = rest
                .split_whitespace()
                .filter(|p| p.contains(".so="))
                .map(|p| p.to_string())
                .collect();
        } else if let Some(rest) = line.strip_prefix("Required By     : ") {
            required_by = rest
                .split_whitespace()
                .filter(|&p| p != "None")
                .map(|p| p.to_string())
                .collect();
        }
    }

    // Flush the last package
    if let Some(name) = current_name {
        flush(name, &mut installed_sonames, &mut required_by, &mut warnings);
    }

    warnings
}

/// Build a quick-lookup map from warnings: package → skipped deps it needs.
pub fn warnings_map(warnings: Vec<DepWarning>) -> HashMap<String, Vec<String>> {
    warnings
        .into_iter()
        .map(|w| (w.package, w.depends_on_skipped))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_dep() {
        let skipped: HashSet<String> = ["qt6-base".to_string()].into();
        let result = parse_dep_names("gtk3>=3.24 qt6-base libxt", &skipped);
        assert_eq!(result, vec!["qt6-base"]);
    }

    #[test]
    fn no_match_returns_empty() {
        let skipped: HashSet<String> = ["linux".to_string()].into();
        let result = parse_dep_names("gtk3 libxt nss", &skipped);
        assert!(result.is_empty());
    }

    #[test]
    fn strips_version_constraint() {
        let skipped: HashSet<String> = ["nss".to_string()].into();
        let result = parse_dep_names("nss>=3.90", &skipped);
        assert_eq!(result, vec!["nss"]);
    }
}
