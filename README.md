# tickerforge (Rust)

[![codecov](https://codecov.io/gh/mesias/tickerforge-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/mesias/tickerforge-rs)
[![CI](https://github.com/mesias/tickerforge-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mesias/tickerforge-rs/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://github.com/mesias/tickerforge-rs/blob/main/Cargo.toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/mesias/tickerforge-rs/blob/main/LICENSE)

[![rustfmt](https://img.shields.io/badge/rustfmt-000000?logo=rust&logoColor=white)](https://github.com/rust-lang/rustfmt)
[![clippy](https://img.shields.io/badge/clippy-000000?logo=rust&logoColor=white)](https://github.com/rust-lang/rust-clippy)

Rust library that loads the default [`tickerforge-spec`](https://github.com/mesias/tickerforge-spec) YAML tree from the [`tickerforge-spec-data`](https://github.com/mesias/tickerforge-spec) crate (git dependency, same content as the Python `tickerforge-spec-data` wheel) and generates or parses **futures** tickers (parity with [`tickerforge-py`](https://github.com/mesias/tickerforge-py)). **Options** tickers are supported for B3 per `spec/contracts/b3/options.yaml` (see `OptionGenerator`).

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

### Parsing tickers (smart parsing)

The parser accepts **full tickers** (`INDM26`) or **root symbols** (`IND`).

Full tickers derive year/month directly from the string — no reference date required.
Root symbols resolve the front-month contract via the generator; the date defaults to today when omitted.

Four free functions cover every combination of spec / date:

```rust
use tickerforge::{
    parse_ticker, parse_ticker_date, parse_ticker_spec, parse_ticker_date_spec,
    load_spec,
};

// Full ticker — default spec, no date needed
let parsed = parse_ticker("INDM26").expect("parse");
assert_eq!(parsed.symbol, "IND");
assert_eq!(parsed.year, 2026);
assert_eq!(parsed.month, 6);

// Root symbol — default spec, explicit date
let parsed = parse_ticker_date("IND", "2026-06-01").expect("parse");

// Full ticker — custom spec, no date
let spec = load_spec().expect("spec");
let parsed = parse_ticker_spec("DOLK26", &spec).expect("parse");

// Root symbol — custom spec, explicit date
let parsed = parse_ticker_date_spec("DOL", "2026-04-15", &spec).expect("parse");
```

`TickerParser` wraps a loaded spec for repeated calls.
`new()` loads the bundled spec and panics on failure (the spec is compiled in, so this should never happen).
Use `try_new()` for a fallible alternative.

```rust
use tickerforge::TickerParser;

let parser = TickerParser::new();
let parsed = parser.parse("INDM26").expect("parse");
let parsed = parser.parse_date("IND", "2026-06-01").expect("parse");
```

### Builder pattern

`TickerParser::builder()` provides a fluent API for configuration and one-shot parsing.
The builder uses **typestate** generics: `parse()` is only available at compile time after `ticker()` has been called.

```rust
use tickerforge::TickerParser;

// Build a reusable parser (default spec)
let parser = TickerParser::builder().build().expect("build");
parser.parse("INDM26").expect("parse");

// Build a reusable parser (custom spec path)
let parser = TickerParser::builder()
    .spec_path(std::path::Path::new("/path/to/spec"))
    .build()
    .expect("build");

// One-shot parse — full ticker
let parsed = TickerParser::builder()
    .ticker("INDM26")
    .parse()
    .expect("parse");

// One-shot parse — root symbol with date
let parsed = TickerParser::builder()
    .ticker("IND")
    .reference_date("2026-06-01")
    .parse()
    .expect("parse");

// One-shot parse — custom spec + date
let parsed = TickerParser::builder()
    .spec_path(std::path::Path::new("/path/to/spec"))
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

### Options (B3)

```rust
use tickerforge::options_ticker::OptionGenerator;

let gen = OptionGenerator::bundled().expect("load");
let t = gen
    .generate_equity("PETR4", "2026-01-16", true, 35, 0)
    .expect("equity option");
assert_eq!(t, "PETRA35");
```

## What is supported

- YAML spec loading (exchanges, contract cycles, expiration rules, futures `contracts/**/*.yaml`)
- B3-focused futures generation and parsing (same public surface as Python)
- B3 options from `spec/contracts/b3/options.yaml` (equity month codes, IBOV month letters **A–L**, DOL/IDI futures-style month codes)
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
