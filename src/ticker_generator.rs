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

/// Build a futures ticker string from contract metadata and expiry month.
pub fn format_contract_ticker(
    contract: &ContractSpec,
    year: i32,
    month: u32,
) -> Result<String, String> {
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

fn format_ticker(contract: &ContractSpec, year: i32, month: u32) -> Result<String, String> {
    format_contract_ticker(contract, year, month)
}

pub(crate) fn resolve_last_trading_day(
    expiration_date: NaiveDate,
    rule: &crate::models::ExpirationRule,
    calendar: &crate::calendars::ExchangeCalendar,
) -> NaiveDate {
    let offset = rule.effective_last_trading_day_offset();
    if offset < 0 {
        let days_back = ((-offset) * 7 + 10) as i64;
        let sessions = calendar.sessions_in_range(
            expiration_date - chrono::Duration::days(days_back),
            expiration_date,
        );
        let prior: Vec<NaiveDate> = sessions
            .into_iter()
            .filter(|&d| d < expiration_date)
            .collect();
        let idx = prior.len() as isize + offset as isize;
        if idx >= 0 && (idx as usize) < prior.len() {
            return prior[idx as usize];
        }
    }
    expiration_date
}

pub(crate) fn is_front_eligible(
    as_of: NaiveDate,
    expiration: NaiveDate,
    rule: &crate::models::ExpirationRule,
    calendar: &crate::calendars::ExchangeCalendar,
) -> bool {
    let ltd = resolve_last_trading_day(expiration, rule, calendar);
    if rule.should_roll_on_last_trading_day() {
        as_of < ltd
    } else {
        as_of <= ltd
    }
}

pub(crate) fn is_contract_tradeable(
    as_of: NaiveDate,
    expiration: NaiveDate,
    rule: &crate::models::ExpirationRule,
    calendar: &crate::calendars::ExchangeCalendar,
) -> bool {
    let ltd = resolve_last_trading_day(expiration, rule, calendar);
    as_of <= ltd
}

/// Collect still-tradeable `(year, month)` pairs scanned forward from
/// `as_of_date.year() .. as_of_date.year() + 4`, in ascending order.
pub(crate) fn collect_eligible_forward(
    contract: &ContractSpec,
    as_of_date: NaiveDate,
    spec: &SpecRepository,
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
            if is_front_eligible(as_of_date, expiration_date, rule, &cal) {
                eligible.push((year, month));
            }
        }
    }
    Ok(eligible)
}

/// Collect already-expired `(year, month)` pairs scanned backward over
/// `as_of_date.year() - 4 ..= as_of_date.year()`, returned sorted so the
/// MOST RECENTLY expired contract is first (descending by expiration date).
fn collect_eligible_backward(
    contract: &ContractSpec,
    as_of_date: NaiveDate,
    spec: &SpecRepository,
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

    // Pairs with their expiration date so we can sort by it.
    let mut expired: Vec<(NaiveDate, i32, u32)> = Vec::new();
    for year in (as_of_date.year() - 4)..=as_of_date.year() {
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
            if !is_front_eligible(as_of_date, expiration_date, rule, &cal) {
                expired.push((expiration_date, year, month));
            }
        }
    }
    // Most-recently-expired first: sort by expiration date descending.
    expired.sort_by_key(|b| std::cmp::Reverse(b.0));
    Ok(expired.into_iter().map(|(_, y, m)| (y, m)).collect())
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
    let eligible = collect_eligible_forward(contract, as_of_date, spec)?;

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

/// Signed-offset variant of [`generate_ticker_for_contract`].
///
/// - `offset >= 0` → `offset`-th still-tradeable contract from the front
///   (same as the unsigned `generate_ticker_for_contract`).
/// - `offset < 0` → `(-offset - 1)`-th most-recently-EXPIRED contract
///   (`-1` = the contract that most recently rolled off).
pub fn generate_ticker_for_contract_signed(
    contract: &ContractSpec,
    as_of: &str,
    spec: &SpecRepository,
    offset: isize,
) -> Result<String, String> {
    if as_of.is_empty() {
        return Err("empty as_of date".to_string());
    }
    let as_of_date = coerce_date(as_of)?;

    if offset >= 0 {
        let eligible = collect_eligible_forward(contract, as_of_date, spec)?;
        if eligible.is_empty() {
            return Err(format!(
                "No eligible contract found for {} at {as_of_date}",
                contract.symbol
            ));
        }
        let idx = offset as usize;
        if idx >= eligible.len() {
            return Err(format!(
                "Offset {offset} is out of range for {}",
                contract.symbol
            ));
        }
        let (year, month) = eligible[idx];
        format_ticker(contract, year, month)
    } else {
        let expired = collect_eligible_backward(contract, as_of_date, spec)?;
        if expired.is_empty() {
            return Err(format!(
                "No expired contract found for {} at {as_of_date}",
                contract.symbol
            ));
        }
        let idx = ((-offset) - 1) as usize;
        if idx >= expired.len() {
            return Err(format!(
                "Offset {offset} is out of range for {}",
                contract.symbol
            ));
        }
        let (year, month) = expired[idx];
        format_ticker(contract, year, month)
    }
}

/// Front-month ticker for today (offset 0).
pub fn gen_ticker_ctr(contract: &ContractSpec, spec: &SpecRepository) -> Result<String, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    generate_ticker_for_contract(contract, &today, spec, 0)
}

/// Signed-offset ticker for today.  See [`generate_ticker_for_contract_signed`].
pub fn gen_ticker_ctr_signed(
    contract: &ContractSpec,
    spec: &SpecRepository,
    offset: isize,
) -> Result<String, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    generate_ticker_for_contract_signed(contract, &today, spec, offset)
}

impl ContractSpec {
    /// Front-month ticker for today using the bundled default spec.
    pub fn trading_symbol_today(&self) -> Result<String, String> {
        let spec = load_spec()?;
        gen_ticker_ctr(self, &spec)
    }

    /// Front-month ticker for `as_of` (`YYYY-MM-DD`) using the bundled default spec.
    pub fn trading_symbol_for(&self, as_of: &str, offset: usize) -> Result<String, String> {
        let spec = load_spec()?;
        generate_ticker_for_contract(self, as_of, &spec, offset)
    }

    /// Front-month ticker for today with an explicit [`SpecRepository`].
    pub fn trading_symbol_today_with_spec(&self, spec: &SpecRepository) -> Result<String, String> {
        gen_ticker_ctr(self, spec)
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

    /// Front-month ticker for today (offset 0).
    pub fn gen(&self, symbol: &str) -> Result<String, String> {
        let contract = self.spec.get_contract(symbol).map_err(|e| e.to_string())?;
        gen_ticker_ctr(contract, &self.spec)
    }

    /// Signed-offset ticker for today.  See [`generate_ticker_for_contract_signed`].
    pub fn gen_signed(&self, symbol: &str, offset: isize) -> Result<String, String> {
        let contract = self.spec.get_contract(symbol).map_err(|e| e.to_string())?;
        gen_ticker_ctr_signed(contract, &self.spec, offset)
    }

    pub fn generate(&self, symbol: &str, date: &str, offset: usize) -> Result<String, String> {
        let contract = self.spec.get_contract(symbol).map_err(|e| e.to_string())?;
        generate_ticker_for_contract(contract, date, &self.spec, offset)
    }
}
