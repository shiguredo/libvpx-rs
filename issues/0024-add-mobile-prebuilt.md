# iOS / Android 向け prebuilt を追加する

- Created: 2026-10-03
- Completed: {YYYY-MM-DD}
- Branch: feature/update-libvpx-mobile-prebuilt
- Polished: {YYYY-MM-DD}

## 目的

iOS / Android アプリから libvpx-rs を利用するときに、ソースビルドを要求せず prebuilt のダウンロードで使えるようにする。aom-rs / opus-rs と同じくモバイル向け prebuilt を配布する。

## 現状

- `build.rs` の `get_target_platform()` は Linux / macOS / Windows のみ対応し、iOS / Android では `unsupported target` で panic する
- `build_from_source_unix()` はターゲット共通の configure 呼び出しのみで、iOS / Android のクロスコンパイルに対応していない
- `.github/workflows/release.yml` はデスクトップ向け prebuilt のみ配布する
- libvpx は arm64 シミュレーター (`arm64-iphonesimulator-gcc`) を公式サポートしない
  - `configure` の `all_platforms` に存在せず `--target=arm64-iphonesimulator-gcc` は `Unrecognized toolchain` で失敗する
  - 公式の `build/make/iosbuild.sh` も実機 arm64 (`arm64-darwin-gcc`) と x86_64 シミュレーター (`x86_64-iphonesimulator-gcc`) のみを対象にしている
  - configure の解析ロジック自体は arm64 シミュレーターを構造的に処理できるため、パッチで対応する

## 設計方針

追加する prebuilt とビルド方法の対応は次のとおり。

| prebuilt | Rust ターゲット | configure ターゲット | 備考 |
|---|---|---|---|
| `ios_arm64` | `aarch64-apple-ios` | `arm64-darwin-gcc` | configure が iphoneos SDK の clang を自動選択する |
| `ios-sim_arm64` | `aarch64-apple-ios-sim` | `arm64-iphonesimulator-gcc` | 下記パッチを適用する |
| `android_arm64` | `aarch64-linux-android` | `arm64-android-gcc` | NDK の clang ラッパーを指定する |
| `android_x86_64` | `x86_64-linux-android` | `x86_64-android-gcc` | NDK の clang ラッパーと NASM が必要 |

- libvpx の `git clone` 後に、arm64 シミュレーター向けのときだけ `build.rs` が次のパッチを当てる
  - `configure` の `all_platforms` に `arm64-iphonesimulator-gcc` を追加する
  - `build/make/configure.sh` の `*-iphonesimulator-*` で、arm64 のときだけ `-miphoneos-version-min` ではなく `-mios-simulator-version-min=14.0` を使う (`-miphoneos-version-min` のままだと clang が実機 iOS と誤判定し、configure のリンク検査と生成物のプラットフォームが壊れる)
  - 置換対象が見つからない場合は panic し、upstream の変更を検知する。upstream が対応したらパッチを削除する
- Android は `ANDROID_NDK_HOME` の NDK ツールチェーンを `CC` / `CXX` / `AR` / `AS` / `LD` に指定する
  - API level は `ANDROID_PLATFORM` (未指定時 21、数値または `android-<数値>`)
  - ホストは Linux / macOS / Windows の NDK ツールチェーンに対応する
- bindgen にはターゲットに合わせた clang 引数 (`--target` と `--sysroot` / `-isysroot`) を渡す
- iOS 実機は libvpx の既定の最小バージョン (7.0) でビルドする
  - 13.0 以上を指定すると clang が `___chkstk_darwin` を参照するが、このシンボルは iOS 13 以降の libSystem にしか公開されない。Rust の `aarch64-apple-ios` は iOS 10.0 向けにリンクするため未定義シンボルでリンクに失敗する
  - 7.0 では clang がインラインのスタックプローブを生成するため外部シンボルに依存しない
- arm64 シミュレーターは 14.0 固定 (Rust の `aarch64-apple-ios-sim` の下限)
- CI と Release は aom-rs / opus-rs と同じく `.github/workflows/mobile.yml` を共用する
  - CI は source-build で `cargo test --lib --no-run` まで実行し、静的ライブラリのリンクを検証する
  - Release はアップロード後に prebuilt をダウンロードして再リンク検証する
- prebuilt アーカイブには `lib/libvpx.a`、`bindings.rs`、`LICENSE`、`PATENTS` を同梱する

## 完了条件

- 4 ターゲットの prebuilt が GitHub Release にアップロードされる
- `cargo build --target <target>` (prebuilt パス) が各ターゲットでリンクできることを CI で検証する
- `README.md` にモバイルの動作要件とソースビルド手順を記載する

## 解決方法

{記入}
