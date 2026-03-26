//! Pydantic-aligned models for futures contracts and exchanges.

use serde::Deserialize;
use std::collections::HashMap;

use crate::schedule::ExchangeSchedule;

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub symbol: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sessions: HashMap<String, HashMap<String, String>>,
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
    #[serde(default)]
    pub contract_multiplier: Option<f64>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

fn default_ticker_format() -> String {
    "{symbol}{month_code}{yy}".to_string()
}

/// Loaded spec repository (futures + shared cycles/rules).
#[derive(Debug, Clone)]
pub struct SpecRepository {
    pub exchanges: HashMap<String, Exchange>,
    pub contracts: HashMap<String, ContractSpec>,
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
}

/// Parsed futures ticker (matches Python `ParsedTicker`).
#[derive(Debug, Clone)]
pub struct ParsedFuturesTicker {
    pub symbol: String,
    pub year: i32,
    pub month: u32,
    pub contract: ContractSpec,
}
