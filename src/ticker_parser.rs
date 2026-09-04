//! Futures and options ticker parsing.
//!
//! # Quick start
//!
//! ```rust,no_run
//! // Simplest — panics if bundled spec is broken (should never happen)
//! let parsed = tickerforge::TickerParser::new().parse("INDM26").unwrap();
//!
//! // Parse any ticker (futures or option) with AnyParsedTicker
//! let any = tickerforge::parse_any_ticker("PETRA30").unwrap();
//!
//! // Builder — reusable parser with custom spec
//! let parser = tickerforge::TickerParser::builder()
//!     .spec("/path/to/spec")
//!     .build()
//!     .unwrap();
//!
//! // Builder — one-shot parse with exchange filter
//! let parsed = tickerforge::TickerParser::builder()
//!     .ticker("IND")
//!     .reference_date("2026-06-01")
//!     .exchange("B3")
//!     .parse()
//!     .unwrap();
//! ```

use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use regex::Regex;

use crate::calendars::get_calendar;
use crate::contract_cycle::resolve_contract_months;
use crate::models::{ParsedEquityTicker, ParsedFuturesTicker, ParsedOptionTicker, SpecRepository};
use crate::month_codes::code_to_month;
use crate::options_ticker::OptionParser;
use crate::pattern_index::equity_decode_month_code;
use crate::spec_loader::{load_spec, load_spec_from_path};
use crate::ticker_generator::{generate_ticker_for_contract, generate_ticker_for_contract_signed};

fn coerce_reference_date(reference_date: Option<&str>) -> NaiveDate {
    if let Some(s) = reference_date {
        if let Ok(d) = NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
            return d;
        }
    }
    chrono::Local::now().date_naive()
}

fn try_parse_full_ticker(
    ticker: &str,
    spec: &SpecRepository,
) -> Result<Option<ParsedFuturesTicker>, String> {
    for (re, contract) in &spec.pattern_index().futures {
        let Some(caps) = re.captures(ticker) else {
            continue;
        };
        let month_code: char = caps["month_code"].chars().next().unwrap();
        let month = code_to_month(month_code)?;
        let yy: i32 = caps["yy"].parse().map_err(|e| format!("yy: {e}"))?;
        let year = 2000 + yy;

        let cycle = spec
            .contract_cycles
            .get(&contract.contract_cycle)
            .ok_or_else(|| format!("unknown cycle {}", contract.contract_cycle))?;
        let valid_months = resolve_contract_months(cycle, year)?;
        if !valid_months.contains(&month) {
            continue;
        }

        return Ok(Some(ParsedFuturesTicker {
            symbol: contract.symbol.clone(),
            year,
            month,
            tick_size: contract.tick_size,
            ctr_std: contract.ctr_std,
            ctr_size: contract.ctr_size,
            contract: contract.clone(),
            reference_date: None,
            is_trading_session: None,
            contract_offset: None,
            is_valid: None,
        }));
    }

    Ok(None)
}

fn try_resolve_root_symbol(
    ticker: &str,
    spec: &SpecRepository,
    reference_date: Option<&str>,
) -> Result<Option<ParsedFuturesTicker>, String> {
    let key = ticker.to_uppercase();
    let contract = match spec.contracts.get(&key) {
        Some(c) => c,
        None => return Ok(None),
    };

    let ref_date = coerce_reference_date(reference_date);
    let date_str = ref_date.format("%Y-%m-%d").to_string();
    let full_ticker = generate_ticker_for_contract(contract, &date_str, spec, 0)?;
    let mut result = try_parse_full_ticker(&full_ticker, spec)?;
    if let Some(ref mut parsed) = result {
        let cal = get_calendar(&contract.exchange);
        let sessions = cal.sessions_in_range(ref_date, ref_date);
        parsed.reference_date = Some(ref_date);
        parsed.is_trading_session = Some(!sessions.is_empty());
        let rule = spec
            .expiration_rules
            .get(&contract.expiration_rule)
            .ok_or_else(|| format!("unknown rule {}", contract.expiration_rule))?;
        let expiration = crate::expiration_rules::resolve_expiration(
            contract,
            parsed.year,
            parsed.month,
            rule,
            &cal,
        )?;
        parsed.is_valid = Some(crate::ticker_generator::is_contract_tradeable(
            ref_date, expiration, rule, &cal,
        ));
    }
    Ok(result)
}

fn load_spec_for_builder(spec_path: Option<&Path>) -> Result<SpecRepository, String> {
    match spec_path {
        Some(p) => load_spec_from_path(p),
        None => load_spec(),
    }
}

/// If `ticker` matches the `SYMBOL[n]` or `SYMBOL[n@roll]` bracket-tag syntax, return the
/// uppercased root, the signed offset, and the optional roll condition.
fn parse_tagged_root(ticker: &str) -> Option<(String, Option<isize>, Option<String>)> {
    static TAG_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = TAG_RE.get_or_init(|| {
        Regex::new(r"(?i)^([A-Za-z][A-Za-z0-9]*)\[(?:(-?\d+)?@(roll)|(-?\d+))]$")
            .expect("valid tag regex")
    });
    let caps = re.captures(ticker)?;
    let root = caps[1].to_uppercase();
    if caps.get(3).is_some() {
        let offset = caps.get(2).and_then(|m| m.as_str().parse::<isize>().ok());
        let cond = caps[3].to_lowercase();
        Some((root, offset, Some(cond)))
    } else {
        let offset: isize = caps[4].parse().ok()?;
        Some((root, Some(offset), None))
    }
}

fn is_roll_day(
    contract: &crate::models::ContractSpec,
    ref_date: NaiveDate,
    spec: &SpecRepository,
) -> Result<bool, String> {
    let cycle = spec
        .contract_cycles
        .get(&contract.contract_cycle)
        .ok_or_else(|| format!("unknown cycle {}", contract.contract_cycle))?;
    let rule = spec
        .expiration_rules
        .get(&contract.expiration_rule)
        .ok_or_else(|| format!("unknown rule {}", contract.expiration_rule))?;
    let cal = get_calendar(&contract.exchange);

    for year in (ref_date.year() - 1)..=(ref_date.year() + 1) {
        let months = crate::contract_cycle::resolve_contract_months(cycle, year)?;
        for month in months {
            if let Ok(expiration_date) =
                crate::expiration_rules::resolve_expiration(contract, year, month, rule, &cal)
            {
                let ltd =
                    crate::ticker_generator::resolve_last_trading_day(expiration_date, rule, &cal);
                if ref_date == ltd {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Resolve a `SYMBOL[n]` tagged root to a parsed futures ticker.
fn try_resolve_tagged_root(
    ticker: &str,
    spec: &SpecRepository,
    reference_date: Option<&str>,
    exchange: Option<&str>,
) -> Result<Option<ParsedFuturesTicker>, String> {
    let Some((root, offset_opt, condition)) = parse_tagged_root(ticker) else {
        return Ok(None);
    };
    let contract = match spec.contracts.get(&root) {
        Some(c) => c,
        None => return Ok(None),
    };
    if let Some(ex) = exchange {
        if !contract.exchange.eq_ignore_ascii_case(ex) {
            return Ok(None);
        }
    }

    let ref_date = coerce_reference_date(reference_date);
    let rule = spec
        .expiration_rules
        .get(&contract.expiration_rule)
        .ok_or_else(|| format!("unknown rule {}", contract.expiration_rule))?;

    let offset = match offset_opt {
        Some(o) => o,
        None => {
            if rule.should_roll_on_last_trading_day() {
                0
            } else {
                1
            }
        }
    };

    if let Some(cond) = condition.as_deref() {
        if cond == "roll" && !is_roll_day(contract, ref_date, spec)? {
            return Err(format!(
                "Ticker '{}[{}@roll]' is not valid on {} because it is not the last trading day of the expiring contract.",
                contract.symbol, offset, ref_date.format("%Y-%m-%d")
            ));
        }
    }

    let date_str = ref_date.format("%Y-%m-%d").to_string();
    let full_ticker = generate_ticker_for_contract_signed(contract, &date_str, spec, offset)?;
    let mut result = try_parse_full_ticker(&full_ticker, spec)?;
    if let Some(ref mut parsed) = result {
        let cal = get_calendar(&contract.exchange);
        let sessions = cal.sessions_in_range(ref_date, ref_date);
        parsed.reference_date = Some(ref_date);
        parsed.is_trading_session = Some(!sessions.is_empty());
        parsed.contract_offset = Some(offset);
        let expiration = crate::expiration_rules::resolve_expiration(
            contract,
            parsed.year,
            parsed.month,
            rule,
            &cal,
        )?;
        parsed.is_valid = Some(crate::ticker_generator::is_contract_tradeable(
            ref_date, expiration, rule, &cal,
        ));
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Unified parsing: futures + options
// ---------------------------------------------------------------------------

/// A parsed ticker that may be either a futures contract or an option.
#[derive(Debug, Clone)]
pub enum AnyParsedTicker {
    /// A futures contract ticker.
    Futures(ParsedFuturesTicker),
    /// An option ticker.
    Option(ParsedOptionTicker),
    /// An equity ticker.
    Equity(ParsedEquityTicker),
}

impl AnyParsedTicker {
    /// Full trading symbol string (e.g. `DOLN26`, `PETRA30`, `DOLK26C5000`).
    pub fn ticker(&self) -> Result<String, String> {
        self.format_ticker()
    }

    /// Rebuild the exchange ticker string from this parsed result.
    pub fn format_ticker(&self) -> Result<String, String> {
        match self {
            AnyParsedTicker::Futures(f) => f.format_ticker(),
            AnyParsedTicker::Option(o) => {
                let spec = load_spec()?;
                o.format_ticker(&spec)
            }
            AnyParsedTicker::Equity(e) => Ok(e.format_ticker()),
        }
    }
}

fn parse_any_inner(
    ticker: &str,
    spec: &SpecRepository,
    reference_date: Option<&str>,
    exchange: Option<&str>,
) -> Result<AnyParsedTicker, String> {
    // Check equities first.
    let key = ticker.to_uppercase();
    if let Some(eq) = spec.equities.get(&key) {
        let mut matches_exchange = true;
        if let Some(ex) = exchange {
            if !eq.exchange.eq_ignore_ascii_case(ex) {
                matches_exchange = false;
            }
        }
        if matches_exchange {
            return Ok(AnyParsedTicker::Equity(ParsedEquityTicker {
                symbol: eq.symbol.clone(),
                equity: eq.clone(),
            }));
        }
    }

    // Collect futures candidates.
    let mut futures_candidates: Vec<ParsedFuturesTicker> = Vec::new();
    for (re, contract) in &spec.pattern_index().futures {
        if let Some(ex) = exchange {
            if !contract.exchange.eq_ignore_ascii_case(ex) {
                continue;
            }
        }
        let Some(caps) = re.captures(ticker) else {
            continue;
        };
        let month_code: char = caps["month_code"].chars().next().unwrap();
        let month = code_to_month(month_code)?;
        let yy: i32 = caps["yy"]
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        let year = 2000 + yy;
        let cycle = spec
            .contract_cycles
            .get(&contract.contract_cycle)
            .ok_or_else(|| format!("unknown cycle {}", contract.contract_cycle))?;
        let valid_months = resolve_contract_months(cycle, year)?;
        if !valid_months.contains(&month) {
            continue;
        }
        futures_candidates.push(ParsedFuturesTicker {
            symbol: contract.symbol.clone(),
            year,
            month,
            tick_size: contract.tick_size,
            ctr_std: contract.ctr_std,
            ctr_size: contract.ctr_size,
            contract: contract.clone(),
            reference_date: None,
            is_trading_session: None,
            contract_offset: None,
            is_valid: None,
        });
    }

    // Collect option candidates.
    let option_candidates = OptionParser::parse_options(ticker, spec, exchange);

    let total = futures_candidates.len() + option_candidates.len();

    if total > 1 {
        let mut descs: Vec<String> = Vec::new();
        for f in &futures_candidates {
            descs.push(format!(
                "  - future on {}: {}",
                f.contract.exchange, f.symbol
            ));
        }
        for o in &option_candidates {
            descs.push(format!(
                "  - {} option on {}: {}",
                o.kind, o.exchange, o.underlying_or_symbol
            ));
        }
        return Err(format!(
            "Ambiguous ticker '{ticker}' matched {total} instruments:\n{}\nPass exchange= to disambiguate.",
            descs.join("\n")
        ));
    }

    if let Some(mut f) = futures_candidates.into_iter().next() {
        if let Some(ref_date_str) = reference_date {
            let ref_date = coerce_reference_date(Some(ref_date_str));
            let cal = get_calendar(&f.contract.exchange);
            let sessions = cal.sessions_in_range(ref_date, ref_date);
            f.reference_date = Some(ref_date);
            f.is_trading_session = Some(!sessions.is_empty());
            let rule = spec
                .expiration_rules
                .get(&f.contract.expiration_rule)
                .ok_or_else(|| format!("unknown rule {}", f.contract.expiration_rule))?;
            let expiration = crate::expiration_rules::resolve_expiration(
                &f.contract,
                f.year,
                f.month,
                rule,
                &cal,
            )?;
            f.is_valid = Some(crate::ticker_generator::is_contract_tradeable(
                ref_date, expiration, rule, &cal,
            ));
        }
        return Ok(AnyParsedTicker::Futures(f));
    }
    if let Some(o) = option_candidates.into_iter().next() {
        return Ok(AnyParsedTicker::Option(o));
    }

    // Try bracket-tag root resolution (e.g. `DOL[1]`, `IND[-1]`).
    if let Some(result) = try_resolve_tagged_root(ticker, spec, reference_date, exchange)? {
        return Ok(AnyParsedTicker::Futures(result));
    }

    // Try root symbol resolution (futures only).
    // try_resolve_root_symbol already populates reference_date / is_trading_session.
    if let Some(result) = try_resolve_root_symbol(ticker, spec, reference_date)? {
        if let Some(ex) = exchange {
            if !result.contract.exchange.eq_ignore_ascii_case(ex) {
                return Err(format!("Unable to parse ticker: {ticker}"));
            }
        }
        return Ok(AnyParsedTicker::Futures(result));
    }

    Err(format!("Unable to parse ticker: {ticker}"))
}

/// Parse any ticker (futures or option) using the **bundled default spec**.
pub fn parse_any_ticker(ticker: &str) -> Result<AnyParsedTicker, String> {
    let spec = load_spec()?;
    parse_any_inner(ticker, &spec, None, None)
}

/// Parse any ticker with an explicit `reference_date` using the **bundled default spec**.
pub fn parse_any_ticker_date(ticker: &str, date: &str) -> Result<AnyParsedTicker, String> {
    let spec = load_spec()?;
    parse_any_inner(ticker, &spec, Some(date), None)
}

/// Parse any ticker using a **custom [`SpecRepository`]**.
pub fn parse_any_ticker_spec(
    ticker: &str,
    spec: &SpecRepository,
) -> Result<AnyParsedTicker, String> {
    parse_any_inner(ticker, spec, None, None)
}

/// Parse any ticker with an explicit `reference_date` and a **custom [`SpecRepository`]**.
pub fn parse_any_ticker_date_spec(
    ticker: &str,
    date: &str,
    spec: &SpecRepository,
) -> Result<AnyParsedTicker, String> {
    parse_any_inner(ticker, spec, Some(date), None)
}

/// Parse any ticker restricted to a single **exchange** (case-insensitive).
///
/// Useful when a ticker might match contracts on multiple markets.
pub fn parse_any_ticker_exchange(ticker: &str, exchange: &str) -> Result<AnyParsedTicker, String> {
    let spec = load_spec()?;
    parse_any_inner(ticker, &spec, None, Some(exchange))
}

// ---------------------------------------------------------------------------
// Lightweight classification (no calendars / validity / generator)
// ---------------------------------------------------------------------------

/// Asset class returned by [`classify_ticker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetType {
    /// Futures contract.
    Future,
    /// Option contract.
    Option,
    /// Cash equity.
    Equity,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Future => write!(f, "future"),
            AssetType::Option => write!(f, "option"),
            AssetType::Equity => write!(f, "equity"),
        }
    }
}

/// Call or put side for classified options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionSide {
    Call,
    Put,
}

impl std::fmt::Display for OptionSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionSide::Call => write!(f, "call"),
            OptionSide::Put => write!(f, "put"),
        }
    }
}

/// Lightweight ticker identity: type and root without calendars or validity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickerClass {
    pub asset_type: AssetType,
    pub root: String,
    pub exchange: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub option_type: Option<OptionSide>,
    pub strike: Option<String>,
    pub underlying: Option<String>,
}

fn classify_futures(ticker: &str, spec: &SpecRepository) -> Result<Vec<TickerClass>, String> {
    let mut results = Vec::new();
    for (re, contract) in &spec.pattern_index().futures {
        let Some(caps) = re.captures(ticker) else {
            continue;
        };
        let month_code: char = caps["month_code"].chars().next().unwrap();
        let month = code_to_month(month_code)?;
        let yy: i32 = caps["yy"]
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        results.push(TickerClass {
            asset_type: AssetType::Future,
            root: contract.symbol.clone(),
            exchange: Some(contract.exchange.clone()),
            year: Some(2000 + yy),
            month: Some(month),
            option_type: None,
            strike: None,
            underlying: None,
        });
    }
    Ok(results)
}

fn classify_options(ticker: &str, spec: &SpecRepository) -> Result<Vec<TickerClass>, String> {
    let index = spec.pattern_index();
    let mut results = Vec::new();

    for eq in &index.equity_options {
        let Some(caps) = eq.pattern.captures(ticker) else {
            continue;
        };
        let mc_char = caps["month_code"].chars().next().unwrap();
        let Some((month, is_call)) =
            equity_decode_month_code(mc_char, &eq.call_month_codes, &eq.put_month_codes)
        else {
            continue;
        };
        results.push(TickerClass {
            asset_type: AssetType::Option,
            root: eq.underlying.clone(),
            exchange: Some(eq.exchange.clone()),
            year: None,
            month: Some(month),
            option_type: Some(if is_call {
                OptionSide::Call
            } else {
                OptionSide::Put
            }),
            strike: Some(caps["strike"].to_string()),
            underlying: Some(eq.underlying.clone()),
        });
    }

    for ne in &index.nonequity_options {
        let Some(caps) = ne.pattern.captures(ticker) else {
            continue;
        };
        let month_code: char = caps["month_code"].chars().next().unwrap();
        let month = code_to_month(month_code)?;
        let yy: i32 = caps["yy"]
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        let opt_type_str = &caps["option_type"];
        let option_type = if opt_type_str == ne.option_type_codes.call {
            OptionSide::Call
        } else {
            OptionSide::Put
        };
        results.push(TickerClass {
            asset_type: AssetType::Option,
            root: ne.symbol.clone(),
            exchange: Some(ne.exchange.clone()),
            year: Some(2000 + yy),
            month: Some(month),
            option_type: Some(option_type),
            strike: Some(caps["strike"].to_string()),
            underlying: None,
        });
    }

    Ok(results)
}

fn pick_unique_class(ticker: &str, candidates: Vec<TickerClass>) -> Result<TickerClass, String> {
    match candidates.len() {
        1 => Ok(candidates.into_iter().next().unwrap()),
        0 => Err(format!("Unable to classify ticker: {ticker}")),
        n => {
            let detail: Vec<String> = candidates
                .iter()
                .map(|m| {
                    format!(
                        "  - {} on {}: {}",
                        m.asset_type,
                        m.exchange.as_deref().unwrap_or("?"),
                        m.root
                    )
                })
                .collect();
            Err(format!(
                "Ambiguous ticker '{ticker}' matched {n} instruments:\n{}\nPass exchange= to disambiguate.",
                detail.join("\n")
            ))
        }
    }
}

fn classify_inner(
    ticker: &str,
    spec: &SpecRepository,
    exchange: Option<&str>,
) -> Result<TickerClass, String> {
    let key = ticker.to_uppercase();
    if let Some(eq) = spec.equities.get(&key) {
        let matches_exchange = exchange
            .map(|ex| eq.exchange.eq_ignore_ascii_case(ex))
            .unwrap_or(true);
        if matches_exchange {
            return Ok(TickerClass {
                asset_type: AssetType::Equity,
                root: eq.symbol.clone(),
                exchange: Some(eq.exchange.clone()),
                year: None,
                month: None,
                option_type: None,
                strike: None,
                underlying: None,
            });
        }
    }

    let mut candidates = classify_futures(ticker, spec)?;
    candidates.extend(classify_options(ticker, spec)?);
    if let Some(ex) = exchange {
        candidates.retain(|c| {
            c.exchange
                .as_ref()
                .map(|e| e.eq_ignore_ascii_case(ex))
                .unwrap_or(false)
        });
    }
    if !candidates.is_empty() {
        return pick_unique_class(ticker, candidates);
    }

    if let Some((root, _, _)) = parse_tagged_root(ticker) {
        if let Some(contract) = spec.contracts.get(&root) {
            let matches_exchange = exchange
                .map(|ex| contract.exchange.eq_ignore_ascii_case(ex))
                .unwrap_or(true);
            if matches_exchange {
                return Ok(TickerClass {
                    asset_type: AssetType::Future,
                    root: contract.symbol.clone(),
                    exchange: Some(contract.exchange.clone()),
                    year: None,
                    month: None,
                    option_type: None,
                    strike: None,
                    underlying: None,
                });
            }
        }
        return Err(format!("Unable to classify ticker: {ticker}"));
    }

    if let Some(contract) = spec.contracts.get(&key) {
        let matches_exchange = exchange
            .map(|ex| contract.exchange.eq_ignore_ascii_case(ex))
            .unwrap_or(true);
        if matches_exchange {
            return Ok(TickerClass {
                asset_type: AssetType::Future,
                root: contract.symbol.clone(),
                exchange: Some(contract.exchange.clone()),
                year: None,
                month: None,
                option_type: None,
                strike: None,
                underlying: None,
            });
        }
    }

    Err(format!("Unable to classify ticker: {ticker}"))
}

/// Classify a ticker into asset type and root without calendars or validity.
///
/// Faster than [`parse_any_ticker`] for UI filters and routing: skips expiration
/// rules, trading calendars, front-month generation, and cycle-month checks.
pub fn classify_ticker(ticker: &str) -> Result<TickerClass, String> {
    let spec = load_spec()?;
    classify_inner(ticker, &spec, None)
}

/// Classify using a custom [`SpecRepository`].
pub fn classify_ticker_spec(ticker: &str, spec: &SpecRepository) -> Result<TickerClass, String> {
    classify_inner(ticker, spec, None)
}

/// Classify restricted to a single exchange (case-insensitive).
pub fn classify_ticker_exchange(ticker: &str, exchange: &str) -> Result<TickerClass, String> {
    let spec = load_spec()?;
    classify_inner(ticker, &spec, Some(exchange))
}

/// Classify with a custom spec and exchange filter.
pub fn classify_ticker_spec_exchange(
    ticker: &str,
    spec: &SpecRepository,
    exchange: &str,
) -> Result<TickerClass, String> {
    classify_inner(ticker, spec, Some(exchange))
}

// ===========================================================================
// Typestate builder
// ===========================================================================

/// Typestate marker: no ticker has been set on the builder.
pub struct NoTicker;
/// Typestate marker: a ticker has been set on the builder.
pub struct HasTicker;

/// Builder for [`TickerParser`] and one-shot parse operations.
///
/// The generic parameter `T` tracks whether a ticker has been supplied:
///
/// - [`NoTicker`] — only [`build()`](TickerParserBuilder::build) is available
///   (returns a reusable [`TickerParser`]).
/// - [`HasTicker`] — both [`build()`](TickerParserBuilder::build) and
///   [`parse()`](TickerParserBuilder::parse) are available.
///
/// Methods shared between both states: [`spec_path`](TickerParserBuilder::spec_path),
/// [`ticker`](TickerParserBuilder::ticker),
/// [`reference_date`](TickerParserBuilder::reference_date).
pub struct TickerParserBuilder<T = NoTicker> {
    spec_path: Option<PathBuf>,
    ticker: Option<String>,
    reference_date: Option<String>,
    exchange: Option<String>,
    _state: std::marker::PhantomData<T>,
}

// --- Methods available in any state -----------------------------------------

impl<T> TickerParserBuilder<T> {
    /// Set a custom spec directory.  When omitted the bundled default is used.
    pub fn spec_path(mut self, path: &Path) -> TickerParserBuilder<T> {
        self.spec_path = Some(path.to_path_buf());
        TickerParserBuilder {
            spec_path: self.spec_path,
            ticker: self.ticker,
            reference_date: self.reference_date,
            exchange: self.exchange,
            _state: std::marker::PhantomData,
        }
    }

    /// Set a custom spec directory from a string path.
    ///
    /// Convenience wrapper around [`spec_path`](Self::spec_path) for callers
    /// that already have a `&str`.
    pub fn spec(self, path: &str) -> TickerParserBuilder<T> {
        self.spec_path(Path::new(path))
    }

    /// Set a `reference_date` (`YYYY-MM-DD`) for root-symbol resolution.
    ///
    /// Only meaningful when the input is a root symbol; ignored for full
    /// tickers.
    pub fn reference_date(mut self, date: &str) -> TickerParserBuilder<T> {
        self.reference_date = Some(date.to_string());
        TickerParserBuilder {
            spec_path: self.spec_path,
            ticker: self.ticker,
            reference_date: self.reference_date,
            exchange: self.exchange,
            _state: std::marker::PhantomData,
        }
    }

    /// Restrict parsing to a specific exchange (case-insensitive, e.g. `"B3"` or `"CME"`).
    ///
    /// Returns an error if the ticker doesn't match any contract on that exchange.
    pub fn exchange(mut self, exchange: &str) -> TickerParserBuilder<T> {
        self.exchange = Some(exchange.to_string());
        TickerParserBuilder {
            spec_path: self.spec_path,
            ticker: self.ticker,
            reference_date: self.reference_date,
            exchange: self.exchange,
            _state: std::marker::PhantomData,
        }
    }

    /// Build a **reusable** [`TickerParser`].
    ///
    /// `ticker` and `reference_date` are ignored — use `parse()` on the
    /// resulting [`TickerParser`] instead.
    pub fn build(self) -> Result<TickerParser, String> {
        let spec = load_spec_for_builder(self.spec_path.as_deref())?;
        Ok(TickerParser { spec })
    }
}

// --- Transition from NoTicker → HasTicker -----------------------------------

impl TickerParserBuilder<NoTicker> {
    /// Set the ticker string, enabling one-shot [`parse()`](TickerParserBuilder::parse).
    pub fn ticker(self, ticker: &str) -> TickerParserBuilder<HasTicker> {
        TickerParserBuilder {
            spec_path: self.spec_path,
            ticker: Some(ticker.to_string()),
            reference_date: self.reference_date,
            exchange: self.exchange,
            _state: std::marker::PhantomData,
        }
    }
}

// --- Keeping HasTicker state when ticker is set again -----------------------

impl TickerParserBuilder<HasTicker> {
    /// Replace the ticker string (stays in [`HasTicker`] state).
    pub fn ticker(mut self, ticker: &str) -> TickerParserBuilder<HasTicker> {
        self.ticker = Some(ticker.to_string());
        TickerParserBuilder {
            spec_path: self.spec_path,
            ticker: self.ticker,
            reference_date: self.reference_date,
            exchange: self.exchange,
            _state: std::marker::PhantomData,
        }
    }

    /// **One-shot**: load spec, parse the ticker, and return the result.
    ///
    /// Supports futures and options; returns [`AnyParsedTicker`].
    pub fn parse(self) -> Result<AnyParsedTicker, String> {
        let ticker = self.ticker.expect("ticker is set in HasTicker state");
        let spec = load_spec_for_builder(self.spec_path.as_deref())?;
        parse_any_inner(
            &ticker,
            &spec,
            self.reference_date.as_deref(),
            self.exchange.as_deref(),
        )
    }
}

// ===========================================================================
// TickerParser (stateful wrapper)
// ===========================================================================

/// Reusable parser that holds a loaded [`SpecRepository`].
///
/// Create via [`TickerParser::new()`] (panics on spec failure),
/// [`TickerParser::try_new()`], or [`TickerParser::builder()`].
pub struct TickerParser {
    pub spec: SpecRepository,
}

impl Default for TickerParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TickerParser {
    /// Load the **bundled default spec** and return a ready parser.
    ///
    /// # Panics
    ///
    /// Panics if the bundled spec cannot be loaded.  For a fallible
    /// constructor use [`try_new()`](Self::try_new) or
    /// [`builder().build()`](TickerParserBuilder::build).
    pub fn new() -> Self {
        Self {
            spec: load_spec().expect("failed to load bundled spec"),
        }
    }

    /// Fallible constructor — loads the bundled default spec.
    pub fn try_new() -> Result<Self, String> {
        Ok(Self { spec: load_spec()? })
    }

    /// Load a spec from a custom path.
    pub fn with_spec_path(path: &Path) -> Result<Self, String> {
        Ok(Self {
            spec: load_spec_from_path(path)?,
        })
    }

    /// Load a spec from a custom path given as a string.
    ///
    /// Convenience wrapper around [`with_spec_path`](Self::with_spec_path).
    pub fn with_spec(path: &str) -> Result<Self, String> {
        Self::with_spec_path(Path::new(path))
    }

    /// Start a [`TickerParserBuilder`].
    pub fn builder() -> TickerParserBuilder<NoTicker> {
        TickerParserBuilder {
            spec_path: None,
            ticker: None,
            reference_date: None,
            exchange: None,
            _state: std::marker::PhantomData,
        }
    }

    /// Parse using the parser's spec; root symbols resolve for **today**.
    ///
    /// Supports futures and options; returns [`AnyParsedTicker`].
    pub fn parse(&self, ticker: &str) -> Result<AnyParsedTicker, String> {
        parse_any_inner(ticker, &self.spec, None, None)
    }

    /// Parse using the parser's spec with an optional exchange filter.
    ///
    /// Supports futures and options; returns [`AnyParsedTicker`].
    pub fn parse_exchange(&self, ticker: &str, exchange: &str) -> Result<AnyParsedTicker, String> {
        parse_any_inner(ticker, &self.spec, None, Some(exchange))
    }

    /// Parse using the parser's spec with an explicit `reference_date`.
    ///
    /// Supports futures and options; returns [`AnyParsedTicker`].
    pub fn parse_date(
        &self,
        ticker: &str,
        reference_date: &str,
    ) -> Result<AnyParsedTicker, String> {
        parse_any_inner(ticker, &self.spec, Some(reference_date), None)
    }
}

#[cfg(test)]
mod classify_unit_tests {
    use super::*;

    #[test]
    fn pick_unique_class_ambiguous_message() {
        let matches = vec![
            TickerClass {
                asset_type: AssetType::Future,
                root: "A".into(),
                exchange: Some("B3".into()),
                year: None,
                month: None,
                option_type: None,
                strike: None,
                underlying: None,
            },
            TickerClass {
                asset_type: AssetType::Option,
                root: "B".into(),
                exchange: Some("B3".into()),
                year: None,
                month: None,
                option_type: None,
                strike: None,
                underlying: None,
            },
        ];
        let err = pick_unique_class("FOO", matches).unwrap_err();
        assert!(err.contains("Ambiguous ticker"));
        assert!(err.contains("Pass exchange= to disambiguate."));
    }

    #[test]
    fn pick_unique_class_empty_raises() {
        let err = pick_unique_class("FOO", vec![]).unwrap_err();
        assert!(err.contains("Unable to classify"));
    }

    #[test]
    fn pattern_index_reused_across_classify() {
        let spec = load_spec().expect("spec");
        classify_ticker_spec("INDM26", &spec).expect("classify");
        let first = spec.pattern_index() as *const _;
        classify_ticker_spec("DOLN26", &spec).expect("classify");
        let second = spec.pattern_index() as *const _;
        assert_eq!(first, second);
    }
}
