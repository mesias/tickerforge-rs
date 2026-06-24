//! Pydantic-aligned models for futures contracts and exchanges.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

use crate::options_models::OptionRule;
use crate::schedule::ExchangeSchedule;

/// One clock-time trading window; YAML uses the map key as `name`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSegment {
    pub name: String,
    pub start: String,
    pub end: String,
}

fn mapping_to_segments(m: serde_yaml::Mapping) -> Result<Vec<SessionSegment>, String> {
    let mut segments = Vec::new();
    for (k, val) in m {
        let name = k
            .as_str()
            .ok_or_else(|| "session key must be a string".to_string())?
            .to_string();
        let inner = match val {
            serde_yaml::Value::Mapping(m) => m,
            _ => return Err(format!("session '{name}' must be a mapping with start/end")),
        };
        let start = inner
            .get(serde_yaml::Value::String("start".into()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("session '{name}' missing start"))?
            .to_string();
        let end = inner
            .get(serde_yaml::Value::String("end".into()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("session '{name}' missing end"))?
            .to_string();
        segments.push(SessionSegment { name, start, end });
    }
    Ok(segments)
}

fn validate_sessions(segments: &[SessionSegment]) -> Result<(), String> {
    if segments.is_empty() {
        return Ok(());
    }
    if !segments[0].name.eq_ignore_ascii_case("regular") {
        return Err("first session segment must be 'regular' (case-insensitive)".to_string());
    }
    Ok(())
}

fn deserialize_asset_sessions<'de, D>(deserializer: D) -> Result<Vec<SessionSegment>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_yaml::Value::deserialize(deserializer)?;
    match v {
        serde_yaml::Value::Mapping(m) => {
            if m.is_empty() {
                return Err(Error::custom("sessions must not be empty"));
            }
            let segments = mapping_to_segments(m).map_err(Error::custom)?;
            validate_sessions(&segments).map_err(Error::custom)?;
            Ok(segments)
        }
        _ => Err(Error::custom("sessions must be a YAML mapping")),
    }
}

fn deserialize_contract_sessions<'de, D>(deserializer: D) -> Result<Vec<SessionSegment>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_yaml::Value::deserialize(deserializer)?;
    match v {
        serde_yaml::Value::Null => Ok(Vec::new()),
        serde_yaml::Value::Mapping(m) => {
            if m.is_empty() {
                return Ok(Vec::new());
            }
            let segments = mapping_to_segments(m).map_err(Error::custom)?;
            validate_sessions(&segments).map_err(Error::custom)?;
            Ok(segments)
        }
        _ => Err(Error::custom("sessions must be a YAML mapping")),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub symbol: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_asset_sessions")]
    pub sessions: Vec<SessionSegment>,
}

impl Asset {
    /// True if there is exactly one trading band (no implicit pauses between segments).
    pub fn is_unique_session(&self) -> bool {
        self.sessions.len() == 1
    }

    /// The sole session when [`Self::is_unique_session`]; otherwise `None`.
    pub fn default_session(&self) -> Option<&SessionSegment> {
        if self.sessions.len() == 1 {
            self.sessions.first()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EquitySpec {
    pub symbol: String,
    pub exchange: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default, rename = "contract_standard")]
    pub ctr_std: Option<u32>,
    #[serde(default, rename = "contract_size")]
    pub ctr_size: Option<f64>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_asset_sessions")]
    pub sessions: Vec<SessionSegment>,
    #[serde(default)]
    pub exchange_timezone: Option<String>,
}

impl EquitySpec {
    pub fn regular_session(&self) -> Option<&SessionSegment> {
        self.sessions.first()
    }
    pub fn is_unique_session(&self) -> bool {
        self.sessions.len() == 1
    }
    pub fn default_session(&self) -> Option<&SessionSegment> {
        if self.sessions.len() == 1 {
            self.sessions.first()
        } else {
            None
        }
    }
    pub fn regular_session_start_end(&self) -> Option<(&str, &str)> {
        let seg = self.regular_session()?;
        Some((seg.start.as_str(), seg.end.as_str()))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Exchange {
    pub code: String,
    #[serde(default)]
    pub mic: Option<String>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub assets: HashMap<String, Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContractCycle {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub months: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpirationRule {
    #[serde(default)]
    pub name: String,
    pub r#type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub weekday: Option<String>,
    #[serde(default)]
    pub day: Option<i32>,
    #[serde(default)]
    pub n: Option<i32>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContractSpec {
    pub symbol: String,
    pub exchange: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_ticker_format")]
    pub ticker_format: String,
    pub contract_cycle: String,
    pub expiration_rule: String,
    #[serde(default, rename = "contract_standard")]
    pub ctr_std: Option<u32>,
    #[serde(default, rename = "contract_size")]
    pub ctr_size: Option<f64>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Copied at load time from `exchanges/*.yaml` for this symbol (not in contract YAML).
    #[serde(default, deserialize_with = "deserialize_contract_sessions")]
    pub sessions: Vec<SessionSegment>,
    #[serde(default)]
    pub exchange_timezone: Option<String>,
}

impl ContractSpec {
    /// The regular band (first segment; clock times in [`Self::exchange_timezone`]).
    pub fn regular_session(&self) -> Option<&SessionSegment> {
        self.sessions.first()
    }

    /// True if there is exactly one trading band (no implicit pauses between segments).
    pub fn is_unique_session(&self) -> bool {
        self.sessions.len() == 1
    }

    /// The sole session when there is only one band; `None` if zero or multiple segments.
    pub fn default_session(&self) -> Option<&SessionSegment> {
        if self.sessions.len() == 1 {
            self.sessions.first()
        } else {
            None
        }
    }

    /// Start and end clock times for the regular session, e.g. `("09:00", "18:30")`.
    pub fn regular_session_start_end(&self) -> Option<(&str, &str)> {
        let seg = self.regular_session()?;
        Some((seg.start.as_str(), seg.end.as_str()))
    }
}

fn default_ticker_format() -> String {
    "{symbol}{month_code}{yy}".to_string()
}

/// Loaded spec repository (futures + options + shared cycles/rules).
#[derive(Debug, Clone)]
pub struct SpecRepository {
    pub exchanges: HashMap<String, Exchange>,
    pub contracts: HashMap<String, ContractSpec>,
    /// Option rules loaded from all `options:` blocks in `contracts/**/*.yaml`.
    pub options: Vec<OptionRule>,
    pub equities: HashMap<String, EquitySpec>,
    pub contract_cycles: HashMap<String, ContractCycle>,
    pub expiration_rules: HashMap<String, ExpirationRule>,
    pub schedules: HashMap<String, ExchangeSchedule>,
}

impl SpecRepository {
    pub fn get_exchange(&self, code: &str) -> Result<&Exchange, String> {
        let key = code.to_uppercase();
        self.exchanges
            .get(&key)
            .ok_or_else(|| format!("Unknown exchange: {code}"))
    }

    pub fn get_contract(&self, symbol: &str) -> Result<&ContractSpec, String> {
        let key = symbol.to_uppercase();
        self.contracts
            .get(&key)
            .ok_or_else(|| format!("Unknown contract: {symbol}"))
    }

    pub fn get_equity(&self, symbol: &str) -> Result<&EquitySpec, String> {
        let key = symbol.to_uppercase();
        self.equities
            .get(&key)
            .ok_or_else(|| format!("Unknown equity: {symbol}"))
    }
}

/// Parsed futures ticker.
#[derive(Debug, Clone)]
pub struct ParsedFuturesTicker {
    pub symbol: String,
    pub year: i32,
    pub month: u32,
    pub tick_size: Option<f64>,
    pub ctr_std: Option<u32>,
    pub ctr_size: Option<f64>,
    pub contract: ContractSpec,
    /// The date used for root-symbol resolution.  `None` when a full ticker
    /// was parsed (no date context).
    pub reference_date: Option<chrono::NaiveDate>,
    /// Whether [`reference_date`] is an actual exchange trading session.
    /// `None` when a full ticker was parsed.
    pub is_trading_session: Option<bool>,
}

/// Parsed equity ticker.
#[derive(Debug, Clone)]
pub struct ParsedEquityTicker {
    pub symbol: String,
    pub equity: EquitySpec,
}

/// Parsed option ticker.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedOptionTicker {
    /// `"equity"`, `"index"`, `"dollar"`, or `"interest_rate"`.
    pub kind: String,
    /// Full underlying symbol (`"PETR4"`) for equity; root symbol (`"IBOV"`, `"DOL"`, `"IDI"`)
    /// for other types.
    pub underlying_or_symbol: String,
    /// Contract year (`2000 + yy`).  `None` for equity options (no year in ticker).
    pub year: Option<i32>,
    /// Contract month (1–12).
    pub month: u32,
    /// `true` = call, `false` = put.
    pub is_call: bool,
    /// Raw strike string as it appears in the ticker (e.g. `"5000"`, `"120000"`).
    pub strike: String,
    /// Exchange code (e.g. `"B3"`).
    pub exchange: String,
    /// Minimum price increment from the option rule.
    pub tick_size: Option<f64>,
    pub ctr_std: Option<u32>,
    pub ctr_size: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract_spec(sessions: Vec<SessionSegment>) -> ContractSpec {
        ContractSpec {
            symbol: "Y".into(),
            exchange: "B3".into(),
            description: None,
            ticker_format: default_ticker_format(),
            contract_cycle: "m".into(),
            expiration_rule: "r".into(),
            ctr_std: None,
            ctr_size: None,
            tick_size: None,
            currency: None,
            aliases: vec![],
            sessions,
            exchange_timezone: None,
        }
    }

    #[test]
    fn default_session_is_some_only_for_single_segment() {
        let one = sample_contract_spec(vec![SessionSegment {
            name: "regular".into(),
            start: "09:00".into(),
            end: "18:00".into(),
        }]);
        assert!(one.is_unique_session());
        assert_eq!(
            one.default_session().map(|s| s.name.as_str()),
            Some("regular")
        );

        let multi = sample_contract_spec(vec![
            SessionSegment {
                name: "regular".into(),
                start: "09:00".into(),
                end: "12:00".into(),
            },
            SessionSegment {
                name: "afternoon".into(),
                start: "13:00".into(),
                end: "18:00".into(),
            },
        ]);
        assert!(!multi.is_unique_session());
        assert!(multi.default_session().is_none());
    }

    #[test]
    fn empty_contract_spec_sessions_no_default() {
        let empty = sample_contract_spec(vec![]);
        assert!(!empty.is_unique_session());
        assert!(empty.default_session().is_none());
    }
}
