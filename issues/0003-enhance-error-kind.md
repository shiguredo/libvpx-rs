# Error に構造化アクセサを追加する

Created: 2026-05-11

Model: Opus 4.7

## 背景

現在の `Error` (`src/lib.rs`) は次のフィールドを持つ。

```rust
pub struct Error {
    code: sys::vpx_codec_err_t,
    function: &'static str,
    reason: Option<&'static str>,
    detail: Option<String>,
}
```

公開アクセサは `std::fmt::Display` のみで、利用者は文字列マッチでエラー種別を判定
するしかない。ラッパー内部のテストも `err.to_string().contains(...)` で判定して
いる (`reconfigure_rejects_invalid_params` など)。

## 課題

- エラー文言を一文字変えるだけで利用者・テストが壊れる脆さがある。
- libvpx 由来の `vpx_codec_err_t` と、ラッパー由来の検査失敗 (`invalid_param`) を
  区別する手段がない。
- `code` / `function` / `reason` / `detail` を個別に取得する手段がない。

## 根拠

`/review-diff-code` のレビュー (Imp-8、Sug-3) での指摘。`reconfigure` の新規テスト
群が文字列マッチに依存しており、将来の文言整理コストの懸念が顕在化している。

## 対応案

- 公開アクセサを追加する: `code() -> vpx_codec_err_t`、`function() -> &str`、
  `reason() -> Option<&str>` (現在 `pub(crate)` 相当)、`detail() -> Option<&str>`。
- `vpx_codec_err_t` がそのまま外に出ると `sys::*` を公開することになるため、
  ラッパー独自の `ErrorKind` enum を導入して `vpx_codec_err_t` の主要バリアントに
  対応させる案も検討する。
- `ErrorKind` に `InvalidParam { reason: &'static str }` のようなバリアントを
  入れ、ラッパー検査の失敗時にも構造化された情報を返せるようにする。
- テストを `err.kind() == ErrorKind::InvalidParam { reason: "..." }` の形に
  書き直す。

## 影響

公開 API への型追加のみ。既存ユーザーへの後方互換は保てる。CHANGES.md は `[ADD]`。
優先度は中。
