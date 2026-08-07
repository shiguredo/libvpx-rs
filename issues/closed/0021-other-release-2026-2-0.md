# 2026.2.0 を正式リリースする

- Created: 2026-08-06
- Completed: 2026-08-07
- Branch: release/2026.2.0
- Polished: {YYYY-MM-DD}

## 目的

2026.2.0 を正式リリースする。リリースブロッカー (I42016 の SIGSEGV、Vp9Profile::Profile2 の init 常時失敗、ReconfigureParams の外部構築不能) の修正と、公開ドキュメント (README) の修正が完了した後に実施する。

## 現状

- `Cargo.toml` のバージョンは `2026.2.0-canary.1`
- `CHANGES.md` の `## develop` セクションに 2026.2.0 の変更が蓄積されている
- 前回リリースは 2026.1.0 (2026-03-31、release/2026.1.0 ブランチで実施し develop にマージ)

## 設計方針

release/2026.2.0 ブランチで以下の手順を実施する (詳細は RELEASE.md を参照):

1. バージョンを `2026.2.0` に変更 (canary 外し)
2. `CHANGES.md` の `## develop` セクションを `## 2026.2.0` + リリース日 (2026-08-06) に変更し、エントリ順序を規約 (CHANGE → ADD → UPDATE → FIX) に整える
3. 新しい空の `## develop` セクションを追加
4. タグ `2026.2.0` を push して release.yml を実行 (prebuilt 生成 + crates.io publish)
5. develop にマージ

## 完了条件

- crates.io に `shiguredo_libvpx` 2026.2.0 が公開されている
- GitHub Releases の 2026.2.0 に全プラットフォームの prebuilt アーカイブと SHA256 がアップロードされている
- タグ 2026.2.0 の crates.io パッケージで `cargo build` (source-build なし) が通る
- develop にリリース内容がマージされている

## 解決方法

却下する。正式リリースを実施するための issue であり、リリース用の issue は不要とする方針に従う。
