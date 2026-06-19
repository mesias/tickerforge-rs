//! Expiration date resolution from rules + exchange calendar.

use chrono::{Datelike, NaiveDate, Weekday};

use crate::calendars::ExchangeCalendar;
use crate::models::{ContractSpec, ExpirationRule};

fn weekday_name_to_number(name: &str) -> Option<Weekday> {
    match name.to_lowercase().as_str() {
        "monday" => Some(Weekday::Mon),
        "tuesday" => Some(Weekday::Tue),
        "wednesday" => Some(Weekday::Wed),
        "thursday" => Some(Weekday::Thu),
        "friday" => Some(Weekday::Fri),
        "saturday" => Some(Weekday::Sat),
        "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
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

pub fn month_sessions(cal: &ExchangeCalendar, year: i32, month: u32) -> Vec<NaiveDate> {
    let last_day = days_in_month(year, month);
    let month_start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let month_end = NaiveDate::from_ymd_opt(year, month, last_day).unwrap();
    let cal_first = cal.first_session();
    let cal_last = cal.last_session();
    if month_end < cal_first || month_start > cal_last {
        return Vec::new();
    }
    let clip_start = month_start.max(cal_first);
    let clip_end = month_end.min(cal_last);
    if clip_start > clip_end {
        return Vec::new();
    }
    cal.sessions_in_range(clip_start, clip_end)
}

fn resolve_first_business_day(cal: &ExchangeCalendar, year: i32, month: u32) -> NaiveDate {
    let s = month_sessions(cal, year, month);
    s[0]
}

fn resolve_last_business_day(cal: &ExchangeCalendar, year: i32, month: u32) -> NaiveDate {
    let s = month_sessions(cal, year, month);
    *s.last().unwrap()
}

fn resolve_nth_business_day(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
    n: i32,
) -> Result<NaiveDate, String> {
    let s = month_sessions(cal, year, month);
    let n = n as usize;
    if n < 1 || n > s.len() {
        return Err(format!(
            "Invalid nth business day '{n}' for {year}-{month:02}"
        ));
    }
    Ok(s[n - 1])
}

fn resolve_fixed_day(cal: &ExchangeCalendar, year: i32, month: u32, day: i32) -> NaiveDate {
    let last_day = days_in_month(year, month) as i32;
    let d = day.min(last_day).max(1) as u32;
    let target = NaiveDate::from_ymd_opt(year, month, d).unwrap();
    let sessions = month_sessions(cal, year, month);
    for session_day in &sessions {
        if *session_day >= target {
            return *session_day;
        }
    }
    *sessions.last().unwrap()
}

fn resolve_nearest_weekday_to_day(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
    weekday_name: &str,
    day: i32,
) -> Result<NaiveDate, String> {
    let wd = weekday_name_to_number(weekday_name)
        .ok_or_else(|| format!("bad weekday name {weekday_name}"))?;
    let sessions = month_sessions(cal, year, month);
    let weekday_sessions: Vec<NaiveDate> =
        sessions.into_iter().filter(|d| d.weekday() == wd).collect();
    if weekday_sessions.is_empty() {
        return Err(format!(
            "No sessions on weekday '{weekday_name}' for {year}-{month:02}"
        ));
    }
    let last_day = days_in_month(year, month) as i32;
    let d = day.min(last_day).max(1) as u32;
    let target = NaiveDate::from_ymd_opt(year, month, d).unwrap();
    weekday_sessions
        .into_iter()
        .min_by_key(|v| (v.signed_duration_since(target).num_days().abs(), *v))
        .ok_or_else(|| "empty weekday_sessions".to_string())
}

fn resolve_nth_weekday_of_month(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
    weekday_name: &str,
    n: i32,
) -> Result<NaiveDate, String> {
    let wd = weekday_name_to_number(weekday_name)
        .ok_or_else(|| format!("bad weekday name {weekday_name}"))?;
    let sessions = month_sessions(cal, year, month);
    let weekday_sessions: Vec<NaiveDate> =
        sessions.into_iter().filter(|d| d.weekday() == wd).collect();
    let n = n as usize;
    if n < 1 || n > weekday_sessions.len() {
        return Err(format!(
            "Invalid nth weekday '{n}' for weekday '{weekday_name}' in {year}-{month:02}"
        ));
    }
    Ok(weekday_sessions[n - 1])
}

/// Compute expiration calendar date for a contract month.
fn resolve_last_weekday_of_month(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
    weekday_name: &str,
) -> Result<NaiveDate, String> {
    let wd = weekday_name_to_number(weekday_name)
        .ok_or_else(|| format!("bad weekday name {weekday_name}"))?;
    let sessions = month_sessions(cal, year, month);
    let weekday_sessions: Vec<NaiveDate> =
        sessions.into_iter().filter(|d| d.weekday() == wd).collect();
    if weekday_sessions.is_empty() {
        return Err(format!(
            "No sessions on weekday '{weekday_name}' for {year}-{month:02}"
        ));
    }
    Ok(*weekday_sessions.last().unwrap())
}

fn resolve_second_business_day_prior_to_month(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
) -> Result<NaiveDate, String> {
    let (prev_year, prev_month) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    let sessions = month_sessions(cal, prev_year, prev_month);
    if sessions.len() < 2 {
        return Err(format!(
            "Not enough sessions in preceding month for {year}-{month:02}"
        ));
    }
    Ok(sessions[sessions.len() - 2])
}

fn resolve_business_day_prior_to_day_of_preceding_month(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
    day: i32,
) -> Result<NaiveDate, String> {
    let (prev_year, prev_month) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    let last_day = days_in_month(prev_year, prev_month) as i32;
    let d = day.min(last_day).max(1) as u32;
    let target = NaiveDate::from_ymd_opt(prev_year, prev_month, d).unwrap();
    let sessions = month_sessions(cal, prev_year, prev_month);
    let filtered_sessions: Vec<NaiveDate> = sessions.into_iter().filter(|s| *s < target).collect();
    if filtered_sessions.is_empty() {
        return Err("No sessions found before target day in preceding month".to_string());
    }
    Ok(*filtered_sessions.last().unwrap())
}

fn resolve_nth_business_day_from_end(
    cal: &ExchangeCalendar,
    year: i32,
    month: u32,
    n: i32,
) -> Result<NaiveDate, String> {
    let sessions = month_sessions(cal, year, month);
    let n = n as usize;
    if n < 1 || n > sessions.len() {
        return Err(format!(
            "Invalid nth business day from end '{n}' for {year}-{month:02}"
        ));
    }
    Ok(sessions[sessions.len() - n])
}

/// Compute expiration calendar date for a contract month.
pub fn resolve_expiration(
    contract: &ContractSpec,
    year: i32,
    month: u32,
    expiration_rule: &ExpirationRule,
    cal: &ExchangeCalendar,
) -> Result<NaiveDate, String> {
    let _ = contract;
    match expiration_rule.r#type.as_str() {
        "first_business_day" => Ok(resolve_first_business_day(cal, year, month)),
        "last_business_day" => Ok(resolve_last_business_day(cal, year, month)),
        "nth_business_day" => {
            let n = expiration_rule
                .n
                .ok_or_else(|| "nth_business_day rule requires 'n'".to_string())?;
            resolve_nth_business_day(cal, year, month, n)
        }
        "fixed_day" => {
            let day = expiration_rule
                .day
                .ok_or_else(|| "fixed_day rule requires 'day'".to_string())?;
            Ok(resolve_fixed_day(cal, year, month, day))
        }
        "nearest_weekday_to_day" => {
            let weekday = expiration_rule
                .weekday
                .as_deref()
                .ok_or_else(|| "nearest_weekday_to_day requires weekday".to_string())?;
            let day = expiration_rule
                .day
                .ok_or_else(|| "nearest_weekday_to_day requires day".to_string())?;
            resolve_nearest_weekday_to_day(cal, year, month, weekday, day)
        }
        "nth_weekday_of_month" => {
            let weekday = expiration_rule
                .weekday
                .as_deref()
                .ok_or_else(|| "nth_weekday_of_month requires weekday".to_string())?;
            let n = expiration_rule
                .n
                .ok_or_else(|| "nth_weekday_of_month requires n".to_string())?;
            resolve_nth_weekday_of_month(cal, year, month, weekday, n)
        }
        "last_weekday_of_month" => {
            let weekday = expiration_rule
                .weekday
                .as_deref()
                .ok_or_else(|| "last_weekday_of_month requires weekday".to_string())?;
            resolve_last_weekday_of_month(cal, year, month, weekday)
        }
        "second_business_day_prior_to_month" => {
            resolve_second_business_day_prior_to_month(cal, year, month)
        }
        "business_day_prior_to_day_of_preceding_month" => {
            let day = expiration_rule
                .day
                .ok_or_else(|| "business_day_prior_to_day_of_preceding_month requires day".to_string())?;
            resolve_business_day_prior_to_day_of_preceding_month(cal, year, month, day)
        }
        "nth_business_day_from_end" => {
            let n = expiration_rule
                .n
                .ok_or_else(|| "nth_business_day_from_end requires n".to_string())?;
            resolve_nth_business_day_from_end(cal, year, month, n)
        }
        "schedule" => Err("schedule expiration rules need external schedule data".to_string()),
        other => Err(format!("Unsupported expiration rule type: {other}")),
    }
}
