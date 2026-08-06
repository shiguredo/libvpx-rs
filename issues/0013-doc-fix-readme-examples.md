# README のデコード例がコンパイル不能で、reconfigure の記載がない

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/update-readme
- Polished: {YYYY-MM-DD}

## 目的

crates.io の表紙となる README のコード例を実際にコンパイルできる状態にし、2026.2.0 の新規 API (`Encoder::reconfigure`) を記載する。

## 現状

- `README.md` のデコード例 2 箇所 (`while let Some(frame) = decoder.next_frame()` の形) は、`Decoder::next_frame()` が `Result<Option<DecodedFrame>, Error>` を返すようになった (2026.1.0 の変更) のに未反映で、コンパイル不能
- `Encoder::reconfigure` / `ReconfigureParams` の記載が README に一切ない (設定テーブルにも存在しない)。`ReconfigureParams` の FPS 動的変更に関する注意 (PTS 単調性・`force_keyframe` での境界明示) も利用者に伝わっていない
- `EncodeOptions` テーブルに「デフォルト」列があるが `EncodeOptions` は `Default` を実装していない

## 設計方針

- デコード例を `Result` 対応 (`?` によるエラー伝播) に書き直す
- エンコード節に `Encoder::reconfigure` の使用例を追加し、設定テーブルに `ReconfigureParams` を追加する
- `EncodeOptions` の「デフォルト」表記を実態に合わせる
- コード例がコンパイルできることを CI で担保する仕組み (examples/ への移植等) を検討する

## 完了条件

- README の全コード例が実際にコンパイルできる
- `Encoder::reconfigure` / `ReconfigureParams` の説明と使用例が README に記載されている
- FPS 動的変更の注意事項が README に記載されている
