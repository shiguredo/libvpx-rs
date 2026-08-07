# 正式リリース手順を文書化する (RELEASE.md)

- Created: 2026-08-06
- Completed: 2026-08-07
- Branch: feature/update-release-procedure
- Polished: {YYYY-MM-DD}

## 目的

canary リリースと正式リリースの手順をリポジトリに文書化し、人為的ミスに依存しないリリース運用にする。

## 現状

- `canary.py` は canary 番号のインクリメントと「次マイナー + canary.0」の 2 方向のみ対応しており、`2026.2.0-canary.1 → 2026.2.0` の canary 外し変換ができない
- 正式リリースに必要な「Cargo.toml の `-canary.X` 除去」「CHANGES.md の `## develop` セクションのバージョン化 + リリース日追記」「release ブランチ作成」「タグ push」「develop へのマージ」の手順がリポジトリ内のどこにも文書化されていない
- 前回リリース (2026.1.0) は release/2026.1.0 ブランチで実施され develop にマージされているが、その手順は残っていない
- `canary.py` の `run_cargo_update` (`cargo update shiguredo_libvpx`) は本リポジトリでは no-op で、他クレートからの流用の残骸

## 設計方針

- リリース手順を RELEASE.md として文書化する (canary リリース手順と正式リリース手順の両方、タグ名と Cargo.toml の一致確認、CHANGES.md のリリース化を含むチェックリスト)
- 必要に応じて `canary.py` に canary 外しモードを追加する

## 完了条件

- RELEASE.md がリポジトリに存在し、canary リリースと正式リリースの手順が記載されている
- 手順に従えばタグ名と Cargo.toml バージョンの不一致が発生しない

## 解決方法

却下する。canary リリース・正式リリースという概念を勝手に作り出しており、リリース手順の文書化 (RELEASE.md) は不要と判断する。リリース用の issue はすべて不要とする方針に従う。
