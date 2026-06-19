mod common;

use chrono::NaiveDate;
use tickerforge::options_models::OptionRule;
use tickerforge::{
    load_spec, parse_any_ticker, parse_any_ticker_date, parse_any_ticker_date_spec,
    parse_any_ticker_exchange, parse_any_ticker_spec, AnyParsedTicker, ParsedFuturesTicker,
    TickerForge, TickerParser,
};

use crate::common::spec_path;

/// Unwrap `AnyParsedTicker::Futures` from the unified enum.
/// Panics (with a helpful message) if the result is an option.
fn as_futures(any: AnyParsedTicker) -> ParsedFuturesTicker {
    match any {
        AnyParsedTicker::Futures(f) => f,
        AnyParsedTicker::Option(o) => {
            panic!("expected Futures but got Option: {:?}", o)
        }
        AnyParsedTicker::Equity(e) => {
            panic!("expected Futures but got Equity: {:?}", e)
        }
    }
}

// ===========================================================================
// Existing tests (adapted to new API)
// ===========================================================================

#[test]
fn parse_ind_ticker() {
    let parser = TickerParser::with_spec_path(&spec_path()).expect("parser");
    let parsed = as_futures(parser.parse("INDM26").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.month, 6);
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.contract.exchange, "B3");
}

#[test]
fn generate_and_parse_round_trip() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let parser = TickerParser::with_spec_path(&spec_path()).expect("parser");
    let generated = forge.generate("IND", "2026-06-01", 0).expect("gen");
    let parsed = as_futures(parser.parse_date(&generated, "2026-06-01").expect("parse"));
    assert_eq!(generated, "INDM26");
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn parse_invalid_ticker_errors() {
    let parser = TickerParser::with_spec_path(&spec_path()).expect("parser");
    assert!(parser.parse("INVALID").is_err());
}

// ===========================================================================
// Free functions: parse_any_ticker
// ===========================================================================

#[test]
fn parse_ticker_full_ind() {
    let parsed = as_futures(parse_any_ticker("INDM26").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn parse_ticker_full_dol() {
    let parsed = as_futures(parse_any_ticker("DOLK26").expect("parse"));
    assert_eq!(parsed.symbol, "DOL");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 5);
}

#[test]
fn parse_ticker_full_win() {
    let parsed = as_futures(parse_any_ticker("WINM26").expect("parse"));
    assert_eq!(parsed.symbol, "WIN");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn parse_ticker_root_symbol_uses_today() {
    let parsed = as_futures(parse_any_ticker("IND").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn parse_ticker_unknown_errors() {
    let result = parse_any_ticker("ZZZZ");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unable to parse ticker"));
}

// ===========================================================================
// Free functions: parse_any_ticker_date
// ===========================================================================

#[test]
fn parse_ticker_date_full_ignores_date() {
    let parsed = as_futures(parse_any_ticker_date("INDM26", "1990-01-01").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn parse_ticker_date_root_ind() {
    let parsed = as_futures(parse_any_ticker_date("IND", "2026-06-01").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn parse_ticker_date_root_dol() {
    let parsed = as_futures(parse_any_ticker_date("DOL", "2026-04-15").expect("parse"));
    assert_eq!(parsed.symbol, "DOL");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn parse_ticker_date_root_win() {
    let parsed = as_futures(parse_any_ticker_date("WIN", "2026-04-15").expect("parse"));
    assert_eq!(parsed.symbol, "WIN");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

// ===========================================================================
// Free functions: parse_any_ticker_spec / parse_any_ticker_date_spec
// ===========================================================================

#[test]
fn parse_ticker_spec_full() {
    let spec = load_spec().expect("spec");
    let parsed = as_futures(parse_any_ticker_spec("INDM26", &spec).expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn parse_ticker_spec_root() {
    let spec = load_spec().expect("spec");
    let parsed = as_futures(parse_any_ticker_spec("IND", &spec).expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn parse_ticker_spec_unknown_errors() {
    let spec = load_spec().expect("spec");
    let result = parse_any_ticker_spec("ZZZZ", &spec);
    assert!(result.is_err());
}

#[test]
fn parse_ticker_date_spec_full() {
    let spec = load_spec().expect("spec");
    let parsed =
        as_futures(parse_any_ticker_date_spec("DOLK26", "2026-04-15", &spec).expect("parse"));
    assert_eq!(parsed.symbol, "DOL");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 5);
}

#[test]
fn parse_ticker_date_spec_root() {
    let spec = load_spec().expect("spec");
    let parsed = as_futures(parse_any_ticker_date_spec("IND", "2026-06-01", &spec).expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn parse_ticker_date_spec_unknown_errors() {
    let spec = load_spec().expect("spec");
    let result = parse_any_ticker_date_spec("ZZZZ", "2026-01-01", &spec);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unable to parse ticker"));
}

// ===========================================================================
// tick_size / lot_size
// ===========================================================================

#[test]
fn parsed_ticker_has_tick_size_and_lot_size() {
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("IND").expect("contract");
    let parsed = as_futures(parse_any_ticker_spec("INDM26", &spec).expect("parse"));
    assert_eq!(parsed.tick_size, contract.tick_size);
    assert_eq!(parsed.lot_size, contract.lot_size);
}

// ===========================================================================
// TickerParser — new() (panicking), try_new(), parse, parse_date
// ===========================================================================

#[test]
fn ticker_parser_new_panicking() {
    let parser = TickerParser::new();
    let parsed = as_futures(parser.parse("INDM26").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn ticker_parser_try_new() {
    let parser = TickerParser::try_new().expect("parser");
    let parsed = as_futures(parser.parse("IND").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn ticker_parser_parse_date_root() {
    let parser = TickerParser::new();
    let parsed = as_futures(parser.parse_date("IND", "2026-06-01").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn ticker_parser_parse_date_full_ignores_date() {
    let parser = TickerParser::new();
    let parsed = as_futures(parser.parse_date("INDM26", "1990-01-01").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

// ===========================================================================
// Builder → build() → reusable TickerParser
// ===========================================================================

#[test]
fn builder_build_default_spec() {
    let parser = TickerParser::builder().build().expect("build");
    let parsed = as_futures(parser.parse("INDM26").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
}

#[test]
fn builder_build_custom_spec() {
    let parser = TickerParser::builder()
        .spec_path(&spec_path())
        .build()
        .expect("build");
    let parsed = as_futures(parser.parse("DOLK26").expect("parse"));
    assert_eq!(parsed.symbol, "DOL");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 5);
}

// ===========================================================================
// Builder → parse() — one-shot
// ===========================================================================

#[test]
fn builder_parse_full_ticker() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("INDM26")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn builder_parse_root_with_date() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("IND")
            .reference_date("2026-06-01")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn builder_parse_root_without_date() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("DOL")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "DOL");
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn builder_parse_custom_spec() {
    let parsed = as_futures(
        TickerParser::builder()
            .spec_path(&spec_path())
            .ticker("WINM26")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "WIN");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn builder_parse_custom_spec_with_date() {
    let parsed = as_futures(
        TickerParser::builder()
            .spec_path(&spec_path())
            .ticker("IND")
            .reference_date("2026-06-01")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert!(parsed.month >= 1 && parsed.month <= 12);
}

#[test]
fn builder_parse_unknown_errors() {
    let result = TickerParser::builder().ticker("ZZZZ").parse();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unable to parse ticker"));
}

#[test]
fn builder_parse_full_ignores_date() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("INDM26")
            .reference_date("1990-01-01")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn builder_date_before_ticker() {
    let parsed = as_futures(
        TickerParser::builder()
            .reference_date("2026-06-01")
            .ticker("IND")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
}

// ===========================================================================
// spec() / with_spec() — convenience &str overloads
// ===========================================================================

#[test]
fn builder_build_custom_spec_str() {
    let path = spec_path();
    let parser = TickerParser::builder()
        .spec(path.to_str().unwrap())
        .build()
        .expect("build");
    let parsed = as_futures(parser.parse("DOLK26").expect("parse"));
    assert_eq!(parsed.symbol, "DOL");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 5);
}

#[test]
fn builder_parse_custom_spec_str() {
    let path = spec_path();
    let parsed = as_futures(
        TickerParser::builder()
            .spec(path.to_str().unwrap())
            .ticker("WINM26")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "WIN");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn ticker_parser_with_spec() {
    let path = spec_path();
    let parser = TickerParser::with_spec(path.to_str().unwrap()).expect("parser");
    let parsed = as_futures(parser.parse("INDM26").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

// ===========================================================================
// Warnings: full ticker + reference_date
// ===========================================================================

#[test]
fn parse_full_ticker_with_date_ignores_date() {
    let parsed = as_futures(parse_any_ticker_date("WINQ25", "2030-01-01").expect("parse"));
    assert_eq!(parsed.symbol, "WIN");
    assert_eq!(parsed.year, 2025);
    assert_eq!(parsed.month, 8);
}

#[test]
fn builder_parse_full_ticker_with_date_warns_but_succeeds() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("WINQ25")
            .reference_date("2030-01-01")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "WIN");
    assert_eq!(parsed.year, 2025);
    assert_eq!(parsed.month, 8);
}

// ===========================================================================
// is_trading_session / reference_date
// ===========================================================================

#[test]
fn full_ticker_has_no_session_info() {
    let parsed = as_futures(parse_any_ticker("INDM26").expect("parse"));
    assert!(parsed.reference_date.is_none());
    assert!(parsed.is_trading_session.is_none());
}

#[test]
fn root_symbol_on_weekday_is_trading_session() {
    // 2026-04-15 is a Wednesday — B3 session
    let parsed = as_futures(parse_any_ticker_date("IND", "2026-04-15").expect("parse"));
    assert_eq!(
        parsed.reference_date,
        Some(NaiveDate::from_ymd_opt(2026, 4, 15).unwrap())
    );
    assert_eq!(parsed.is_trading_session, Some(true));
}

#[test]
fn root_symbol_on_weekend_is_not_trading_session() {
    // 2026-04-18 is a Saturday
    let parsed = as_futures(parse_any_ticker_date("IND", "2026-04-18").expect("parse"));
    assert_eq!(
        parsed.reference_date,
        Some(NaiveDate::from_ymd_opt(2026, 4, 18).unwrap())
    );
    assert_eq!(parsed.is_trading_session, Some(false));
}

#[test]
fn root_symbol_on_holiday_is_not_trading_session() {
    // 2026-04-21 is Tiradentes (B3 holiday, a Tuesday)
    let parsed = as_futures(parse_any_ticker_date("IND", "2026-04-21").expect("parse"));
    assert_eq!(
        parsed.reference_date,
        Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap())
    );
    assert_eq!(parsed.is_trading_session, Some(false));
}

#[test]
fn root_symbol_without_date_has_session_info() {
    let parsed = as_futures(parse_any_ticker("IND").expect("parse"));
    assert!(parsed.reference_date.is_some());
    assert!(parsed.is_trading_session.is_some());
}

#[test]
fn builder_root_symbol_session_info() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("DOL")
            .reference_date("2026-04-15")
            .parse()
            .expect("parse"),
    );
    assert_eq!(
        parsed.reference_date,
        Some(NaiveDate::from_ymd_opt(2026, 4, 15).unwrap())
    );
    assert_eq!(parsed.is_trading_session, Some(true));
}

#[test]
fn builder_full_ticker_no_session_info() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("DOLK26")
            .parse()
            .expect("parse"),
    );
    assert!(parsed.reference_date.is_none());
    assert!(parsed.is_trading_session.is_none());
}

#[test]
fn ticker_parser_default_matches_new() {
    let a = as_futures(TickerParser::default().parse("INDM26").expect("parse"));
    let b = as_futures(TickerParser::new().parse("INDM26").expect("parse"));
    assert_eq!(a.symbol, b.symbol);
    assert_eq!(a.year, b.year);
    assert_eq!(a.month, b.month);
}

#[test]
fn parse_exchange_on_parser_matches_freestanding() {
    let parser = TickerParser::new();
    let a = parser.parse_exchange("INDM26", "B3").expect("parse");
    let b = parse_any_ticker_exchange("INDM26", "B3").expect("parse");
    assert!(matches!(a, AnyParsedTicker::Futures(_)));
    assert!(matches!(b, AnyParsedTicker::Futures(_)));
}

#[test]
fn parse_any_ticker_date_invalid_reference_falls_back_to_local_date() {
    // Malformed date string: coerce_reference_date ignores it and uses "today"
    let parsed = as_futures(parse_any_ticker_date("IND", "not-a-valid-date").expect("parse"));
    assert_eq!(parsed.symbol, "IND");
}

#[test]
fn parse_any_ticker_exchange_root_wrong_exchange_fails() {
    let err = parse_any_ticker_exchange("IND", "CME").unwrap_err();
    assert!(err.contains("Unable to parse ticker"));
}

#[test]
fn builder_exchange_filters_root_symbol() {
    let parsed = as_futures(
        TickerParser::builder()
            .exchange("B3")
            .ticker("IND")
            .reference_date("2026-06-01")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.contract.exchange, "B3");
}

#[test]
fn builder_has_ticker_replace_ticker() {
    let parsed = as_futures(
        TickerParser::builder()
            .ticker("ZZZZ")
            .ticker("INDM26")
            .parse()
            .expect("parse"),
    );
    assert_eq!(parsed.symbol, "IND");
}

#[test]
fn ambiguous_ticker_when_two_equities_share_root() {
    let mut spec = load_spec().expect("spec");
    for rule in &mut spec.options {
        if let OptionRule::Equity(e) = rule {
            e.underlyings.push("PETR3".to_string());
            break;
        }
    }
    let err = parse_any_ticker_spec("PETRA30", &spec).unwrap_err();
    assert!(err.contains("Ambiguous ticker"));
    assert!(err.contains("Pass exchange="));
}

#[test]
fn parse_new_b3_futures() {
    let parser = TickerParser::with_spec_path(&spec_path()).expect("parser");

    // 1. ETR (Ethereum, last Friday of Jan 2026 -> 2026-01-30)
    let parsed_etr = as_futures(parser.parse("ETRF26").expect("parse"));
    assert_eq!(parsed_etr.symbol, "ETR");
    assert_eq!(parsed_etr.year, 2026);
    assert_eq!(parsed_etr.month, 1);
    assert_eq!(parsed_etr.lot_size, Some(1.0));

    // 2. SOL (Solana, last Friday of Jan 2026 -> 2026-01-30)
    let parsed_sol = as_futures(parser.parse("SOLF26").expect("parse"));
    assert_eq!(parsed_sol.symbol, "SOL");
    assert_eq!(parsed_sol.lot_size, Some(1.0));

    // 3. SJC (Soybean CBOT, 2nd business day prior to March 2026 -> 2026-02-26)
    let parsed_sjc = as_futures(parser.parse("SJCH26").expect("parse"));
    assert_eq!(parsed_sjc.symbol, "SJC");
    assert_eq!(parsed_sjc.lot_size, Some(1.0));

    // 4. SOY (Soybean FOB Santos, business day prior to 16th of Feb 2026 -> 2026-02-13)
    let parsed_soy = as_futures(parser.parse("SOYH26").expect("parse"));
    assert_eq!(parsed_soy.symbol, "SOY");
    assert_eq!(parsed_soy.lot_size, Some(1.0));

    // 5. GLD (Gold, 3rd to last business day of Jan 2026 -> 2026-01-28)
    let parsed_gld = as_futures(parser.parse("GLDF26").expect("parse"));
    assert_eq!(parsed_gld.symbol, "GLD");
    assert_eq!(parsed_gld.lot_size, Some(1.0));

    // 6. BIT (Bitcoin, last Friday of Jan 2026 -> 2026-01-30)
    let parsed_bit = as_futures(parser.parse("BITF26").expect("parse"));
    assert_eq!(parsed_bit.symbol, "BIT");
    assert_eq!(parsed_bit.lot_size, Some(1.0));
    assert_eq!(parsed_bit.tick_size, Some(2.0));

    // 7. ISP (S&P 500, third Friday of March 2026 -> 2026-03-20)
    let parsed_isp = as_futures(parser.parse("ISPH26").expect("parse"));
    assert_eq!(parsed_isp.symbol, "ISP");
    assert_eq!(parsed_isp.lot_size, Some(50.0));
    assert_eq!(parsed_isp.tick_size, Some(0.25));

    // 8. WSP (Micro S&P 500, third Friday of March 2026 -> 2026-03-20)
    let parsed_wsp = as_futures(parser.parse("WSPH26").expect("parse"));
    assert_eq!(parsed_wsp.symbol, "WSP");
    assert_eq!(parsed_wsp.lot_size, Some(2.5));
    assert_eq!(parsed_wsp.tick_size, Some(0.25));

    // 9. ETH (Hydrous Ethanol, last business day of Jan 2026 -> 2026-01-30)
    let parsed_eth = as_futures(parser.parse("ETHF26").expect("parse"));
    assert_eq!(parsed_eth.symbol, "ETH");
    assert_eq!(parsed_eth.lot_size, Some(1.0));
    assert_eq!(parsed_eth.tick_size, Some(0.50));

    // Let's also resolve expiration dates to verify they work
    use tickerforge::calendars::get_calendar;
    use tickerforge::expiration_rules::resolve_expiration;

    let spec = load_spec().expect("spec");
    let cal = get_calendar("B3");

    // ETR: last Friday of Jan 2026
    let etr_rule = spec.expiration_rules.get("last_friday").expect("rule");
    assert_eq!(
        resolve_expiration(&parsed_etr.contract, 2026, 1, etr_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 1, 30).unwrap()
    );

    // SJC: second business day prior to March 2026 (Feb 2026 has 20 business days; 2nd to last is Feb 26)
    let sjc_rule = spec
        .expiration_rules
        .get("second_business_day_prior_to_month")
        .expect("rule");
    assert_eq!(
        resolve_expiration(&parsed_sjc.contract, 2026, 3, sjc_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 26).unwrap()
    );

    // SOY: business day prior to 16th of Feb 2026
    let soy_rule = spec
        .expiration_rules
        .get("business_day_prior_to_16th_of_preceding_month")
        .expect("rule");
    assert_eq!(
        resolve_expiration(&parsed_soy.contract, 2026, 3, soy_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 13).unwrap()
    );

    // GLD: 3rd to last business day of Jan 2026
    let gld_rule = spec
        .expiration_rules
        .get("third_to_last_business_day")
        .expect("rule");
    assert_eq!(
        resolve_expiration(&parsed_gld.contract, 2026, 1, gld_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 1, 28).unwrap()
    );

    // BIT: last Friday of Jan 2026
    assert_eq!(
        resolve_expiration(&parsed_bit.contract, 2026, 1, etr_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 1, 30).unwrap()
    );

    // ISP: third Friday of March 2026
    let isp_rule = spec.expiration_rules.get("third_friday").expect("rule");
    assert_eq!(
        resolve_expiration(&parsed_isp.contract, 2026, 3, isp_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 3, 20).unwrap()
    );

    // WSP: third Friday of March 2026
    assert_eq!(
        resolve_expiration(&parsed_wsp.contract, 2026, 3, isp_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 3, 20).unwrap()
    );

    // ETH: last business day of Jan 2026
    let eth_rule = spec
        .expiration_rules
        .get("last_business_day")
        .expect("rule");
    assert_eq!(
        resolve_expiration(&parsed_eth.contract, 2026, 1, eth_rule, &cal).unwrap(),
        NaiveDate::from_ymd_opt(2026, 1, 30).unwrap()
    );
}
