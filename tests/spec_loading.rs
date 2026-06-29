mod common;

use tickerforge::{load_spec_from_path, TickerForge};

use crate::common::spec_path;

#[test]
fn load_spec_reads_b3_exchange_and_contracts() {
    let spec = load_spec_from_path(&spec_path()).expect("load");
    let exchange = spec.get_exchange("B3").expect("B3");
    assert_eq!(exchange.code, "B3");
    assert!(exchange.assets.contains_key("IND"));
    let contract = spec.get_contract("IND").expect("IND");
    assert_eq!(contract.symbol, "IND");
    assert_eq!(contract.ticker_format, "{symbol}{month_code}{yy}");

    let dol = spec.get_contract("DOL").expect("DOL");
    assert_eq!(dol.tick_size, Some(0.5));
    assert_eq!(dol.regular_session_start_end(), Some(("09:00", "18:30")));
    assert_eq!(dol.exchange_timezone.as_deref(), Some("America/Sao_Paulo"));
    assert_eq!(dol.sessions[0].name, "regular");
    assert_eq!(dol.sessions[0].start, "09:00");
    assert_eq!(dol.sessions[0].end, "18:30");
}

#[test]
fn contract_trading_symbol_matches_forge() {
    let spec = load_spec_from_path(&spec_path()).expect("load");
    let dol = spec.get_contract("DOL").expect("DOL");
    let forge = TickerForge::with_spec_path(&spec_path()).expect("forge");

    assert_eq!(
        dol.trading_symbol_for_with_spec(&spec, "2026-03-15", 0)
            .expect("with_spec"),
        forge.generate("DOL", "2026-03-15", 0).expect("gen")
    );

    let forge_default = TickerForge::new().expect("forge default");
    assert_eq!(
        dol.trading_symbol_for("2026-03-15", 0)
            .expect("default spec"),
        forge_default
            .generate("DOL", "2026-03-15", 0)
            .expect("gen default")
    );

    assert_eq!(
        dol.trading_symbol_today_with_spec(&spec)
            .expect("today with_spec"),
        forge.gen("DOL").expect("gen today")
    );
    assert_eq!(
        dol.trading_symbol_today().expect("today default"),
        forge_default.gen("DOL").expect("gen today default")
    );
}
