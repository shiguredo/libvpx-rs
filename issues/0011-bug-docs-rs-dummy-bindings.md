# DOCS_RS 向けダミーバインディングが型チェックに耐えず、DOCS_RS 環境変数がリビルドに反映されない

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-docs-rs-dummy-bindings
- Polished: {YYYY-MM-DD}

## 目的

docs.rs 向けビルド経路 (`DOCS_RS=1`) を堅牢にする。

## 現状

`build.rs` の DOCS_RS 分岐は、構造体宣言 7 個だけのダミー `bindings.rs` を出力して処理を終える。しかし `src/lib.rs` は `sys::vpx_codec_err_t_VPX_CODEC_OK` 等の定数・`vpx_codec_*` 関数を多数参照しており、ダミーだけでは `DOCS_RS=1 cargo check` / `cargo build` が **147 件の E0425 等のエラーで失敗する** (実測済み)。

`cargo doc --no-deps` が通っているのは、rustdoc が関数本体の名前解決を検査しないためであり、「ダミーが正しい」わけではない。lib.rs の参照が増える・rustdoc が厳格化される等で docs.rs ビルドが publish 後に壊れるリスクを内包する。

また `build.rs` の `cargo::rerun-if-env-changed` に `DOCS_RS` が含まれておらず、`DOCS_RS=1` で一度ビルドした後に環境変数を外しても build.rs が再実行されず、ダミーバインディングが残って通常ビルドが失敗し続ける (実測済み。README が `DOCS_RS=1 cargo doc` を公式手順として案内しているため、この罠にはまるのは必然)。

## 設計方針

以下のいずれか (優先順):

1. `cargo::rerun-if-env-changed=DOCS_RS` を追加する (最低限)
2. ダミーバインディングを lib.rs が参照する定数・関数・型まで含めて整備する
3. docs.rs 向けにはチェックイン済みの生成済み bindings.rs を使う

あわせて `DOCS_RS=1 cargo check` が通ることを CI で検証する。

## 完了条件

- `DOCS_RS=1 cargo check` がエラーなく完了する (またはダミーに依存しない構成になっている)
- `DOCS_RS=1` でビルドした後に環境変数を外して `cargo check` しても成功する (リビルドが正しく走る)
