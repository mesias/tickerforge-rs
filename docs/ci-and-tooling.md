# CI and tooling

## Coverage and coverage gates

See **[Coverage and quality gates (llvm-cov)](coverage-and-quality-gates.md)** for how LCOV/Cobertura XML reports and the **80% line coverage** minimum are enforced in CI and pre-commit.

## GitHub Actions

Workflow: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

- **lint** — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
- **test** — `cargo llvm-cov` (tests + LCOV + `--fail-under-lines 80`), then `cargo llvm-cov report` for **Cobertura XML** (`cobertura.xml`), artifact upload of that file, and Codecov upload of `lcov.info` + `cobertura.xml` (optional `CODECOV_TOKEN`)

## pre-commit

[`.pre-commit-config.yaml`](../.pre-commit-config.yaml) runs `fmt`, `clippy`, and **`cargo-llvm-cov`** (LCOV + Cobertura XML + 80% line floor). Install [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) first (`cargo install cargo-llvm-cov` or `cargo binstall cargo-llvm-cov`). Details: [coverage-and-quality-gates.md](coverage-and-quality-gates.md).

```bash
pip install pre-commit
pre-commit install
```

## MSRV

`rust-version` in `Cargo.toml` is **1.74** (match your team policy; bump when using newer language features).
