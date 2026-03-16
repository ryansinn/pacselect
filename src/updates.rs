use anyhow::{bail, Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PackageUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
}

/// Run `checkupdates` (from pacman-contrib) and return all pending updates.
///
/// Exit codes used by checkupdates:
///   0  → updates available (stdout contains the list)
///   1  → error
///   2  → no updates available
pub fn get_pending_updates() -> Result<Vec<PackageUpdate>> {
    let output = Command::new("checkupdates")
        .output()
        .context(
            "Failed to run 'checkupdates'. \
             Make sure pacman-contrib is installed: sudo pacman -S pacman-contrib",
        )?;

    match output.status.code() {
        Some(2) => return Ok(Vec::new()), // no updates — clean exit
        Some(1) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("checkupdates reported an error:\n{}", stderr.trim());
        }
        Some(0) => {} // updates available — fall through
        code => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "checkupdates exited with unexpected code {:?}:\n{}",
                code,
                stderr.trim()
            );
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let updates = stdout
        .lines()
        .filter_map(parse_update_line)
        .collect();

    Ok(updates)
}

/// Parse a single line of checkupdates output.
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
