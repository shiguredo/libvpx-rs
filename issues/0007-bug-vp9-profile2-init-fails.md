# Vp9Profile::Profile2 が常に初期化に失敗し、supported_codecs() が虚偽の報告をする

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-vp9-profile2-init-failure
- Polished: {YYYY-MM-DD}

## 目的

使用不能な公開 API (`Vp9Profile::Profile2` のエンコード) が「利用可能」として公開されたままになるのを防ぐ。

## 現状

`Vp9Profile::Profile2` (`src/lib.rs`) は `Encoder::init` が `g_profile` に 2 を設定するのみで、`g_bit_depth` を設定しない。libvpx の `validate_config` (vp9_cx_iface.c) は「`g_profile > PROFILE_1 && g_bit_depth == VPX_BITS_8` なら ERROR」を返すため、`Vp9Profile::Profile2` を指定した `Encoder::new` は **常に** 失敗する (実測済み。`vpx_codec_enc_init_ver` が "Invalid parameter" を返す)。

一方 `supported_codecs()` (`src/codec_info.rs`) は VP9 のエンコードプロファイルとして Profile0 と Profile2 を「利用可能」と報告する。`README.md` の「VP9 High Bitdepth (10-bit / Profile 2) デコード対応」も、エンコード側 (Profile2) とデコード側 (10-bit ストリーム) の両方が未検証のまま宣伝されている。

## 設計方針

以下のいずれか。

1. `g_bit_depth` / `g_input_bit_depth` / `VPX_CODEC_USE_HIGHBITDEPTH` の設定を実装して Profile2 を真に動作させる
2. 1 を実施するまでの間、`Vp9Profile::Profile2` と `supported_codecs()` の報告を実挙動に合わせる (Profile2 を利用可能と報告しない)

## 完了条件

- `Vp9Profile::Profile2` を指定した `Encoder::new` の挙動と `supported_codecs()` の報告が一致している
- Profile2 を「利用可能」と報告する場合: 10-bit ラウンドトリップテスト (エンコード → デコード → `DecodedFrame::is_high_depth()` が true) が追加され、実際に動作することが検証されている
- Profile2 を利用不可とする場合: テストで `Encoder::new` が Err を返すこと、`supported_codecs()` が Profile2 を含まないことが検証されている
