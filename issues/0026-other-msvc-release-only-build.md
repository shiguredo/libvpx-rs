# Windows の MSVC ビルドを Release 構成のみにして CI の所要時間を短縮する

- Created: 2026-10-14
- Completed: {YYYY-MM-DD}
- Branch: feature/update-msvc-release-only-build
- Polished: {YYYY-MM-DD}

## 目的

Windows の CI / Release で不要な Debug 構成のビルドを止め、ジョブの所要時間とマシンリソースを削減する。

## 現状

- libvpx の MSVC ターゲットでは configure が Visual Studio のソリューションを生成し、make が Debug|x64 と Release|x64 の 2 構成をビルドする
  - build/make/gen_msvs_sln.sh が vcxproj の構成を集めて vpx.sln.mk の BUILD_TARGETS に積み、build/make/Makefile の all が両構成を対象にする
- build.rs の `build_from_source_windows()` が使うのは `x64/Release/vpxmd.lib` だけであり、Debug 構成の `x64/Debug/vpxmdd.lib` は生成するだけで使っていない
- そのため `.github/workflows/ci.yml` の `test-windows` と `.github/workflows/release.yml` の `build-prebuilt-windows` は C のビルドに本来の約 2 倍の時間がかかり、timeout を 45 分から 60 分に延ばして対応している

## 設計方針

- Release 構成だけをビルドする手順を確立する
  - 構成別のターゲット (`Release_x64` / `Debug_x64`) は生成物の vpx.sln.mk にあり、プロジェクト生成後に `-include` で読み込まれる。fresh な configure 直後に `make target=solution Release_x64` を実行すると `No rule to make target 'Release_x64'` になるため、プロジェクト生成 → Release のみのビルド → make install の順序を決めて検証する
  - make install は Release のみのビルドでも `include/vpx/` のヘッダーと `lib/x64/vpxmd.lib` を配置するため、bindgen が読むヘッダーと build.rs が使う静的ライブラリは揃う
- 上流が生成する内部ターゲット名に依存するため、対象の存在検証は iOS シミュレーター向けパッチと同じ方針で行い、上流の変更を検知できるようにする
- 短縮できた場合は `.github/workflows/ci.yml` と `.github/workflows/release.yml` の timeout を再評価する

## 完了条件

- Windows の CI と Release で Release 構成だけがビルドされ、`cargo test --features source-build` と prebuilt の生成が通る
- `test-windows` の所要時間が短縮され、timeout を 60 分から戻せることを確認する
