# CI の検査を強化する (clippy --all-targets、docs-rs 警告、MSRV 検証)

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/update-ci-checks
- Polished: {YYYY-MM-DD}
- Updated: 2026-10-03

## 目的

ローカルフック (prek) と CI の検査範囲を揃え、ドキュメント欠落と MSRV 違反を CI で検出できるようにする。

## 現状

- `.github/workflows/ci.yml` の fmt-clippy ジョブは `cargo clippy --features source-build -- -D warnings` で、`--all-targets` が無い。`tests/` と `src/lib.rs` の `#[cfg(test)]` モジュールは clippy の対象外。一方 `prek.toml` は `--all-targets` 付きで、ローカルと CI で検査範囲が食い違う
- docs-rs ジョブは `cargo doc --no-deps` のみで `RUSTDOCFLAGS="-D warnings"` が無く、`#![warn(missing_docs)]` の警告や壊れた intra-doc link を検出できない
- `Cargo.toml` の `rust-version` (MSRV) は 1.93 で、`rust-toolchain.toml` の `channel` も 1.93 に固定されている。CI の全ジョブはこのツールチェーンで実行されるため MSRV 違反は検出されるが、MSRV 専用の検証ジョブは無い

## 設計方針

- fmt-clippy ジョブに `--all-targets` を追加して prek と揃える
- docs-rs ジョブに `RUSTDOCFLAGS="-D warnings"` を追加する

## 完了条件

- CI の clippy が `--all-targets` で実行されている
- docs-rs ジョブが `-D warnings` で実行されている
