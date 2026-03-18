use anyhow::{bail, Context, Result};
use std::process::Command;

/// Install (upgrade) the given packages using pacman.
///
/// Runs `sudo pacman -S --needed --noconfirm <packages>`.
/// The sync database **must already be current** before this is called —
/// `depcheck::sync_db()` is responsible for that.  We deliberately omit `-y`
/// here to avoid doing a second DB sync, which would recreate the partial-
/// upgrade anti-pattern warned about at:
/// https://wiki.archlinux.org/title/System_maintenance#Partial_upgrades_are_unsupported
pub fn install_packages(packages: &[&str]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-S")
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

/// Perform a complete system upgrade (`sudo pacman -Syu`).
///
/// This is the Arch-recommended upgrade path and avoids partial upgrades
/// entirely.  Use this when you want to include system/core and KDE packages
/// in the upgrade.
pub fn full_upgrade() -> Result<()> {
    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-Syu")
        .status()
        .context(
            "Failed to launch 'sudo pacman'. \
             Make sure sudo is configured correctly.",
        )?;

    if !status.success() {
        bail!(
            "pacman -Syu exited with non-zero status ({}). \
             Check the output above for details.",
            status
        );
    }

    Ok(())
}

/// Update pacselect itself via the given AUR helper.
pub fn self_update_via_helper(helper: &str) -> Result<()> {
    let status = Command::new(helper)
        .args(["-S", "--noconfirm", "pacselect"])
        .status()
        .with_context(|| format!("Failed to launch '{}'", helper))?;

    if !status.success() {
        bail!(
            "{} exited with non-zero status ({}). \
             Check the output above for details.",
            helper,
            status
        );
    }

    Ok(())
}
