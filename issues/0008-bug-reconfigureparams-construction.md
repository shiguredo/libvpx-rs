# ReconfigureParams が #[non_exhaustive] のため外部クレートから構造体リテラルで構築できない

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-reconfigureparams-construction
- Polished: 2026-08-07

## 目的

`Encoder::reconfigure` が外部クレートから使いやすい形で呼び出せるようにする。

## 現状

`ReconfigureParams` (`src/lib.rs`) には `#[non_exhaustive]` が付いており、定義クレートの外からは構造体リテラル (`ReconfigureParams { target_bitrate: Some(500_000), ..ReconfigureParams::default() }` のような `..base` を含む形式も含む) での構築が E0639 ("cannot create non-exhaustive struct using struct expression") でコンパイルエラーになる (実測済み)。この制約は edition 非依存の rustc の仕様であり、エディション 2024 固有ではない。

`ReconfigureParams` の全フィールドは `pub` のため、外部クレートは `let mut p = ReconfigureParams::default(); p.target_bitrate = Some(500_000);` のようなミューテーションでは構築できる。実害は「構造体リテラルで一度に構築できない」という API の使いにくさと、shiguredo-rust 規約「`#[non_exhaustive]` を使わないこと」への違反にある。

クレート内のテスト (`src/lib.rs` の `#[cfg(test)]` モジュール) は同一クレート内のため構造体リテラルが使え、この問題は検出されていない。

## 設計方針

`#[non_exhaustive]` を外す (shiguredo-rust 規約「`#[non_exhaustive]` を使わないこと」にも合致)。フィールドを追加する必要が生じた時点で破壊的変更として扱う。

検証は `tests/` 配下の統合テストで行う (`tests/` は別クレートとしてコンパイルされるため、外部クレート視点の E0639 をそのまま再現できる)。`tests/` からは `Encoder` の private な設定読み出しゲッター (`self.cfg` 系、`src/lib.rs` の `#[cfg(test)]` ブロック) にアクセスできないため、「設定変更が反映されること」は既存の `reconfigure_low_quantizer_yields_far_more_bytes_than_high_quantizer` と同じように、量子化レンジ変更による出力バイト量の変化で間接的に検証する (既存テスト並みの閾値で no-op 退化を検出する)。

なおテストファイルの命名は shiguredo-rust 規約「特定のモジュールに対応しないテストには `test_` / `prop_` プレフィックスを付けないこと」に従い、既存の `tests/psnr.rs` と同様に prefix なしのファイル名にする。

`ReconfigureParams` は未リリース機能であるため、`CHANGES.md` への記載は不要 (shiguredo-changelog 規約「開発ブランチ内の中間状態の修正は記載しないこと」。既存の `[ADD]` エントリが `#[non_exhaustive]` なしの最終状態を表す)。

なお issue 0004 (`ReconfigureParams` の FPS フィールドを `Option<Fps>` に変更) と実装順の依存がある。本 issue 完了後に 0004 でフィールドを変更すると、外部クレートの構造体リテラル構築が破壊される (外部から見える破壊的変更になる)。0004 を先に実装する場合は、そのフィールド変更に追随して本 issue のテストを更新する。

## 完了条件

- `#[non_exhaustive]` が外れ、外部クレートから `ReconfigureParams` を構造体リテラルで構築できる
- `tests/` 配下の統合テストで、外部クレート相当のコードから `ReconfigureParams` を構造体リテラルで構築し、`Encoder::reconfigure` を呼び、設定変更が反映される (出力バイト量が変化する、既存テスト並みの閾値) ことが検証されている
