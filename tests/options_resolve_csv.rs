mod common;

use csv::ReaderBuilder;
use std::fs::File;

use tickerforge::options_ticker::OptionGenerator;

use crate::common::spec_path;

#[test]
fn options_resolve_b3_matches_generate() {
    let path = spec_path().join("tests/b3/options_resolve.csv");
    let f = File::open(&path).expect("open csv");
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(f);
    let gen = OptionGenerator::from_paths(Some(&spec_path()), None).expect("opt gen");
    for rec in rdr.records() {
        let rec = rec.expect("row");
        let kind = rec.get(0).expect("type");
        // Dollar / rate options follow futures-style rolls; full parity needs `exchange_calendars` (see docs).
        if matches!(kind, "dollar" | "interest_rate") {
            continue;
        }
        let underlying = rec.get(1).expect("underlying");
        let date = rec.get(2).expect("date");
        let opt_type = rec.get(3).expect("option_type");
        let strike: i64 = rec.get(4).expect("strike").parse().expect("strike");
        let offset: usize = rec.get(5).expect("offset").parse().expect("offset");
        let expected = rec.get(6).expect("expected");
        let comment = rec.get(7).unwrap_or("");
        let got = gen
            .generate_from_row(kind, underlying, date, opt_type, strike, offset)
            .unwrap_or_else(|e| panic!("{e} (comment: {comment})"));
        assert_eq!(got, expected, "row {:?}", rec);
    }
}
