//! Options ticker generation (B3) and parsing.

use chrono::{Datelike, NaiveDate};

use crate::calendars::get_calendar;
use crate::contract_cycle::resolve_contract_months;
use crate::dates::is_month_in_calendar_range;
use crate::expiration_rules::{month_sessions, resolve_expiration};
use crate::models::{ContractSpec, SpecRepository};
use crate::month_codes::month_to_code;
use crate::options_models::{OptionRule, OptionTypeCodes};
use crate::options_spec::load_option_rules;

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|e| e.to_string())
}

/// Strip last digit from equity root (PETR4 → PETR).
pub fn equity_root(underlying: &str) -> String {
    let mut cs: Vec<char> = underlying.chars().collect();
    if let Some(last) = cs.last() {
        if last.is_ascii_digit() {
            cs.pop();
        }
    }
    cs.into_iter().collect()
}

fn still_tradeable(as_of: NaiveDate, expiration: NaiveDate, contract: &ContractSpec) -> bool {
    if contract.symbol == "DOL" || contract.symbol == "WDO" {
        as_of < expiration
    } else {
        as_of <= expiration
    }
}

fn synthetic_contract(symbol: &str, exchange: &str, cycle: &str, exp_rule: &str) -> ContractSpec {
    ContractSpec {
        symbol: symbol.to_string(),
        exchange: exchange.to_string(),
        description: None,
        ticker_format: "{symbol}{month_code}{yy}".to_string(),
        contract_cycle: cycle.to_string(),
        expiration_rule: exp_rule.to_string(),
        contract_multiplier: None,
        tick_size: None,
        currency: None,
        aliases: vec![],
        sessions: vec![],
        exchange_timezone: None,
    }
}

fn collect_eligible_months(
    spec: &SpecRepository,
    contract: &ContractSpec,
    as_of: NaiveDate,
) -> Result<Vec<(i32, u32)>, String> {
    let cycle = spec
        .contract_cycles
        .get(&contract.contract_cycle)
        .ok_or_else(|| format!("unknown cycle {}", contract.contract_cycle))?;
    let rule = spec
        .expiration_rules
        .get(&contract.expiration_rule)
        .ok_or_else(|| format!("unknown rule {}", contract.expiration_rule))?;
    let cal = get_calendar(&contract.exchange);
    let mut eligible: Vec<(i32, u32)> = Vec::new();
    for year in as_of.year()..as_of.year() + 4 {
        let months = resolve_contract_months(cycle, year)?;
        for month in months {
            if !is_month_in_calendar_range(&cal, year, month) {
                continue;
            }
            if month_sessions(&cal, year, month).is_empty() {
                continue;
            }
            let expiration_date = match resolve_expiration(contract, year, month, rule, &cal) {
                Ok(d) => d,
                Err(_) => continue,
            };
            if still_tradeable(as_of, expiration_date, contract) {
                eligible.push((year, month));
            }
        }
    }
    Ok(eligible)
}

/// Parsed option ticker (structured fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedOptionTicker {
    pub kind: String,
    pub underlying_or_symbol: String,
    pub year: i32,
    pub month: u32,
    pub is_call: bool,
    pub strike: i64,
}

pub struct OptionGenerator {
    pub spec: SpecRepository,
    pub rules: Vec<OptionRule>,
}

impl OptionGenerator {
    pub fn new(spec: SpecRepository, rules: Vec<OptionRule>) -> Self {
        Self { spec, rules }
    }

    /// Load futures spec and B3 options rules from the bundled default paths.
    pub fn bundled() -> Result<Self, String> {
        Self::from_spec_and_options_paths(None, None)
    }

    /// Load from a custom spec root directory (uses `spec_root/contracts/b3/options.yaml` for options).
    pub fn with_spec_root(spec_root: &std::path::Path) -> Result<Self, String> {
        let options = spec_root.join("contracts").join("b3").join("options.yaml");
        Self::from_spec_and_options_paths(Some(spec_root), Some(&options))
    }

    /// Load with optional spec root and optional `options.yaml` path (same defaults as [`bundled()`]).
    pub fn from_spec_and_options_paths(
        spec_path: Option<&std::path::Path>,
        options_path: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        let spec = match spec_path {
            Some(p) => crate::spec_loader::load_spec_from_path(p)?,
            None => crate::spec_loader::load_spec()?,
        };
        let rules = load_option_rules(options_path)?;
        Ok(Self::new(spec, rules))
    }

    /// Ibovespa options use equity-style month letters **A–L** in the ticker (not futures F–Z).
    fn index_ibov_month_letter(month: u32) -> char {
        "ABCDEFGHIJKL"
            .chars()
            .nth((month - 1) as usize)
            .unwrap_or('A')
    }

    #[allow(clippy::too_many_arguments)]
    fn gen_symbol_option(
        &self,
        rule_symbol: &str,
        date: &str,
        is_call: bool,
        strike: i64,
        offset: usize,
        expiration_rule: &str,
        contract_cycle: &str,
        exchange: &str,
        opt_codes: &OptionTypeCodes,
        month_char: impl Fn(u32) -> Result<char, String>,
    ) -> Result<String, String> {
        let as_of = parse_date(date)?;
        let contract = synthetic_contract(rule_symbol, exchange, contract_cycle, expiration_rule);
        let eligible = collect_eligible_months(&self.spec, &contract, as_of)?;
        if offset >= eligible.len() {
            return Err("offset out of range".to_string());
        }
        let (year, month) = eligible[offset];
        let mc = month_char(month)?;
        let yy = format!("{:02}", year.rem_euclid(100));
        let ot = if is_call {
            opt_codes.call.chars().next().unwrap()
        } else {
            opt_codes.put.chars().next().unwrap()
        };
        let strike_s = format!("{:06}", strike);
        Ok(format!("{rule_symbol}{mc}{yy}{ot}{strike_s}"))
    }

    /// Equity option `{root}{month_code}{strike}`.
    pub fn generate_equity(
        &self,
        underlying: &str,
        date: &str,
        is_call: bool,
        strike: i64,
        offset: usize,
    ) -> Result<String, String> {
        let rule = self
            .rules
            .iter()
            .find_map(|r| match r {
                OptionRule::Equity(e) => Some(e),
                _ => None,
            })
            .ok_or_else(|| "no equity option rule".to_string())?;
        let u = underlying.to_uppercase();
        if !rule.underlyings.iter().any(|x| x.eq_ignore_ascii_case(&u)) {
            return Err(format!("underlying not listed: {underlying}"));
        }
        let as_of = parse_date(date)?;
        let contract = synthetic_contract(
            &u,
            &rule.exchange,
            &rule.contract_cycle,
            &rule.expiration_rule,
        );
        let eligible = collect_eligible_months(&self.spec, &contract, as_of)?;
        if offset >= eligible.len() {
            return Err("offset out of range".to_string());
        }
        let (_year, month) = eligible[offset];
        let mc = if is_call {
            rule.call_month_codes
                .get((month - 1) as usize)
                .ok_or_else(|| "call month code".to_string())?
        } else {
            rule.put_month_codes
                .get((month - 1) as usize)
                .ok_or_else(|| "put month code".to_string())?
        };
        let ch = mc
            .chars()
            .next()
            .ok_or_else(|| "empty month code".to_string())?;
        let root = equity_root(&u);
        Ok(format!("{root}{ch}{strike}"))
    }

    pub fn generate_index(
        &self,
        symbol: &str,
        date: &str,
        is_call: bool,
        strike: i64,
        offset: usize,
    ) -> Result<String, String> {
        let rule = self
            .rules
            .iter()
            .find_map(|r| match r {
                OptionRule::Index(i) if i.symbol.eq_ignore_ascii_case(symbol) => Some(i),
                _ => None,
            })
            .ok_or_else(|| format!("no index rule for {symbol}"))?;
        self.gen_symbol_option(
            &rule.symbol,
            date,
            is_call,
            strike,
            offset,
            &rule.expiration_rule,
            &rule.contract_cycle,
            &rule.exchange,
            &rule.option_type_codes,
            |m| Ok(Self::index_ibov_month_letter(m)),
        )
    }

    pub fn generate_dollar(
        &self,
        date: &str,
        is_call: bool,
        strike: i64,
        offset: usize,
    ) -> Result<String, String> {
        let rule = self
            .rules
            .iter()
            .find_map(|r| match r {
                OptionRule::Dollar(d) => Some(d),
                _ => None,
            })
            .ok_or_else(|| "no dollar option rule".to_string())?;
        self.gen_symbol_option(
            &rule.symbol,
            date,
            is_call,
            strike,
            offset,
            &rule.expiration_rule,
            &rule.contract_cycle,
            &rule.exchange,
            &rule.option_type_codes,
            |m| month_to_code(m).map_err(|e| e.to_string()),
        )
    }

    pub fn generate_interest_rate(
        &self,
        date: &str,
        is_call: bool,
        strike: i64,
        offset: usize,
    ) -> Result<String, String> {
        let rule = self
            .rules
            .iter()
            .find_map(|r| match r {
                OptionRule::InterestRate(i) => Some(i),
                _ => None,
            })
            .ok_or_else(|| "no interest rate option rule".to_string())?;
        self.gen_symbol_option(
            &rule.symbol,
            date,
            is_call,
            strike,
            offset,
            &rule.expiration_rule,
            &rule.contract_cycle,
            &rule.exchange,
            &rule.option_type_codes,
            |m| month_to_code(m).map_err(|e| e.to_string()),
        )
    }

    /// Dispatch for CSV tests (`kind` = equity|index|dollar|interest_rate).
    pub fn generate_from_row(
        &self,
        kind: &str,
        underlying: &str,
        date: &str,
        opt_type: &str,
        strike: i64,
        offset: usize,
    ) -> Result<String, String> {
        let is_call = opt_type.eq_ignore_ascii_case("call");
        match kind {
            "equity" => self.generate_equity(underlying, date, is_call, strike, offset),
            "index" => self.generate_index(underlying, date, is_call, strike, offset),
            "dollar" => self.generate_dollar(date, is_call, strike, offset),
            "interest_rate" => self.generate_interest_rate(date, is_call, strike, offset),
            _ => Err(format!("unknown option kind: {kind}")),
        }
    }
}

/// Placeholder parser (round-trip tests use generation).
pub struct OptionParser;
