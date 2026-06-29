# Offset tag syntax `SYMBOL[n]` (Rust)

`tickerforge-rs` supports a bracket-tag syntax on futures root symbols that
resolves the nth contract in the tradeable-contract list:

```
SYMBOL[n]
```

- `n >= 0` — the nth **still-tradeable** contract, counted from the front month.
  `DOL[0]` (or plain `DOL`) is the front month, `DOL[1]` is the next tradeable
  contract, and so on. This is the same indexing used by the existing unsigned
  `generate_ticker_for_contract(.., offset: usize)`.
- `n < 0` — the nth **most-recently-EXPIRED** contract. `IND[-1]` is the contract
  that most recently rolled off, `IND[-2]` is the one before that, etc.

The tag works for **every futures contract** in the spec — monthly (DOL) and
bimonthly (WIN/IND) cycles alike — and requires no YAML or schema changes.
Options are unaffected; the syntax is futures-only.

## Why a signed sibling?

Rust's `usize` cannot represent negative offsets, so the existing public API is
kept unchanged and **new signed siblings** are added alongside it (the same
"keep original, add new" pattern used elsewhere in the crate):

| Existing (unsigned, unchanged) | New (signed) |
| --- | --- |
| `generate_ticker_for_contract(.., offset: usize)` | `generate_ticker_for_contract_signed(.., offset: isize)` |
| `gen_ticker_ctr(..)` (offset 0, today) | `gen_ticker_ctr_signed(.., offset: isize)` (today) |
| `TickerForge::gen(symbol)` (offset 0, today) | `TickerForge::gen_signed(symbol, offset: isize)` (today) |

## Parsing

`parse_any_ticker*` recognises the tag automatically. When a full-ticker or
option match is not found, the parser tries `SYMBOL[n]`: it looks up the root in
`spec.contracts`, calls `generate_ticker_for_contract_signed`, re-parses the
resulting full ticker, and stamps `reference_date` / `is_trading_session` /
`contract_offset` on the `ParsedFuturesTicker`.

```rust
use tickerforge::parse_any_ticker_date;

// DOL[1] on 2026-06-29 -> DOLQ26
let any = parse_any_ticker_date("DOL[1]", "2026-06-29")?;
assert_eq!(any.ticker()?, "DOLQ26");

// IND[-1] on 2026-06-18 -> previous IND contract (INDM26).
// IND front month on that date is INDQ26.
let prev = parse_any_ticker_date("IND[-1]", "2026-06-18")?;
assert_eq!(prev.ticker()?, "INDM26");
```

`ParsedFuturesTicker.contract_offset` is `Some(n)` for tagged input and `None`
for plain roots (`DOL`) and full tickers (`DOLN26`).

## Generation

```rust
use tickerforge::{TickerForge, generate_ticker_for_contract_signed, load_spec};

// Front month + 1, for an explicit date.
let spec = load_spec()?;
let contract = spec.get_contract("DOL")?;
let t = generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, 1)?;
assert_eq!(t, "DOLQ26");

// Most-recently-expired DOL contract.
let t = generate_ticker_for_contract_signed(contract, "2026-06-29", &spec, -1)?;
assert_eq!(t, "DOLM26");

// Today, signed offset.
let forge = TickerForge::new()?;
let front_plus_one = forge.gen_signed("DOL", 1)?;
```

## Errors

- Out-of-range offsets (`DOL[999]`, `DOL[-999]`) return an `Err` whose message
  contains `out of range`.
- Unknown root (`ZZZ[1]`) returns the existing
  `"Unable to parse ticker: ..."` error.
- If an `exchange` filter is supplied and the resolved contract's exchange does
  not match, parsing falls through to the same `"Unable to parse ticker"` error.

## Semantics summary

| Input | Meaning |
| --- | --- |
| `DOL`   | front month (offset 0); `contract_offset = None` |
| `DOL[0]` | front month; `contract_offset = Some(0)` |
| `DOL[1]` | next tradeable contract after the front |
| `IND[-1]` | most-recently-EXPIRED IND contract |
| `IND[-2]` | the expired contract before `IND[-1]` |
| `DOLN26` | full ticker, parsed as-is; `contract_offset = None` |
