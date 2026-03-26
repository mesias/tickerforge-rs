mod common;

use tickerforge::TickerForge;

use crate::common::spec_path;

#[test]
fn generate_ind_front_contract_before_expiry() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let ticker = forge.generate("IND", "2026-06-01", 0).expect("gen");
    assert_eq!(ticker, "INDM26");
}

#[test]
fn generate_ind_rolls_after_expiry() {
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");
    let ticker = forge.generate("IND", "2026-06-18", 0).expect("gen");
    assert_eq!(ticker, "INDQ26");
}
