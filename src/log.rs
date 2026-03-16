use anyhow::Result;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;

fn log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("pacselect")
        .join("history.log")
}

/// Append one run's result to the history log.
///
/// Format (plain text, one block per run):
/// ```
/// [2026-03-16 14:32:05]
/// UPDATED (3): firefox ghostty zen-browser-bin
/// SKIPPED (17): linux plasma-workspace kwin ...
/// ```
pub fn write_run(
    updated: &[&str],
    skipped: &[&str],
    aborted: bool,
    dry_run: bool,
) -> Result<()> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");

    let mode = if dry_run {
        " [dry-run]"
    } else if aborted {
        " [aborted]"
    } else {
        ""
    };

    writeln!(file, "[{}]{}", ts, mode)?;

    if updated.is_empty() {
        writeln!(file, "UPDATED (0):")?;
    } else {
        writeln!(file, "UPDATED ({}): {}", updated.len(), updated.join(" "))?;
    }

    if skipped.is_empty() {
        writeln!(file, "SKIPPED (0):")?;
    } else {
        writeln!(file, "SKIPPED ({}): {}", skipped.len(), skipped.join(" "))?;
    }

    writeln!(file)?;
    Ok(())
}
