mod common;

use tickerforge::{TickerForge, TickerParser};

use crate::common::spec_path;

#[test]
fn parse_ind_ticker() {
    let parser = TickerParser::with_spec_path(&spec_path()).expect("parser");
    let parsed = parser.parse("INDM26", Some("2026-01-01")).expect("parse");
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
    let parsed = parser.parse(&generated, Some("2026-06-01")).expect("parse");
    assert_eq!(generated, "INDM26");
    assert_eq!(parsed.symbol, "IND");
    assert_eq!(parsed.year, 2026);
    assert_eq!(parsed.month, 6);
}

#[test]
fn parse_invalid_ticker_errors() {
    let parser = TickerParser::with_spec_path(&spec_path()).expect("parser");
    assert!(parser.parse("INVALID", None).is_err());
}
