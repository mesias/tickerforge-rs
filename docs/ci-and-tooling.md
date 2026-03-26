# CI and tooling

## GitHub Actions

Workflow: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

- **lint** — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
- **test** — `cargo test`, then `cargo llvm-cov` and Codecov upload (optional `CODECOV_TOKEN`)

## pre-commit

[`.pre-commit-config.yaml`](../.pre-commit-config.yaml) runs `fmt`, `clippy`, and `cargo test` in one local hook.

```bash
pip install pre-commit
pre-commit install
```

## MSRV

`rust-version` in `Cargo.toml` is **1.74** (match your team policy; bump when using newer language features).
