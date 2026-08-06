# release.yml にタグ名と Cargo.toml バージョンの整合検証を追加する

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/update-release-version-guard
- Polished: {YYYY-MM-DD}

## 目的

リリース時の人為的ミス (タグ名と Cargo.toml のバージョン不一致) による事故を CI で検出する。

## 現状

`.github/workflows/release.yml` の `github-release` ジョブはタグ名をそのままリリース名にするだけで、`cargo publish` はタグコミットの Cargo.toml のバージョンで publish する。両者の一致を検証するステップが存在しない。

- タグ `2026.2.0` を push したのに Cargo.toml が `2026.2.0-canary.1` のままの場合、prebuilt はリリース `2026.2.0` にアップロードされ、`cargo publish` は canary.1 を publish しようとして失敗する。リリース `2026.2.0` は残るため、利用者が prebuilt をダウンロードしてしまう
- 逆にタグ `2026.2.0-canary.2` を Cargo.toml が `2026.2.0` のまま push すると、ユーザーの `cargo build` が prebuilt ダウンロード 404 (build.rs の `download_prebuilt`) で panic する事故がどの CI にも検出されない

## 設計方針

`github-release` (または `publish`) の直前に、タグ名と Cargo.toml の `package.version` を照合するステップを追加し、不一致なら fail させる。

## 完了条件

- タグ名と Cargo.toml のバージョンが一致しない場合にワークフローが fail する
- 一致する場合に従来どおり動作する
