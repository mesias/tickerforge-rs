//! Load YAML spec from disk (futures contracts + options from all markets).

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::calendars::register_schedules;
use crate::models::{
    Asset, ContractCycle, ContractSpec, EquitySpec, Exchange, ExpirationRule, SpecRepository,
};
use crate::options_spec::load_all_option_rules;
use crate::schedule::load_schedules;

const LOAD_SPEC_CACHE_MAX: usize = 8;

struct LoadSpecCache {
    map: HashMap<PathBuf, Arc<SpecRepository>>,
    order: VecDeque<PathBuf>,
}

impl LoadSpecCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &Path) -> Option<Arc<SpecRepository>> {
        let arc = self.map.get(key)?.clone();
        if let Some(pos) = self.order.iter().position(|p| p == key) {
            let path = self.order.remove(pos).expect("index valid");
            self.order.push_back(path);
        }
        Some(arc)
    }

    fn insert(&mut self, key: PathBuf, value: Arc<SpecRepository>) {
        if self.map.contains_key(&key) {
            if let Some(pos) = self.order.iter().position(|p| p == &key) {
                self.order.remove(pos);
            }
        } else {
            while self.map.len() >= LOAD_SPEC_CACHE_MAX {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                } else {
                    break;
                }
            }
        }
        self.order.push_back(key.clone());
        self.map.insert(key, value);
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

fn load_spec_cache() -> &'static Mutex<LoadSpecCache> {
    static CACHE: OnceLock<Mutex<LoadSpecCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(LoadSpecCache::new()))
}

/// Drop all cached [`load_spec`] / [`load_spec_from_path`] results.
pub fn clear_load_spec_cache() {
    if let Ok(mut cache) = load_spec_cache().lock() {
        cache.clear();
    }
}

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

fn load_equities(spec_root: &Path) -> Result<Vec<EquitySpec>, String> {
    let mut equities = Vec::new();
    let equities_dir = spec_root.join("equities");
    if !equities_dir.exists() {
        return Ok(equities);
    }
    let mut stack = vec![equities_dir];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "yaml").unwrap_or(false) {
                let m = read_yaml_mapping(&p)?;
                let Some(serde_yaml::Value::Sequence(items)) =
                    m.get(serde_yaml::Value::String("equities".into()))
                else {
                    continue;
                };
                for item in items {
                    let e: EquitySpec = serde_yaml::from_value(item.clone())
                        .map_err(|err| format!("equity item in {}: {err}", p.display()))?;
                    equities.push(e);
                }
            }
        }
    }
    Ok(equities)
}

fn default_spec_path() -> PathBuf {
    tickerforge_spec_data::default_spec_root()
}

/// Load the default futures spec tree from the `tickerforge-spec-data` crate
/// (canonical YAML under <https://github.com/mesias/tickerforge-spec>).
///
/// Results are cached by resolved absolute path (up to 8 entries). Call
/// [`clear_load_spec_cache`] if the YAML on disk changed and must be reloaded.
pub fn load_spec() -> Result<SpecRepository, String> {
    load_spec_at(default_spec_path())
}

/// Load futures spec from a directory (must contain `exchanges/`, `contracts/`, `schemas/`, etc.).
///
/// Results are cached by resolved absolute path (up to 8 entries).
pub fn load_spec_from_path(path: &Path) -> Result<SpecRepository, String> {
    load_spec_at(path.to_path_buf())
}

fn load_spec_at(spec_root: PathBuf) -> Result<SpecRepository, String> {
    if !spec_root.exists() {
        return Err(format!("Spec path does not exist: {}", spec_root.display()));
    }
    let spec_root = spec_root
        .canonicalize()
        .map_err(|e| format!("spec path: {e}"))?;

    if !spec_root.is_dir() {
        return Err(format!("Spec path does not exist: {}", spec_root.display()));
    }

    {
        let mut cache = load_spec_cache()
            .lock()
            .map_err(|_| "load_spec cache poisoned".to_string())?;
        if let Some(cached) = cache.get(&spec_root) {
            return Ok((*cached).clone());
        }
    }

    let loaded = Arc::new(load_spec_uncached(&spec_root)?);

    {
        let mut cache = load_spec_cache()
            .lock()
            .map_err(|_| "load_spec cache poisoned".to_string())?;
        cache.insert(spec_root, Arc::clone(&loaded));
    }

    Ok((*loaded).clone())
}

fn load_spec_uncached(spec_root: &Path) -> Result<SpecRepository, String> {
    let exchanges = load_exchanges(spec_root)?;
    let (contract_cycles, expiration_rules) = load_cycles_and_rules(spec_root)?;

    let mut contracts: HashMap<String, ContractSpec> = HashMap::new();
    for c in load_contracts(spec_root)? {
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

    for c in contracts.values_mut() {
        let ex_key = c.exchange.to_uppercase();
        let sym_key = c.symbol.to_uppercase();
        if let Some(ex) = exchanges.get(&ex_key) {
            c.exchange_timezone = ex.timezone.clone();
            if let Some(asset) = ex.assets.get(&sym_key) {
                c.sessions = asset.sessions.clone();
            }
        }
    }

    let options = load_all_option_rules(spec_root)?;

    let mut equities: HashMap<String, EquitySpec> = HashMap::new();
    for mut eq in load_equities(spec_root)? {
        let ex_key = eq.exchange.to_uppercase();
        if let Some(ex) = exchanges.get(&ex_key) {
            eq.exchange_timezone = ex.timezone.clone();
        }
        equities.insert(eq.symbol.to_uppercase(), eq);
    }

    let schedules = load_schedules(spec_root)?;
    register_schedules(schedules.clone());

    Ok(SpecRepository {
        exchanges,
        contracts,
        options,
        equities,
        contract_cycles,
        expiration_rules,
        schedules,
        pattern_index: std::sync::OnceLock::new(),
    })
}
