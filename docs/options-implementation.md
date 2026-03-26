# Options implementation

## YAML

Loaded from `spec/contracts/b3/options.yaml` via `load_option_rules()` (`options_spec.rs`).

Serde enum `OptionRule` distinguishes `equity`, `index`, `dollar`, `interest_rate`.

## Ticker formats

- **Equity** — `{root}{month_code}{strike}` with `call_month_codes` / `put_month_codes` (A–L). Root = underlying with trailing digit removed (e.g. `PETR4` → `PETR`).
- **IBOV index** — `{symbol}{month_code}{yy}{C|P}{strike}` with **month letters A–L** (January=A, June=F, …), *not* futures FGHJKMNQUVXZ.
- **DOL / IDI** — Same template as index, but month codes use **futures** `month_to_code` (FGHJKMNQUVXZ). Strike is zero-padded to 6 digits in the ticker.

## Tests

`tests/options_resolve_csv.rs` runs rows from `spec/tests/b3/options_resolve.csv`, skipping `dollar` and `interest_rate` until calendar parity with Python’s exchange sessions is improved (see `calendar-strategy.md`).
