# Implementation overview

## Phases

1. **Futures parity with tickerforge-py** — `load_spec` / `load_spec_from_path`, `TickerForge` / `TickerParser` (`new` vs `with_spec_path`), `generate_ticker_for_contract`, `parse_ticker`, month codes, contract cycles, expiration rules, `bdays`-backed calendars.
2. **Options** — Load `spec/contracts/b3/options.yaml`, `OptionGenerator` for equity / IBOV / DOL / IDI, CSV tests where calendar parity allows.
3. **Tooling** — `rustfmt`, `clippy -D warnings`, GitHub Actions, pre-commit, optional Codecov via `cargo llvm-cov`.

## Module map (Rust ↔ Python)

| Python | Rust |
|--------|------|
| `models.py` | `models.rs`, `options_models.rs` |
| `spec_loader.py` | `spec_loader.rs`, `options_spec.rs` |
| `month_codes.py` | `month_codes.rs` |
| `contract_cycle.py` | `contract_cycle.rs` |
| `calendars.py` | `calendars.rs` |
| `expiration_rules.py` | `expiration_rules.rs` |
| `ticker_generator.py` | `ticker_generator.rs` |
| `ticker_parser.py` | `ticker_parser.rs` |
| — | `options_ticker.rs`, `dates.rs` |

## Parity checklist

- [x] `load_spec` / `load_spec_from_path` (bundled default vs custom path)
- [x] IND generation and parse round-trip (representative dates)
- [x] Expiration spot checks (IND Jun, DOL Apr)
- [x] CSV: `futures_resolve` rows for WIN/IND
- [x] CSV: `options_resolve` equity + index (dollar/rate gated on calendar docs)
- [ ] Full `futures_resolve` / `options_resolve` vs `exchange_calendars` (needs calendar backend parity)
