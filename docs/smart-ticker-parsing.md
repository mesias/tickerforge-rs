# Smart Ticker Parsing

## Summary

The ticker parser accepts both **full tickers** (`INDM26`, `DOLK26`, `WINZ25`, `PETRA30`, `IBOVK26C120000`) and **root symbols** (`IND`, `DOL`, `WIN`).

`parse_any_ticker*` (and the `TickerParser` methods) parse **futures and options** in a single call, returning an `AnyParsedTicker` enum. The legacy `parse_ticker*` free functions remain available and return `ParsedFuturesTicker` for backward compatibility.

Previously, a `reference_date` was always required to interpret the 2-digit year. The new behaviour removes this requirement for full tickers and only uses the date when resolving root symbols to their front-month contract.

## Behaviour

### Full futures ticker (e.g. `INDM26`)

Year and month are extracted directly from the ticker string:

- **Month** is decoded from the standard futures month code (`M` = June).
- **Year** is `2000 + yy` (`26` = 2026).

`reference_date` is **ignored** when a full ticker is provided.

### Option ticker (e.g. `PETRA30`, `IBOVK26C120000`)

Matched against option rules from `SpecRepository.options` (all markets):

- **Equity** — pattern `{equity_root}{month_code}{strike}` using `call_month_codes`/`put_month_codes`.
- **Non-equity** — pattern `{symbol}{month_code}{yy}{C|P}{strike}`.

No `reference_date` needed; the ticker encodes all date information (equity options have `year = None`).

### Root symbol (e.g. `IND`)

When the input does not match any `ticker_format` pattern but matches a known contract symbol, the parser resolves the front-month contract:

1. If a reference date is provided, it is used as the as-of date.
2. If omitted, **today** is used.
3. The generator produces the full ticker for that front-month, which is then parsed with the full-ticker path.

### Unknown input

Returns `Err(String)` containing `"Unable to parse ticker"`.

### Ambiguous ticker

When a ticker matches instruments on multiple markets/types and no `exchange` filter is applied, returns `Err(String)` listing all matches and a hint to disambiguate with `exchange=`.

## `AnyParsedTicker` enum

```rust
pub enum AnyParsedTicker {
    Futures(ParsedFuturesTicker),
    Option(ParsedOptionTicker),
}
```

Pattern-match on the variant to access the inner struct:

```rust
match parse_any_ticker("PETRA30")? {
    AnyParsedTicker::Futures(f) => println!("futures: {} {}/{}", f.symbol, f.month, f.year),
    AnyParsedTicker::Option(o) => println!("option: {} strike {} is_call={}", o.underlying_or_symbol, o.strike, o.is_call),
}
```

## API — `parse_any_ticker*` free functions

Five free functions cover every combination:

| Function | Spec | Date | Exchange |
|---|---|---|---|
| `parse_any_ticker(ticker)` | bundled default | today (root symbols) | none |
| `parse_any_ticker_date(ticker, date)` | bundled default | explicit | none |
| `parse_any_ticker_spec(ticker, spec)` | custom | today | none |
| `parse_any_ticker_date_spec(ticker, date, spec)` | custom | explicit | none |
| `parse_any_ticker_exchange(ticker, exchange)` | bundled default | today | explicit |

The legacy `parse_ticker*` free functions (futures-only, return `ParsedFuturesTicker`) remain unchanged for backward compatibility:

| Function | Spec | Date |
|---|---|---|
| `parse_ticker(ticker)` | bundled default | today (for root symbols) |
| `parse_ticker_date(ticker, date)` | bundled default | explicit |
| `parse_ticker_spec(ticker, spec)` | custom | today |
| `parse_ticker_date_spec(ticker, date, spec)` | custom | explicit |

## API — `TickerParser` methods

`TickerParser` methods now return `AnyParsedTicker`:

| Method | Date | Exchange |
|---|---|---|
| `parser.parse(ticker)` | today | none |
| `parser.parse_exchange(ticker, exchange)` | today | explicit |
| `parser.parse_date(ticker, date)` | explicit | none |

## Builder pattern (typestate)

`TickerParser::builder()` returns a `TickerParserBuilder<NoTicker>`.  The generic parameter tracks whether a ticker has been set at **compile time**:

| State | `build()` | `parse()` |
|---|---|---|
| `NoTicker` | available | **not available** (compile error) |
| `HasTicker` | available | available |

Builder methods: `spec_path(&Path)`, `spec(&str)`, `ticker(&str)`, `reference_date(&str)`, `exchange(&str)`.

| Terminal method | Returns | When to use |
|---|---|---|
| `.build()` | `Result<TickerParser, String>` | Build a reusable parser |
| `.parse()` | `Result<AnyParsedTicker, String>` | One-shot: load spec + parse in one call |

### Examples

```rust
use tickerforge::{parse_any_ticker, parse_any_ticker_exchange, AnyParsedTicker, TickerParser};

// Parse any ticker — futures or option
match parse_any_ticker("PETRA30").unwrap() {
    AnyParsedTicker::Futures(_) => unreachable!(),
    AnyParsedTicker::Option(o) => {
        assert_eq!(o.underlying_or_symbol, "PETR4");
        assert!(o.is_call);
        assert_eq!(o.month, 1);
    }
}

// Exchange filter
let any = parse_any_ticker_exchange("ESM26", "CME").unwrap();

// Builder with exchange filter
let any = TickerParser::builder()
    .ticker("DOLK26")
    .exchange("B3")
    .parse()
    .unwrap();

// Legacy futures-only parse (unchanged API)
use tickerforge::parse_ticker;
let futures = parse_ticker("INDM26").unwrap();
assert_eq!(futures.year, 2026);
```

## `ParsedFuturesTicker` fields

| Field | Type | Description |
|---|---|---|
| `symbol` | `String` | Root symbol (e.g. `IND`) |
| `year` | `i32` | Contract year |
| `month` | `u32` | Contract month (1–12) |
| `tick_size` | `Option<f64>` | Minimum price increment from the contract spec |
| `lot_size` | `Option<f64>` | Contract multiplier from the contract spec |
| `contract` | `ContractSpec` | Full contract specification object |
| `reference_date` | `Option<NaiveDate>` | Date used for root-symbol resolution; `None` for full tickers |
| `is_trading_session` | `Option<bool>` | Whether `reference_date` is an exchange trading session; `None` for full tickers |

### `is_trading_session` resolution flow

![is_trading_session resolution flow](is_trading_session_flow.svg)

When a **full ticker** is parsed, both `reference_date` and `is_trading_session` are `None` — no date context exists.

When a **root symbol** is parsed, the date (explicit or today) is used to resolve the front-month contract. The parser then checks the exchange calendar to determine whether that date is an actual trading session:

- **Weekday with exchange open** → `is_trading_session = Some(true)`
- **Weekend or exchange holiday** → `is_trading_session = Some(false)`

## `ParsedOptionTicker` fields

| Field | Type | Description |
|---|---|---|
| `kind` | `String` | `"equity"` / `"index"` / `"dollar"` / `"interest_rate"` |
| `underlying_or_symbol` | `String` | Full underlying (equity) or root symbol (others) |
| `year` | `Option<i32>` | Contract year; `None` for equity options |
| `month` | `u32` | Contract month (1–12) |
| `is_call` | `bool` | `true` = call, `false` = put |
| `strike` | `String` | Raw strike string from the ticker |
| `exchange` | `String` | Exchange code |
| `tick_size` | `Option<f64>` | Minimum price increment |
| `lot_size` | `Option<f64>` | Contract multiplier |

## Test coverage

`tests/ticker_parsing.rs` — 45 futures parsing tests (full tickers, root symbols, dates, sessions, builder).

`tests/option_parsing.rs` — 30 option parsing tests:

- `spec_loads_options_from_multiple_markets`, `spec_options_include_equity_type`, `spec_options_include_index_type`
- `parse_equity_call_option`, `parse_equity_put_option`, `parse_equity_call_june`, `parse_equity_put_december`, `parse_equity_option_tick_and_lot`
- `parse_ibov_call_option`, `parse_ibov_put_option`
- `parse_dol_call_option`, `parse_dol_put_option`
- `parse_idi_call_option`, `parse_idi_put_option`
- `futures_still_parse_via_any`, `cme_futures_parse_via_any`
- `dol_future_not_ambiguous_with_option`, `dol_option_not_ambiguous_with_future`
- `exchange_filter_*` (4 tests), `unknown_*` (2 tests)
- `option_parser_*` low-level API (3 tests), `parse_any_ticker_spec_*` (2 tests)
