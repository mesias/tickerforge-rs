//! Shared date/month helpers for session grids.

use chrono::{Datelike, NaiveDate};

use crate::calendars::ExchangeCalendar;

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1)
        .unwrap()
        .pred_opt()
        .unwrap()
        .day()
}

pub fn is_month_in_calendar_range(cal: &ExchangeCalendar, year: i32, month: u32) -> bool {
    let first_session = cal.first_session();
    let last_session = cal.last_session();
    let month_start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let ld = days_in_month(year, month);
    let month_end = NaiveDate::from_ymd_opt(year, month, ld).unwrap();
    !(month_end < first_session || month_start > last_session)
}
