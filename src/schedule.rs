//! Rule-based exchange schedule engine.
//!
//! Loads holiday rules from `spec/schedules/<exchange>.yaml` and evaluates them
//! for any year using the Computus algorithm for Easter-relative holidays.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use chrono::{Datelike, NaiveDate, Weekday};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// YAML model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleYaml {
    pub exchange: String,
    pub timezone: String,
    #[serde(default)]
    pub holidays: HolidayRules,
    #[serde(default)]
    pub early_closes: Option<EarlyCloseRules>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HolidayRules {
    #[serde(default)]
    pub fixed: Vec<FixedRule>,
    #[serde(default)]
    pub easter_offset: Vec<EasterOffsetRule>,
    #[serde(default)]
    pub nth_weekday: Vec<NthWeekdayRule>,
    #[serde(default)]
    pub last_weekday: Vec<LastWeekdayRule>,
    #[serde(default)]
    pub overrides: Vec<OverrideRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixedRule {
    pub month: u32,
    pub day: u32,
    pub name: String,
    pub from_year: Option<i32>,
    pub to_year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EasterOffsetRule {
    pub offset: i32,
    pub name: String,
    pub from_year: Option<i32>,
    pub to_year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NthWeekdayRule {
    pub month: u32,
    pub weekday: String,
    pub nth: u32,
    pub name: String,
    pub from_year: Option<i32>,
    pub to_year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastWeekdayRule {
    pub month: u32,
    pub weekday: String,
    pub name: String,
    pub from_year: Option<i32>,
    pub to_year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverrideRule {
    pub date: String,
    pub action: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EarlyCloseRules {
    #[serde(default)]
    pub fixed: Vec<serde_yaml::Value>,
    #[serde(default)]
    pub easter_offset: Vec<serde_yaml::Value>,
}

// ---------------------------------------------------------------------------
// Computus (Anonymous Gregorian algorithm)
// ---------------------------------------------------------------------------

fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).unwrap()
}

// ---------------------------------------------------------------------------
// Weekday helpers
// ---------------------------------------------------------------------------

fn weekday_from_name(name: &str) -> Option<Weekday> {
    match name.to_lowercase().as_str() {
        "monday" => Some(Weekday::Mon),
        "tuesday" => Some(Weekday::Tue),
        "wednesday" => Some(Weekday::Wed),
        "thursday" => Some(Weekday::Thu),
        "friday" => Some(Weekday::Fri),
        _ => None,
    }
}

fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, nth: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let diff = (weekday.num_days_from_monday() as i32
        - first.weekday().num_days_from_monday() as i32)
        .rem_euclid(7);
    let first_occ = first + chrono::Duration::days(diff as i64);
    first_occ + chrono::Duration::weeks((nth - 1) as i64)
}

fn last_weekday_of_month(year: i32, month: u32, weekday: Weekday) -> NaiveDate {
    let last_day = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap() - chrono::Duration::days(1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - chrono::Duration::days(1)
    };
    let diff = (last_day.weekday().num_days_from_monday() as i32
        - weekday.num_days_from_monday() as i32)
        .rem_euclid(7);
    last_day - chrono::Duration::days(diff as i64)
}

fn rule_applies(from_year: Option<i32>, to_year: Option<i32>, year: i32) -> bool {
    if let Some(fy) = from_year {
        if year < fy {
            return false;
        }
    }
    if let Some(ty) = to_year {
        if year > ty {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// ExchangeSchedule
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ExchangeSchedule {
    pub exchange: String,
    pub timezone: String,
    rules: HolidayRules,
    holiday_cache: HashMap<i32, BTreeSet<NaiveDate>>,
}

impl ExchangeSchedule {
    pub fn from_yaml(data: ScheduleYaml) -> Self {
        ExchangeSchedule {
            exchange: data.exchange,
            timezone: data.timezone,
            rules: data.holidays,
            holiday_cache: HashMap::new(),
        }
    }

    pub fn holidays_for_year(&mut self, year: i32) -> &BTreeSet<NaiveDate> {
        if self.holiday_cache.contains_key(&year) {
            return &self.holiday_cache[&year];
        }

        let mut holidays = BTreeSet::new();
        let easter = easter_sunday(year);

        for rule in &self.rules.fixed {
            if !rule_applies(rule.from_year, rule.to_year, year) {
                continue;
            }
            if let Some(d) = NaiveDate::from_ymd_opt(year, rule.month, rule.day) {
                holidays.insert(d);
            }
        }

        for rule in &self.rules.easter_offset {
            if !rule_applies(rule.from_year, rule.to_year, year) {
                continue;
            }
            holidays.insert(easter + chrono::Duration::days(rule.offset as i64));
        }

        for rule in &self.rules.nth_weekday {
            if !rule_applies(rule.from_year, rule.to_year, year) {
                continue;
            }
            if let Some(wd) = weekday_from_name(&rule.weekday) {
                holidays.insert(nth_weekday_of_month(year, rule.month, wd, rule.nth));
            }
        }

        for rule in &self.rules.last_weekday {
            if !rule_applies(rule.from_year, rule.to_year, year) {
                continue;
            }
            if let Some(wd) = weekday_from_name(&rule.weekday) {
                holidays.insert(last_weekday_of_month(year, rule.month, wd));
            }
        }

        for rule in &self.rules.overrides {
            if let Ok(d) = NaiveDate::parse_from_str(&rule.date, "%Y-%m-%d") {
                if d.year() != year {
                    continue;
                }
                match rule.action.as_str() {
                    "add" => {
                        holidays.insert(d);
                    }
                    "remove" => {
                        holidays.remove(&d);
                    }
                    _ => {}
                }
            }
        }

        holidays.retain(|d| d.weekday() != Weekday::Sat && d.weekday() != Weekday::Sun);
        self.holiday_cache.insert(year, holidays);
        &self.holiday_cache[&year]
    }

    pub fn is_session(&mut self, d: NaiveDate) -> bool {
        if d.weekday() == Weekday::Sat || d.weekday() == Weekday::Sun {
            return false;
        }
        !self.holidays_for_year(d.year()).contains(&d)
    }

    pub fn sessions_in_range(&mut self, start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
        let mut result = Vec::new();
        let mut d = start;
        while d <= end {
            if self.is_session(d) {
                result.push(d);
            }
            d = match d.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

pub fn load_schedule(path: &Path) -> Result<ExchangeSchedule, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let data: ScheduleYaml =
        serde_yaml::from_str(&raw).map_err(|e| format!("YAML {}: {e}", path.display()))?;
    Ok(ExchangeSchedule::from_yaml(data))
}

pub fn load_schedules(spec_root: &Path) -> Result<HashMap<String, ExchangeSchedule>, String> {
    let schedules_dir = spec_root.join("schedules");
    let mut result = HashMap::new();
    if !schedules_dir.is_dir() {
        return Ok(result);
    }
    let mut paths: Vec<_> = fs::read_dir(&schedules_dir)
        .map_err(|e| format!("read schedules dir: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "yaml").unwrap_or(false))
        .collect();
    paths.sort();

    for yaml_path in paths {
        let schedule = load_schedule(&yaml_path)?;
        result.insert(schedule.exchange.to_uppercase(), schedule);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easter_2026() {
        assert_eq!(
            easter_sunday(2026),
            NaiveDate::from_ymd_opt(2026, 4, 5).unwrap()
        );
    }

    #[test]
    fn easter_2024() {
        assert_eq!(
            easter_sunday(2024),
            NaiveDate::from_ymd_opt(2024, 3, 31).unwrap()
        );
    }

    #[test]
    fn easter_2023() {
        assert_eq!(
            easter_sunday(2023),
            NaiveDate::from_ymd_opt(2023, 4, 9).unwrap()
        );
    }
}
