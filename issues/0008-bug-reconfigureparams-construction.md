# ReconfigureParams が #[non_exhaustive] のため外部クレートから構築できない

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-reconfigureparams-construction
- Polished: {YYYY-MM-DD}

## 目的

`Encoder::reconfigure` が外部クレートから実際に使えるようにする。

## 現状

`ReconfigureParams` (`src/lib.rs`) には `#[non_exhaustive]` が付いており、エディション 2024 の現在の rustc では、外部クレートから

```rust
ReconfigureParams {
    target_bitrate: Some(500_000),
    ..ReconfigureParams::default()
}
```

のような構造体リテラルでの構築が E0639 ("cannot create non-exhaustive struct using struct expression") でコンパイルエラーになる (実測済み)。外部クレートが入手できるのは全フィールド `None` の `Default::default()` のみで、`Encoder::reconfigure` は `is_empty()` が真のとき何もせず `Ok(())` を返すため、**reconfigure は外部から実質的に no-op しか呼び出せない**。

クレート内のテスト (`src/lib.rs` の `#[cfg(test)]` モジュール) は同一クレート内のため構造体リテラルが使え、この問題は検出されていない。`README.md` にも reconfigure の記載がない。

## 設計方針

`#[non_exhaustive]` を外す (shiguredo-rust 規約「`#[non_exhaustive]` を使わないこと」にも合致)。フィールドを追加する必要が生じた時点で破壊的変更として扱う。あわせて、外部クレートから reconfigure を使うコードがコンパイルできることを検証するテスト (または CI での利用例) を追加する。

## 完了条件

- `#[non_exhaustive]` が外れ、外部クレートから `ReconfigureParams` をフィールド指定で構築できる
- 外部クレートから `Encoder::reconfigure` を呼び、設定変更が反映されることが検証されている
