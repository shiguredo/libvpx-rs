# PBT と fuzzing のテスト基盤を整備する

Created: 2026-05-11

Model: Opus 4.7

## 背景

AGENTS.md は以下を規定する。

- 「PBT は proptest を使うこと」
- 「Fuzzing は cargo-fuzz を使うこと」
- 「PBT のファイル名は `pbt/tests/prop_<module>.rs` とし、`src/<module>.rs` に
  対応させること」
- 「PBT でカバーできるものを単体テストで書かない」

しかし現状のリポジトリには `pbt/` ディレクトリも proptest 依存も存在せず、
`reconfigure_rejects_invalid_params` のような入力テーブル駆動の単体テストが
`src/lib.rs` 内に書かれている。

## 課題

- 規約と実装が乖離している。
- `apply_dynamic_cfg` の境界値検査 (`target_bitrate` の 0 / 999 / 1000 /
  1_000_000_000 / `usize::MAX`、`min_quantizer` / `max_quantizer` の関係、
  `fps_*` の `1_000_000_000` 境界など) は proptest の `Strategy` で網羅できる。
- libvpx の C 境界に渡る入力 (ImageData の各バリアントのバッファ長など) は
  fuzzing でクラッシュ耐性を確認する余地がある。

## 根拠

`/review-diff-code` のレビュー (Imp-7、M-6) での指摘。`feature/change-reconfigure`
で追加された入力テーブル駆動テストが PBT 化候補としてふさわしい。

## 対応案

- `pbt/` ワークスペースメンバーを追加する。`pbt/Cargo.toml` で proptest を依存に
  入れ、`pbt/tests/prop_encoder.rs` などにファイルを置く。
- `reconfigure_rejects_invalid_params` の検査ロジックを `apply_dynamic_cfg` の
  プロパティとして書き直す。例: 「`target_bitrate >= 1000` かつ
  `<= 1_000_000_000_000` のときのみ成功し、それ以外はエラー」。
- `Encoder::encode` のプレーンサイズ検査もプロパティ化する。
- `fuzz/` ワークスペースを追加し、`Decoder::decode` と `Encoder::encode` の
  fuzzing harness を整備する。

## 影響

依存追加とディレクトリ構造の変更。CHANGES.md は `### misc`。優先度は中
(規約遵守と将来の安全性向上)。
