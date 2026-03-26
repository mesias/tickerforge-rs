mod common;

use csv::ReaderBuilder;
use std::fs::File;

use tickerforge::TickerForge;

use crate::common::spec_path;

/// Drive examples from `futures_resolve.csv`. Full-file parity requires `exchange_calendars`-aligned
/// sessions (see `docs/calendar-strategy.md`); we assert **WIN** and **IND** rows which match `bdays` in tests.
#[test]
fn futures_resolve_b3_win_ind_rows() {
    let path = spec_path().join("tests/b3/futures_resolve.csv");
    let f = File::open(&path).expect("open csv");
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(f);
    let forge = TickerForge::new(Some(&spec_path())).expect("forge");
    for rec in rdr.records() {
        let rec = rec.expect("row");
        let symbol = rec.get(0).expect("symbol");
        if !matches!(symbol, "WIN" | "IND") {
            continue;
        }
        let date = rec.get(1).expect("date");
        let offset: usize = rec.get(2).expect("offset").parse().expect("offset int");
        let expected = rec.get(3).expect("expected");
        let comment = rec.get(4).unwrap_or("");
        let got = forge
            .generate(symbol, date, offset)
            .unwrap_or_else(|e| panic!("{e} (row comment: {comment})"));
        assert_eq!(got, expected, "row {:?}", rec);
    }
}
