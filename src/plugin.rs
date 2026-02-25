//! Ansible plugin management — embeds piloteer.py and installs it into
//! the user's Ansible plugin directory.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// The embedded strategy plugin source.
const STRATEGY_PLUGIN: &str = include_str!("../ansible_plugin/strategies/piloteer.py");

/// Default install target: ~/.ansible/plugins/strategy/
fn default_strategy_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".ansible").join("plugins").join("strategy"))
}

/// Returns the path where the plugin was (or would be) installed.
pub fn plugin_path() -> Result<PathBuf> {
    Ok(default_strategy_dir()?.join("piloteer.py"))
}

/// Install the embedded piloteer.py strategy plugin.
/// Returns the path it was written to.
///
/// If `force` is false and the file already exists with identical content,
/// this is a no-op.
pub fn install_plugin(force: bool) -> Result<PathBuf> {
    let dir = default_strategy_dir()?;
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create directory: {}", dir.display()))?;

    let dest = dir.join("piloteer.py");

    if dest.exists() && !force {
        let existing = fs::read_to_string(&dest)
            .with_context(|| format!("Failed to read existing plugin at {}", dest.display()))?;
        if existing == STRATEGY_PLUGIN {
            return Ok(dest);
        }
    }

    fs::write(&dest, STRATEGY_PLUGIN)
        .with_context(|| format!("Failed to write plugin to {}", dest.display()))?;

    Ok(dest)
}

/// Ensure the plugin is installed (silent auto-install on startup).
/// Prints a message only if the plugin was freshly installed or updated.
pub fn ensure_plugin() {
    match install_plugin(false) {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "Warning: Could not auto-install Ansible strategy plugin: {}",
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_plugin_not_empty() {
        assert!(!STRATEGY_PLUGIN.is_empty());
        assert!(STRATEGY_PLUGIN.contains("StrategyModule"));
    }

    #[test]
    fn test_default_strategy_dir() {
        let dir = default_strategy_dir().unwrap();
        assert!(dir.ends_with(".ansible/plugins/strategy"));
    }
}
