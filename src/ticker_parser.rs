//! Futures ticker parsing.

use chrono::{Datelike, NaiveDate};

use regex::Regex;

use crate::contract_cycle::resolve_contract_months;
use crate::models::{ContractSpec, ParsedFuturesTicker, SpecRepository};
use crate::month_codes::code_to_month;
use crate::spec_loader::{load_spec, load_spec_from_path};

fn coerce_reference_date(reference_date: Option<&str>) -> NaiveDate {
    if let Some(s) = reference_date {
        if let Ok(d) = NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            return d;
        }
    }
    chrono::Local::now().date_naive()
}

fn pattern_for_contract(contract: &ContractSpec) -> Result<Regex, String> {
    let mut escaped = regex::escape(&contract.ticker_format);
    escaped = escaped.replace("\\{symbol\\}", &regex::escape(&contract.symbol));
    escaped = escaped.replace("\\{month_code\\}", "(?P<month_code>[FGHJKMNQUVXZ])");
    escaped = escaped.replace("\\{yy\\}", "(?P<yy>\\d{2})");
    let pattern = format!("^{escaped}$");
    Regex::new(&pattern).map_err(|e| e.to_string())
}

/// Parse a futures ticker (matches Python `parse_ticker`).
pub fn parse_ticker(
    ticker: &str,
    spec: &SpecRepository,
    reference_date: Option<&str>,
) -> Result<ParsedFuturesTicker, String> {
    let ref_date = coerce_reference_date(reference_date);
    let reference_century = (ref_date.year() / 100) * 100;

    for contract in spec.contracts.values() {
        let re = pattern_for_contract(contract)?;
        let Some(caps) = re.captures(ticker) else {
            continue;
        };
        let month_code: char = caps["month_code"].chars().next().unwrap();
        let month = code_to_month(month_code)?;
        let yy: i32 = caps["yy"].parse().map_err(|e| format!("yy: {e}"))?;
        let mut year = reference_century + yy;
        if year < ref_date.year() - 50 {
            year += 100;
        } else if year > ref_date.year() + 50 {
            year -= 100;
        }

        let cycle = spec
            .contract_cycles
            .get(&contract.contract_cycle)
            .ok_or_else(|| format!("unknown cycle {}", contract.contract_cycle))?;
        let valid_months = resolve_contract_months(cycle, year)?;
        if !valid_months.contains(&month) {
            continue;
        }

        return Ok(ParsedFuturesTicker {
            symbol: contract.symbol.clone(),
            year,
            month,
            contract: contract.clone(),
        });
    }

    Err(format!("Unable to parse ticker: {ticker}"))
}

/// Stateful parser matching Python `TickerParser`.
pub struct TickerParser {
    pub spec: SpecRepository,
}

impl TickerParser {
    pub fn new() -> Result<Self, String> {
        Ok(Self { spec: load_spec()? })
    }

    pub fn with_spec_path(path: &std::path::Path) -> Result<Self, String> {
        Ok(Self {
            spec: load_spec_from_path(path)?,
        })
    }

    pub fn parse(
        &self,
        ticker: &str,
        reference_date: Option<&str>,
    ) -> Result<ParsedFuturesTicker, String> {
        parse_ticker(ticker, &self.spec, reference_date)
    }
}
