//! Futures ticker generation.

use chrono::{Datelike, NaiveDate};

use crate::calendars::get_calendar;
use crate::contract_cycle::resolve_contract_months;
use crate::dates::is_month_in_calendar_range;
use crate::expiration_rules::{month_sessions, resolve_expiration};
use crate::models::{ContractSpec, SpecRepository};
use crate::month_codes::month_to_code;
use crate::spec_loader::{load_spec, load_spec_from_path};

fn coerce_date(as_of: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(as_of.trim(), "%Y-%m-%d")
        .map_err(|e| format!("invalid date {as_of}: {e}"))
}

fn format_ticker(contract: &ContractSpec, year: i32, month: u32) -> Result<String, String> {
    let mc = month_to_code(month)?;
    let yy = format!("{:02}", year.rem_euclid(100));
    let mut out = contract.ticker_format.clone();
    out = out.replace("{symbol}", &contract.symbol);
    out = out.replace("{month_code}", &mc.to_string());
    out = out.replace("{yy}", &yy);
    out = out.replace("{year}", &year.to_string());
    out = out.replace("{month}", &month.to_string());
    Ok(out)
}

fn still_tradeable(as_of: NaiveDate, expiration: NaiveDate, contract: &ContractSpec) -> bool {
    if contract.symbol == "DOL" || contract.symbol == "WDO" {
        as_of < expiration
    } else {
        as_of <= expiration
    }
}

/// Generate ticker string for a contract (matches Python `generate_ticker_for_contract`).
pub fn generate_ticker_for_contract(
    contract: &ContractSpec,
    as_of: &str,
    spec: &SpecRepository,
    offset: usize,
) -> Result<String, String> {
    if as_of.is_empty() {
        return Err("empty as_of date".to_string());
    }
    let as_of_date = coerce_date(as_of)?;
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
    for year in as_of_date.year()..as_of_date.year() + 4 {
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
            if still_tradeable(as_of_date, expiration_date, contract) {
                eligible.push((year, month));
            }
        }
    }

    if eligible.is_empty() {
        return Err(format!(
            "No eligible contract found for {} at {as_of_date}",
            contract.symbol
        ));
    }
    if offset >= eligible.len() {
        return Err(format!(
            "Offset {offset} is out of range for {}",
            contract.symbol
        ));
    }
    let (year, month) = eligible[offset];
    format_ticker(contract, year, month)
}

impl ContractSpec {
    /// Front-month ticker for today using the bundled default spec.
    pub fn trading_symbol_today(&self) -> Result<String, String> {
        let spec = load_spec()?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        generate_ticker_for_contract(self, &today, &spec, 0)
    }

    /// Front-month ticker for `as_of` (`YYYY-MM-DD`) using the bundled default spec.
    pub fn trading_symbol_for(&self, as_of: &str, offset: usize) -> Result<String, String> {
        let spec = load_spec()?;
        generate_ticker_for_contract(self, as_of, &spec, offset)
    }

    /// Front-month ticker for today with an explicit [`SpecRepository`].
    pub fn trading_symbol_today_with_spec(&self, spec: &SpecRepository) -> Result<String, String> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        generate_ticker_for_contract(self, &today, spec, 0)
    }

    /// Front-month ticker for `as_of` with an explicit [`SpecRepository`].
    pub fn trading_symbol_for_with_spec(
        &self,
        spec: &SpecRepository,
        as_of: &str,
        offset: usize,
    ) -> Result<String, String> {
        generate_ticker_for_contract(self, as_of, spec, offset)
    }
}

/// High-level API matching Python `TickerForge`.
pub struct TickerForge {
    pub spec: SpecRepository,
}

impl TickerForge {
    pub fn new() -> Result<Self, String> {
        Ok(Self { spec: load_spec()? })
    }

    pub fn with_spec_path(path: &std::path::Path) -> Result<Self, String> {
        Ok(Self {
            spec: load_spec_from_path(path)?,
        })
    }

    pub fn generate(&self, symbol: &str, date: &str, offset: usize) -> Result<String, String> {
        let contract = self.spec.get_contract(symbol).map_err(|e| e.to_string())?;
        generate_ticker_for_contract(contract, date, &self.spec, offset)
    }
}
