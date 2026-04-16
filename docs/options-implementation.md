# Options implementation

## YAML — multi-market loading

Option rules are loaded from **all** `spec/contracts/**/*.yaml` files that contain an `options:` key, via `load_all_option_rules(spec_root)` in `options_spec.rs`. This mirrors Python's `_load_options`.

For backward-compatibility, `load_option_rules(path)` still loads from a single file (used by `OptionGenerator`).

The bundled spec currently provides:

| Market | File |
|--------|------|
| B3 | `spec/contracts/b3/options.yaml` |

Additional markets are supported as soon as a `options:` block appears in any `contracts/<market>/*.yaml` file.

`SpecRepository.options: Vec<OptionRule>` holds all loaded option rules alongside futures contracts.

## Serde model

The serde enum `OptionRule` (in `options_models.rs`) distinguishes four variants by the YAML `type:` field:

| `type` | Rust variant | Use case |
|--------|-------------|---------|
| `equity` | `EquityOptionRule` | Options on listed equities (e.g. PETR4, VALE3) |
| `index` | `IndexOptionRule` | Options on equity indices (e.g. IBOV) |
| `dollar` | `DollarOptionRule` | Options on USD/BRL (e.g. DOL) |
| `interest_rate` | `InterestRateOptionRule` | Options on interest-rate indices (e.g. IDI) |

## Ticker formats

### Equity — `{equity_root}{month_code}{strike}`

- `equity_root(underlying)` strips one trailing digit: `PETR4` → `PETR`, `BOVA11` → `BOVA1`.
- `month_code` is a letter from `call_month_codes` (A–L) for calls, `put_month_codes` (M–X) for puts.
- No year in the ticker (`year = None` on `ParsedOptionTicker`).

Examples: `PETRA30` (PETR4 Jan call, strike 30), `PETRM30` (PETR4 Jan put).

### Index / Dollar / Interest Rate — `{symbol}{month_code}{yy}{C|P}{strike}`

- `month_code` uses futures month codes **FGHJKMNQUVXZ** (same as futures contracts).
- Year is `2000 + yy`.
- Call/put code is specified in `option_type_codes` in the YAML.

Examples: `IBOVK26C120000` (IBOV May 2026 call, strike 120000), `DOLK26C5000`.

## `ParsedOptionTicker` fields

| Field | Type | Description |
|-------|------|-------------|
| `kind` | `String` | `"equity"` / `"index"` / `"dollar"` / `"interest_rate"` |
| `underlying_or_symbol` | `String` | Full underlying (`"PETR4"`) or root symbol (`"IBOV"`) |
| `year` | `Option<i32>` | Contract year (`2000 + yy`). `None` for equity options |
| `month` | `u32` | Contract month (1–12) |
| `is_call` | `bool` | `true` = call, `false` = put |
| `strike` | `String` | Raw strike string from the ticker (e.g. `"120000"`) |
| `exchange` | `String` | Exchange code (e.g. `"B3"`) |
| `tick_size` | `Option<f64>` | Minimum price increment from the option rule |
| `lot_size` | `Option<f64>` | Contract multiplier from the option rule |

## `OptionParser` API

```rust
// Collect all matches (may be 0, 1, or many)
let candidates: Vec<ParsedOptionTicker> =
    OptionParser::parse_options(ticker, &spec, exchange_filter);

// Return single match or Err for 0 / multiple matches
let parsed: ParsedOptionTicker =
    OptionParser::parse_option(ticker, &spec)?;

// With optional exchange filter
let parsed: ParsedOptionTicker =
    OptionParser::parse_option_exchange(ticker, &spec, Some("B3"))?;
```

Ambiguity returns an `Err(String)` listing all matched instruments and a hint to pass `exchange=`.

## `AnyParsedTicker` — unified enum

Use `parse_any_ticker*` (see `smart-ticker-parsing.md`) to parse both futures and options without knowing in advance which kind the ticker is.

## Tests

- `tests/options_resolve_csv.rs` — generation round-trip against `spec/tests/b3/options_resolve.csv`.
- `tests/option_parsing.rs` — 30 parsing tests covering equity, index, dollar, interest-rate options, DOL disambiguation, exchange filter, CME futures via `parse_any_ticker`.
