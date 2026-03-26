# Spec Integration

## Scope

This document covers Rust-specific behavior for consuming the canonical data from `tickerforge-spec`.

For the language-neutral contract, see the spec repo docs on rule-based schedules and session segments. This file only explains how the Rust crate maps those rules into its own runtime types.

## Rule-Based Schedules

The Rust implementation loads schedule definitions from `spec/schedules/*.yaml` during spec loading.

Current integration points:

- `src/schedule.rs`: loads YAML schedules and evaluates session dates
- `src/calendars.rs`: chooses between a spec-backed calendar and a fallback backend
- `src/spec_loader.rs`: loads schedules and registers them for later calendar lookups

The important boundary is that expiration logic consumes calendar sessions, not schedule YAML directly. That keeps schedule loading separate from ticker resolution.

## Session Segments

The canonical YAML stores `sessions` as a mapping keyed by segment name. Rust converts that mapping into an ordered `Vec<SessionSegment>`.

Current behavior:

- `src/models.rs` defines `SessionSegment`
- custom deserialization converts the YAML mapping into ordered segments
- the segment name comes from the YAML key
- validation requires the first segment to be `regular`
- merged `ContractSpec` values inherit session data from the owning asset when needed

This lets the spec remain compact while the Rust API exposes an ordered session list.

## Why This Lives Here

These details are specific to the Rust crate's internal model and loader boundaries. They do not belong in `tickerforge-spec`, because the spec repo should define the canonical format without prescribing Rust data structures.
