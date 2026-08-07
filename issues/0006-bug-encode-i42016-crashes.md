# I42016 入力のエンコードでホストプロセスが SIGSEGV で落ちる

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-encode-i42016-crash
- Polished: 2026-08-07

## 目的

`ImageFormat::I42016` を指定したエンコードで libvpx が SIGSEGV を起こし、ホストプロセスごと落ちる経路を塞ぐ。

## 現状

`Encoder::new` (`src/lib.rs` の `Encoder::new` / `Encoder::init`) は `ImageFormat` とコーデック・プロファイルの組合せを検査しない。`vpx_codec_enc_init_ver` に flags=0 を渡し、`g_bit_depth` / `g_input_bit_depth` も設定しない (libvpx デフォルトの `VPX_BITS_8` のまま)。

- init (`vpx_codec_enc_init_ver`) には画像フォーマットの検査がなく、encode 時の最初のフォーマット検査である `validate_img` (vp9_cx_iface.c) も I42016 を全プロファイルで許可するため、VP9 Profile0 + I42016 は init を通過する
- 一方 `encode()` で `image2yuvconfig` (vp9_iface_common.c) が入力フォーマットの HIGHBITDEPTH ビットから `YV12_FLAG_HIGHBITDEPTH` を立て、エンコーダー内部が「use_highbitdepth=1 × bit_depth=8 × profile=0」の libvpx が想定しない混合状態に入る
- 結果、`vpx_codec_encode` が SIGSEGV を起こし、ホストプロセスごと落ちる

再現条件 (実測): VP9 (Profile0) + `ImageFormat::I42016` + 128x128 + `EncodingDeadline::Realtime` + 20 フレームの encode で SIGSEGV (exit 139) を再現。バイナリや入力パターンによっては 1 フレーム目でもクラッシュし、20 フレーム回すと確実に再現する。

`ImageFormat::I42216` / `I44416` / `I44016` はこの経路ではクラッシュしない。VP9 の `validate_img` が Profile 1 / 3 のみ許可し、本ラッパが公開する `Vp9Profile` は Profile0 / Profile2 のみのため、encode 時に `VPX_CODEC_INVALID_PARAM` で拒否される (実測済み)。VP8 も 16-bit 系を全拒否する。これらの構造的に使用不能な組合せの検査は issue 0012 で扱う (0006 の I42016 拒否はコーデックを問わず適用されるため、VP8 + I42016 も 0006 で拒否される)。

## 設計方針

`Encoder::new` で `ImageFormat::I42016` を `VPX_CODEC_INVALID_PARAM` で拒否する。真に 16-bit (10-bit / Profile 2) エンコードをサポートする場合は、`g_bit_depth` / `g_input_bit_depth` / `VPX_CODEC_USE_HIGHBITDEPTH` を設定できる公開 API の追加が前提となるため、本 issue では拒否に留める (Profile 2 の 16-bit 対応は issue 0022 の範疇。0022 が 16-bit 対応を実装した際は、本 issue の I42016 拒否を解除する所掌を 0022 側で持つ)。

`ImageFormat::I42216` / `I44416` / `I44016` の拒否はクラッシュが原因ではなく、`Vp9Profile` に Profile 1 / 3 が無い構造的な使用不能のためであり、issue 0012 の組合せ検査に委ねる。本 issue は I42016 の拒否に限定する。

拒否の実装は既存の `invalid_param` ヘルパー (src/lib.rs) を使い、reason 文言は「16-bit 入力は bit depth 設定 API が未実装のためサポート外」である旨が分かるものにする (issue 0012 のエラー文言方針と揃える)。16-bit 系の拒否テストは issue 0015 とも関わるため、本 issue は I42016 の回帰テストに限定し、他の 16-bit 系フォーマットの拒否テストは issue 0015 に委ねる。

## 完了条件

- `ImageFormat::I42016` を指定した `Encoder::new` が `VPX_CODEC_INVALID_PARAM` を返す
- 再現条件 (VP9 Profile0 + I42016 + 128x128 + `EncodingDeadline::Realtime`) と同じパラメータで `Encoder::new` が Err を返す回帰テストが追加されている (拒否により encode 経路に到達せず、クラッシュが発生しないことを確認できる)。エラーが `VPX_CODEC_INVALID_PARAM` であることは `Error::reason()` の文言 (bit depth 設定 API 未実装の旨) で検証する
- `CHANGES.md` に [FIX] として記載される
