//! Integration tests for option ticker parsing and multi-market `parse_any_ticker*`.
//!
//! Mirrors `tests/test_option_parsing.py` in tickerforge-py.

use tickerforge::{
    load_spec, parse_any_ticker, parse_any_ticker_exchange, parse_any_ticker_spec, AnyParsedTicker,
    OptionParser, ParsedOptionTicker,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unwrap_option(any: AnyParsedTicker) -> ParsedOptionTicker {
    match any {
        AnyParsedTicker::Option(o) => o,
        AnyParsedTicker::Futures(f) => panic!("expected Option, got Futures: {:?}", f),
    }
}

fn unwrap_futures_symbol(any: AnyParsedTicker) -> String {
    match any {
        AnyParsedTicker::Futures(f) => f.symbol,
        AnyParsedTicker::Option(o) => panic!("expected Futures, got Option: {:?}", o),
    }
}

// ===========================================================================
// Spec loading
// ===========================================================================

#[test]
fn spec_loads_options_from_multiple_markets() {
    let spec = load_spec().expect("spec");
    assert!(
        !spec.options.is_empty(),
        "expected at least one option rule"
    );
}

#[test]
fn spec_options_include_equity_type() {
    let spec = load_spec().expect("spec");
    let has_equity = spec
        .options
        .iter()
        .any(|r| matches!(r, tickerforge::options_models::OptionRule::Equity(_)));
    assert!(has_equity, "expected at least one equity option rule");
}

#[test]
fn spec_options_include_index_type() {
    let spec = load_spec().expect("spec");
    let has_index = spec
        .options
        .iter()
        .any(|r| matches!(r, tickerforge::options_models::OptionRule::Index(_)));
    assert!(has_index, "expected at least one index option rule");
}

// ===========================================================================
// B3 Equity Options
// ===========================================================================

#[test]
fn parse_equity_call_option() {
    // equity_root("PETR4") = "PETR", call code A = January → "PETRA30"
    let o = unwrap_option(parse_any_ticker("PETRA30").expect("parse"));
    assert_eq!(o.kind, "equity");
    assert!(o.is_call);
    assert_eq!(o.underlying_or_symbol, "PETR4");
    assert_eq!(o.month, 1);
    assert!(o.year.is_none(), "equity options have no year");
    assert_eq!(o.strike, "30");
    assert_eq!(o.exchange, "B3");
}

#[test]
fn parse_equity_put_option() {
    // equity_root("PETR4") = "PETR", put code M = January → "PETRM30"
    let o = unwrap_option(parse_any_ticker("PETRM30").expect("parse"));
    assert_eq!(o.kind, "equity");
    assert!(!o.is_call);
    assert_eq!(o.underlying_or_symbol, "PETR4");
    assert_eq!(o.month, 1);
    assert_eq!(o.strike, "30");
    assert_eq!(o.exchange, "B3");
}

#[test]
fn parse_equity_call_june() {
    // equity_root("VALE3") = "VALE", call code F = June → "VALEF50"
    let o = unwrap_option(parse_any_ticker("VALEF50").expect("parse"));
    assert!(o.is_call);
    assert_eq!(o.underlying_or_symbol, "VALE3");
    assert_eq!(o.month, 6);
    assert_eq!(o.strike, "50");
}

#[test]
fn parse_equity_put_december() {
    // equity_root("ITUB4") = "ITUB", put code X = December → "ITUBX25"
    let o = unwrap_option(parse_any_ticker("ITUBX25").expect("parse"));
    assert!(!o.is_call);
    assert_eq!(o.underlying_or_symbol, "ITUB4");
    assert_eq!(o.month, 12);
    assert_eq!(o.strike, "25");
}

#[test]
fn parse_equity_option_tick_and_lot() {
    let o = unwrap_option(parse_any_ticker("PETRA30").expect("parse"));
    assert_eq!(o.tick_size, Some(0.01));
    assert_eq!(o.lot_size, Some(1.0));
}

// ===========================================================================
// B3 Index Options (IBOV)
// ===========================================================================

#[test]
fn parse_ibov_call_option() {
    // K = May (futures code), 2026 → IBOVK26C120000
    let o = unwrap_option(parse_any_ticker("IBOVK26C120000").expect("parse"));
    assert_eq!(o.kind, "index");
    assert!(o.is_call);
    assert_eq!(o.underlying_or_symbol, "IBOV");
    assert_eq!(o.month, 5);
    assert_eq!(o.year, Some(2026));
    assert_eq!(o.strike, "120000");
    assert_eq!(o.exchange, "B3");
}

#[test]
fn parse_ibov_put_option() {
    let o = unwrap_option(parse_any_ticker("IBOVK26P100000").expect("parse"));
    assert_eq!(o.kind, "index");
    assert!(!o.is_call);
    assert_eq!(o.underlying_or_symbol, "IBOV");
    assert_eq!(o.month, 5);
    assert_eq!(o.year, Some(2026));
    assert_eq!(o.strike, "100000");
}

// ===========================================================================
// B3 Dollar Options
// ===========================================================================

#[test]
fn parse_dol_call_option() {
    // K = May
    let o = unwrap_option(parse_any_ticker("DOLK26C5000").expect("parse"));
    assert_eq!(o.kind, "dollar");
    assert!(o.is_call);
    assert_eq!(o.underlying_or_symbol, "DOL");
    assert_eq!(o.month, 5);
    assert_eq!(o.year, Some(2026));
    assert_eq!(o.strike, "5000");
    assert_eq!(o.exchange, "B3");
}

#[test]
fn parse_dol_put_option() {
    let o = unwrap_option(parse_any_ticker("DOLK26P4800").expect("parse"));
    assert!(!o.is_call);
    assert_eq!(o.underlying_or_symbol, "DOL");
    assert_eq!(o.strike, "4800");
}

// ===========================================================================
// B3 Interest Rate Options (IDI)
// ===========================================================================

#[test]
fn parse_idi_call_option() {
    // F = January
    let o = unwrap_option(parse_any_ticker("IDIF26C100000").expect("parse"));
    assert_eq!(o.kind, "interest_rate");
    assert!(o.is_call);
    assert_eq!(o.underlying_or_symbol, "IDI");
    assert_eq!(o.month, 1);
    assert_eq!(o.year, Some(2026));
    assert_eq!(o.strike, "100000");
    assert_eq!(o.exchange, "B3");
}

#[test]
fn parse_idi_put_option() {
    let o = unwrap_option(parse_any_ticker("IDIF26P95000").expect("parse"));
    assert!(!o.is_call);
    assert_eq!(o.underlying_or_symbol, "IDI");
    assert_eq!(o.strike, "95000");
}

// ===========================================================================
// Futures still work via parse_any_ticker (backward compat)
// ===========================================================================

#[test]
fn futures_still_parse_via_any() {
    let sym = unwrap_futures_symbol(parse_any_ticker("INDM26").expect("parse"));
    assert_eq!(sym, "IND");
}

#[test]
fn cme_futures_parse_via_any() {
    let any = parse_any_ticker("ESM26").expect("parse");
    match any {
        AnyParsedTicker::Futures(f) => {
            assert_eq!(f.symbol, "ES");
            assert_eq!(f.year, 2026);
            assert_eq!(f.month, 6);
            assert_eq!(f.contract.exchange, "CME");
        }
        AnyParsedTicker::Option(_) => panic!("expected Futures"),
    }
}

// ===========================================================================
// DOL disambiguation: future vs option
// ===========================================================================

#[test]
fn dol_future_not_ambiguous_with_option() {
    // DOLK26 is a complete futures ticker — no option suffix
    let any = parse_any_ticker("DOLK26").expect("parse");
    assert!(
        matches!(any, AnyParsedTicker::Futures(_)),
        "DOLK26 should be a futures ticker"
    );
}

#[test]
fn dol_option_not_ambiguous_with_future() {
    // DOLK26C5000 has the option suffix — must parse as option
    let any = parse_any_ticker("DOLK26C5000").expect("parse");
    assert!(
        matches!(any, AnyParsedTicker::Option(_)),
        "DOLK26C5000 should be an option"
    );
}

// ===========================================================================
// Exchange filter
// ===========================================================================

#[test]
fn exchange_filter_future_b3() {
    let any = parse_any_ticker_exchange("INDM26", "B3").expect("parse");
    assert!(matches!(any, AnyParsedTicker::Futures(_)));
}

#[test]
fn exchange_filter_future_cme() {
    let any = parse_any_ticker_exchange("ESM26", "CME").expect("parse");
    assert!(matches!(any, AnyParsedTicker::Futures(_)));
}

#[test]
fn exchange_filter_excludes_wrong_market() {
    let result = parse_any_ticker_exchange("ESM26", "B3");
    assert!(result.is_err(), "ESM26 on B3 should fail");
    assert!(result.unwrap_err().contains("Unable to parse ticker"));
}

#[test]
fn exchange_filter_on_option() {
    let o = unwrap_option(parse_any_ticker_exchange("PETRA30", "B3").expect("parse"));
    assert_eq!(o.exchange, "B3");
    assert_eq!(o.kind, "equity");
}

#[test]
fn exchange_filter_option_wrong_market_errors() {
    let result = parse_any_ticker_exchange("PETRA30", "CME");
    assert!(result.is_err());
}

// ===========================================================================
// Unknown tickers
// ===========================================================================

#[test]
fn unknown_ticker_errors() {
    let result = parse_any_ticker("ZZZZ");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unable to parse ticker"));
}

#[test]
fn unknown_option_format_errors() {
    let result = parse_any_ticker("FAKEA99");
    assert!(result.is_err());
}

// ===========================================================================
// OptionParser low-level API
// ===========================================================================

#[test]
fn option_parser_parse_option_equity() {
    let spec = load_spec().expect("spec");
    let o = OptionParser::parse_option("PETRA30", &spec).expect("parse");
    assert_eq!(o.kind, "equity");
    assert_eq!(o.underlying_or_symbol, "PETR4");
    assert!(o.is_call);
    assert_eq!(o.month, 1);
}

#[test]
fn option_parser_parse_option_exchange_filter() {
    let spec = load_spec().expect("spec");
    let o = OptionParser::parse_option_exchange("DOLK26C5000", &spec, Some("B3")).expect("parse");
    assert_eq!(o.kind, "dollar");
    assert_eq!(o.exchange, "B3");
}

#[test]
fn option_parser_unknown_returns_error() {
    let spec = load_spec().expect("spec");
    let result = OptionParser::parse_option("ZZZA99", &spec);
    assert!(result.is_err());
}

// ===========================================================================
// parse_any_ticker_spec
// ===========================================================================

#[test]
fn parse_any_ticker_spec_futures() {
    let spec = load_spec().expect("spec");
    let any = parse_any_ticker_spec("WINM26", &spec).expect("parse");
    assert!(matches!(any, AnyParsedTicker::Futures(_)));
}

#[test]
fn parse_any_ticker_spec_option() {
    let spec = load_spec().expect("spec");
    let any = parse_any_ticker_spec("IBOVK26C120000", &spec).expect("parse");
    assert!(matches!(any, AnyParsedTicker::Option(_)));
}
