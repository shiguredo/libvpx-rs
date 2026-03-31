# 変更履歴

- UPDATE
  - 後方互換がある変更
- ADD
  - 後方互換がある追加
- CHANGE
  - 後方互換のない変更
- FIX
  - バグ修正

## develop

- [CHANGE] `Decoder::next_frame()` の戻り値を `Option<DecodedFrame>` から `Result<Option<DecodedFrame>, Error>` に変更する
  - 未対応の画像フォーマットに対して panic ではなくエラーを返すようにする
  - @voluntas
- [FIX] `Encoder::new()` で `vpx_img_alloc()` の失敗時に未定義動作が発生する問題を修正する
  - 戻り値の NULL チェックを追加し、失敗時はエラーを返すようにする
  - @voluntas
- [ADD] `supported_codecs()` API を追加する
  - VP8/VP9 のデコード・エンコード対応状況と VP9 エンコードプロファイル情報を返す
  - @voluntas
- [ADD] シンボル書き換え機能を追加する
  - 静的ライブラリ内の全シンボルに `shiguredo_vpx_` プレフィックスを付与し、他ライブラリとの衝突を回避する
  - source-build / prebuilt 両パスに対応する
  - @voluntas
- [UPDATE] libvpx v1.16.0 に更新する
  - @voluntas
- [ADD] Windows (MSYS2/MinGW) 向けのビルド・CI 対応を追加する
  - Windows 向け prebuilt バイナリを提供する
  - `build.rs` で Windows の prebuilt ダウンロード・SHA256 検証 (`certutil`) に対応する
  - @voluntas
- [CHANGE] エンコーダーのピクセルフォーマット対応を追加する
  - `ImageFormat` enum を追加する (I420, Yv12, Nv12, I422, I444, I440, I42016, I42216, I44416, I44016)
  - `ImageData` enum を追加する
  - `EncoderConfig::new()` に `image_format` 引数を追加する
  - `Encoder::encode()` のシグネチャを `encode(y, u, v, options)` から `encode(image, options)` に変更する
  - @voluntas
- [CHANGE] エンコーダー/デコーダーの API を再設計する
  - `Decoder::new_vp8()` / `Decoder::new_vp9()` を `Decoder::new(DecoderConfig)` に統合する
  - `Encoder::new_vp8(&EncoderConfig)` / `Encoder::new_vp9(&EncoderConfig)` を `Encoder::new(EncoderConfig)` に統合する
  - `Encoder::encode()` に `&EncodeOptions` 引数を追加する
    - `force_keyframe` フラグでキーフレームの強制生成が可能になる
  - @voluntas
- [CHANGE] `EncoderConfig` を再設計する
  - `EncoderConfig` から `Default` 実装を削除する
  - `EncoderConfig::new(width, height, codec)` コンストラクターを追加する
  - `vp9_config: Option<Vp9Config>` / `vp8_config: Option<Vp8Config>` を `codec: CodecConfig` に統合する
  - @voluntas
- [CHANGE] `DecoderCodec` / `DecoderConfig` / `CodecConfig` / `Vp9Profile` / `EncodeOptions` を追加する
  - `Vp9Config` に `profile: Vp9Profile` フィールドを追加する
  - @voluntas
- [CHANGE] prebuilt バイナリダウンロード機能を追加する
  - `source-build` feature でソースからのビルドに切り替え可能にする
  - @voluntas
- [CHANGE] ビルド依存の `toml` クレートを `shiguredo_toml` に置き換える
  - @voluntas

## 2025.1.0

**リリース日**: 2025-09-26
