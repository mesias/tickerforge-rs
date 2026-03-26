mod common;

use chrono::NaiveDate;
use tickerforge::calendars::get_calendar;
use tickerforge::expiration_rules::resolve_expiration;
use tickerforge::load_spec;

use crate::common::spec_path;

#[test]
fn resolve_nearest_weekday_to_day_for_ind() {
    let spec = load_spec(Some(&spec_path())).expect("load");
    let contract = spec.get_contract("IND").expect("IND");
    let rule = spec
        .expiration_rules
        .get(&contract.expiration_rule)
        .expect("rule");
    let cal = get_calendar(&contract.exchange);
    let expiration = resolve_expiration(contract, 2026, 6, rule, &cal).expect("exp");
    assert_eq!(expiration, NaiveDate::from_ymd_opt(2026, 6, 17).unwrap());
}

#[test]
fn resolve_first_business_day_for_dol() {
    let spec = load_spec(Some(&spec_path())).expect("load");
    let contract = spec.get_contract("DOL").expect("DOL");
    let rule = spec
        .expiration_rules
        .get(&contract.expiration_rule)
        .expect("rule");
    let cal = get_calendar(&contract.exchange);
    let expiration = resolve_expiration(contract, 2026, 4, rule, &cal).expect("exp");
    assert_eq!(expiration, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
}
