//! Exchange trading calendars.
//!
//! When a spec-driven `ExchangeSchedule` is registered (loaded from
//! `spec/schedules/<exchange>.yaml`), sessions are computed from the schedule
//! rules.  Otherwise falls back to `bdays` holiday calendars.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bdays::calendars::brazil::BrazilExchange;
use bdays::calendars::us::USSettlement;
use bdays::calendars::WeekendsOnly;
use bdays::{HolidayCalendar, HolidayCalendarCache};
use chrono::NaiveDate;

use crate::schedule::ExchangeSchedule;

const RANGE_MIN: (i32, u32, u32) = (1990, 1, 1);
const RANGE_MAX: (i32, u32, u32) = (2035, 12, 31);

fn range_min_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(RANGE_MIN.0, RANGE_MIN.1, RANGE_MIN.2).unwrap()
}

fn range_max_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(RANGE_MAX.0, RANGE_MAX.1, RANGE_MAX.2).unwrap()
}

enum CalendarBackend {
    Bdays(HolidayCalendarCache<NaiveDate>),
    Spec(ExchangeSchedule),
}

/// Exchange calendar that can be backed by either `bdays` or a spec-driven schedule.
pub struct ExchangeCalendar {
    backend: CalendarBackend,
    first_session: NaiveDate,
    last_session: NaiveDate,
}

impl ExchangeCalendar {
    pub fn from_bdays<H: HolidayCalendar<NaiveDate>>(holiday_cal: H) -> Self {
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
            backend: CalendarBackend::Bdays(cache),
            first_session,
            last_session,
        }
    }

    pub fn from_schedule(mut schedule: ExchangeSchedule) -> Self {
        let dmin = range_min_date();
        let dmax = range_max_date();

        let mut d = dmin;
        while !schedule.is_session(d) {
            d = d
                .succ_opt()
                .expect("range must contain at least one session");
        }
        let first_session = d;

        let mut d = dmax;
        while !schedule.is_session(d) {
            d = d
                .pred_opt()
                .expect("range must contain at least one session");
        }
        let last_session = d;

        ExchangeCalendar {
            backend: CalendarBackend::Spec(schedule),
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

    /// All trading days in `[start, end]` clipped to the calendar range.
    pub fn sessions_in_range(&self, start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
        let clip_start = start.max(range_min_date());
        let clip_end = end.min(range_max_date());
        if clip_start > clip_end {
            return Vec::new();
        }
        match &self.backend {
            CalendarBackend::Bdays(cache) => {
                let mut out = Vec::new();
                let mut d = clip_start;
                loop {
                    if cache.is_bday(d) {
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
            CalendarBackend::Spec(schedule) => {
                let mut sched = schedule.clone();
                sched.sessions_in_range(clip_start, clip_end)
            }
        }
    }
}

fn calendar_for_exchange_code(code: &str) -> ExchangeCalendar {
    match code.to_uppercase().as_str() {
        "B3" => ExchangeCalendar::from_bdays(BrazilExchange),
        "CME" | "ICE" | "EUREX" => ExchangeCalendar::from_bdays(USSettlement),
        _ => ExchangeCalendar::from_bdays(WeekendsOnly),
    }
}

static CAL_CACHE: Mutex<Option<HashMap<String, Arc<ExchangeCalendar>>>> = Mutex::new(None);
static SCHEDULE_REGISTRY: Mutex<Option<HashMap<String, ExchangeSchedule>>> = Mutex::new(None);

/// Register spec-driven schedules. Clears the calendar cache so subsequent
/// `get_calendar` calls pick up the new schedules.
pub fn register_schedules(schedules: HashMap<String, ExchangeSchedule>) {
    {
        let mut guard = SCHEDULE_REGISTRY.lock().unwrap();
        *guard = Some(schedules);
    }
    let mut guard = CAL_CACHE.lock().unwrap();
    *guard = None;
}

/// Resolve calendar for an exchange code (cached). Prefers a registered
/// `ExchangeSchedule` over the `bdays` fallback.
pub fn get_calendar(exchange_code: &str) -> Arc<ExchangeCalendar> {
    let key = exchange_code.to_uppercase();
    let mut guard = CAL_CACHE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    let map = guard.as_mut().unwrap();
    map.entry(key.clone())
        .or_insert_with(|| {
            let sched_guard = SCHEDULE_REGISTRY.lock().unwrap();
            if let Some(ref registry) = *sched_guard {
                if let Some(schedule) = registry.get(&key) {
                    return Arc::new(ExchangeCalendar::from_schedule(schedule.clone()));
                }
            }
            Arc::new(calendar_for_exchange_code(&key))
        })
        .clone()
}
