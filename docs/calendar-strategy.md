# Calendar strategy

## Default backend: `bdays`

The crate uses [`bdays`](https://crates.io/crates/bdays) with:

- **B3** → `bdays::calendars::brazil::BrazilExchange`
- **CME / ICE / EUREX** (fallback) → `USSettlement` or `WeekendsOnly` where appropriate

`HolidayCalendarCache` covers a fixed range (1990–2035) for fast `is_bday` checks.

## Parity with Python

Python uses [`exchange_calendars`](https://github.com/rsheftel/pandas_market_calendars) (e.g. B3 → BVMF). Session sets can differ from `bdays` (holidays, early closes, horizon). That affects:

- **FX futures** (DOL, WDO, DI1) rolling vs `spec/tests/b3/futures_resolve.csv`
- **Dollar / rate options** rows in `options_resolve.csv`

Integration tests assert **WIN/IND** futures rows and **equity + index** options rows that match `bdays` today. Other CSV rows are left for when a closer calendar backend or precomputed sessions are added.

## Skipped months

If `resolve_expiration` fails for a month (e.g. rare `third_friday` edge cases), that month is skipped when building the eligible chain so generation does not panic.
