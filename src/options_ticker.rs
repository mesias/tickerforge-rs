//! Options ticker generation (B3) and parsing (all markets).

use regex::Regex;

use chrono::{Datelike, NaiveDate};

use crate::calendars::get_calendar;
use crate::contract_cycle::resolve_contract_months;
use crate::dates::is_month_in_calendar_range;
use crate::expiration_rules::{month_sessions, resolve_expiration};
use crate::models::{ContractSpec, ParsedOptionTicker, SpecRepository};
use crate::month_codes::{code_to_month, month_to_code};
use crate::options_models::{OptionRule, OptionTypeCodes};

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|e| e.to_string())
}

/// Strip one trailing digit from an equity underlying symbol.
///
/// `"PETR4"` → `"PETR"`, `"BOVA11"` → `"BOVA1"`.
/// Mirrors Python `_equity_root`.
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
        ctr_std: None,
        ctr_size: None,
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

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Build a combined character class string for all equity call + put codes.
fn equity_all_codes(call_codes: &[String], put_codes: &[String]) -> String {
    let mut s = String::new();
    for c in call_codes.iter().chain(put_codes.iter()) {
        if let Some(ch) = c.chars().next() {
            s.push(ch);
        }
    }
    s
}

/// Map an equity option month code character back to `(month, is_call)`.
fn equity_decode_month_code(
    ch: char,
    call_codes: &[String],
    put_codes: &[String],
) -> Option<(u32, bool)> {
    let upper = ch.to_ascii_uppercase().to_string();
    if let Some(idx) = call_codes.iter().position(|c| *c == upper) {
        return Some(((idx as u32) + 1, true));
    }
    if let Some(idx) = put_codes.iter().position(|c| *c == upper) {
        return Some(((idx as u32) + 1, false));
    }
    None
}

const FUTURES_MONTH_CODES: &str = "FGHJKMNQUVXZ";

/// Try to match `ticker` against all equity option underlyings in `rule`.
fn match_equity_options(
    ticker: &str,
    rule: &crate::options_models::EquityOptionRule,
) -> Vec<ParsedOptionTicker> {
    let all_codes = equity_all_codes(&rule.call_month_codes, &rule.put_month_codes);
    let codes_escaped: String = regex::escape(&all_codes);

    let mut results = Vec::new();
    for underlying in &rule.underlyings {
        let root = equity_root(underlying);
        let pattern = format!(
            "^{}(?P<month_code>[{codes_escaped}])(?P<strike>\\d+)$",
            regex::escape(&root)
        );
        let re = match Regex::new(&pattern) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let Some(caps) = re.captures(ticker) else {
            continue;
        };
        let mc_char = caps["month_code"].chars().next().unwrap();
        let Some((month, is_call)) =
            equity_decode_month_code(mc_char, &rule.call_month_codes, &rule.put_month_codes)
        else {
            continue;
        };
        results.push(ParsedOptionTicker {
            kind: "equity".to_string(),
            underlying_or_symbol: underlying.clone(),
            year: None,
            month,
            is_call,
            strike: caps["strike"].to_string(),
            exchange: rule.exchange.clone(),
            tick_size: rule.tick_size,
            ctr_std: rule.ctr_std,
            ctr_size: rule.ctr_size,
        });
    }
    results
}

/// Try to match `ticker` against a non-equity option rule (index / dollar / interest_rate).
#[allow(clippy::too_many_arguments)]
fn match_nonequity_option(
    ticker: &str,
    kind: &str,
    symbol: &str,
    exchange: &str,
    opt_codes: &OptionTypeCodes,
    tick_size: Option<f64>,
    ctr_std: Option<u32>,
    ctr_size: Option<f64>,
) -> Option<ParsedOptionTicker> {
    let call_esc = regex::escape(&opt_codes.call);
    let put_esc = regex::escape(&opt_codes.put);
    let pattern = format!(
        "^{}(?P<month_code>[{FUTURES_MONTH_CODES}])(?P<yy>\\d{{2}})(?P<opt_type>{call_esc}|{put_esc})(?P<strike>\\d+)$",
        regex::escape(symbol)
    );
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(ticker)?;

    let mc_char = caps["month_code"].chars().next().unwrap();
    let month = code_to_month(mc_char).ok()?;
    let yy: i32 = caps["yy"].parse().ok()?;
    let year = 2000 + yy;
    let opt_type_str = &caps["opt_type"];
    let is_call = opt_type_str == opt_codes.call;

    Some(ParsedOptionTicker {
        kind: kind.to_string(),
        underlying_or_symbol: symbol.to_string(),
        year: Some(year),
        month,
        is_call,
        strike: caps["strike"].to_string(),
        exchange: exchange.to_string(),
        tick_size,
        ctr_std,
        ctr_size,
    })
}

// ---------------------------------------------------------------------------
// OptionParser
// ---------------------------------------------------------------------------

/// Parser for option tickers.
///
/// Uses the option rules already loaded into [`SpecRepository`] — no separate
/// file loading required.
pub struct OptionParser;

impl OptionParser {
    /// Collect all option ticker candidates for `ticker`.
    ///
    /// Returns a list that may be empty (no match), have one element (unambiguous),
    /// or multiple elements (ambiguous across markets or option types).
    /// Optionally filter by `exchange` (case-insensitive).
    pub fn parse_options(
        ticker: &str,
        spec: &SpecRepository,
        exchange: Option<&str>,
    ) -> Vec<ParsedOptionTicker> {
        let mut results: Vec<ParsedOptionTicker> = Vec::new();

        for rule in &spec.options {
            let mut candidates = match rule {
                OptionRule::Equity(r) => match_equity_options(ticker, r),
                 OptionRule::Index(r) => match_nonequity_option(
                    ticker,
                    "index",
                    &r.symbol,
                    &r.exchange,
                    &r.option_type_codes,
                    r.tick_size,
                    r.ctr_std,
                    r.ctr_size,
                )
                .into_iter()
                .collect(),
                OptionRule::Dollar(r) => match_nonequity_option(
                    ticker,
                    "dollar",
                    &r.symbol,
                    &r.exchange,
                    &r.option_type_codes,
                    r.tick_size,
                    r.ctr_std,
                    r.ctr_size,
                )
                .into_iter()
                .collect(),
                OptionRule::InterestRate(r) => match_nonequity_option(
                    ticker,
                    "interest_rate",
                    &r.symbol,
                    &r.exchange,
                    &r.option_type_codes,
                    r.tick_size,
                    r.ctr_std,
                    r.ctr_size,
                )
                .into_iter()
                .collect(),
            };

            if let Some(ex) = exchange {
                candidates.retain(|c| c.exchange.eq_ignore_ascii_case(ex));
            }

            results.extend(candidates);
        }

        results
    }

    /// Parse an option ticker, returning a single [`ParsedOptionTicker`].
    ///
    /// Returns `Err` if the ticker is unknown or matches multiple instruments.
    pub fn parse_option(ticker: &str, spec: &SpecRepository) -> Result<ParsedOptionTicker, String> {
        Self::parse_option_exchange(ticker, spec, None)
    }

    /// Parse an option ticker with an optional exchange filter.
    pub fn parse_option_exchange(
        ticker: &str,
        spec: &SpecRepository,
        exchange: Option<&str>,
    ) -> Result<ParsedOptionTicker, String> {
        let candidates = Self::parse_options(ticker, spec, exchange);
        match candidates.len() {
            1 => Ok(candidates.into_iter().next().unwrap()),
            0 => Err(format!("Unable to parse option ticker: {ticker}")),
            n => {
                let descs: Vec<String> = candidates
                    .iter()
                    .map(|c| {
                        format!(
                            "  - {} option on {}: {}",
                            c.kind, c.exchange, c.underlying_or_symbol
                        )
                    })
                    .collect();
                Err(format!(
                    "Ambiguous ticker '{ticker}' matched {n} option instruments:\n{}\nPass exchange= to disambiguate.",
                    descs.join("\n")
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OptionGenerator
// ---------------------------------------------------------------------------

/// Generates option tickers from a loaded [`SpecRepository`].
///
/// Option rules are taken directly from `spec.options` — no separate loading needed.
pub struct OptionGenerator {
    pub spec: SpecRepository,
}

impl OptionGenerator {
    /// Wrap an already-loaded [`SpecRepository`].
    pub fn new(spec: SpecRepository) -> Self {
        Self { spec }
    }

    /// Load the bundled default spec.
    pub fn bundled() -> Result<Self, String> {
        Ok(Self::new(crate::spec_loader::load_spec()?))
    }

    /// Load from a custom spec root directory.
    pub fn with_spec_root(spec_root: &std::path::Path) -> Result<Self, String> {
        Ok(Self::new(crate::spec_loader::load_spec_from_path(
            spec_root,
        )?))
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
            .spec
            .options
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
            .spec
            .options
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
            .spec
            .options
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
            .spec
            .options
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
