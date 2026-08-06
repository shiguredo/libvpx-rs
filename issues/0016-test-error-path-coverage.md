# エラーパス・境界値のテストを追加する

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/refactor-error-path-tests
- Polished: {YYYY-MM-DD}

## 目的

`Encoder` / `Decoder` のエラーパスと機能パスのうち、テストが存在しない経路を固定する。

## 現状

以下のパスが未テスト (いずれも `src/lib.rs`):

- `Encoder::encode` / `Encoder::finish` の iter 未ドレインエラーパス (`ensure_iter_drained` がエラーを返す分岐。reconfigure のみテスト済み)
- `Decoder::decode` / `Decoder::finish` の iter 未ドレインエラーパス
- `Encoder::encode` のプレーンサイズ不一致エラーパス (フォーマット不一致はテスト済みだが、正しいフォーマットで誤ったバッファ長を渡すケースは未テスト)
- `Decoder::next_frame` の unsupported image format エラーパス
- `Error::detail` (`vpx_codec_error_detail` 経由) の取得パス
- `EncodedFrame::is_keyframe()` / `width()` / `height()` (`tests/psnr.rs` は `force_keyframe: true` でエンコードしているのに `is_keyframe()` を一度も assert していない)
- VP8 / VP9 固有設定 (`Vp9Config` / `Vp8Config` の各 `vpx_codec_control_` 経由パス) は全テストがデフォルト設定のため一度も実行されない
- FPS 変更後の `reconfigure` → `encode` 継続 (PTS 単調性の扱い) のテスト

## 設計方針

- 上記の各パスを 1 テスト 1 観点で追加する (テストコメントは日本語)
- VP8 / VP9 固有設定は、libvpx が受け付ける代表値 (aq_mode / row_mt / ARNR / token_partitions 等) でエンコーダー生成とエンコード成功を確認する

## 完了条件

- 上記の全パスにテストが存在し、CI で通る
- テストコメントが日本語で付与されている
