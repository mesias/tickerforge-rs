//! Options contract rules (from `spec/contracts/**/options.yaml`).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OptionRule {
    Equity(EquityOptionRule),
    Index(IndexOptionRule),
    Dollar(DollarOptionRule),
    InterestRate(InterestRateOptionRule),
}

#[derive(Debug, Clone, Deserialize)]
pub struct EquityOptionRule {
    pub exchange: String,
    #[serde(default)]
    pub description: Option<String>,
    pub option_style: String,
    pub ticker_format: String,
    #[serde(default)]
    pub contract_multiplier: Option<f64>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub call_month_codes: Vec<String>,
    pub put_month_codes: Vec<String>,
    pub contract_cycle: String,
    pub expiration_rule: String,
    pub underlyings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexOptionRule {
    pub symbol: String,
    pub exchange: String,
    #[serde(default)]
    pub description: Option<String>,
    pub option_style: String,
    pub ticker_format: String,
    #[serde(default)]
    pub contract_multiplier: Option<f64>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub option_type_codes: OptionTypeCodes,
    pub contract_cycle: String,
    pub expiration_rule: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DollarOptionRule {
    pub symbol: String,
    pub exchange: String,
    #[serde(default)]
    pub description: Option<String>,
    pub option_style: String,
    pub ticker_format: String,
    #[serde(default)]
    pub contract_multiplier: Option<f64>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub option_type_codes: OptionTypeCodes,
    pub contract_cycle: String,
    pub expiration_rule: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InterestRateOptionRule {
    pub symbol: String,
    pub exchange: String,
    #[serde(default)]
    pub description: Option<String>,
    pub option_style: String,
    pub ticker_format: String,
    #[serde(default)]
    pub contract_multiplier: Option<f64>,
    #[serde(default)]
    pub tick_size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub option_type_codes: OptionTypeCodes,
    pub contract_cycle: String,
    pub expiration_rule: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionTypeCodes {
    pub call: String,
    pub put: String,
}
