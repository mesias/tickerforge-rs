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

use chrono::NaiveDate;
use regex::Regex;

use crate::calendars::get_calendar;
use crate::contract_cycle::resolve_contract_months;
use crate::models::{ParsedEquityTicker, ParsedFuturesTicker, ParsedOptionTicker, SpecRepository};
use crate::month_codes::code_to_month;
use crate::options_ticker::OptionParser;
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

fn pattern_for_contract(contract: &crate::models::ContractSpec) -> Result<Regex, String> {
    let mut escaped = regex::escape(&contract.ticker_format);
    escaped = escaped.replace("\\{symbol\\}", &regex::escape(&contract.symbol));
    escaped = escaped.replace("\\{month_code\\}", "(?P<month_code>[FGHJKMNQUVXZ])");
    escaped = escaped.replace("\\{yy\\}", "(?P<yy>\\d{2})");
    let pattern = format!("^{escaped}$");
    Regex::new(&pattern).map_err(|e| e.to_string())
}

fn try_parse_full_ticker(
    ticker: &str,
    spec: &SpecRepository,
) -> Result<Option<ParsedFuturesTicker>, String> {
    for contract in spec.contracts.values() {
        let re = pattern_for_contract(contract)?;
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
    }
    Ok(result)
}

fn load_spec_for_builder(spec_path: Option<&Path>) -> Result<SpecRepository, String> {
    match spec_path {
        Some(p) => load_spec_from_path(p),
        None => load_spec(),
    }
}

/// If `ticker` matches the `SYMBOL[n]` bracket-tag syntax, return the
/// uppercased root and the signed offset.  Returns `None` for full tickers
/// (`DOLN26`), plain roots (`DOL`), and anything else without a `[n]` tag.
fn parse_tagged_root(ticker: &str) -> Option<(String, isize)> {
    static TAG_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = TAG_RE.get_or_init(|| {
        Regex::new(r"^([A-Za-z][A-Za-z0-9]*)\[(-?\d+)]$").expect("valid tag regex")
    });
    let caps = re.captures(ticker)?;
    let root = caps[1].to_uppercase();
    let offset: isize = caps[2].parse().ok()?;
    Some((root, offset))
}

/// Resolve a `SYMBOL[n]` tagged root to a parsed futures ticker.
fn try_resolve_tagged_root(
    ticker: &str,
    spec: &SpecRepository,
    reference_date: Option<&str>,
    exchange: Option<&str>,
) -> Result<Option<ParsedFuturesTicker>, String> {
    let Some((root, offset)) = parse_tagged_root(ticker) else {
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
    let date_str = ref_date.format("%Y-%m-%d").to_string();
    let full_ticker = generate_ticker_for_contract_signed(contract, &date_str, spec, offset)?;
    let mut result = try_parse_full_ticker(&full_ticker, spec)?;
    if let Some(ref mut parsed) = result {
        let cal = get_calendar(&contract.exchange);
        let sessions = cal.sessions_in_range(ref_date, ref_date);
        parsed.reference_date = Some(ref_date);
        parsed.is_trading_session = Some(!sessions.is_empty());
        parsed.contract_offset = Some(offset);
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
    for contract in spec.contracts.values() {
        if let Some(ex) = exchange {
            if !contract.exchange.eq_ignore_ascii_case(ex) {
                continue;
            }
        }
        let re = pattern_for_contract(contract)?;
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

    if let Some(f) = futures_candidates.into_iter().next() {
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
