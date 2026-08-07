# VP9 Profile2 (10/12-bit) エンコードを真にサポートする

- Created: 2026-08-07
- Completed: {YYYY-MM-DD}
- Branch: feature/add-vp9-profile2-high-bitdepth
- Polished: {YYYY-MM-DD}

## 目的

`Vp9Profile::Profile2` を指定した `Encoder::new` を実際に動作させ、10-bit エンコードを利用可能にする。

## 現状

`Vp9Profile::Profile2` (`src/lib.rs`) は `Encoder::init` が `g_profile` に 2 を設定するのみで、`g_bit_depth` / `g_input_bit_depth` を設定せず、`vpx_codec_enc_init_ver` に flags=0 (`VPX_CODEC_USE_HIGHBITDEPTH` なし) を渡す。libvpx の `validate_config` (vp9_cx_iface.c) は「`g_profile > PROFILE_1 && g_bit_depth == VPX_BITS_8` なら ERROR」を返すため、`Vp9Profile::Profile2` を指定した `Encoder::new` は常に失敗する。

また `Vp9Profile::Profile2` は「10/12-bit」と両義的であり、10-bit と 12-bit を弁別する手段が `Vp9Config` に存在しない。

## 設計方針

- `Encoder::init` で `g_bit_depth` / `g_input_bit_depth` / `VPX_CODEC_USE_HIGHBITDEPTH` を設定できる公開 API を追加する
- 10-bit 固定 (`g_bit_depth=10` / `g_input_bit_depth=10`) で実装し、12-bit は別途拡張とする
- 入力フォーマットは `ImageFormat::I42016` に限定する。Profile2 + それ以外のフォーマットは `Encoder::new` で拒否する (issue 0012 の組合せ検査と整合)
- issue 0006 の I42016 拒否との干渉を解消する (0006 は「0007 が 16-bit 対応を実装した際は、本 issue の I42016 拒否を解除する所掌を 0007 側で持つ」と明記している)

## 完了条件

- `Vp9Profile::Profile2` + `ImageFormat::I42016` を指定した `Encoder::new` が成功する
- 10-bit ラウンドトリップテスト (エンコード → デコード → `DecodedFrame::is_high_depth()` が true) が追加され、実際に動作することが検証されている
- `supported_codecs()` が Profile2 を「利用可能」と報告する内容と実挙動が一致している
