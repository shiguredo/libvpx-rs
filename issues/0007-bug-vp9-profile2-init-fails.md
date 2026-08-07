# Vp9Profile::Profile2 が常に初期化に失敗し、supported_codecs() が虚偽の報告をする

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-vp9-profile2-init-failure
- Polished: 2026-08-07

## 目的

使用不能な公開 API (`Vp9Profile::Profile2` のエンコード) が「利用可能」として公開されたままになるのを防ぐ。

## 現状

`Encoder::init` (`src/lib.rs`) は `Vp9Profile::Profile2` に対して `g_profile` に 2 を設定するのみで、`g_bit_depth` / `g_input_bit_depth` を設定しない。`vpx_codec_enc_config_default` は `g_bit_depth = VPX_BITS_8` を既定値として設定する (vp9_cx_iface.c の cfg map) ため、libvpx の `validate_config` (vp9_cx_iface.c) の「`g_profile > PROFILE_1 && g_bit_depth == VPX_BITS_8` なら ERROR」の検査に抵触し、`Vp9Profile::Profile2` を指定した `Encoder::new` は常に失敗する (実測済み。`vpx_codec_enc_init_ver` が "Invalid parameter" を返す)。

一方 `supported_codecs()` (`src/codec_info.rs`) は VP9 のエンコードプロファイルとして Profile0 と Profile2 を「利用可能」と報告する。`README.md` の `Vp9Profile` の設定一覧 (`README.md` の `| Profile2 | 10/12-bit 4:2:0 |`) も Profile2 を列挙している。

## 設計方針

本 issue では `Vp9Profile::Profile2` を真に動作させる試みは行わず、公開情報を実挙動に合わせる。真の 10/12-bit 対応 (bit depth 設定の公開 API 設計) は別 issue で扱う (Profile2 の真の対応は issue 0022 の範疇)。

- `supported_codecs()` の VP9 エンコードプロファイル報告から Profile2 を除外し、実際に動作する Profile0 のみを報告する
- `Vp9Profile::Profile2` の variant は残す (公開 API の破壊的変更を避ける)。doc コメントに「現状は使用不能 (init が失敗する)。bit depth 設定 API 未実装のため」と明記する
- `Vp9EncodingProfile::Profile2` (`src/codec_info.rs`) の variant も残すが、`supported_codecs()` が報告しなくなるため、doc コメントを「現状は利用不可 (bit depth 設定 API 未実装のため。issue 0022 の実装後に再報告される)」と明記する
- `Encoder::new` が `Vp9Profile::Profile2` に対して Err を返すことをテストで固定する。`Encoder::init` で `Vp9Profile::Profile2` を事前検査し、libvpx 側の `validate_config` に到達する前に `invalid_param` ヘルパーで明示的な reason (bit depth 設定 API 未実装の旨) を返す (libvpx 由来の "Invalid parameter" では原因が伝わらないため、issue 0006 と同じくラッパー側で明示文言を返す)
- `README.md` の `Vp9Profile` 設定一覧の Profile2 行を実挙動に合わせて修正する (エンコードは Profile0 のみ対応の旨)

なお issue 0006 は「`ImageFormat::I42016` を指定した `Encoder::new` を `VPX_CODEC_INVALID_PARAM` で拒否する」設計であり、本 issue の方針 (Profile2 を利用不可扱いにする) と整合する。0006 の I42016 拒否の解除所掌は issue 0022 が持つ。

## 完了条件

- `supported_codecs()` が VP9 のエンコードプロファイルとして Profile0 のみを報告する
- `Vp9Profile::Profile2` を指定した `Encoder::new` に関するテストが存在し、Err を返すことが検証されている (エラーが `VPX_CODEC_INVALID_PARAM` であること、reason 文言に「bit depth 設定 API 未実装」の旨が含まれることを `Error::reason()` で検証する)
- `Vp9Profile::Profile2` / `Vp9EncodingProfile::Profile2` の doc コメントが実挙動と一致している
- `README.md` の `Vp9Profile` 設定一覧が実挙動と一致している
- `CHANGES.md` に [FIX] として記載される
