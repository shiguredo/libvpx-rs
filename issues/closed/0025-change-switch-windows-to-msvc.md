# Windows のビルドを MinGW から MSVC に切り替える

- Created: 2026-10-14
- Completed: 2026-10-14
- Branch: feature/change-switch-windows-to-msvc
- Polished: 2026-10-14

## 目的

Windows のビルドツールチェーンを MSVC に統一し、MSVC でビルドされた他のネイティブライブラリと同じバイナリにリンクできるようにする。

現状の libvpx-rs は Windows だけ MinGW-w64 (GCC) でビルドしている。MSVC でビルドされたライブラリと 1 つのバイナリに混在リンクすることができないため、Windows で複数のネイティブ依存をまとめて使う構成が成立しない。

## 現状

- `build.rs` の `build_from_source_windows()` が `./configure --target=x86_64-win64-gcc` を `sh` + `make` で実行する MinGW-w64 固定。`build_from_source_platform()` は `CARGO_CFG_TARGET_ENV` を見ておらず、Windows なら常にこの経路を通る
- `get_target_platform()` は msvc / gnu を区別せず `windows_x86_64` を返すため、`x86_64-pc-windows-msvc` と `x86_64-pc-windows-gnu` で同じ prebuilt アーカイブを取得する
- `download_prebuilt()` はアーカイブ内の `lib/libvpx.a` を決め打ちでコピーする。MSVC の成果物は `vpxmd.lib` であり、リンク名が探す `vpx.lib` とも名前が一致しないためこのままでは取得できない (`find_static_library()` は `libvpx.a` と `vpx.lib` の 2 つを探索できる)
- `.github/workflows/release.yml` の `build-prebuilt-windows` は MSYS2 MINGW64 + `x86_64-pc-windows-gnu` で prebuilt を生成し、`.github/workflows/ci.yml` の `test-windows` も同じ構成
- `README.md` は Windows の前提として MSYS2 + MINGW64 (`gcc` / `make` / `nasm` / `clang`) を要求している
- MSVC と MinGW の静的ライブラリは CRT が異なるため混在リンクできない
  - libvpx (MinGW) の未定義シンボル: `___chkstk_ms`, `__mingw_fprintf`, `__mingw_vsnprintf`, `__imp__setjmp`
  - MSVC でビルドされたライブラリの未定義シンボル: `__chkstk`, `__imp___acrt_iob_func`, `__imp__beginthreadex`

## 設計方針

- Windows は MSVC のみをサポートし、MinGW (`x86_64-pc-windows-gnu`) は非対応とする。非対応ターゲットでは紛らわしいリンクエラーではなく明示的に panic する
- libvpx 上流 v1.17.0 の configure は `x86_64-win64-vs17` / `x86_64-win64-vs18` をサポートする (x86_64 に clangcl ターゲットは無い)。configure は Visual Studio ソリューションを生成し、`make` が `msbuild` を呼んでビルドする。configure の `--target` は CI / 利用環境にインストールされた Visual Studio のバージョンに合わせる
- configure と GNU make は POSIX シェル上で動くため、MSVC 経路でも `sh` / `make` / `nasm` は引き続き必要 (MSYS2 や Git for Windows のシェルを利用する)。コンパイラだけを MSVC に切り替える
- Visual Studio 環境の検出は `cc` クレートの `windows_registry` (`msbuild.exe` / `cl.exe` 探索) を build-dependency に追加して行う
- MSVC のビルド成果物は `lib/libvpx.a` ではなく `<src>/x64/Release/vpxmd.lib` (CRT を動的リンクする既定。`--enable-static-msvcrt` 指定時は `vpxmt.lib`) に出力される。Rust の MSVC ターゲットは既定で CRT を動的リンクするため、既定の `vpxmd.lib` を使う
- `rustc-link-lib=static=vpx` は `vpx.lib` を探すため、生成した `vpxmd.lib` を `vpx.lib` にリネームしてコピーする (`find_static_library()` とリンク名の整合を取る)
- 静的ライブラリのみ必要なので、MSVC 経路でも examples / tools / docs / unit tests を無効化する (`*-vs*` は unit tests / webm_io / libyuv を既定で soft-enable する)
- シンボル書き換え (`llvm-nm` / `llvm-objcopy`) は COFF の `.lib` と x64 (先頭 `_` なし) を前提にそのまま適用し、リンク検証で確認する

## 完了条件

- `x86_64-pc-windows-msvc` で `cargo build --features source-build` が通り、`vpx.lib` を生成してシンボル書き換えまで完了する
- prebuilt (`vpx.lib`) をダウンロードして `x86_64-pc-windows-msvc` でリンク・テストが通る
- MSVC でビルドされた他のネイティブライブラリと同一バイナリにリンクできる
- `.github/workflows/ci.yml` に Windows MSVC のビルド・テストジョブがあり、MinGW 前提のジョブが無い
- `.github/workflows/release.yml` が `lib/vpx.lib` を含む `windows_x86_64` アーカイブを生成する
- `README.md` / `CHANGES.md` の Windows 前提 (MSYS2 + MINGW64) を MSVC 向けに更新する

## 解決方法

`build.rs` の Windows 経路を MSVC に切り替え、`.github/workflows/` の Windows ジョブと `README.md` / `CHANGES.md` を MSVC 向けに更新した。

- `x86_64-pc-windows-msvc` のみをサポートする。`validate_windows_target()` で `TARGET` を検証し、MinGW は CRT が、UWP は WINAPI ファミリが、ARM64 はアーキテクチャが異なるため、リンク時の分かりにくいエラーではなく明示的に panic させる。`LIBVPX_TARGET` による指定よりも先に検証する
- `cc` クレート (build-dependency) で Visual Studio を検出し、configure の `--target` に `x86_64-win64-vs17` / `x86_64-win64-vs18` のいずれかを選ぶ
  - upstream の configure は x86_64 向けに vs14 以降のターゲットを持つが、本クレートは Visual Studio 2022 (v17) と 2026 (v18) のみを受け付ける
- configure が生成する Makefile は PATH 上の msbuild.exe を呼び出すため、検出した msbuild.exe のディレクトリを PATH の先頭に追加する
  - msbuild.exe が見つからない場合、make はビルドをスキップしたまま成功する。`<src>/x64/Release/vpxmd.lib` の存在確認で検出する
- MSVC の成果物 `x64/Release/vpxmd.lib` はリンク名 (`rustc-link-lib=static=vpx`) と場所も名前も一致しないため `lib/vpx.lib` として配置し、`static_library_name()` で prebuilt / ソースビルドの静的ライブラリ名 (`libvpx.a` / `vpx.lib`) を切り替える
- シンボル書き換え (`llvm-nm` / `llvm-objcopy`) は COFF の `.lib` と x64 (先頭 `_` なし) のまま適用できる。ただし MSVC の Release 構成は既定で `/GL` (LTCG) でビルドされ、そのオブジェクトは llvm-objcopy が解釈できないため、プロジェクト生成スクリプトをパッチして `/GL` を無効化する (最適化は `/O2` のまま維持される)
- cargo が build script に渡す `TARGET` は、make が子プロセスへ export する `target` と大文字小文字だけが異なる。MSBuild (Windows 上の .NET) は環境変数の辞書に両方を追加できず CL タスクが失敗する (error MSB6001) ため、MSVC 経路の configure / make には `TARGET` を渡さない
- `.github/workflows/ci.yml` の `test-windows` を `x86_64-pc-windows-msvc` に切り替え、シンボル書き換え (定義済み外部シンボルのプレフィックス) と CRT (MSVC を参照し MinGW を含まない) の検証を追加する
  - MSVC のソリューションは Debug / Release の 2 構成をビルドするため、timeout を 60 分にする
- `.github/workflows/release.yml` の `build-prebuilt-windows` が `lib/vpx.lib` を収録したアーカイブを作成し、シンボル書き換えを検証してからアップロードし、アップロード後に prebuilt を取得してリンク・テストする
- `README.md` の動作要件と `CHANGES.md` の `## develop` を MSVC 向けに更新する

検証は次のとおり。

- ローカル (macOS) で `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --features source-build -- -D warnings`、`cargo test --workspace --features source-build`、prebuilt 経路の `cargo test`、`DOCS_RS=1 cargo doc --no-deps`、`prek run --all-files` が成功する
- upstream libvpx v1.17.0 の configure を実行し、`x86_64-win64-vs17` の生成物 (`OutDir` / `TargetName=vpxmd` / `make install` の配置 / `--disable-*` の効果) を確認する
- `x86_64-pc-windows-msvc` 向け COFF オブジェクトで `llvm-nm` / `llvm-objcopy --redefine-syms` の動作を確認する
- GitHub Actions の `windows-2025-vs2026` ジョブが成功し、MSVC でのソースビルド、シンボル書き換え、Rust のリンク、テスト (25 + 4 + 1 件) が通ることを確認する

完了条件の「MSVC でビルドされた他のネイティブライブラリと同一バイナリにリンクできる」について、CI が検証するのは MSVC の CRT を参照していることと MinGW の CRT が混入していないことまでで、他ライブラリとの混在リンクそのものは検証していない。prebuilt (`lib/vpx.lib`) のダウンロードとリンク・テストは、次回リリースの `build-prebuilt-windows` で初めて実行される。
