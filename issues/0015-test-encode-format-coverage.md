# 非 I420 / NV12 フォーマットのラウンドトリップテストを追加し、PSNR 閾値を厳格化する

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/refactor-encode-format-tests
- Polished: {YYYY-MM-DD}

## 目的

`ImageFormat` として公開している 10 形式のうち、テストが存在しない 8 形式 (Yv12 / I422 / I444 / I440 / I42016 / I42216 / I44416 / I44016) のエンコード・デコード経路をテストで固定し、既存テストの検出力を上げる。

## 現状

- `tests/psnr.rs` は I420 と NV12 の 4 テストのみ。`ImageFormat` の残り 8 形式は `Encoder::init` のプレーンサイズ計算を含めて一度も実行されない
- Yv12 は libvpx の UV_FLIP レイアウトを正しく扱えているかの検証がなく、将来壊れても検出できない
- VP9 の 10-bit (Profile 2 / I42016) デコードは README で「対応」として宣伝しているが、`DecodedFrame::is_high_depth()` を検証するテストがない
- `MIN_PSNR_DB = 25.0` は実測 57 dB に対し検出力が無い (顕著な劣化以外を検出できない)
- `encode_frame` (`tests/psnr.rs`) は deadline / threads を指定しておらず、CI プラットフォーム間で厳密には非決定論的

## 設計方針

- 使用可能な組合せ (VP8 / VP9 × Yv12、VP9 × I422 / I444 / I440、16-bit 系は bit depth 対応 (別 issue) が入るまで 16-bit フォーマットの拒否テストとして) のラウンドトリップテストを追加する
- Yv12 は U/V に異なる値のパターンを入れ、色差の入れ替えを PSNR で検出できるようにする
- `threads = 1` + `EncodingDeadline::Realtime` で決定論化し、`MIN_PSNR_DB` を実測に基づいて引き上げる (目安 40-45 dB)
- 10-bit ストリームのデコードテストは、外部 (ffmpeg 等) から既知の VP9 10-bit ストリームを用意するか、テスト専用のエンコード経路を検討する

## 完了条件

- 使用可能な全 ImageFormat のラウンドトリップテストが追加され、CI で安定して通る
- `is_high_depth()` が true を返す 16-bit デコードのテストが存在する (または「非対応」としてドキュメント化)
- PSNR 閾値が実測に基づく値に引き上げられている
