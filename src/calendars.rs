//! Exchange trading calendars via `bdays` (business days ≈ sessions for equity-style hours).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bdays::calendars::brazil::BrazilExchange;
use bdays::calendars::us::USSettlement;
use bdays::calendars::WeekendsOnly;
use bdays::{HolidayCalendar, HolidayCalendarCache};
use chrono::NaiveDate;

/// Calendar bounds used for session iteration (wide enough for tests and golden files).
const RANGE_MIN: (i32, u32, u32) = (1990, 1, 1);
const RANGE_MAX: (i32, u32, u32) = (2035, 12, 31);

fn range_min_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(RANGE_MIN.0, RANGE_MIN.1, RANGE_MIN.2).unwrap()
}

fn range_max_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(RANGE_MAX.0, RANGE_MAX.1, RANGE_MAX.2).unwrap()
}

/// Mirrors Python `exchange_calendars` usage: sessions are trading days from `bdays`.
pub struct ExchangeCalendar {
    cache: HolidayCalendarCache<NaiveDate>,
    first_session: NaiveDate,
    last_session: NaiveDate,
}

impl ExchangeCalendar {
    pub fn new<H: HolidayCalendar<NaiveDate>>(holiday_cal: H) -> Self {
        let dmin = range_min_date();
        let dmax = range_max_date();
        let cache = HolidayCalendarCache::new(holiday_cal, dmin, dmax);

        let mut d = dmin;
        while !cache.is_bday(d) {
            d = d
                .succ_opt()
                .expect("range must contain at least one business day");
        }
        let first_session = d;

        let mut d = dmax;
        while !cache.is_bday(d) {
            d = d
                .pred_opt()
                .expect("range must contain at least one business day");
        }
        let last_session = d;

        ExchangeCalendar {
            cache,
            first_session,
            last_session,
        }
    }

    #[inline]
    pub fn first_session(&self) -> NaiveDate {
        self.first_session
    }

    #[inline]
    pub fn last_session(&self) -> NaiveDate {
        self.last_session
    }

    /// All business days in `[start, end]` clipped to the calendar cache range.
    pub fn sessions_in_range(&self, start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
        let clip_start = start.max(range_min_date());
        let clip_end = end.min(range_max_date());
        if clip_start > clip_end {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut d = clip_start;
        loop {
            if self.cache.is_bday(d) {
                out.push(d);
            }
            if d >= clip_end {
                break;
            }
            d = match d.succ_opt() {
                Some(x) => x,
                None => break,
            };
        }
        out
    }
}

fn calendar_for_exchange_code(code: &str) -> ExchangeCalendar {
    match code.to_uppercase().as_str() {
        "B3" => ExchangeCalendar::new(BrazilExchange),
        "CME" | "ICE" | "EUREX" => ExchangeCalendar::new(USSettlement),
        _ => ExchangeCalendar::new(WeekendsOnly),
    }
}

static CAL_CACHE: Mutex<Option<HashMap<String, Arc<ExchangeCalendar>>>> = Mutex::new(None);

/// Resolve calendar for an exchange code (cached), matching Python `get_calendar` aliases.
pub fn get_calendar(exchange_code: &str) -> Arc<ExchangeCalendar> {
    let key = exchange_code.to_uppercase();
    let mut guard = CAL_CACHE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    let map = guard.as_mut().unwrap();
    map.entry(key.clone())
        .or_insert_with(|| Arc::new(calendar_for_exchange_code(&key)))
        .clone()
}
