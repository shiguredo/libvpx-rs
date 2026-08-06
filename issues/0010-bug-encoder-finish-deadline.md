# Encoder::finish() が deadline を REALTIME 固定で libvpx に渡す

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-encoder-finish-deadline
- Polished: {YYYY-MM-DD}

## 目的

`Encoder::finish()` による末尾フレームのエンコードが、設定した `EncodingDeadline` と異なる品質モードで行われるのを防ぐ。

## 現状

`Encoder::finish()` (`src/lib.rs`) は `vpx_codec_encode` に `VPX_DL_REALTIME` を固定で渡している。libvpx は flush 呼び出しでも `pick_quickcompress_mode` (vp9_cx_iface.c / vp8_cx_iface.c) が走り、`deadline == VPX_DL_REALTIME` のときエンコーダーのモードを REALTIME に切り替える。`EncodingDeadline::Good` / `Best` を指定したエンコーダーでも、flush される末尾フレーム (VP9 の lag 分を含む) は REALTIME 品質でエンコードされる。

## 設計方針

`Encoder::finish()` が `self.deadline` の値を `vpx_codec_encode` に渡すように変更する。

## 完了条件

- `finish()` が `EncoderConfig::deadline` の値を使用している
- `EncodingDeadline` ごとに flush が問題なく動作するテストが追加されている
