# Encoder::new で ImageFormat とコーデック・プロファイルの組合せを検査する

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/change-validate-format-codec-combination
- Polished: {YYYY-MM-DD}

## 目的

`EncoderConfig` の `image_format` と `codec` (および `Vp9Profile`) の組合せで「絶対にエンコードできない」ものを `Encoder::new` の時点で拒否し、エラー発生時点を統一する。

## 現状

`Encoder::new` (`src/lib.rs`) は width / height / ビットレート / FPS / 量子化レンジの検査のみで、フォーマットとコーデックの組合せは検査しない。libvpx の検証は encode 時にしか走らず、以下の組合せは `Encoder::new` が成功した後に初回 `encode()` で `VPX_CODEC_INVALID_PARAM` になる (libvpx の `validate_img`、vp8_cx_iface.c / vp9_cx_iface.c)。

- VP8 + I422 / I444 / I440 / I42016 / I42216 / I44416 / I44016 (VP8 は YV12 / I420 / NV12 のみ許可)
- VP9 (Profile0) + I422 / I444 / I440 (Profile 1 必須だが `Vp9Profile` に存在しない)
- VP9 (Profile0) + I42216 / I44416 / I44016 (Profile 1 / 3 必須だが同様)

`Vp9Profile` に Profile 1 / 3 が無い以上、これらのフォーマットは構造的に使用不能。

## 設計方針

`Encoder::new` (または `Encoder::init`) でフォーマット × コーデック × プロファイルの組合せを事前検証し、使用不能な組合せを `VPX_CODEC_INVALID_PARAM` で即時拒否する。16-bit 系フォーマットの扱いは、拒否方針の場合はエラー文言を「bit depth 設定 API 未実装」である旨が分かるものにする。

## 完了条件

- 上記の使用不能な組合せすべてについて `Encoder::new` が `VPX_CODEC_INVALID_PARAM` を返す
- 使用可能な組合せ (VP8/VP9 × I420 / Yv12 / Nv12 等) が従来どおり動作する
- 各組合せのテーブル駆動テストが追加されている
- `CHANGES.md` に [CHANGE] としてエラー発生時点の変更が記載される
