# Encoder::finish() が deadline を REALTIME 固定で libvpx に渡す

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-encoder-finish-deadline
- Polished: 2026-08-07

## 目的

`Encoder::finish()` による末尾フレームのエンコードが、設定した `EncodingDeadline` と異なる品質モードで行われるのを防ぐ。

## 現状

`Encoder::finish()` (`src/lib.rs`) は `vpx_codec_encode` に `VPX_DL_REALTIME` を固定で渡している。libvpx は flush 呼び出し (`img == NULL`) でも `pick_quickcompress_mode` (vp9_cx_iface.c / vp8_cx_iface.c) が走り、`deadline == VPX_DL_REALTIME` のときエンコーダーのモードを REALTIME に切り替える。`EncodingDeadline::Good` / `Best` を指定した VP9 エンコーダーでも、flush される末尾フレーム (lag 分を含む) は REALTIME 品質でエンコードされる。

VP8 は one-pass では `lag_in_frames` が内部的に無効になる (vp8_cx_iface.c) ため、flush で追加フレームは出力されず、本不具合の実害は VP9 に限られる (`finish()` 自体は VP8 でも deadline を渡すべきだが、影響はフレーム品質に及ばない)。

一方 `Encoder::encode()` は `self.deadline` を `VPX_DL_*` 定数にマッピングして渡しており、`finish()` だけが固定値になっている。

## 設計方針

`Encoder::finish()` が `self.deadline` (`Encoder::init` で `EncoderConfig::deadline` からコピーされる) を、`Encoder::encode()` と同じマッピングで `vpx_enc_deadline_t` に変換して `vpx_codec_encode` に渡すように変更する。変換は共通ヘルパーに抽出し、`encode()` と `finish()` が同一の変換関数を使うようにする (将来のマッピング変更で乖離しないため)。

なお issue 0009 も `Encoder::finish()` を変更する (`drained` フラグのリセット)。同じ関数内の別の行を変更するため論理的に共存できるが、編集順には注意する。

## 完了条件

- `finish()` が `self.deadline` に基づく deadline を、`encode()` と共通の変換で `vpx_codec_encode` に渡している (コードレベルで検証可能)
- VP9 の `lag_in_frames` 指定エンコーダーで、`finish()` 後のドレインが末尾 lag 分フレームを欠落なく出力し、かつその末尾フレームの品質 (PSNR) が同じエンコーダーの `encode()` フェーズ出力の品質から許容差内に収まることをテストで検証する (バグ時は flush が REALTIME になるため顕著に劣化する。許容差の具体的な値はテスト実装時に実測で確定し、バグありの状態で必ず違反する値にする)。決定論性のため `threads = 1` を指定する
  - 「GOOD 設定と REALTIME 設定の出力を直接比較する方式」は、encode フェーズのレート制御状態とコンテンツ依存で出力が逆転しうる (実測で確認済み) ため、本バグを検出できない。比較基準は必ず「同じエンコーダーの encode() フェーズ出力」とすること
- `CHANGES.md` に [FIX] として記載される
