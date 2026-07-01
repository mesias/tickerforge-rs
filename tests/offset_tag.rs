mod common;

use crate::common::spec_path;
use tickerforge::{
    generate_ticker_for_contract_signed, load_spec, parse_any_ticker_date, AnyParsedTicker,
    ParsedFuturesTicker, TickerForge,
};

fn as_futures(any: AnyParsedTicker) -> ParsedFuturesTicker {
    match any {
        AnyParsedTicker::Futures(f) => f,
        AnyParsedTicker::Option(o) => panic!("expected Futures but got Option: {:?}", o),
        AnyParsedTicker::Equity(e) => panic!("expected Futures but got Equity: {:?}", e),
    }
}

// ---------------------------------------------------------------------------
// Positive offsets
// ---------------------------------------------------------------------------

#[test]
fn parse_dol_offset_one_matches_generate() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("DOL").expect("DOL contract");

    let any = parse_any_ticker_date("DOL[1]", "2026-06-29").expect("parse");
    let parsed = as_futures(any);

    let from_signed =
        generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, 1).expect("signed");
    let from_forge = forge.generate("DOL", "2026-06-29", 1).expect("forge");

    assert_eq!(parsed.ticker().expect("ticker"), "DOLQ26");
    assert_eq!(parsed.ticker().expect("ticker"), from_signed);
    assert_eq!(parsed.ticker().expect("ticker"), from_forge);
    assert_eq!(parsed.contract_offset, Some(1));
}

#[test]
fn parse_win_offset_two_matches_generate() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let any = parse_any_ticker_date("WIN[2]", "2026-06-29").expect("parse");
    let parsed = as_futures(any);

    assert_eq!(parsed.ticker().expect("ticker"), "WINZ26");
    assert_eq!(
        parsed.ticker().expect("ticker"),
        forge.generate("WIN", "2026-06-29", 2).expect("forge")
    );
    assert_eq!(parsed.contract_offset, Some(2));
}

#[test]
fn parse_ccm_offset_one_matches_generate() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("CCM").expect("CCM contract");

    let any = parse_any_ticker_date("CCM[1]", "2026-09-01").expect("parse");
    let parsed = as_futures(any);

    let from_signed =
        generate_ticker_for_contract_signed(contract, "2026-09-01", &spec, 1).expect("signed");
    let from_forge = forge.generate("CCM", "2026-09-01", 1).expect("forge");

    assert_eq!(parsed.ticker().expect("ticker"), "CCMX26");
    assert_eq!(parsed.ticker().expect("ticker"), from_signed);
    assert_eq!(parsed.ticker().expect("ticker"), from_forge);
    assert_eq!(parsed.contract_offset, Some(1));
}

#[test]
fn parse_bgi_offset_six_matches_generate() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("BGI").expect("BGI contract");

    let any = parse_any_ticker_date("BGI[6]", "2026-09-01").expect("parse");
    let parsed = as_futures(any);

    let from_signed =
        generate_ticker_for_contract_signed(contract, "2026-09-01", &spec, 6).expect("signed");
    let from_forge = forge.generate("BGI", "2026-09-01", 6).expect("forge");

    assert_eq!(parsed.ticker().expect("ticker"), "BGIH27");
    assert_eq!(parsed.ticker().expect("ticker"), from_signed);
    assert_eq!(parsed.ticker().expect("ticker"), from_forge);
    assert_eq!(parsed.contract_offset, Some(6));
}

#[test]
fn parse_ccm_crosses_year() {
    // September reference: CCM[3] lands in March of the next calendar year.
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let any = parse_any_ticker_date("CCM[3]", "2026-09-01").expect("parse");
    let parsed = as_futures(any);

    assert_eq!(parsed.ticker().expect("ticker"), "CCMH27");
    assert_eq!(
        parsed.ticker().expect("ticker"),
        forge.generate("CCM", "2026-09-01", 3).expect("forge")
    );
    assert_eq!(parsed.year, 2027);
    assert_eq!(parsed.month, 3);
    assert_eq!(parsed.contract_offset, Some(3));
}

#[test]
fn parse_bgi_crosses_year() {
    // Late-year reference: BGI[6] spans into the next calendar year.
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let any = parse_any_ticker_date("BGI[6]", "2026-12-31").expect("parse");
    let parsed = as_futures(any);

    assert_eq!(parsed.ticker().expect("ticker"), "BGIN27");
    assert_eq!(
        parsed.ticker().expect("ticker"),
        forge.generate("BGI", "2026-12-31", 6).expect("forge")
    );
    assert_eq!(parsed.year, 2027);
    assert_eq!(parsed.month, 7);
    assert_eq!(parsed.contract_offset, Some(6));
}

// ---------------------------------------------------------------------------
// Negative offsets
// ---------------------------------------------------------------------------

#[test]
fn parse_ind_negative_one_is_previous_contract() {
    // On 2026-06-18 the IND front month is INDQ26 (rolls after expiry),
    // so IND[-1] must be the most-recently-expired contract: INDM26.
    let any = parse_any_ticker_date("IND[-1]", "2026-06-18").expect("parse");
    let parsed = as_futures(any);
    assert_eq!(parsed.ticker().expect("ticker"), "INDM26");
    assert_eq!(parsed.contract_offset, Some(-1));
}

#[test]
fn generate_signed_negative_one_returns_previous_dol() {
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("DOL").expect("DOL contract");
    let ticker =
        generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, -1).expect("signed");
    assert_eq!(ticker, "DOLM26");
}

#[test]
fn generate_signed_negative_two_returns_prior_dol() {
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("DOL").expect("DOL contract");
    let ticker =
        generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, -2).expect("signed");
    assert_eq!(ticker, "DOLK26");
}

// ---------------------------------------------------------------------------
// DOL[0] vs plain DOL
// ---------------------------------------------------------------------------

#[test]
fn dol_offset_zero_equals_plain_dol_ticker() {
    let tagged = as_futures(parse_any_ticker_date("DOL[0]", "2026-06-29").expect("tagged"));
    let plain = as_futures(parse_any_ticker_date("DOL", "2026-06-29").expect("plain"));

    assert_eq!(tagged.ticker().expect("ticker"), "DOLN26");
    assert_eq!(
        tagged.ticker().expect("ticker"),
        plain.ticker().expect("ticker")
    );

    assert_eq!(tagged.contract_offset, Some(0));
    assert_eq!(plain.contract_offset, None);
}

// ---------------------------------------------------------------------------
// contract_offset field semantics
// ---------------------------------------------------------------------------

#[test]
fn contract_offset_some_for_tagged_none_for_full() {
    let tagged = as_futures(parse_any_ticker_date("DOL[1]", "2026-06-29").expect("tagged"));
    assert_eq!(tagged.contract_offset, Some(1));

    let full = as_futures(parse_any_ticker_date("INDM26", "2026-06-18").expect("full"));
    assert_eq!(full.contract_offset, None);
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn out_of_range_positive_errors() {
    let err = parse_any_ticker_date("DOL[999]", "2026-06-29").unwrap_err();
    assert!(err.contains("out of range"));
}

#[test]
fn out_of_range_negative_errors() {
    let err = parse_any_ticker_date("DOL[-999]", "2026-06-29").unwrap_err();
    assert!(err.contains("out of range"));
}

#[test]
fn unknown_root_errors() {
    let err = parse_any_ticker_date("ZZZ[1]", "2026-06-29").unwrap_err();
    assert!(err.contains("Unable to parse ticker"));
}

#[test]
fn generate_signed_out_of_range_positive_errors() {
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("DOL").expect("DOL contract");
    let err = generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, 999).unwrap_err();
    assert!(err.contains("out of range"));
}

#[test]
fn generate_signed_out_of_range_negative_errors() {
    let spec = load_spec().expect("spec");
    let contract = spec.get_contract("DOL").expect("DOL contract");
    let err = generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, -999).unwrap_err();
    assert!(err.contains("out of range"));
}

// ---------------------------------------------------------------------------
// TickerForge::gen_signed (today)
// ---------------------------------------------------------------------------

#[test]
fn gen_signed_today_matches_generate_today() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    assert_eq!(
        forge.gen_signed("DOL", 1).expect("gen_signed"),
        forge.generate("DOL", &today, 1).expect("generate"),
    );
}

// ---------------------------------------------------------------------------
// reference_date / is_trading_session stamping for tagged roots
// ---------------------------------------------------------------------------

#[test]
fn tagged_root_stamps_reference_date_and_session() {
    // 2026-04-15 is a Wednesday — B3 trading session.
    let parsed = as_futures(parse_any_ticker_date("DOL[1]", "2026-04-15").expect("parse"));
    assert_eq!(
        parsed.reference_date,
        Some(chrono::NaiveDate::from_ymd_opt(2026, 4, 15).unwrap()),
    );
    assert_eq!(parsed.is_trading_session, Some(true));
    assert_eq!(parsed.contract_offset, Some(1));
}

// ---------------------------------------------------------------------------
// Conditional roll-day tag validation
// ---------------------------------------------------------------------------

#[test]
fn parse_dol_roll_tag_valid_on_roll_day() {
    // For DOLN26 (July 2026), the expiration is July 1st, 2026.
    // DOL rolls off on expiration day, so the last trading day is June 30th, 2026.
    let ref_roll = "2026-06-30";

    let any = parse_any_ticker_date("DOL[@roll]", ref_roll).expect("parse shortcut");
    let parsed = as_futures(any);
    assert_eq!(parsed.ticker().expect("ticker"), "DOLQ26");
    assert_eq!(parsed.contract_offset, Some(1));

    let any_zero = parse_any_ticker_date("DOL[0@roll]", ref_roll).expect("parse explicit zero");
    let parsed_zero = as_futures(any_zero);
    assert_eq!(parsed_zero.ticker().expect("ticker"), "DOLN26");
    assert_eq!(parsed_zero.contract_offset, Some(0));
}

#[test]
fn parse_dol_roll_tag_invalid_on_non_roll_day() {
    let ref_other = "2026-06-29";
    let err = parse_any_ticker_date("DOL[@roll]", ref_other).unwrap_err();
    assert!(err.contains("is not valid on 2026-06-29"));
}

#[test]
fn parse_win_roll_tag_index_future() {
    // For WINM26 (June 2026), the expiration is June 17th, 2026.
    // WIN remains tradeable on expiration day, so the last trading day is June 17th.
    let ref_roll = "2026-06-17";
    let ref_other = "2026-06-16";

    let any = parse_any_ticker_date("WIN[@roll]", ref_roll).expect("parse shortcut");
    let parsed = as_futures(any);
    assert_eq!(parsed.ticker().expect("ticker"), "WINQ26");
    assert_eq!(parsed.contract_offset, Some(1));

    let any_zero = parse_any_ticker_date("WIN[0@roll]", ref_roll).expect("parse explicit zero");
    let parsed_zero = as_futures(any_zero);
    assert_eq!(parsed_zero.ticker().expect("ticker"), "WINM26");
    assert_eq!(parsed_zero.contract_offset, Some(0));

    let err = parse_any_ticker_date("WIN[@roll]", ref_other).unwrap_err();
    assert!(err.contains("is not valid on 2026-06-16"));
}

// ---------------------------------------------------------------------------
// is_valid Flag Verification
// ---------------------------------------------------------------------------

#[test]
fn test_is_valid_flag_for_roll_day() {
    let any = parse_any_ticker_date("DOL[@roll]", "2026-06-30").expect("parse");
    let parsed = as_futures(any);
    assert_eq!(parsed.is_valid, Some(true));
}

#[test]
fn test_is_valid_flag_for_expired_contracts() {
    // DOLQ24 (August 2024 contract) has expired as of July 1st, 2026
    let any = parse_any_ticker_date("DOLQ24", "2026-07-01").expect("parse");
    let parsed = as_futures(any);
    assert_eq!(parsed.is_valid, Some(false));
}

#[test]
fn test_is_valid_flag_for_active_contracts() {
    // DOLQ26 (August 2026 contract) is active as of July 1st, 2026
    let any = parse_any_ticker_date("DOLQ26", "2026-07-01").expect("parse");
    let parsed = as_futures(any);
    assert_eq!(parsed.is_valid, Some(true));
}
