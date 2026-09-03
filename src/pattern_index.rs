//! Precompiled futures/options regexes for one [`SpecRepository`](crate::models::SpecRepository).

use regex::Regex;

use crate::models::ContractSpec;
use crate::options_models::{OptionRule, OptionTypeCodes};
use crate::options_ticker::equity_root;

const FUTURES_MONTH_CODES: &str = "FGHJKMNQUVXZ";

/// Precompiled futures and options patterns for fast classify/parse matching.
#[derive(Clone)]
pub struct PatternIndex {
    pub futures: Vec<(Regex, ContractSpec)>,
    pub equity_options: Vec<EquityOptionPattern>,
    pub nonequity_options: Vec<NonequityOptionPattern>,
}

impl std::fmt::Debug for PatternIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatternIndex")
            .field("futures", &self.futures.len())
            .field("equity_options", &self.equity_options.len())
            .field("nonequity_options", &self.nonequity_options.len())
            .finish()
    }
}

/// Equity option regex + underlying metadata.
#[derive(Clone)]
pub struct EquityOptionPattern {
    pub pattern: Regex,
    pub underlying: String,
    pub exchange: String,
    pub call_month_codes: Vec<String>,
    pub put_month_codes: Vec<String>,
    pub tick_size: Option<f64>,
    pub ctr_std: Option<u32>,
    pub ctr_size: Option<f64>,
}

/// Non-equity option regex + rule metadata.
#[derive(Clone)]
pub struct NonequityOptionPattern {
    pub pattern: Regex,
    pub kind: String,
    pub symbol: String,
    pub exchange: String,
    pub option_type_codes: OptionTypeCodes,
    pub tick_size: Option<f64>,
    pub ctr_std: Option<u32>,
    pub ctr_size: Option<f64>,
}

/// Build a futures ticker regex from a contract's `ticker_format`.
pub fn pattern_for_contract(contract: &ContractSpec) -> Result<Regex, String> {
    let mut escaped = regex::escape(&contract.ticker_format);
    escaped = escaped.replace("\\{symbol\\}", &regex::escape(&contract.symbol));
    escaped = escaped.replace("\\{month_code\\}", "(?P<month_code>[FGHJKMNQUVXZ])");
    escaped = escaped.replace("\\{yy\\}", "(?P<yy>\\d{2})");
    let pattern = format!("^{escaped}$");
    Regex::new(&pattern).map_err(|e| e.to_string())
}

fn equity_all_codes(call_codes: &[String], put_codes: &[String]) -> String {
    let mut s = String::new();
    for c in call_codes.iter().chain(put_codes.iter()) {
        if let Some(ch) = c.chars().next() {
            s.push(ch);
        }
    }
    s
}

fn patterns_for_equity_option(
    rule: &crate::options_models::EquityOptionRule,
) -> Vec<EquityOptionPattern> {
    if rule.underlyings.is_empty() {
        return Vec::new();
    }
    let all_codes = equity_all_codes(&rule.call_month_codes, &rule.put_month_codes);
    let codes_escaped = regex::escape(&all_codes);
    let mut out = Vec::new();
    for underlying in &rule.underlyings {
        let root = equity_root(underlying);
        let pattern = format!(
            "^{}(?P<month_code>[{codes_escaped}])(?P<strike>\\d+)$",
            regex::escape(&root)
        );
        let Ok(re) = Regex::new(&pattern) else {
            continue;
        };
        out.push(EquityOptionPattern {
            pattern: re,
            underlying: underlying.clone(),
            exchange: rule.exchange.clone(),
            call_month_codes: rule.call_month_codes.clone(),
            put_month_codes: rule.put_month_codes.clone(),
            tick_size: rule.tick_size,
            ctr_std: rule.ctr_std,
            ctr_size: rule.ctr_size,
        });
    }
    out
}

fn pattern_for_nonequity_option(
    kind: &str,
    symbol: &str,
    exchange: &str,
    opt_codes: &OptionTypeCodes,
    tick_size: Option<f64>,
    ctr_std: Option<u32>,
    ctr_size: Option<f64>,
) -> Option<NonequityOptionPattern> {
    let call_esc = regex::escape(&opt_codes.call);
    let put_esc = regex::escape(&opt_codes.put);
    let pattern = format!(
        "^{}(?P<month_code>[{FUTURES_MONTH_CODES}])(?P<yy>\\d{{2}})(?P<option_type>{call_esc}|{put_esc})(?P<strike>\\d+)$",
        regex::escape(symbol)
    );
    let re = Regex::new(&pattern).ok()?;
    Some(NonequityOptionPattern {
        pattern: re,
        kind: kind.to_string(),
        symbol: symbol.to_string(),
        exchange: exchange.to_string(),
        option_type_codes: opt_codes.clone(),
        tick_size,
        ctr_std,
        ctr_size,
    })
}

/// Build a full pattern index for `spec` (expensive; callers should cache via OnceLock).
pub fn build_pattern_index(spec: &crate::models::SpecRepository) -> PatternIndex {
    let mut futures = Vec::with_capacity(spec.contracts.len());
    for contract in spec.contracts.values() {
        if let Ok(re) = pattern_for_contract(contract) {
            futures.push((re, contract.clone()));
        }
    }

    let mut equity_options = Vec::new();
    let mut nonequity_options = Vec::new();
    for rule in &spec.options {
        match rule {
            OptionRule::Equity(r) => {
                equity_options.extend(patterns_for_equity_option(r));
            }
            OptionRule::Index(r) => {
                if let Some(entry) = pattern_for_nonequity_option(
                    "index",
                    &r.symbol,
                    &r.exchange,
                    &r.option_type_codes,
                    r.tick_size,
                    r.ctr_std,
                    r.ctr_size,
                ) {
                    nonequity_options.push(entry);
                }
            }
            OptionRule::Dollar(r) => {
                if let Some(entry) = pattern_for_nonequity_option(
                    "dollar",
                    &r.symbol,
                    &r.exchange,
                    &r.option_type_codes,
                    r.tick_size,
                    r.ctr_std,
                    r.ctr_size,
                ) {
                    nonequity_options.push(entry);
                }
            }
            OptionRule::InterestRate(r) => {
                if let Some(entry) = pattern_for_nonequity_option(
                    "interest_rate",
                    &r.symbol,
                    &r.exchange,
                    &r.option_type_codes,
                    r.tick_size,
                    r.ctr_std,
                    r.ctr_size,
                ) {
                    nonequity_options.push(entry);
                }
            }
        }
    }

    PatternIndex {
        futures,
        equity_options,
        nonequity_options,
    }
}

/// Map an equity option month code character back to `(month, is_call)`.
pub fn equity_decode_month_code(
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
