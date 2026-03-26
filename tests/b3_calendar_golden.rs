mod common;

use csv::ReaderBuilder;
use std::fs::File;
use std::io::BufReader;
use xz2::read::XzDecoder;

use chrono::NaiveDate;
use tickerforge::calendars::get_calendar;
use tickerforge::TickerForge;

use crate::common::spec_path;

const DATE_MIN: &str = "2023-01-01";
const DATE_MAX: &str = "2026-12-31";

/// Golden WIN/IND/DOL names vs generate(); mirrors Python test_b3_calendar_golden.py.
#[test]
fn b3_win_ind_dol_calendar_matches_generator() {
    let xz_path = spec_path().join("tests/b3/B3_2023_2028_WIN_IND_DOL_calendar_FIXED.csv.xz");
    if !xz_path.exists() {
        eprintln!(
            "SKIP: missing golden calendar fixture: {}",
            xz_path.display()
        );
        return;
    }

    let forge = TickerForge::new(Some(&spec_path())).expect("forge");
    let cal = get_calendar("B3");
    let cal_first = cal.first_session();
    let cal_last = cal.last_session();
    let date_min = NaiveDate::parse_from_str(DATE_MIN, "%Y-%m-%d").unwrap();
    let date_max = NaiveDate::parse_from_str(DATE_MAX, "%Y-%m-%d").unwrap();

    let file = File::open(&xz_path).expect("open xz");
    let decoder = XzDecoder::new(BufReader::new(file));
    let mut rdr = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_reader(decoder);

    let mut checked = 0u64;
    for result in rdr.records() {
        let rec = result.expect("row");
        let date_str = rec.get(0).expect("date column");
        let row_date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        if row_date < date_min || row_date > date_max {
            continue;
        }
        if row_date < cal_first || row_date > cal_last {
            continue;
        }

        let win = rec.get(2).unwrap_or("").trim().to_string();
        let ind = rec.get(5).unwrap_or("").trim().to_string();
        let dol = rec.get(8).unwrap_or("").trim().to_string();

        let has_contracts = !win.is_empty();
        assert_eq!(
            !ind.is_empty(),
            has_contracts,
            "row {date_str}: IND presence mismatch"
        );
        assert_eq!(
            !dol.is_empty(),
            has_contracts,
            "row {date_str}: DOL presence mismatch"
        );

        if has_contracts {
            let date_iso = row_date.format("%Y-%m-%d").to_string();
            let got_win = forge
                .generate("WIN", &date_iso, 0)
                .unwrap_or_else(|e| panic!("WIN {date_str}: {e}"));
            assert_eq!(got_win, win, "WIN mismatch on {date_str}");

            let got_ind = forge
                .generate("IND", &date_iso, 0)
                .unwrap_or_else(|e| panic!("IND {date_str}: {e}"));
            assert_eq!(got_ind, ind, "IND mismatch on {date_str}");

            let got_dol = forge
                .generate("DOL", &date_iso, 0)
                .unwrap_or_else(|e| panic!("DOL {date_str}: {e}"));
            assert_eq!(got_dol, dol, "DOL mismatch on {date_str}");

            checked += 1;
        }
    }
    assert!(checked > 900, "expected >900 checked rows, got {checked}");
}
