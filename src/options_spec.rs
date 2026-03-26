//! Load `options.yaml` files under `spec/contracts/**`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::options_models::OptionRule;

fn default_options_path() -> PathBuf {
    tickerforge_spec_data::default_spec_root()
        .join("contracts")
        .join("b3")
        .join("options.yaml")
}

/// Load B3 options rules from `spec/contracts/b3/options.yaml` (or given path).
pub fn load_option_rules(path: Option<&Path>) -> Result<Vec<OptionRule>, String> {
    let p = path
        .map(Path::to_path_buf)
        .unwrap_or_else(default_options_path);
    let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let root: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| format!("YAML {}: {e}", p.display()))?;
    let opts = root
        .get("options")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| format!("expected 'options' list in {}", p.display()))?;
    let mut out = Vec::new();
    for item in opts {
        let rule: OptionRule =
            serde_yaml::from_value(item.clone()).map_err(|e| format!("option rule: {e}"))?;
        out.push(rule);
    }
    Ok(out)
}
