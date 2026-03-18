use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PackageUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
}

/// All pending updates, bundled with AUR metadata.
pub struct PendingUpdates {
    /// Every pending update (official sync DB + AUR merged).
    pub all: Vec<PackageUpdate>,
    /// Names of updates that came from the AUR, not the official sync DB.
    pub aur_names: HashSet<String>,
    /// The AUR helper that was found (paru / yay), if any.
    pub aur_helper: Option<&'static str>,
}

/// Detect the first available AUR helper by trying `--version`.
pub fn detect_aur_helper() -> Option<&'static str> {
    for &helper in &["paru", "yay"] {
        if Command::new(helper)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(helper);
        }
    }
    None
}

/// Run `checkupdates` (from pacman-contrib) and merge in AUR-only updates.
///
/// Even if checkupdates reports no official updates (exit 2), AUR updates
/// are still queried so that packages like pacselect surface correctly.
///
/// Exit codes used by checkupdates:
///   0  → updates available
///   1  → error
///   2  → no updates available
pub fn get_pending_updates() -> Result<PendingUpdates> {
    let aur_helper = detect_aur_helper();

    let output = Command::new("checkupdates")
        .output()
        .context(
            "Failed to run 'checkupdates'. \
             Make sure pacman-contrib is installed: sudo pacman -S pacman-contrib",
        )?;

    let mut all: Vec<PackageUpdate> = match output.status.code() {
        Some(2) => Vec::new(), // no official updates — still check AUR below
        Some(1) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("checkupdates reported an error:\n{}", stderr.trim());
        }
        Some(0) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().filter_map(parse_update_line).collect()
        }
        code => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "checkupdates exited with unexpected code {:?}:\n{}",
                code,
                stderr.trim()
            );
        }
    };

    // Merge AUR-only updates, deduplicating by name.
    let mut aur_names: HashSet<String> = HashSet::new();
    if let Some(helper) = aur_helper {
        let official_names: HashSet<String> = all.iter().map(|u| u.name.clone()).collect();
        for u in get_aur_updates(helper) {
            if !official_names.contains(&u.name) {
                aur_names.insert(u.name.clone());
                all.push(u);
            }
        }
    }

    Ok(PendingUpdates { all, aur_names, aur_helper })
}

/// Query the given AUR helper for AUR-only package upgrades.
///
/// Both `paru -Qua` and `yay -Qua` restrict output to AUR packages only
/// and produce the same line format as `checkupdates`:
///   package_name  old_version  ->  new_version
fn get_aur_updates(helper: &str) -> Vec<PackageUpdate> {
    let Ok(output) = Command::new(helper)
        .args(["-Qua"])
        .env("NO_COLOR", "1")
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_update_line)
        .collect()
}

/// Parse a single line of checkupdates / paru -Qua / yay -Qua output.
/// Expected format: `package_name old_version -> new_version`
fn parse_update_line(line: &str) -> Option<PackageUpdate> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 4 || parts[2] != "->" {
        return None;
    }
    Some(PackageUpdate {
        name: parts[0].to_string(),
        old_version: parts[1].to_string(),
        new_version: parts[3].to_string(),
    })
}
