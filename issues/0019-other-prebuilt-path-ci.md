# prebuilt 配布パスを CI で検証する

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/update-prebuilt-ci
- Polished: {YYYY-MM-DD}

## 目的

ユーザーの大半が使う prebuilt ダウンロード・SHA256 検証・展開・リンクのパス (`cargo build` のデフォルト経路) が壊れたままリリースされるのを防ぐ。

## 現状

- `.github/workflows/ci.yml` の全ジョブが `--features source-build` でビルドしており、build.rs の `download_prebuilt` 系 (ダウンロード URL、SHA256 検証、tar 展開、`lib/libvpx.a` + `bindings.rs` の配置) は CI で一度も実行されない
- アーカイブ URL の形式 (`libvpx-<target>.tar.gz`) とアーカイブ内レイアウトは release.yml と build.rs の 2 箇所に手書きされており、片方だけ変えると即死する。ズレはユーザー環境でしか発覚しない
- release.yml の `build-prebuilt` はアーカイブを作るだけで、アーカイブを展開して実際にリンク・ビルドが通るかは検証せず publish に進む

## 設計方針

- CI (またはリリースワークフロー) に `cargo build` (source-build なし) のジョブを追加し、prebuilt パスを検証する
- 理想は canary リリース後に、そのリリースの prebuilt アーカイブでビルドを検証するワークフロー

## 完了条件

- prebuilt ダウンロード → SHA256 検証 → 展開 → リンク → テストまで通るジョブが CI に存在する
- アーカイブ形式を変更しても CI で検出できる
