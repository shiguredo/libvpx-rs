# src/lib.rs を機能単位のモジュールに分割する

Created: 2026-05-11

Model: Opus 4.7

## 背景

`src/lib.rs` は `feature/change-reconfigure` のマージ後に 2200 行を超える。
内訳は概ね次のとおり。

- `Error` / `Display` / `std::error::Error` 実装
- `DecoderCodec` / `DecoderConfig` / `Decoder` / `DecodedFrame`
- `ImageFormat` / `ImageData` / `PlaneSizes`
- `EncoderConfig` / `EncodingDeadline` / `RateControlMode` / `Vp9Profile` /
  `Vp9Config` / `Vp8Config` / `ContentType` / `CodecConfig` / `ArnrConfig` /
  `EncodeOptions` / `ReconfigureParams`
- `Encoder` / `EncodedFrame` / `apply_dynamic_cfg` 等の共通ロジック
- `#[cfg(test)] mod tests`

## 課題

- AGENTS.md は「テストファイルが長くなった場合はファイル内で `mod` を使って分割
  すること」「テストが長くなるのはモジュール自体が大きすぎるサイン」と明記する。
  現状はこのサインに該当する。
- AGENTS.md は「単体テストのファイル名は `tests/test_<module>.rs` とし、
  `src/<module>.rs` に対応させること」と規定するが、現状は `src/lib.rs` 内の
  `#[cfg(test)] mod tests` に集中している。
- 1 ファイルが大きすぎて、Decoder と Encoder の責務境界が読みづらい。

## 根拠

`/review-diff-code` のレビュー (Imp-7) での指摘。`feature/change-reconfigure` で
360 行近いテストが追加されてさらに肥大化したため、独立 issue 化した。

## 対応案

以下のような分割を想定する。

- `src/error.rs`: `Error` 型と `invalid_param` ヘルパー
- `src/decoder.rs`: `DecoderCodec` / `DecoderConfig` / `Decoder` / `DecodedFrame`
- `src/image.rs`: `ImageFormat` / `ImageData` / `PlaneSizes`
- `src/encoder/config.rs`: `EncoderConfig` 系 (`EncodingDeadline` /
  `RateControlMode` / `Vp9Profile` / `Vp9Config` / `Vp8Config` / `ContentType` /
  `CodecConfig` / `ArnrConfig` / `EncodeOptions` / `ReconfigureParams` /
  `apply_dynamic_cfg` の宣言)
- `src/encoder/mod.rs`: `Encoder` / `EncodedFrame` 本体
- `tests/test_decoder.rs` / `tests/test_encoder.rs`: 既存の `mod tests` から
  移植する単体テスト

`pub(crate)` の公開範囲とフィールド可視性を再点検する必要がある。`Error::function`
など現在 `pub(crate)` でもない項目をクレート内で共有する都合上、`pub(crate)` への
昇格が必要なフィールドが出る。

優先度は中。新規機能ではなくリファクタリング。CHANGES.md は `### misc` で扱う。
