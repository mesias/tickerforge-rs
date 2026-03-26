mod common;

use tickerforge::contract_cycle::resolve_contract_months;
use tickerforge::{load_spec, month_codes};

use crate::common::spec_path;

#[test]
fn month_code_round_trip() {
    assert_eq!(month_codes::month_to_code(1).unwrap(), 'F');
    assert_eq!(month_codes::month_to_code(12).unwrap(), 'Z');
    assert_eq!(month_codes::code_to_month('F').unwrap(), 1);
    assert_eq!(month_codes::code_to_month('z').unwrap(), 12);
}

#[test]
fn resolve_contract_months_for_common_cycles() {
    let spec = load_spec(Some(&spec_path())).expect("load");
    let monthly = spec.contract_cycles.get("monthly").expect("monthly");
    let quarterly = spec.contract_cycles.get("quarterly").expect("quarterly");
    let bimonthly_even = spec
        .contract_cycles
        .get("bimonthly_even")
        .expect("bimonthly_even");

    assert_eq!(
        resolve_contract_months(monthly, 2026).unwrap(),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    );
    assert_eq!(
        resolve_contract_months(quarterly, 2026).unwrap(),
        vec![3, 6, 9, 12]
    );
    assert_eq!(
        resolve_contract_months(bimonthly_even, 2026).unwrap(),
        vec![2, 4, 6, 8, 10, 12]
    );
}
