# Windows のビルドを MinGW から MSVC に切り替える

- Created: 2026-10-14
- Completed: {YYYY-MM-DD}
- Branch: feature/change-switch-windows-to-msvc
- Polished: {YYYY-MM-DD}

## 目的

Windows のビルドツールチェーンを MSVC に統一し、MSVC でビルドされた他のネイティブライブラリと同じバイナリにリンクできるようにする。

現状の libvpx-rs は Windows だけ MinGW-w64 (GCC) でビルドしている。MSVC でビルドされたライブラリと 1 つのバイナリに混在リンクすることができないため、Windows で複数のネイティブ依存をまとめて使う構成が成立しない。

## 現状

- `build.rs` の `build_from_source_windows()` が `./configure --target=x86_64-win64-gcc` を `sh` + `make` で実行する MinGW-w64 固定。`build_from_source_platform()` は `CARGO_CFG_TARGET_ENV` を見ておらず、Windows なら常にこの経路を通る
- `get_target_platform()` は msvc / gnu を区別せず `windows_x86_64` を返すため、`x86_64-pc-windows-msvc` と `x86_64-pc-windows-gnu` で同じ prebuilt アーカイブを取得する
- `download_prebuilt()` はアーカイブ内の `lib/libvpx.a` を決め打ちでコピーする。MSVC では `vpx.lib` になるためこのままでは取得できない (`find_static_library()` は既に `vpx.lib` を探索できる)
- `.github/workflows/release.yml` の `build-prebuilt-windows` は MSYS2 MINGW64 + `x86_64-pc-windows-gnu` で prebuilt を生成し、`.github/workflows/ci.yml` の `test-windows` も同じ構成
- `README.md` は Windows の前提として MSYS2 + MINGW64 (`gcc` / `make` / `nasm` / `clang`) を要求している
- MSVC と MinGW の静的ライブラリは CRT が異なるため混在リンクできない
  - libvpx (MinGW): `___chkstk_ms`, `__mingw_fprintf`, `__mingw_vsnprintf`, `CONFIG_STATIC_MSVCRT`
  - MSVC でビルドされたライブラリ: `__chkstk`, `__imp___acrt_iob_func`, `__imp__beginthreadex`

## 設計方針

- Windows は MSVC のみをサポートし、MinGW (`x86_64-pc-windows-gnu`) は非対応とする。非対応ターゲットでは紛らわしいリンクエラーではなく明示的に panic する
- libvpx 上流 v1.17.0 の configure は `x86_64-win64-vs17` / `x86_64-win64-vs18` をサポートする。MSVC 経路は configure が生成する Visual Studio ソリューションを `msbuild` でビルドする (x86_64 に clangcl ターゲットは無い)
- Visual Studio 環境の検出は `cc` クレートの `windows_registry` (`msbuild.exe` / `cl.exe` 探索) を build-dependency に追加して行う
- MSVC のビルド成果物は `lib/libvpx.a` ではなく `<src>/x64/Release/vpx.lib` に出力されるため、探索先を変更する
- シンボル書き換え (`llvm-nm` / `llvm-objcopy`) は COFF の `.lib` と x64 (先頭 `_` なし) を前提にそのまま適用し、リンク検証で確認する

## 完了条件

- `x86_64-pc-windows-msvc` で `cargo build --features source-build` が通り、`vpx.lib` を生成してシンボル書き換えまで完了する
- prebuilt (`vpx.lib`) をダウンロードして `x86_64-pc-windows-msvc` でリンク・テストが通る
- MSVC でビルドされた他のネイティブライブラリと同一バイナリにリンクできる
- `.github/workflows/ci.yml` に Windows MSVC のビルド・テストジョブがあり、MinGW 前提のジョブが無い
- `.github/workflows/release.yml` が `lib/vpx.lib` を含む `windows_x86_64` アーカイブを生成する
- `README.md` / `CHANGES.md` の Windows 前提 (MSYS2 + MINGW64) を MSVC 向けに更新する
