# I42016 入力のエンコードでホストプロセスが SIGSEGV で落ちる

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-encode-i42016-crash
- Polished: {YYYY-MM-DD}

## 目的

`ImageFormat::I42016` を指定したエンコードで libvpx が SIGSEGV を起こし、ホストプロセスごと落ちる経路を塞ぐ。

## 現状

`Encoder::new` (`src/lib.rs` の `Encoder::new` / `Encoder::init`) は `ImageFormat` とコーデック・プロファイルの組合せを検査せず、`vpx_codec_enc_init_ver` に flags=0 を渡し、`g_bit_depth` / `g_input_bit_depth` も設定しない (libvpx デフォルトの `VPX_BITS_8` のまま)。

- libvpx の `validate_img` (vp9_cx_iface.c) は I42016 を全プロファイルで許可するため init は成功する
- 一方 `encode()` で `image2yuvconfig` が入力フォーマットの HIGHBITDEPTH ビットから `YV12_FLAG_HIGHBITDEPTH` を立て、エンコーダー内部が「use_highbitdepth=1 × bit_depth=8 × profile=0」の libvpx が想定しない混合状態に入る
- 結果、`vpx_codec_encode` が SIGSEGV または無限ハングを起こす

再現条件 (実測):

- VP9 (Profile0) + `ImageFormat::I42016` + 128x128 + `EncodingDeadline::Realtime` + 20 フレームの encode で SIGSEGV (exit 139) を再現

`ImageFormat::I42216` / `I44416` / `I44016` も同じ経路で 16-bit 入力を許可しており、同種のリスクがある。

## 設計方針

`Encoder::new` で 16-bit 系 `ImageFormat` (I42016 / I42216 / I44416 / I44016) を `VPX_CODEC_INVALID_PARAM` で拒否する。真に 16-bit (10-bit / Profile 2) エンコードをサポートする場合は、`g_bit_depth` / `g_input_bit_depth` / `VPX_CODEC_USE_HIGHBITDEPTH` を設定できる公開 API の追加が前提となるため、本 issue では拒否に留める。

## 完了条件

- `ImageFormat::I42016` を指定した `Encoder::new` が `VPX_CODEC_INVALID_PARAM` を返す
- 拒否のテスト (`Encoder::new` が Err を返すこと) が追加されている
- 16-bit 系フォーマットで encode を繰り返してもクラッシュしないことがテストで確認できる
