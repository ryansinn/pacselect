use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub filter_sets: FilterSets,

    #[serde(default)]
    pub filters: FilterConfig,
}

/// Toggle the built-in filter categories on/off.
#[derive(Debug, Serialize, Deserialize)]
pub struct FilterSets {
    /// Block system/core packages (kernel, systemd, glibc, mesa, drivers …).
    /// Default: true
    #[serde(default = "default_true")]
    pub system_core: bool,

    /// Block KDE core session packages (plasma-*, kwin, kf6-* …).
    /// Default: true
    #[serde(default = "default_true")]
    pub kde_core: bool,
}

impl Default for FilterSets {
    fn default() -> Self {
        Self {
            system_core: true,
            kde_core: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FilterConfig {
    /// Extra package patterns to always skip, in addition to the built-in
    /// lists. Supports the same glob syntax: "pkg*" or "*-suffix".
    #[serde(default)]
    pub extra_skip: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load config from the given path. Returns a default config if the file
    /// does not exist; returns an error if the file is present but malformed.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let cfg: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(cfg)
    }
}

/// Return the text of a sample config file (used by --gen-config).
pub fn sample_config() -> &'static str {
    r#"# pacselect configuration
# Default location: ~/.config/pacselect/config.toml

[filter_sets]
# Block system/core packages (kernel, systemd, glibc, mesa, GPU drivers, …).
# Disabling this is dangerous — only do so for intentional full upgrades.
system_core = true

# Block KDE core session packages (plasma-*, kwin, kf6-*, breeze, sddm, …).
# Disable if you are happy restarting your Plasma session after updates.
kde_core = true

[filters]
# Additional package patterns to ALWAYS skip, on top of the built-in lists.
# Supports prefix globs ("myapp*") and suffix globs ("*-git").
extra_skip = [
    # "spotify",
    # "proprietary-*",
]
"#
}
