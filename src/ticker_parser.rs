//! Futures ticker parsing.
//!
//! # Quick start
//!
//! ```rust,no_run
//! // Simplest — panics if bundled spec is broken (should never happen)
//! let parsed = tickerforge::TickerParser::new().parse("INDM26").unwrap();
//!
//! // Builder — reusable parser with custom spec
//! let parser = tickerforge::TickerParser::builder()
//!     .spec("/path/to/spec")
//!     .build()
//!     .unwrap();
//!
//! // Builder — one-shot parse
//! let parsed = tickerforge::TickerParser::builder()
//!     .ticker("IND")
//!     .reference_date("2026-06-01")
//!     .parse()
//!     .unwrap();
//! ```

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use regex::Regex;

use crate::calendars::get_calendar;
use crate::contract_cycle::resolve_contract_months;
use crate::models::{ParsedFuturesTicker, SpecRepository};
use crate::month_codes::code_to_month;
use crate::spec_loader::{load_spec, load_spec_from_path};
use crate::ticker_generator::generate_ticker_for_contract;

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
            lot_size: contract.contract_multiplier,
            contract: contract.clone(),
            reference_date: None,
            is_trading_session: None,
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

fn parse_ticker_inner(
    ticker: &str,
    spec: &SpecRepository,
    reference_date: Option<&str>,
) -> Result<ParsedFuturesTicker, String> {
    if let Some(result) = try_parse_full_ticker(ticker, spec)? {
        if reference_date.is_some() {
            eprintln!(
                "warning: reference_date is ignored for full ticker '{}'; \
                 year and month are derived directly from the ticker string",
                ticker
            );
        }
        return Ok(result);
    }

    if let Some(result) = try_resolve_root_symbol(ticker, spec, reference_date)? {
        return Ok(result);
    }

    Err(format!("Unable to parse ticker: {ticker}"))
}

fn load_spec_for_builder(spec_path: Option<&Path>) -> Result<SpecRepository, String> {
    match spec_path {
        Some(p) => load_spec_from_path(p),
        None => load_spec(),
    }
}

// ---------------------------------------------------------------------------
// Public free functions
// ---------------------------------------------------------------------------

/// Parse a full ticker or root symbol using the **bundled default spec**.
///
/// Root symbols resolve the front-month contract for **today**.
pub fn parse_ticker(ticker: &str) -> Result<ParsedFuturesTicker, String> {
    let spec = load_spec()?;
    parse_ticker_inner(ticker, &spec, None)
}

/// Parse a full ticker or root symbol using the **bundled default spec**
/// with an explicit `reference_date` (`YYYY-MM-DD`).
pub fn parse_ticker_date(
    ticker: &str,
    reference_date: &str,
) -> Result<ParsedFuturesTicker, String> {
    let spec = load_spec()?;
    parse_ticker_inner(ticker, &spec, Some(reference_date))
}

/// Parse a full ticker or root symbol using a **custom [`SpecRepository`]**.
pub fn parse_ticker_spec(
    ticker: &str,
    spec: &SpecRepository,
) -> Result<ParsedFuturesTicker, String> {
    parse_ticker_inner(ticker, spec, None)
}

/// Parse a full ticker or root symbol using a **custom [`SpecRepository`]**
/// and an explicit `reference_date` (`YYYY-MM-DD`).
pub fn parse_ticker_date_spec(
    ticker: &str,
    reference_date: &str,
    spec: &SpecRepository,
) -> Result<ParsedFuturesTicker, String> {
    parse_ticker_inner(ticker, spec, Some(reference_date))
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
            _state: std::marker::PhantomData,
        }
    }

    /// **One-shot**: load spec, parse the ticker, and return the result.
    pub fn parse(self) -> Result<ParsedFuturesTicker, String> {
        let ticker = self.ticker.expect("ticker is set in HasTicker state");
        let spec = load_spec_for_builder(self.spec_path.as_deref())?;
        parse_ticker_inner(&ticker, &spec, self.reference_date.as_deref())
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
            _state: std::marker::PhantomData,
        }
    }

    /// Parse using the parser's spec; root symbols resolve for **today**.
    pub fn parse(&self, ticker: &str) -> Result<ParsedFuturesTicker, String> {
        parse_ticker_inner(ticker, &self.spec, None)
    }

    /// Parse using the parser's spec with an explicit `reference_date`.
    pub fn parse_date(
        &self,
        ticker: &str,
        reference_date: &str,
    ) -> Result<ParsedFuturesTicker, String> {
        parse_ticker_inner(ticker, &self.spec, Some(reference_date))
    }
}
