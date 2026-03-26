//! Load YAML spec from disk (futures contracts only; options loaded separately).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::calendars::register_schedules;
use crate::models::{Asset, ContractCycle, ContractSpec, Exchange, ExpirationRule, SpecRepository};
use crate::schedule::load_schedules;

fn read_yaml_mapping(path: &Path) -> Result<serde_yaml::Mapping, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let v: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| format!("YAML {}: {e}", path.display()))?;
    match v {
        serde_yaml::Value::Mapping(m) => Ok(m),
        _ => Err(format!("Expected YAML mapping in {}", path.display())),
    }
}

fn load_exchanges(spec_root: &Path) -> Result<HashMap<String, Exchange>, String> {
    let mut exchanges = HashMap::new();
    let dir = spec_root.join("exchanges");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("read exchanges dir: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "yaml").unwrap_or(false))
        .collect();
    paths.sort();

    for yaml_path in paths {
        let m = read_yaml_mapping(&yaml_path)?;
        let code = m
            .get(serde_yaml::Value::String("exchange".into()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing 'exchange' in {}", yaml_path.display()))?
            .to_uppercase();

        let mut assets: HashMap<String, Asset> = HashMap::new();
        if let Some(serde_yaml::Value::Mapping(a)) =
            m.get(serde_yaml::Value::String("assets".into()))
        {
            for (k, v) in a {
                let sym = k
                    .as_str()
                    .ok_or_else(|| format!("Invalid asset key in {}", yaml_path.display()))?
                    .to_uppercase();
                let mut map = match v {
                    serde_yaml::Value::Mapping(m) => m.clone(),
                    _ => {
                        return Err(format!(
                            "asset {sym} must be a mapping in {}",
                            yaml_path.display()
                        ))
                    }
                };
                map.insert(
                    serde_yaml::Value::String("symbol".into()),
                    serde_yaml::Value::String(sym.clone()),
                );
                let payload: Asset = serde_yaml::from_value(serde_yaml::Value::Mapping(map))
                    .map_err(|e| format!("asset {sym} in {}: {e}", yaml_path.display()))?;
                assets.insert(sym, payload);
            }
        }

        let ex: Exchange = Exchange {
            code: code.clone(),
            mic: m
                .get(serde_yaml::Value::String("mic".into()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            full_name: m
                .get(serde_yaml::Value::String("full_name".into()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            country: m
                .get(serde_yaml::Value::String("country".into()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            timezone: m
                .get(serde_yaml::Value::String("timezone".into()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            assets,
        };
        exchanges.insert(code, ex);
    }
    Ok(exchanges)
}

#[allow(clippy::type_complexity)]
fn load_cycles_and_rules(
    spec_root: &Path,
) -> Result<
    (
        HashMap<String, ContractCycle>,
        HashMap<String, ExpirationRule>,
    ),
    String,
> {
    let path = spec_root.join("schemas").join("contract_cycles.yaml");
    let m = read_yaml_mapping(&path)?;

    let mut cycles = HashMap::new();
    if let Some(serde_yaml::Value::Mapping(cc)) =
        m.get(serde_yaml::Value::String("contract_cycles".into()))
    {
        for (name, payload) in cc {
            let name = name
                .as_str()
                .ok_or_else(|| format!("contract_cycles key in {}", path.display()))?
                .to_string();
            let mut c: ContractCycle = serde_yaml::from_value(payload.clone())
                .map_err(|e| format!("contract cycle {name}: {e}"))?;
            c.name = name.clone();
            cycles.insert(name, c);
        }
    }

    let mut rules = HashMap::new();
    if let Some(serde_yaml::Value::Mapping(er)) =
        m.get(serde_yaml::Value::String("expiration_rules".into()))
    {
        for (name, payload) in er {
            let name = name
                .as_str()
                .ok_or_else(|| format!("expiration_rules key in {}", path.display()))?
                .to_string();
            let mut r: ExpirationRule = serde_yaml::from_value(payload.clone())
                .map_err(|e| format!("expiration rule {name}: {e}"))?;
            r.name = name.clone();
            rules.insert(name, r);
        }
    }

    Ok((cycles, rules))
}

fn load_contracts(spec_root: &Path) -> Result<Vec<ContractSpec>, String> {
    let mut contracts = Vec::new();
    let contracts_dir = spec_root.join("contracts");
    let mut stack = vec![contracts_dir];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "yaml").unwrap_or(false) {
                let m = read_yaml_mapping(&p)?;
                let Some(serde_yaml::Value::Sequence(items)) =
                    m.get(serde_yaml::Value::String("contracts".into()))
                else {
                    continue;
                };
                for item in items {
                    let c: ContractSpec = serde_yaml::from_value(item.clone())
                        .map_err(|e| format!("contract item in {}: {e}", p.display()))?;
                    contracts.push(c);
                }
            }
        }
    }
    Ok(contracts)
}

fn default_spec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("spec")
}

/// Load futures spec. If `path` is `None`, uses `CARGO_MANIFEST_DIR/spec`.
pub fn load_spec(path: Option<&Path>) -> Result<SpecRepository, String> {
    let spec_root = path
        .map(Path::to_path_buf)
        .unwrap_or_else(default_spec_path)
        .canonicalize()
        .map_err(|e| format!("spec path: {e}"))?;

    if !spec_root.is_dir() {
        return Err(format!("Spec path does not exist: {}", spec_root.display()));
    }

    let exchanges = load_exchanges(&spec_root)?;
    let (contract_cycles, expiration_rules) = load_cycles_and_rules(&spec_root)?;

    let mut contracts: HashMap<String, ContractSpec> = HashMap::new();
    for c in load_contracts(&spec_root)? {
        if !contract_cycles.contains_key(&c.contract_cycle) {
            return Err(format!(
                "Contract {} references unknown cycle '{}'",
                c.symbol, c.contract_cycle
            ));
        }
        if !expiration_rules.contains_key(&c.expiration_rule) {
            return Err(format!(
                "Contract {} references unknown rule '{}'",
                c.symbol, c.expiration_rule
            ));
        }
        contracts.insert(c.symbol.to_uppercase(), c);
    }

    let schedules = load_schedules(&spec_root)?;
    register_schedules(schedules.clone());

    Ok(SpecRepository {
        exchanges,
        contracts,
        contract_cycles,
        expiration_rules,
        schedules,
    })
}
