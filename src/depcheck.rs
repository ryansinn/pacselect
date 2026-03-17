use std::collections::{HashMap, HashSet};
use std::process::Command;

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

/// For each safe package, check whether any of its runtime dependencies are
/// in the skipped set.  If so, installing it alone risks a partial upgrade.
///
/// Refreshes the sync db first so the metadata is current, then uses
/// `pacman -Si <pkgs…>` to query dependency info in a single call.
/// For each safe package, check whether any of its runtime dependencies are
/// in the skipped set.  If so, installing it alone risks a partial upgrade.
///
/// The caller is responsible for calling [`sync_db`] before this function so
/// that `pacman -Si` metadata is current.
pub fn check(safe: &[&str], skipped_names: &HashSet<String>) -> Vec<DepWarning> {
    if safe.is_empty() || skipped_names.is_empty() {
        return Vec::new();
    }

    // Fetch the sync-db info for all safe packages at once.
    let output = match Command::new("pacman")
        .arg("-Si")
        .args(safe)
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_warnings(&stdout, skipped_names)
}

/// Parse `pacman -Si` output into a map of package → dep warnings.
///
/// pacman -Si output looks like:
///   Name            : firefox
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
