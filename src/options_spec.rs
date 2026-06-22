//! Load `options:` blocks from all `spec/contracts/**/*.yaml` files.

use std::fs;
use std::path::Path;

use crate::options_models::OptionRule;

/// Load option rules from all `options:` blocks found recursively under
/// `spec_root/contracts/**/*.yaml`.
///
/// Files that have no `options:` key are silently skipped.  Adding a new
/// market is as simple as dropping a YAML file with an `options:` block into
/// `contracts/<market>/` — no code changes required.
pub fn load_all_option_rules(spec_root: &Path) -> Result<Vec<OptionRule>, String> {
    let contracts_dir = spec_root.join("contracts");
    let mut rules: Vec<OptionRule> = Vec::new();
    let mut stack = vec![contracts_dir];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "yaml").unwrap_or(false) {
                let raw =
                    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
                let root: serde_yaml::Value =
                    serde_yaml::from_str(&raw).map_err(|e| format!("YAML {}: {e}", p.display()))?;
                let Some(opts) = root.get("options").and_then(|v| v.as_sequence()) else {
                    continue;
                };
                for item in opts {
                    let rule: OptionRule = serde_yaml::from_value(item.clone())
                        .map_err(|e| format!("option rule in {}: {e}", p.display()))?;
                    rules.push(rule);
                }
            }
        }
    }
    Ok(rules)
}
