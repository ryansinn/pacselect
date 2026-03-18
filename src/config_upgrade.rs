/// Generic TOML config upgrader.
///
/// Merges a canonical (default) TOML document into an existing user config
/// file, preserving all values and comments the user has already set, while
/// inserting any keys that are missing with their documented defaults.
///
/// # How it works
///
/// Both the canonical template and the user file are parsed with `toml_edit`,
/// which preserves comments, whitespace, and key ordering.  The upgrader then
/// walks every key in the canonical document recursively.  For each key that
/// is absent from the user document it is inserted verbatim (including any
/// inline comment that appears in the canonical template).  Keys the user has
/// already set — even if they differ from the default — are left untouched.
///
/// # Portability
///
/// This module has no dependencies on the rest of the pacselect codebase.
/// Copy `config_upgrade.rs` and its two Cargo dependencies (`toml_edit`,
/// `anyhow`) into any project to reuse the same upgrade logic.
///
/// # Usage
///
/// ```rust
/// let changed = config_upgrade::upgrade(&config_path, MY_CANONICAL_CONFIG)?;
/// if changed {
///     println!("Config updated.");
/// }
/// ```
use anyhow::{Context, Result};
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table};

/// Upgrade the config file at `path` by merging in any keys present in
/// `canonical` that are absent from the user's file.
///
/// Returns `true` if the file was modified, `false` if it was already current.
/// If `path` does not exist the canonical config is written there as-is.
pub fn upgrade(path: &Path, canonical: &str) -> Result<bool> {
    let canonical_doc: DocumentMut = canonical
        .parse()
        .context("Failed to parse canonical config template")?;

    // If no user config exists yet, just write the canonical one.
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }
        std::fs::write(path, canonical)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        return Ok(true);
    }

    let existing_text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;

    let mut user_doc: DocumentMut = existing_text
        .parse()
        .with_context(|| format!("Failed to parse config: {}", path.display()))?;

    let changed = merge_tables(canonical_doc.as_table(), user_doc.as_table_mut());

    if changed {
        std::fs::write(path, user_doc.to_string())
            .with_context(|| format!("Failed to write updated config: {}", path.display()))?;
    }

    Ok(changed)
}

/// Recursively merge keys from `canonical` into `user`.
/// Returns true if any key was inserted.
fn merge_tables(canonical: &Table, user: &mut Table) -> bool {
    let mut changed = false;

    for (key, canonical_item) in canonical.iter() {
        if user.contains_key(key) {
            // Key exists — recurse if both sides are tables, otherwise leave it.
            if let (Some(c_table), Some(u_item)) =
                (canonical_item.as_table(), user.get_mut(key))
            {
                if let Some(u_table) = u_item.as_table_mut() {
                    if merge_tables(c_table, u_table) {
                        changed = true;
                    }
                }
            }
        } else {
            // Key is missing — insert verbatim from the canonical document,
            // which preserves any inline comment or formatting.
            user.insert(key, canonical_item.clone());
            changed = true;
        }
    }

    changed
}

/// Return a human-readable diff of which keys were added.
/// Useful for printing a summary after upgrade.
pub fn added_keys(canonical: &str, user_before: &str) -> Vec<String> {
    let canonical_doc: DocumentMut = match canonical.parse() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let user_doc: DocumentMut = match user_before.parse() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut added = Vec::new();
    collect_missing(canonical_doc.as_table(), user_doc.as_table(), "", &mut added);
    added
}

fn collect_missing(canonical: &Table, user: &Table, prefix: &str, out: &mut Vec<String>) {
    for (key, item) in canonical.iter() {
        let full_key = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", prefix, key)
        };

        if user.contains_key(key) {
            if let (Some(c_table), Some(u_table)) =
                (item.as_table(), user.get(key).and_then(Item::as_table))
            {
                collect_missing(c_table, u_table, &full_key, out);
            }
        } else {
            out.push(full_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pacselect_test_{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn adds_missing_key() {
        let canonical = "[display]\ndescriptions = true\n";
        let existing = "";
        let path = write_temp(existing);
        let changed = upgrade(&path, canonical).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("descriptions"));
    }

    #[test]
    fn preserves_existing_value() {
        let canonical = "[display]\ndescriptions = true\n";
        let existing = "[display]\ndescriptions = false\n";
        let path = write_temp(existing);
        let changed = upgrade(&path, canonical).unwrap();
        assert!(!changed);
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("descriptions = false"));
    }

    #[test]
    fn adds_only_missing_keys_in_existing_section() {
        let canonical = "[display]\ndescriptions = true\nverbose = false\n";
        let existing = "[display]\ndescriptions = false\n";
        let path = write_temp(existing);
        let changed = upgrade(&path, canonical).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("descriptions = false")); // preserved
        assert!(result.contains("verbose = false"));      // added
    }

    #[test]
    fn added_keys_reports_correctly() {
        let canonical = "[display]\ndescriptions = true\nverbose = false\n";
        let existing = "[display]\ndescriptions = false\n";
        let keys = added_keys(canonical, existing);
        assert_eq!(keys, vec!["display.verbose"]);
    }
}
