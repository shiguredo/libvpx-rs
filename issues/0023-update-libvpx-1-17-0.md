# libvpx v1.17.0 に更新する

- Created: 2026-10-03
- Completed: {YYYY-MM-DD}
- Branch: feature/update-libvpx-mobile-prebuilt
- Polished: {YYYY-MM-DD}

## 目的

libvpx v1.17.0 の修正・最適化を取り込み、モバイル prebuilt 対応の土台を v1.17.0 に揃える。

## 現状

- `Cargo.toml` の `[package.metadata.external-dependencies.libvpx]` が `v1.16.0` を指しており、source-build と prebuilt は v1.16.0 で作成される
- v1.17.0 (2026-08-07 リリース) には `vp9_postproc` / `vp8-multi-res-encoding` / `vpx_setup_noise` 等の heap buffer overflow 修正、Arm Neon DotProd と AVX2/AVX512 の最適化、VP9 HBD 入力検証の既定有効化などが含まれる
- v1.17.0 の CHANGELOG に "This release is ABI compatible with the previous release." とあり、ABI は v1.16.0 と互換である

## 設計方針

- `Cargo.toml` の依存バージョンを `v1.17.0` に更新する
- ABI 互換のため bindings の再生成以外のコード変更は不要。既存テストで回帰を確認する
- v1.17.0 の挙動変更が既存実装に影響しないことを確認する
  - VP9 HBD 入力検証の既定有効化: 既存実装は HBD 入力のレンジ検証を行わないが、v1.17.0 側で `VPX_CODEC_INVALID_PARAM` を返す。エラーは既存のエラーパスで処理される
  - 入力画像の U/V stride 不一致の拒否: 既存実装は `vpx_img_alloc` の戻り画像を使うため U/V stride は常に一致する
  - デコード画像の `w`/`h` の意味変更 (stride / アライン値から実寸へ): 既存実装は `d_w`/`d_h` を参照しており影響を受けない
- モバイル prebuilt 対応と同一のリリースで取り込む

## 完了条件

- source-build で v1.17.0 がビルドされ、`cargo test --features source-build` が通る
- `CHANGES.md` に v1.17.0 への更新を記載する
- リリースワークフローで v1.17.0 の prebuilt が作成される

## 解決方法

{記入}
