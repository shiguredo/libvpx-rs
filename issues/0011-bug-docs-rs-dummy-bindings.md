# DOCS_RS 向けダミーバインディングが型チェックに耐えず、DOCS_RS 環境変数がリビルドに反映されない

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-docs-rs-dummy-bindings
- Polished: 2026-08-07

## 目的

docs.rs 向けビルド経路 (`DOCS_RS=1`) を堅牢にする。

## 現状

`build.rs` の DOCS_RS 分岐は、構造体宣言 7 個だけのダミー `bindings.rs` (フィールドなしのユニット構造体) を出力して処理を終える。しかし `src/lib.rs` は `sys::vpx_codec_err_t_VPX_CODEC_OK` 等の定数・`vpx_codec_*` 関数に加え、`sys::vpx_image` のフィールド (`fmt` / `planes` / `d_h` 等) や `sys::vpx_codec_enc_cfg` のフィールド群も多数参照しており、ダミーだけでは `DOCS_RS=1 cargo check` / `cargo build` が 147 件のエラーで失敗する (実測済み。内訳は E0425 (未定義シンボル) 92 件、E0609 (構造体フィールド欠落) 43 件、E0308 5 件、E0599 3 件、E0531 2 件、E0277 2 件)。

`cargo doc --no-deps` が通っているのは、rustdoc が関数本体内の名前解決を検査しないためであり、「ダミーが正しい」わけではない。

また `build.rs` の `cargo::rerun-if-env-changed` に `DOCS_RS` が含まれておらず、`DOCS_RS=1` で一度ビルドした後に環境変数を外しても build.rs が再実行されず、ダミーバインディングが残って通常ビルドが失敗し続ける (実測済み。README が `DOCS_RS=1 cargo doc --no-deps` を公式手順として案内している)。逆に、通常ビルド済みのワークスペースで `DOCS_RS=1 cargo doc` を実行すると、build.rs が再実行されず実バインディングが残ったまま doc が通り、ダミー経路が一度も検証されない。

## 設計方針

以下の 2 点を**両方**実施する (どちらか一方だけでは完了条件を満たせない)。

1. `cargo::rerun-if-env-changed=DOCS_RS` を追加する (完了条件 2 のリビルド問題を解決する前提)
2. `DOCS_RS=1 cargo check` がエラーなく通る構成にする。手段は次のいずれか
   - ダミーバインディングを `src/lib.rs` が参照する定数・関数・型・**フィールド**まで含めて整備する (E0609 の 43 件はフィールド欠落のため、ユニット構造体の全面改修が必要)
   - docs.rs 向けにはチェックイン済みの生成済み `bindings.rs` を使う (対象プラットフォームと libvpx バージョン更新時の再生成手順を決める)。`Cargo.toml` の `include` は `/src/**/*` をソース対象とするため、チェックイン先を `src/` 配下にしないと公開パッケージに含まれず、docs.rs 本番ビルドが失敗する。チェックイン先を `src/` 以外にする場合は `include` を更新する。またチェックイン済み `bindings.rs` を `cargo::rerun-if-changed` で追跡し、更新時に build.rs が再実行されるようにする

あわせて、`.github/workflows/ci.yml` の既存 docs-rs ジョブ (`DOCS_RS: 1` で `cargo doc --no-deps` を実行) に `DOCS_RS=1 cargo check` を追加し、回帰を CI で検証する (issue 0017 の docs-rs ジョブ改修とは別のステップとして追加する)。

## 完了条件

- `DOCS_RS=1 cargo check` がエラーなく完了する (修正前の失敗再現には fresh target が必要。既存 target ではビルドキャッシュで成功してしまうため、再現時は `cargo clean` か `CARGO_TARGET_DIR` を新規指定する。修正後は `rerun-if-env-changed=DOCS_RS` により環境変数の変更だけで build.rs が再実行される)
- `DOCS_RS=1` でビルドした後に環境変数を外して `cargo check` しても成功する (リビルドが正しく走る)。なお環境変数を外した通常ビルドは prebuilt ダウンロードに依存するため、ローカル検証は `--features source-build` を使うか、prebuilt が存在するバージョンで行う
- 順方向 (通常ビルド → `DOCS_RS=1`) で build.rs が再実行され、ダミー経路が実際に通ることも検証されている
- docs-rs ジョブに `DOCS_RS=1 cargo check` が追加され、CI で検証されている
