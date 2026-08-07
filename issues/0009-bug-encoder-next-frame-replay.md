# Encoder::next_frame() の iter リセットで既存パケットが重複して返る

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-encoder-iter-replay
- Polished: 2026-08-07

## 目的

`Encoder::next_frame()` を `None` 確認後に再度呼び出したときに、同じエンコード結果が重複して返る不具合を塞ぐ。

## 現状

`Encoder::next_frame()` (`src/lib.rs`) は `vpx_codec_get_cx_data` が NULL を返したときに `self.iter` を null にリセットする。libvpx の `vpx_codec_pkt_list_get` (vpx_encoder.c) は `!(*iter)` のときリスト先頭から再アームするため、ラッパーが iter を null に戻した状態で `next_frame()` を呼び直すと、**前回の encode の先頭パケットから再び返る**。パケットが重複してストリームに混入する。

- 到達例: `encode()` → `next_frame()` で `None` になるまでドレイン → その後もう一度 `next_frame()` を呼ぶ
- この不具合は VP8 / VP9 の両コーデックで発生する (`vp8e_get_cxdata` / `encoder_get_cxdata` はどちらも `vpx_codec_pkt_list_get` を経由する)
- `None` 後の再呼び出しを禁止する旨のドキュメントもなく、API 契約として無防備
- Decoder 側は新規 `decode()` でのみ `ready_for_new_data` フラグが 0 に戻る仕組みがあり、`Decoder::next_frame()` の iter null リセットではフレームが再返却されないため安全。Encoder 側だけが無防備

## 設計方針

`self.iter` の null リセットは**維持したまま**、ドレイン完了を別フラグ (`drained`) で管理する (iter の再アームは libvpx が encode ごとに pkt_list を初期化する仕様に依存しており、null リセットをやめると先頭パケット欠落を招くため)。

- `Encoder` に `drained: bool` フィールドを追加する。初期値は `true` (エンコード前はドレイン済みとみなす)。フィールドには「ドレイン完了フラグ」の意味を日本語コメントで明記する
- `next_frame()`: `drained` が true なら即 `None` を返す。それ以外は従来どおり `vpx_codec_get_cx_data` を呼び、NULL を返したら `drained = true` にして (従来どおり `self.iter = null` も維持して) `None` を返す
- `encode()` / `finish()`: 成功したら `drained = false` にリセットする (iter は従来どおり null のままで、libvpx の encode ごとの pkt_list 初期化により次回 `next_frame()` が新リスト先頭へ再アームされる)
- `reconfigure()` は `drained` に触れない (パケットを生成しないため `false` に戻す必要がない)
- `ensure_iter_drained` は変更しない (iter の null 判定のまま。encode 直後で `next_frame()` 未呼び出しの状態は iter が null のため許容され、reconfigure の doc 契約「`encode` 直後でまだ `next_frame` を呼んでいない状態は許容する」を維持する)
- `next_frame()` の doc コメントに「`None` 後の再呼び出しは安全で `None` を返す」旨を追記する

## 完了条件

- `None` 確認後に `next_frame()` を呼び直しても `None` が返り、重複パケットが発生しないことがテスト (VP8 / VP9 両方、encode 後ドレインと finish 後ドレインの両経路) で検証されている。テストは encode 直後の `next_frame()` が最低 1 パケットを返す構成 (例: VP9 では `lag_in_frames` 未指定) を前提にし、バグ再発時 (`Some` が返る) を検出できるようにする
- ドレイン完了後の `encode()` / `finish()` / `reconfigure()` が従来どおり動作する (既存のエンコードループテストが回帰しないこと)
- `CHANGES.md` に [FIX] として記載される
