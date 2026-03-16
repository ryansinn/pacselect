use std::collections::HashSet;
use std::process::Command;

/// Return the set of foreign (AUR / manually installed) package names.
///
/// `pacman -Qm` lists every package not present in any sync database.
/// These are typically AUR packages, VCS (-git) builds, or packages
/// installed manually with `pacman -U`.
pub fn foreign_packages() -> HashSet<String> {
    let output = match Command::new("pacman").arg("-Qm").output() {
        Ok(o) => o,
        Err(_) => return HashSet::new(),
    };
    if !output.status.success() {
        return HashSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_lowercase)
        .collect()
}
