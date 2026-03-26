mod common;

use tickerforge::load_spec;

use crate::common::spec_path;

#[test]
fn load_spec_reads_b3_exchange_and_contracts() {
    let spec = load_spec(Some(&spec_path())).expect("load");
    let exchange = spec.get_exchange("B3").expect("B3");
    assert_eq!(exchange.code, "B3");
    assert!(exchange.assets.contains_key("IND"));
    let contract = spec.get_contract("IND").expect("IND");
    assert_eq!(contract.symbol, "IND");
    assert_eq!(contract.ticker_format, "{symbol}{month_code}{yy}");
}
