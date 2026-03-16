use anyhow::{bail, Context, Result};
use std::process::Command;

/// Install (upgrade) the given packages using pacman.
///
/// Runs `sudo pacman -Sy --needed --noconfirm <packages>`.
/// The `-y` refreshes the sync database so the package versions seen by
/// `checkupdates` are actually reachable. The caller is expected to have
/// already obtained user confirmation before calling this function.
pub fn install_packages(packages: &[&str]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-Sy")
        .arg("--needed")
        .arg("--noconfirm")
        .args(packages)
        .status()
        .context(
            "Failed to launch 'sudo pacman'. \
             Make sure sudo is configured correctly.",
        )?;

    if !status.success() {
        bail!(
            "pacman exited with non-zero status ({}). \
             Check the output above for details.",
            status
        );
    }

    Ok(())
}
