# tickerforge (Rust)

[![codecov](https://codecov.io/gh/mesias/tickerforge-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/mesias/tickerforge-rs)
[![CI](https://github.com/mesias/tickerforge-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mesias/tickerforge-rs/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://github.com/mesias/tickerforge-rs/blob/main/Cargo.toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/mesias/tickerforge-rs/blob/main/LICENSE)

[![rustfmt](https://img.shields.io/badge/rustfmt-000000?logo=rust&logoColor=white)](https://github.com/rust-lang/rustfmt)
[![clippy](https://img.shields.io/badge/clippy-000000?logo=rust&logoColor=white)](https://github.com/rust-lang/rust-clippy)

Rust library that loads the default [`tickerforge-spec`](https://github.com/mesias/tickerforge-spec) YAML tree from the [`tickerforge-spec-data`](https://github.com/mesias/tickerforge-spec) crate (git dependency, same content as the Python `tickerforge-spec-data` wheel) and generates or parses **futures and options** tickers from **all supported markets** (parity with [`tickerforge-py`](https://github.com/mesias/tickerforge-py)). Option rules are loaded automatically from all `spec/contracts/**/*.yaml` files.

Trading sessions use the [`bdays`](https://crates.io/crates/bdays) crate (B3 `BrazilExchange`, US `USSettlement` for CME). Full alignment with Python’s `exchange_calendars` is not guaranteed; see [`docs/calendar-strategy.md`](docs/calendar-strategy.md).

## Install

From a git checkout:

```bash
cargo add --git https://github.com/mesias/tickerforge-rs.git tickerforge
```

Or path dependency:

```toml
tickerforge = { path = "../tickerforge-rs" }
```

## Usage

### Generating tickers

```rust
use tickerforge::TickerForge;

let forge = TickerForge::new().expect("spec");
let ticker = forge.generate("IND", "2026-06-01", 0).expect("generate");
assert_eq!(ticker, "INDM26");
```

`TickerForge::new()` loads the bundled default spec from `tickerforge-spec-data`. Use `TickerForge::with_spec_path(path)` (and `TickerParser::with_spec_path(path)`) for a custom spec directory. The default root path is also available as `tickerforge::default_spec_root`.

### Parsing tickers — futures and options (`parse_any_ticker*`)

`parse_any_ticker*` parses **both futures and options** in a single call and returns an `AnyParsedTicker` enum.

```rust
use tickerforge::{parse_any_ticker, parse_any_ticker_exchange, AnyParsedTicker};

// Futures ticker
match parse_any_ticker("INDM26").unwrap() {
    AnyParsedTicker::Futures(f) => {
        assert_eq!(f.symbol, "IND");
        assert_eq!(f.year, 2026);
        assert_eq!(f.month, 6);
    }
    _ => unreachable!(),
}

// B3 equity option (PETR4, January call, strike 30)
match parse_any_ticker("PETRA30").unwrap() {
    AnyParsedTicker::Option(o) => {
        assert_eq!(o.underlying_or_symbol, "PETR4");
        assert!(o.is_call);
        assert_eq!(o.month, 1);
        assert_eq!(o.strike, "30");
    }
    _ => unreachable!(),
}

// CME futures
match parse_any_ticker("ESM26").unwrap() {
    AnyParsedTicker::Futures(f) => assert_eq!(f.contract.exchange, "CME"),
    _ => unreachable!(),
}

// B3 index option
match parse_any_ticker("IBOVK26C120000").unwrap() {
    AnyParsedTicker::Option(o) => {
        assert_eq!(o.underlying_or_symbol, "IBOV");
        assert_eq!(o.month, 5); // K = May
        assert_eq!(o.year, Some(2026));
    }
    _ => unreachable!(),
}

// Exchange filter (returns Err if ticker doesn't match that exchange)
let any = parse_any_ticker_exchange("ESM26", "CME").unwrap();
```

Five free functions cover every combination:

| Function | Spec | Date | Exchange |
|---|---|---|---|
| `parse_any_ticker(ticker)` | bundled | today | — |
| `parse_any_ticker_date(ticker, date)` | bundled | explicit | — |
| `parse_any_ticker_spec(ticker, spec)` | custom | today | — |
| `parse_any_ticker_date_spec(ticker, date, spec)` | custom | explicit | — |
| `parse_any_ticker_exchange(ticker, exchange)` | bundled | today | explicit |

### Parsing tickers — futures only (legacy API)

The `parse_ticker*` free functions remain available and return `ParsedFuturesTicker` directly:

```rust
use tickerforge::{parse_ticker, parse_ticker_date, parse_ticker_spec, parse_ticker_date_spec, load_spec};

let parsed = parse_ticker("INDM26").expect("parse");
let parsed = parse_ticker_date("IND", "2026-06-01").expect("parse");
let spec = load_spec().expect("spec");
let parsed = parse_ticker_spec("DOLK26", &spec).expect("parse");
let parsed = parse_ticker_date_spec("DOL", "2026-04-15", &spec).expect("parse");
```

### `TickerParser` — stateful parsing (futures + options)

`TickerParser` wraps a loaded spec for repeated calls. Methods now return `AnyParsedTicker`.

```rust
use tickerforge::{TickerParser, AnyParsedTicker};

let parser = TickerParser::new();

// Parse any ticker (futures or option)
let any = parser.parse("PETRA30").expect("parse");
let any = parser.parse_exchange("INDM26", "B3").expect("parse");
let any = parser.parse_date("IND", "2026-06-01").expect("parse");
```

### Builder pattern

`TickerParser::builder()` provides a fluent API for configuration and one-shot parsing.
The builder uses **typestate** generics: `parse()` is only available at compile time after `ticker()` has been called.

```rust
use tickerforge::{TickerParser, AnyParsedTicker};

// Build a reusable parser (default spec)
let parser = TickerParser::builder().build().expect("build");
parser.parse("INDM26").expect("parse");

// One-shot parse — full futures ticker
let any = TickerParser::builder()
    .ticker("INDM26")
    .parse()
    .expect("parse");

// One-shot parse — option with exchange filter
let any = TickerParser::builder()
    .ticker("PETRA30")
    .exchange("B3")
    .parse()
    .expect("parse");

// One-shot parse — root symbol with date
let any = TickerParser::builder()
    .ticker("IND")
    .reference_date("2026-06-01")
    .parse()
    .expect("parse");
```

### Contract-centric (tick, session, trading symbol)

`load_spec` / `get_contract` yield a `ContractSpec` with tick size, merged session windows, timezone, and trading-symbol helpers. **Default** helpers call `load_spec()` internally; use the `*_with_spec` variants to reuse an already-loaded `SpecRepository`:

```rust
use tickerforge::load_spec;

let spec = load_spec()?;
let dol = spec.get_contract("DOL")?;

dol.tick_size;
dol.regular_session_start_end();
dol.exchange_timezone;
// `dol.sessions` is `Vec<SessionSegment>` in YAML map key order; map keys become `name`.

// Bundled default spec (no extra `&SpecRepository` argument)
dol.trading_symbol_today()?;
dol.trading_symbol_for("2026-03-15", 0)?;

// Reuse `spec` (e.g. from `load_spec_from_path`)
dol.trading_symbol_today_with_spec(&spec)?;
dol.trading_symbol_for_with_spec(&spec, "2026-03-15", 0)?;
```

### Options — parsing

```rust
use tickerforge::{parse_any_ticker, AnyParsedTicker};

// B3 equity option
match parse_any_ticker("PETRA30").unwrap() {
    AnyParsedTicker::Option(o) => println!("{} {} {}", o.kind, o.underlying_or_symbol, o.strike),
    _ => unreachable!(),
}

// DOL option vs future — no ambiguity
assert!(matches!(parse_any_ticker("DOLK26").unwrap(), AnyParsedTicker::Futures(_)));
assert!(matches!(parse_any_ticker("DOLK26C5000").unwrap(), AnyParsedTicker::Option(_)));
```

### Options — generation (B3)

```rust
use tickerforge::options_ticker::OptionGenerator;

let gen = OptionGenerator::bundled().expect("load");
let t = gen
    .generate_equity("PETR4", "2026-01-16", true, 35, 0)
    .expect("equity option");
assert_eq!(t, "PETRA35");
```

## What is supported

- YAML spec loading (exchanges, contract cycles, expiration rules, futures and **options** from all `contracts/**/*.yaml`)
- Multi-market futures generation and parsing: **B3** (IND, DOL, WIN, …) and **CME** (ES, NQ, …)
- Multi-market options **parsing** via `parse_any_ticker*` and `OptionParser`: B3 equity, index (IBOV), dollar (DOL), interest-rate (IDI); more markets added automatically from spec
- B3 options **generation** via `OptionGenerator` (equity month codes, IBOV month letters **A–L**, DOL/IDI futures-style month codes)
- Unit tests + CSV examples from `spec/tests/` (subset where `bdays` matches golden expectations)

## Run tests

```bash
cargo test
```

Pre-commit (same idea as `tickerforge-py`):

```bash
pip install pre-commit
pre-commit install
pre-commit run --all-files
```

## Spec updates

Copy or sync from [`tickerforge-spec`](https://github.com/mesias/tickerforge-spec):

```bash
rsync -a --delete ../tickerforge-spec/spec/ ./spec/
```

## Implementation notes

See [`docs/`](docs/) for calendar strategy, options mapping, and CI/tooling.
