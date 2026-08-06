# Encoder::next_frame() の iter リセットで既存パケットが重複して返る

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-encoder-iter-replay
- Polished: {YYYY-MM-DD}

## 目的

`Encoder::next_frame()` を `None` 確認後に再度呼び出したときに、同じエンコード結果が重複して返る不具合を塞ぐ。

## 現状

`Encoder::next_frame()` (`src/lib.rs`) は `vpx_codec_get_cx_data` が NULL を返したときに `self.iter` を null にリセットする。libvpx の `vpx_codec_pkt_list_get` (vpx_encoder.c) は `!(*iter)` のときリスト先頭から再アームするため、ラッパーが iter を null に戻した状態で `next_frame()` を呼び直すと、**前回の encode の先頭パケットから再び返る**。パケットが重複してストリームに混入する。

- 到達例: `encode()` → `next_frame()` で `None` になるまでドレイン → その後もう一度 `next_frame()` を呼ぶ
- `None` 後の再呼び出しを禁止する旨のドキュメントもなく、API 契約として無防備
- Decoder 側は libvpx の `ready_for_new_data` フラグが守るため安全で、Encoder 側だけが無防備

## 設計方針

`self.iter` の null リセットをやめ、ドレイン完了 (リスト末尾) を別フラグ (`drained` 等) で管理する。`next_frame()` はドレイン済みなら即 `None` を返し、`ensure_iter_drained` はリスト途中のみを検出する形に変更する。

## 完了条件

- `None` 確認後に `next_frame()` を呼び直しても `None` が返る
- ドレイン完了後の `encode()` / `finish()` / `reconfigure()` が従来どおり動作する
- 重複パケットが発生しないことがテストで検証されている
