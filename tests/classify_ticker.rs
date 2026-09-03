mod common;

use std::path::PathBuf;

use tickerforge::{
    classify_ticker, classify_ticker_spec, classify_ticker_spec_exchange, clear_load_spec_cache,
    load_spec, load_spec_from_path, parse_any_ticker_spec, AnyParsedTicker, AssetType, OptionSide,
};

use crate::common::spec_path;

fn load_test_spec() -> tickerforge::SpecRepository {
    load_spec_from_path(&spec_path()).expect("load spec")
}

#[test]
fn classify_futures_full_ticker() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("INDM26", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Future);
    assert_eq!(classified.root, "IND");
    assert_eq!(classified.year, Some(2026));
    assert_eq!(classified.month, Some(6));
    assert_eq!(classified.exchange.as_deref(), Some("B3"));
}

#[test]
fn classify_futures_root() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("IND", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Future);
    assert_eq!(classified.root, "IND");
    assert_eq!(classified.year, None);
    assert_eq!(classified.month, None);
}

#[test]
fn classify_offset_tag() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("DOL[1]", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Future);
    assert_eq!(classified.root, "DOL");
}

#[test]
fn classify_equity_option() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("PETRA30", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Option);
    assert_eq!(classified.root, "PETR4");
    assert_eq!(classified.option_type, Some(OptionSide::Call));
    assert_eq!(classified.month, Some(1));
    assert_eq!(classified.strike.as_deref(), Some("30"));
}

#[test]
fn classify_nonequity_option() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("DOLK26C5000", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Option);
    assert_eq!(classified.root, "DOL");
    assert_eq!(classified.option_type, Some(OptionSide::Call));
    assert_eq!(classified.year, Some(2026));
    assert_eq!(classified.month, Some(5));
    assert_eq!(classified.strike.as_deref(), Some("5000"));
}

#[test]
fn classify_nonequity_put_option() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("DOLK26P5000", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Option);
    assert_eq!(classified.option_type, Some(OptionSide::Put));
}

#[test]
fn classify_cash_equity() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec("PETR4", &spec).expect("classify");
    assert_eq!(classified.asset_type, AssetType::Equity);
    assert_eq!(classified.root, "PETR4");
    assert_eq!(classified.exchange.as_deref(), Some("B3"));
}

#[test]
fn classify_with_default_spec_loads() {
    clear_load_spec_cache();
    let classified = classify_ticker("INDM26").expect("classify");
    assert_eq!(classified.asset_type, AssetType::Future);
    assert_eq!(classified.root, "IND");
}

#[test]
fn classify_filters_by_exchange() {
    let spec = load_test_spec();
    let classified = classify_ticker_spec_exchange("INDM26", &spec, "B3").expect("classify");
    assert_eq!(classified.exchange.as_deref(), Some("B3"));
    let err = classify_ticker_spec_exchange("INDM26", &spec, "CME").unwrap_err();
    assert!(err.contains("Unable to classify"));
}

#[test]
fn classify_unknown_tag_raises() {
    let spec = load_test_spec();
    let err = classify_ticker_spec("NOSUCH[0]", &spec).unwrap_err();
    assert!(err.contains("Unable to classify"));
}

#[test]
fn classify_tag_wrong_exchange_raises() {
    let spec = load_test_spec();
    let err = classify_ticker_spec_exchange("DOL[1]", &spec, "CME").unwrap_err();
    assert!(err.contains("Unable to classify"));
}

fn parsed_asset_type(any: &AnyParsedTicker) -> AssetType {
    match any {
        AnyParsedTicker::Futures(_) => AssetType::Future,
        AnyParsedTicker::Option(_) => AssetType::Option,
        AnyParsedTicker::Equity(_) => AssetType::Equity,
    }
}

#[test]
fn classify_matches_parse_asset_type() {
    let spec = load_test_spec();
    for ticker in ["INDM26", "PETRA30", "IND", "DOL[0]", "DOLK26C5000", "PETR4"] {
        let classified = classify_ticker_spec(ticker, &spec).expect("classify");
        let parsed = parse_any_ticker_spec(ticker, &spec).expect("parse");
        assert_eq!(
            classified.asset_type,
            parsed_asset_type(&parsed),
            "asset_type mismatch for {ticker}"
        );
    }
}

#[test]
fn classify_unknown_raises() {
    let spec = load_test_spec();
    let err = classify_ticker_spec("NOTAREALTICKER999", &spec).unwrap_err();
    assert!(err.contains("Unable to classify"));
}

#[test]
fn load_spec_missing_path_raises() {
    clear_load_spec_cache();
    let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("does-not-exist-spec-root");
    let err = load_spec_from_path(&missing).unwrap_err();
    assert!(err.contains("Spec path does not exist"));
    clear_load_spec_cache();
}

#[test]
fn load_spec_is_cached() {
    clear_load_spec_cache();
    let first = load_spec().expect("first");
    let second = load_spec().expect("second");
    // Cache returns clones of the same Arc-backed load; pattern_index OnceLock starts empty
    // on each clone, but contracts/options identity of content must match.
    assert_eq!(first.contracts.len(), second.contracts.len());
    assert!(first.contracts.contains_key("IND"));
    assert!(second.contracts.contains_key("IND"));
    clear_load_spec_cache();
    let third = load_spec().expect("third");
    assert!(third.contracts.contains_key("IND"));
}

#[test]
fn pattern_index_reused_on_same_repository() {
    let spec = load_test_spec();
    classify_ticker_spec("INDM26", &spec).expect("classify");
    let first = spec.pattern_index.get().expect("index built") as *const _;
    classify_ticker_spec("DOLN26", &spec).expect("classify");
    let second = spec.pattern_index.get().expect("index still set") as *const _;
    assert_eq!(first, second);
}
