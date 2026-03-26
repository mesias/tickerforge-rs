# tickerforge (Rust)

[![codecov](https://codecov.io/gh/mesias/tickerforge-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/mesias/tickerforge-rs)
[![CI](https://github.com/mesias/tickerforge-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mesias/tickerforge-rs/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://github.com/mesias/tickerforge-rs/blob/main/Cargo.toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/mesias/tickerforge-rs/blob/main/LICENSE)

[![rustfmt](https://img.shields.io/badge/rustfmt-000000?logo=rust&logoColor=white)](https://github.com/rust-lang/rustfmt)
[![clippy](https://img.shields.io/badge/clippy-000000?logo=rust&logoColor=white)](https://github.com/rust-lang/rust-clippy)

Rust library that loads the vendored [`tickerforge-spec`](https://github.com/mesias/tickerforge-spec) tree under `spec/` and generates or parses **futures** tickers (parity with [`tickerforge-py`](https://github.com/mesias/tickerforge-py)). **Options** tickers are supported for B3 per `spec/contracts/b3/options.yaml` (see `OptionGenerator`).

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

### Futures

```rust
use tickerforge::{TickerForge, TickerParser};

let forge = TickerForge::new(None).expect("spec");
let ticker = forge.generate("IND", "2026-06-01", 0).expect("generate");
assert_eq!(ticker, "INDM26");

let parser = TickerParser::new(None).expect("spec");
let parsed = parser.parse(&ticker, Some("2026-06-01")).expect("parse");
assert_eq!(parsed.symbol, "IND");
```

`TickerForge::new(None)` loads `spec/` next to the crate manifest (`CARGO_MANIFEST_DIR/spec`). Pass `Some(path)` to use another directory.

### Options (B3)

```rust
use tickerforge::options_ticker::OptionGenerator;

let gen = OptionGenerator::from_paths(None, None).expect("load");
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
