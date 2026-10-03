use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

// 依存ライブラリの名前
const LIB_NAME: &str = "libvpx";
const LINK_NAME: &str = "vpx";

// シンボル書き換え用のプレフィックス
//
// prebuilt で配布する際、他のライブラリが同じ libvpx シンボル (vpx_codec_encode 等) を
// 使っていると衝突する。この定数のプレフィックスを付けることで回避する。
//
// 変換例:
//   vpx_codec_encode → shiguredo_vpx_codec_encode (vpx_ を shiguredo_vpx_ に置換)
//   vp8_denoiser_free → shiguredo_vpx_vp8_denoiser_free (内部シンボルは単純にプレフィックス付与)
const SYMBOL_PREFIX: &str = "shiguredo_vpx";

fn main() {
    // Cargo.toml か build.rs が更新されたら、依存ライブラリを再ビルドする
    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_SOURCE_BUILD");
    println!("cargo::rerun-if-env-changed=LIBVPX_TARGET");
    println!("cargo::rerun-if-env-changed=ANDROID_NDK_HOME");
    println!("cargo::rerun-if-env-changed=ANDROID_PLATFORM");

    // 各種変数やビルドディレクトリのセットアップ
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("infallible"));
    let output_metadata_path = out_dir.join("metadata.rs");
    let output_bindings_path = out_dir.join("bindings.rs");

    // 各種メタデータを書き込む
    let (url, version) = get_url_and_version();
    fs::write(
        output_metadata_path,
        format!(
            concat!(
                "pub const BUILD_METADATA_REPOSITORY: &str={:?};\n",
                "pub const BUILD_METADATA_VERSION: &str={:?};\n",
            ),
            url, version
        ),
    )
    .expect("failed to write metadata file");

    if env::var("DOCS_RS").is_ok() {
        // Docs.rs 向けのビルドでは git clone ができないので build.rs の処理はスキップして、
        // 代わりに、ドキュメント生成時に最低限必要な定義だけをダミーで出力している。
        //
        // See also: https://docs.rs/about/builds
        fs::write(
            output_bindings_path,
            concat!(
                "pub struct vpx_codec_iface;",
                "pub struct vpx_codec_cx_pkt__bindgen_ty_1__bindgen_ty_1;",
                "pub struct vpx_codec_enc_cfg;",
                "pub struct vpx_image;",
                "pub struct vpx_codec_iter_t;",
                "pub struct vpx_codec_ctx;",
                "pub struct vpx_codec_err_t;",
            ),
        )
        .expect("write file error");
        return;
    }

    let output_lib_dir = if should_use_prebuilt() {
        download_prebuilt(&out_dir)
    } else {
        build_from_source(&out_dir, &output_bindings_path)
    };

    println!("cargo::rustc-link-search={}", output_lib_dir.display());
    println!("cargo::rustc-link-lib=static={LINK_NAME}");
}

// source-build feature が有効でなければ prebuilt を使う
fn should_use_prebuilt() -> bool {
    if env::var("CARGO_FEATURE_SOURCE_BUILD").is_ok() {
        return false;
    }
    true
}

// prebuilt バイナリをダウンロードして展開する
fn download_prebuilt(out_dir: &Path) -> PathBuf {
    let target = get_target_platform();
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is not set");
    let base_url = format!(
        "https://github.com/shiguredo/libvpx-rs/releases/download/{}",
        version
    );
    let archive_name = format!("libvpx-{}.tar.gz", target);
    let archive_url = format!("{}/{}", base_url, archive_name);
    let sha256_url = format!("{}/{}.sha256", base_url, archive_name);

    let archive_path = out_dir.join("prebuilt.tar.gz");
    let sha256_path = out_dir.join("prebuilt.sha256");
    let prebuilt_dir = out_dir.join("prebuilt");
    fs::create_dir_all(&prebuilt_dir).expect("failed to create prebuilt directory");

    // curl でアーカイブをダウンロード
    eprintln!("prebuilt ライブラリをダウンロード中: {}", archive_url);
    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&archive_path)
        .arg(&archive_url)
        .status()
        .expect("failed to execute curl. Ensure curl is installed");
    if !status.success() {
        panic!("failed to download prebuilt library: {}", archive_url);
    }

    // curl で SHA256 チェックサムをダウンロード
    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&sha256_path)
        .arg(&sha256_url)
        .status()
        .expect("failed to execute curl");
    if !status.success() {
        panic!("failed to download SHA256 checksum: {}", sha256_url);
    }

    // SHA256 を検証
    verify_sha256(&archive_path, &sha256_path);

    // tar で展開
    let status = Command::new("tar")
        .args(["xzf"])
        .arg(&archive_path)
        .arg("-C")
        .arg(&prebuilt_dir)
        .status()
        .expect("failed to execute tar. Ensure tar is installed");
    if !status.success() {
        panic!("failed to extract prebuilt archive");
    }

    // ライブラリファイルを OUT_DIR/lib/ にコピー
    //
    // prebuilt アーカイブにはリリースビルド時にシンボル書き換え済みの libvpx.a と
    // #[link_name] 属性付きの bindings.rs が含まれているため、そのままコピーするだけでよい。
    let lib_dir = out_dir.join("lib");
    fs::create_dir_all(&lib_dir).expect("failed to create lib directory");
    fs::copy(
        prebuilt_dir.join("lib").join("libvpx.a"),
        lib_dir.join("libvpx.a"),
    )
    .expect("failed to copy libvpx.a");

    // bindings.rs を OUT_DIR/ にコピー
    fs::copy(
        prebuilt_dir.join("bindings.rs"),
        out_dir.join("bindings.rs"),
    )
    .expect("failed to copy bindings.rs");

    lib_dir
}

// SHA256 チェックサムを検証する
fn verify_sha256(file_path: &Path, sha256_path: &Path) {
    let expected = fs::read_to_string(sha256_path)
        .expect("failed to read SHA256 checksum file")
        .split_whitespace()
        .next()
        .expect("SHA256 checksum file is empty")
        .to_lowercase();

    let actual = compute_sha256(file_path);
    if actual != expected {
        panic!(
            "SHA256 checksum mismatch:\n  expected: {}\n  actual:   {}",
            expected, actual
        );
    }
    eprintln!("SHA256 checksum verified: {}", actual);
}

// ファイルの SHA256 ハッシュを計算する
fn compute_sha256(path: &Path) -> String {
    let output = if cfg!(target_os = "macos") {
        // macOS: shasum を使用
        Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
            .expect("failed to execute shasum. Ensure shasum is installed")
    } else if cfg!(target_os = "windows") {
        // Windows: certutil を使用
        Command::new("certutil")
            .args(["-hashfile"])
            .arg(path)
            .arg("SHA256")
            .output()
            .expect("failed to execute certutil")
    } else {
        // Linux: sha256sum を使用
        Command::new("sha256sum")
            .arg(path)
            .output()
            .expect("failed to execute sha256sum. Ensure coreutils is installed")
    };

    if !output.status.success() {
        panic!("failed to compute SHA256 checksum");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if cfg!(target_os = "windows") {
        // certutil 出力形式:
        // SHA256 hash of <file>:
        // <hash>
        // CertUtil: -hashfile command completed successfully.
        stdout
            .lines()
            .nth(1)
            .expect("unexpected certutil output format")
            .trim()
            .to_lowercase()
    } else {
        // shasum / sha256sum 出力形式: <hash>  <filename>
        stdout
            .split_whitespace()
            .next()
            .expect("unexpected shasum/sha256sum output format")
            .to_lowercase()
    }
}

// ソースからビルドする
fn build_from_source(out_dir: &Path, output_bindings_path: &Path) -> PathBuf {
    let out_build_dir = out_dir.join("build/");
    let src_dir = out_build_dir.join(LIB_NAME);
    let input_header_dir = src_dir.join("include/vpx/");
    let output_lib_dir = src_dir.join("lib/");
    let _ = fs::remove_dir_all(&out_build_dir);
    fs::create_dir(&out_build_dir).expect("failed to create build directory");

    // 依存ライブラリのリポジトリを取得する
    git_clone_external_lib(&out_build_dir);

    // arm64 シミュレーター向けのときは libvpx 本体にパッチを当てる
    apply_libvpx_patches(&src_dir);

    // ソースからビルドする
    build_from_source_platform(&src_dir);

    // 静的ライブラリのシンボルを書き換える
    let callbacks = rewrite_symbols(&output_lib_dir, out_dir);

    // バインディングを生成する
    //
    // parse_callbacks にシンボル書き換え用の ParseCallbacks を渡すことで、
    // 生成されるバインディングに #[link_name = "書き換え後のシンボル名"] が自動付与される。
    bindgen::Builder::default()
        .header(input_header_dir.join("vp8cx.h").display().to_string())
        .header(input_header_dir.join("vp8dx.h").display().to_string())
        .header(input_header_dir.join("vpx_codec.h").display().to_string())
        .header(input_header_dir.join("vpx_decoder.h").display().to_string())
        .header(input_header_dir.join("vpx_encoder.h").display().to_string())
        .clang_args(get_bindgen_clang_args())
        .parse_callbacks(Box::new(callbacks))
        .generate()
        .expect("failed to generate bindings")
        .write_to_file(output_bindings_path)
        .expect("failed to write bindings");

    output_lib_dir
}

// プラットフォームに応じてソースビルドの処理を分岐する
fn build_from_source_platform(src_dir: &Path) {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        build_from_source_windows(src_dir);
    } else {
        build_from_source_unix(src_dir);
    }
}

// Unix 環境でのソースビルド
//
// iOS / Android のクロスコンパイルでは configure に専用のターゲットと
// ツールチェーンを渡す。デスクトップ向けでは従来どおりの configure 呼び出しになる。
fn build_from_source_unix(src_dir: &Path) {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mobile_config = get_mobile_source_build_config(&target_os);

    let mut configure = Command::new("./configure");
    configure
        .arg("--disable-shared")
        .arg("--enable-vp9-highbitdepth")
        .arg(format!("--prefix={}", src_dir.display()));
    for arg in &mobile_config.configure_args {
        configure.arg(arg);
    }
    for (key, value) in &mobile_config.envs {
        configure.env(key, value);
    }
    let success = configure
        .current_dir(src_dir)
        .status()
        .is_ok_and(|status| status.success());
    if !success {
        panic!("[configure] failed to build {LIB_NAME}");
    }

    let success = Command::new("make")
        .current_dir(src_dir)
        .status()
        .is_ok_and(|status| status.success());
    if !success {
        panic!("[make] failed to build {LIB_NAME}");
    }

    let success = Command::new("make")
        .arg("install")
        .current_dir(src_dir)
        .status()
        .is_ok_and(|status| status.success());
    if !success {
        panic!("[make install] failed to build {LIB_NAME}");
    }
}

/// モバイル向けソースビルドの configure 引数と環境変数を保持する
struct MobileSourceBuildConfig {
    /// configure に追加する引数
    configure_args: Vec<String>,
    /// configure に渡す環境変数 (CC / CXX / AR / LD と x86_64 の PATH)
    envs: Vec<(String, String)>,
}

/// ターゲットに応じたモバイル向けソースビルド設定を返す
///
/// デスクトップ向けでは空の設定を返し、従来の configure 呼び出しをそのまま使う。
fn get_mobile_source_build_config(target_os: &str) -> MobileSourceBuildConfig {
    match target_os {
        "ios" => get_ios_source_build_config(),
        "android" => get_android_source_build_config(),
        _ => MobileSourceBuildConfig {
            configure_args: Vec::new(),
            envs: Vec::new(),
        },
    }
}

/// iOS 向けソースビルド設定を返す
fn get_ios_source_build_config() -> MobileSourceBuildConfig {
    // iOS のクロスビルドには Xcode (xcrun) が必要なため、macOS ホスト以外は拒否する。
    // 非 macOS ホストでは configure が成功したように見えて libvpx.a が生成されず、
    // 後続のシンボル書き換えで分かりにくいエラーになる
    if env::consts::OS != "macos" {
        panic!("iOS source-build requires a macOS host with Xcode");
    }

    let rust_target = env::var("TARGET").expect("TARGET is not set");
    let mut configure_args = match rust_target.as_str() {
        // iOS 実機。configure が iphoneos SDK の clang を自動選択する。
        // 最小バージョンは libvpx の既定 (7.0) のまま使う。13.0 以上を指定すると
        // clang が ___chkstk_darwin を参照するようになるが、このシンボルは
        // iOS 13 以降の libSystem にしか公開されず、Rust の aarch64-apple-ios は
        // iOS 10.0 向けにリンクするため未定義になる
        "aarch64-apple-ios" => vec!["--target=arm64-darwin-gcc".to_string()],
        // iOS arm64 シミュレーター。apply_libvpx_patches() で
        // arm64-iphonesimulator-gcc を利用可能にしてから configure する
        "aarch64-apple-ios-sim" => vec!["--target=arm64-iphonesimulator-gcc".to_string()],
        _ => panic!("unsupported iOS target for source-build: {rust_target}"),
    };

    // 静的ライブラリのみ必要なため、実行ファイル (examples / tools) と
    // テストはビルドしない
    configure_args.extend(disable_extra_build_targets());

    MobileSourceBuildConfig {
        configure_args,
        envs: Vec::new(),
    }
}

/// Android 向けソースビルド設定を返す
fn get_android_source_build_config() -> MobileSourceBuildConfig {
    let rust_target = env::var("TARGET").expect("TARGET is not set");
    let (clang_target, configure_target) = match rust_target.as_str() {
        "aarch64-linux-android" => ("aarch64-linux-android", "arm64-android-gcc"),
        "x86_64-linux-android" => ("x86_64-linux-android", "x86_64-android-gcc"),
        _ => panic!("unsupported Android target for source-build: {rust_target}"),
    };

    let api_level = get_android_api_level();
    let toolchain_bin = android_toolchain_bin_dir();
    let clang = toolchain_bin.join(format!("{clang_target}{api_level}-clang"));
    let clangxx = toolchain_bin.join(format!("{clang_target}{api_level}-clang++"));
    let llvm_ar = toolchain_bin.join(exe_name("llvm-ar"));
    for path in [&clang, &clangxx, &llvm_ar] {
        if !path.exists() {
            panic!("Android NDK tool not found: {}", path.display());
        }
    }

    let mut configure_args = vec![
        format!("--target={configure_target}"),
        // Android アプリの共有ライブラリに静的にリンクするため PIC を有効にする。
        // CONFIG_PIC が vpx_config.asm 経由で NASM の PIC マクロにも伝わる
        "--enable-pic".to_string(),
    ];
    configure_args.extend(disable_extra_build_targets());

    let mut envs = vec![
        ("CC".to_string(), clang.display().to_string()),
        ("CXX".to_string(), clangxx.display().to_string()),
        ("AR".to_string(), llvm_ar.display().to_string()),
        ("LD".to_string(), clang.display().to_string()),
    ];
    if rust_target == "x86_64-linux-android" {
        // x86_64 の NASM アラインメント検査は readelf を使う。
        // macOS には readelf がないため、NDK の llvm-readelf を呼ぶシムを
        // PATH に追加する。arm64 はアセンブリをビルドしないため不要
        envs.push(create_readelf_shim_env(&toolchain_bin));
    }

    MobileSourceBuildConfig {
        configure_args,
        envs,
    }
}

/// readelf シムを PATH に追加する環境変数を返す
///
/// configure の NASM アラインメント検査 (`check_asm_align`) は `readelf` を
/// 呼び出すが、macOS には readelf が存在しない。NDK が持つ `llvm-readelf` を
/// 呼び出すシムを OUT_DIR に作成し、PATH の先頭に追加する。
fn create_readelf_shim_env(toolchain_bin: &Path) -> (String, String) {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let shim_dir = out_dir.join("ndk-bin-shims");
    fs::create_dir_all(&shim_dir).expect("failed to create shim directory");

    let llvm_readelf = toolchain_bin.join("llvm-readelf");
    if !llvm_readelf.exists() {
        panic!("Android NDK tool not found: {}", llvm_readelf.display());
    }

    let shim_path = shim_dir.join("readelf");
    fs::write(
        &shim_path,
        format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", llvm_readelf.display()),
    )
    .expect("failed to write readelf shim");

    // 実行権限を付与する (Windows ホストは android_toolchain_bin_dir で拒否するが、
    // Windows 向けのコンパイルでも通るように cfg を付ける)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&shim_path)
            .expect("failed to read shim metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim_path, permissions).expect("failed to set shim permissions");
    }

    let path = env::var("PATH").unwrap_or_default();
    (
        "PATH".to_string(),
        format!("{}:{}", shim_dir.display(), path),
    )
}

/// 静的ライブラリ以外のビルド対象を無効化する configure 引数を返す
fn disable_extra_build_targets() -> Vec<String> {
    [
        "--disable-examples",
        "--disable-tools",
        "--disable-docs",
        "--disable-unit-tests",
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect()
}

/// ANDROID_PLATFORM から API level を取得する
///
/// 値は数値または `android-<数値>` 形式で指定でき、未指定時は 21 とする。
/// NDK が下限未満の API level を引き上げて bindgen と食い違うため、
/// 21 未満は拒否する。
fn get_android_api_level() -> u32 {
    let platform = env::var("ANDROID_PLATFORM").unwrap_or_else(|_| "21".to_string());
    let api_level = platform
        .strip_prefix("android-")
        .unwrap_or(&platform)
        .parse::<u32>()
        .expect("ANDROID_PLATFORM must be a numeric API level or android-<API level>");
    assert!(
        api_level >= 21,
        "ANDROID_PLATFORM must be at least API level 21"
    );
    api_level
}

/// Android NDK のホスト用ツールチェーン bin ディレクトリを返す
fn android_toolchain_bin_dir() -> PathBuf {
    let ndk = PathBuf::from(
        env::var_os("ANDROID_NDK_HOME")
            .expect("ANDROID_NDK_HOME is not set. Set it to the Android NDK directory"),
    );
    // libvpx の configure は CC などのパスをクォートせずに実行するため、
    // スペースを含むパスは早期に拒否する
    if ndk.to_string_lossy().contains(' ') {
        panic!(
            "ANDROID_NDK_HOME must not contain spaces: {}",
            ndk.display()
        );
    }

    // NDK が提供するホスト別のツールチェーンを使う。
    // Linux arm64 と Windows には NDK のツールチェーンが存在しない
    let host_tag = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x86_64",
        // macOS は x86_64 / arm64 とも darwin-x86_64 (fat バイナリ)
        ("macos", _) => "darwin-x86_64",
        (os, arch) => panic!(
            "Android NDK does not provide a toolchain for {os} {arch}. \
             Use an x86_64 Linux host or macOS"
        ),
    };
    ndk.join("toolchains/llvm/prebuilt")
        .join(host_tag)
        .join("bin")
}

// --- libvpx 本体へのパッチ ---
//
// libvpx は arm64 シミュレーター (arm64-iphonesimulator-gcc) を公式サポートしていない。
// 次の 2 点を最小限パッチして利用可能にする。
//
//   1. configure のプラットフォーム一覧に arm64-iphonesimulator-gcc を追加する
//   2. build/make/configure.sh で arm64 のときだけ -mios-simulator-version-min を使う
//
// 2 が必要な理由: iphonesimulator 向けの共通設定は -miphoneos-version-min を
// 使っているが、arm64 でこのフラグと -isysroot <iphonesimulator> を組み合わせると
// clang が実機 iOS 向けと誤判定し、configure のリンク検査と生成物の
// プラットフォームが壊れる。arm64 シミュレーターの最小バージョンは 14.0 のため、
// -mios-simulator-version-min=14.0 と -arch arm64 を明示する。
//
// 置換対象が見つからない場合は upstream の変更を検知するため panic する。
// upstream が arm64 シミュレーターに対応したらこの処理は削除する。

/// arm64 シミュレーター向けに libvpx 本体へパッチを適用する
///
/// iOS 実機や Android、デスクトップ向けでは何もしない。
fn apply_libvpx_patches(src_dir: &Path) {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let rust_target = env::var("TARGET").unwrap_or_default();
    if target_os != "ios" || rust_target != "aarch64-apple-ios-sim" {
        return;
    }

    // configure のプラットフォーム一覧に arm64-iphonesimulator-gcc を追加する
    let configure_path = src_dir.join("configure");
    let configure = fs::read_to_string(&configure_path).expect("failed to read libvpx configure");
    let configure = replace_once(
        "configure",
        &configure,
        "all_platforms=\"${all_platforms} x86_64-iphonesimulator-gcc\"",
        concat!(
            "all_platforms=\"${all_platforms} x86_64-iphonesimulator-gcc\"\n",
            "all_platforms=\"${all_platforms} arm64-iphonesimulator-gcc\"",
        ),
    );
    fs::write(&configure_path, configure).expect("failed to write libvpx configure");

    // arm64 向けの最小バージョン指定を -mios-simulator-version-min に切り替える
    let configure_sh_path = src_dir.join("build/make/configure.sh");
    let configure_sh =
        fs::read_to_string(&configure_sh_path).expect("failed to read libvpx configure.sh");
    let configure_sh = replace_once(
        "build/make/configure.sh",
        &configure_sh,
        concat!(
            "    *-iphonesimulator-*)\n",
            "      add_cflags  \"-miphoneos-version-min=${IOS_VERSION_MIN}\"\n",
            "      add_ldflags \"-miphoneos-version-min=${IOS_VERSION_MIN}\"",
        ),
        concat!(
            "    *-iphonesimulator-*)\n",
            "      if [ \"${tgt_isa}\" = \"arm64\" ]; then\n",
            "        # arm64 は -miphoneos-version-min だと実機 iOS 向けと誤判定されるため\n",
            "        # シミュレーター向けの最小バージョンとアーキテクチャを明示する\n",
            "        add_cflags  \"-arch arm64 -mios-simulator-version-min=14.0\"\n",
            "        add_ldflags \"-arch arm64 -mios-simulator-version-min=14.0\"\n",
            "      else\n",
            "        add_cflags  \"-miphoneos-version-min=${IOS_VERSION_MIN}\"\n",
            "        add_ldflags \"-miphoneos-version-min=${IOS_VERSION_MIN}\"\n",
            "      fi",
        ),
    );
    fs::write(&configure_sh_path, configure_sh).expect("failed to write libvpx configure.sh");
}

/// 文字列を 1 箇所だけ置換する
///
/// 置換対象がちょうど 1 箇所でない場合は upstream の変更を検知するため panic する。
fn replace_once(file: &str, content: &str, from: &str, to: &str) -> String {
    let count = content.matches(from).count();
    if count != 1 {
        panic!(
            "expected exactly one occurrence of the patch target in {file}, found {count}: {from:?}"
        );
    }
    content.replacen(from, to, 1)
}

/// bindgen に渡す clang 引数をターゲットに応じて返す
///
/// iOS / Android ではホストとターゲットのプラットフォームが異なるため、
/// ターゲット triple と SDK / sysroot を明示する必要がある。
fn get_bindgen_clang_args() -> Vec<String> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let rust_target = env::var("TARGET").expect("TARGET is not set");
    match target_os.as_str() {
        "ios" => {
            let (sdk, arch, simulator_suffix, deployment_target) = match rust_target.as_str() {
                // iOS 実機の最小バージョンは libvpx の既定 (7.0) に合わせる
                "aarch64-apple-ios" => ("iphoneos", "arm64", "", "7.0".to_string()),
                // arm64 シミュレーターは 14.0 固定 (Rust ターゲットの下限)
                "aarch64-apple-ios-sim" => {
                    ("iphonesimulator", "arm64", "-simulator", "14.0".to_string())
                }
                _ => panic!("unsupported iOS target for bindgen: {rust_target}"),
            };
            let sdk_path = get_ios_sdk_path(sdk);
            vec![
                format!("--target={arch}-apple-ios{deployment_target}{simulator_suffix}"),
                "-isysroot".to_string(),
                sdk_path,
            ]
        }
        "android" => {
            let clang_target = match rust_target.as_str() {
                "aarch64-linux-android" => "aarch64-linux-android",
                "x86_64-linux-android" => "x86_64-linux-android",
                _ => panic!("unsupported Android target for bindgen: {rust_target}"),
            };
            let api_level = get_android_api_level();
            // NDK ツールチェーンの sysroot は bin の隣にある
            let sysroot = android_toolchain_bin_dir().join("../sysroot");
            vec![
                format!("--target={clang_target}{api_level}"),
                format!("--sysroot={}", sysroot.display()),
            ]
        }
        _ => Vec::new(),
    }
}

/// xcrun で iOS SDK のパスを取得する
fn get_ios_sdk_path(sdk: &str) -> String {
    let output = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .expect("failed to run xcrun. Ensure Xcode is installed");
    if !output.status.success() {
        panic!("failed to find iOS SDK: {sdk}");
    }
    String::from_utf8(output.stdout)
        .expect("invalid iOS SDK path")
        .trim()
        .to_string()
}

// Windows + MSYS2 環境でのソースビルド
fn build_from_source_windows(src_dir: &Path) {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let configure_target = match target_arch.as_str() {
        "x86_64" => "x86_64-win64-gcc",
        _ => panic!("unsupported Windows arch for source-build: {}", target_arch),
    };

    // `configure` はシェルスクリプトなので sh 経由で実行する
    run_with_shell(
        src_dir,
        &format!(
            "./configure --target={configure_target} \
             --disable-shared --enable-vp9-highbitdepth --prefix=\"$(pwd)\""
        ),
        "configure",
    );
    run_with_shell(src_dir, "make", "make");
    run_with_shell(src_dir, "make install", "make install");
}

// shell 経由でコマンドを実行する
fn run_with_shell(src_dir: &Path, command: &str, step_name: &str) {
    let success = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(src_dir)
        .status()
        .is_ok_and(|status| status.success());
    if !success {
        panic!("[{}] failed to build {LIB_NAME} on Windows", step_name);
    }
}

// --- シンボル書き換え ---
//
// 他のライブラリとのシンボル衝突を回避するため、静的ライブラリ内の全シンボルに
// プレフィックスを付与する仕組み。
//
// llvm-nm / llvm-objcopy は rustup の llvm-tools コンポーネントに含まれるものを使用する。
// rust-toolchain.toml に components = ["llvm-tools"] の記載が必要。
//
// プラットフォームごとのシンボル形式の違い:
//   - macOS / iOS (Mach-O): シンボル先頭に `_` が付く (例: _vpx_codec_encode)
//   - Linux / Android (ELF): 先頭 `_` なし (例: vpx_codec_encode)
//   - Windows x64 (COFF): 先頭 `_` なし (例: vpx_codec_encode)
//
// bindgen の generated_link_name_override は返した文字列に \u{1} プレフィックスを
// 自動付加する。\u{1} はコンパイラに「この名前をそのまま使え（マングリングするな）」と
// 指示するため、プラットフォーム固有のシンボル名（macOS / iOS なら _shiguredo_vpx_codec_encode）を
// そのまま返す必要がある。

/// llvm-nm / llvm-objcopy のパスを保持する
struct LlvmTools {
    nm: PathBuf,
    objcopy: PathBuf,
}

/// objcopy 用と bindgen 用の 2 つのリネームマップを保持する
///
/// 2 つのマップが必要な理由:
///   - objcopy_map: ライブラリ内の実シンボル名を書き換えるため、プラットフォーム依存の名前を使う
///   - bindgen_map: Rust コードからリンクする際の名前を指定するため、C シンボル名をキーにする
struct SymbolRenameMaps {
    /// llvm-objcopy の --redefine-syms 用マップ
    ///
    /// キー: 元のシンボル名 (例: macOS / iOS なら _vpx_codec_encode、Linux / Android なら vpx_codec_encode)
    /// 値: 書き換え後のシンボル名 (例: macOS / iOS なら _shiguredo_vpx_codec_encode)
    objcopy_map: HashMap<String, String>,

    /// bindgen の #[link_name] 用マップ
    ///
    /// キー: C シンボル名 (プラットフォーム非依存、例: vpx_codec_encode)
    /// 値: 書き換え後のシンボル名 (プラットフォーム依存、例: macOS / iOS なら _shiguredo_vpx_codec_encode)
    ///
    /// bindgen は \u{1} プレフィックスを付加してマングリングを抑制するため、
    /// 値にはプラットフォーム固有のシンボル名を格納する必要がある。
    bindgen_map: HashMap<String, String>,
}

/// bindgen の ParseCallbacks 実装
///
/// バインディング生成時に、書き換え後のシンボル名を `#[link_name = "..."]` として付与する。
/// これにより lib.rs 側のコード変更なしでシンボル書き換えが透過的に動作する。
#[derive(Debug)]
struct SymbolLinkNameCallbacks {
    /// C シンボル名 → 書き換え後シンボル名のマップ
    rename_map: HashMap<String, String>,
}

impl bindgen::callbacks::ParseCallbacks for SymbolLinkNameCallbacks {
    /// bindgen がバインディングを生成する際に呼ばれるコールバック
    ///
    /// 戻り値が Some の場合、bindgen は #[link_name = "\u{1}<戻り値>"] を生成する。
    /// \u{1} プレフィックスによりコンパイラのシンボルマングリングが抑制されるため、
    /// 戻り値にはプラットフォーム固有のシンボル名を返す必要がある。
    fn generated_link_name_override(
        &self,
        item_info: bindgen::callbacks::ItemInfo<'_>,
    ) -> Option<String> {
        self.rename_map.get(item_info.name).cloned()
    }
}

/// 静的ライブラリのシンボルを書き換え、bindgen 用の ParseCallbacks を返す
///
/// 処理の流れ:
///   1. rustup の sysroot から llvm-nm / llvm-objcopy を探す
///   2. llvm-nm で静的ライブラリの定義済み外部シンボルを収集する
///   3. 収集したシンボルに対してリネームマップを生成する
///   4. マップファイルを書き出し、llvm-objcopy でライブラリ内のシンボルを書き換える
///   5. bindgen 用の ParseCallbacks を返す
fn rewrite_symbols(lib_dir: &Path, out_dir: &Path) -> SymbolLinkNameCallbacks {
    let tools = discover_llvm_tools();
    let lib_path = find_static_library(lib_dir);

    // macOS / iOS の Mach-O ではシンボル先頭に `_` が付くため、
    // プラットフォーム判定してリネームマップの生成時に考慮する
    let is_macho = env::var("CARGO_CFG_TARGET_VENDOR")
        .map(|v| v == "apple")
        .unwrap_or(false);

    // シンボル名の変換ルール
    //
    // vpx_ プレフィックスを持つシンボル (公開 API) は vpx_ を SYMBOL_PREFIX_ に置換する。
    //   例: vpx_codec_encode → shiguredo_vpx_codec_encode
    //
    // それ以外のシンボル (vp8_*, vp9_* 等の内部シンボル) は先頭に SYMBOL_PREFIX_ を付与する。
    //   例: vp8_denoiser_free → shiguredo_vpx_vp8_denoiser_free
    let rename_symbol = |name: &str| -> Option<String> {
        if let Some(rest) = name.strip_prefix("vpx_") {
            Some(format!("{SYMBOL_PREFIX}_{rest}"))
        } else {
            Some(format!("{SYMBOL_PREFIX}_{name}"))
        }
    };

    // 全定義済み外部シンボルを収集してリネームマップを生成する
    let symbols = collect_defined_external_symbols(&tools.nm, &lib_path);
    let maps = build_symbol_rename_maps(&symbols, is_macho, &rename_symbol);

    // マップファイルを書き出してシンボルを書き換える
    let map_file = out_dir.join("symbol_rename_map.txt");
    write_objcopy_rename_map(&maps.objcopy_map, &map_file);
    rewrite_archive_symbols(&tools.objcopy, &lib_path, &map_file);

    SymbolLinkNameCallbacks {
        rename_map: maps.bindgen_map,
    }
}

/// 静的ライブラリのパスを探す
///
/// ビルド結果は Unix 系では libvpx.a、Windows では vpx.lib として出力される。
fn find_static_library(lib_dir: &Path) -> PathBuf {
    let unix_path = lib_dir.join("libvpx.a");
    if unix_path.exists() {
        return unix_path;
    }
    let win_path = lib_dir.join("vpx.lib");
    if win_path.exists() {
        return win_path;
    }
    panic!("static library not found in {}", lib_dir.display());
}

/// rustc --print sysroot の結果を取得する
///
/// llvm-tools は rustup が管理する sysroot 配下にインストールされるため、
/// sysroot のパスを取得して llvm-nm / llvm-objcopy の探索に使用する。
fn get_rustc_sysroot() -> PathBuf {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    if !output.status.success() {
        panic!("rustc --print sysroot failed");
    }
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("invalid UTF-8")
            .trim(),
    )
}

/// Windows 対応の実行ファイル名を生成する
///
/// Windows では実行ファイルに .exe 拡張子が必要。
fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// rustup の sysroot から llvm-nm / llvm-objcopy を探す
///
/// llvm-tools コンポーネントのバイナリは以下のパスに配置される:
///   <sysroot>/lib/rustlib/<host>/bin/llvm-nm
///   <sysroot>/lib/rustlib/<host>/bin/llvm-objcopy
///
/// rust-toolchain.toml に llvm-tools コンポーネントの記載が必要。
///
/// llvm-nm / llvm-objcopy はホスト上で実行するツールなので、クロスコンパイル時は
/// TARGET ではなく HOST のパスから探す必要がある。
/// 例: Windows CI でホストが x86_64-pc-windows-msvc、ターゲットが x86_64-pc-windows-gnu の場合、
/// llvm-tools は msvc 側にのみインストールされている。
fn discover_llvm_tools() -> LlvmTools {
    let sysroot = get_rustc_sysroot();
    // llvm-tools はホスト上で動作するため HOST を使う。
    // クロスコンパイル時に TARGET を使うと、ホスト側にインストールされた
    // llvm-tools が見つからない。
    let host = env::var("HOST").expect("HOST environment variable not set");
    let tools_dir = sysroot.join("lib/rustlib").join(host).join("bin");

    let nm = tools_dir.join(exe_name("llvm-nm"));
    let objcopy = tools_dir.join(exe_name("llvm-objcopy"));

    if !nm.exists() {
        panic!(
            "llvm-nm not found at {}. Run: rustup component add llvm-tools",
            nm.display()
        );
    }
    if !objcopy.exists() {
        panic!(
            "llvm-objcopy not found at {}. Run: rustup component add llvm-tools",
            objcopy.display()
        );
    }

    LlvmTools { nm, objcopy }
}

/// llvm-nm で静的ライブラリから定義済み外部シンボルを収集する
///
/// llvm-nm のオプション:
///   --defined-only: 定義済みシンボルのみ (未定義シンボルを除外)
///   --extern-only: 外部シンボルのみ (ローカルシンボルを除外)
///   --format=just-symbols: シンボル名のみ出力 (アドレスやタイプを省略)
///
/// 出力にはオブジェクトファイル名 (例: vp8_cx_iface.c.o:) も含まれるため、
/// is_c_identifier() でフィルタリングして純粋なシンボル名のみを抽出する。
fn collect_defined_external_symbols(nm_path: &Path, lib_path: &Path) -> Vec<String> {
    let output = Command::new(nm_path)
        .arg("--defined-only")
        .arg("--extern-only")
        .arg("--format=just-symbols")
        .arg(lib_path)
        .output()
        .expect("failed to run llvm-nm");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("llvm-nm failed: {stderr}");
    }

    let stdout = String::from_utf8(output.stdout).expect("llvm-nm output is not valid UTF-8");
    let mut symbols: Vec<String> = stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|s| !s.is_empty() && is_c_identifier(s))
        .collect();
    symbols.sort();
    symbols.dedup();
    symbols
}

/// C 識別子として有効かどうかを判定する
///
/// llvm-nm の --format=just-symbols 出力にはオブジェクトファイル名 (vp8_cx_iface.c.o: 等) も
/// 含まれるため、この関数で C 識別子のみをフィルタリングする。
///
/// macOS / iOS の Mach-O ではシンボル先頭に `_` が付くため、`_` で始まる文字列も受け入れる。
fn is_c_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// objcopy 用と bindgen 用のリネームマップを生成する
///
/// 2 つのマップを生成する理由:
///
/// objcopy_map: ライブラリバイナリ内の実シンボル名を書き換えるためのマップ。
///   macOS / iOS では _vpx_codec_encode → _shiguredo_vpx_codec_encode のように
///   プラットフォーム固有の `_` プレフィックスを含む形で管理する。
///
/// bindgen_map: Rust バインディングの #[link_name] に使うマップ。
///   キーは C シンボル名 (vpx_codec_encode)、値はプラットフォーム固有のシンボル名
///   (_shiguredo_vpx_codec_encode) を格納する。
///   bindgen は generated_link_name_override の戻り値に \u{1} を付加してマングリングを
///   抑制するため、プラットフォーム固有の名前を直接返す必要がある。
fn build_symbol_rename_maps(
    symbols: &[String],
    is_macho: bool,
    rename_symbol: &dyn Fn(&str) -> Option<String>,
) -> SymbolRenameMaps {
    let mut objcopy_map = HashMap::new();
    let mut bindgen_map = HashMap::new();

    for sym in symbols {
        // プラットフォーム固有のプレフィックスを除去して C シンボル名を取得する
        //   macOS / iOS: _vpx_codec_encode → vpx_codec_encode
        //   Linux / Android / Windows: vpx_codec_encode → vpx_codec_encode (変化なし)
        let c_name = if is_macho {
            sym.strip_prefix('_').unwrap_or(sym)
        } else {
            sym.as_str()
        };

        if let Some(new_c_name) = rename_symbol(c_name) {
            // objcopy 用: プラットフォーム固有のプレフィックスを再付与する
            //   macOS / iOS: shiguredo_vpx_codec_encode → _shiguredo_vpx_codec_encode
            //   Linux / Android / Windows: shiguredo_vpx_codec_encode → shiguredo_vpx_codec_encode (変化なし)
            let new_sym = if is_macho {
                format!("_{new_c_name}")
            } else {
                new_c_name.clone()
            };
            objcopy_map.insert(sym.clone(), new_sym.clone());

            // bindgen 用: generated_link_name_override は \u{1} プレフィックスを付加して
            // シンボル名をそのまま使うため、プラットフォーム固有のシンボル名で管理する
            bindgen_map.insert(c_name.to_string(), new_sym);
        }
    }

    SymbolRenameMaps {
        objcopy_map,
        bindgen_map,
    }
}

/// --redefine-syms 用のマップファイルを書き出す
///
/// ファイル形式は 1 行に "旧シンボル名 新シンボル名" を空白区切りで記述する。
/// llvm-objcopy の --redefine-syms オプションで使用される。
fn write_objcopy_rename_map(map: &HashMap<String, String>, path: &Path) {
    let mut lines: Vec<String> = map
        .iter()
        .map(|(old, new)| format!("{old} {new}"))
        .collect();
    // 出力を決定的にするためソートする
    lines.sort();
    fs::write(path, lines.join("\n")).expect("failed to write symbol rename map");
}

/// llvm-objcopy でアーカイブ内のシンボルを書き換える
///
/// --redefine-syms はマップファイルに従ってシンボル名を一括置換する。
/// ライブラリファイルはインプレースで更新される。
fn rewrite_archive_symbols(objcopy_path: &Path, lib_path: &Path, map_file: &Path) {
    let status = Command::new(objcopy_path)
        .arg("--redefine-syms")
        .arg(map_file)
        .arg(lib_path)
        .status()
        .expect("failed to run llvm-objcopy");
    if !status.success() {
        panic!("llvm-objcopy failed");
    }
}

// --- 既存のヘルパー関数 ---

// Rust のモバイルターゲット、または OS とアーキテクチャからプラットフォーム名を生成する
fn get_target_platform() -> String {
    if let Ok(target) = env::var("LIBVPX_TARGET") {
        return target;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let rust_target = env::var("TARGET").expect("TARGET is not set");

    // 同じ OS とアーキテクチャでも Catalyst や別の ABI は prebuilt と互換にならない。
    // prebuilt と一致するターゲットだけを受け入れ、異なる ABI への誤リンクを防ぐ。
    if target_os == "ios" || target_os == "android" {
        return match rust_target.as_str() {
            "aarch64-apple-ios" => "ios_arm64",
            "aarch64-apple-ios-sim" => "ios-sim_arm64",
            "aarch64-linux-android" => "android_arm64",
            "x86_64-linux-android" => "android_x86_64",
            _ => panic!("unsupported mobile target: {rust_target}"),
        }
        .to_string();
    }

    match (target_os.as_str(), target_arch.as_str()) {
        ("linux", "x86_64") => format!("{}_x86_64", detect_linux_distro()),
        ("linux", "aarch64") => format!("{}_arm64", detect_linux_distro()),
        ("macos", "aarch64") => "macos_arm64".to_string(),
        ("windows", "x86_64") => "windows_x86_64".to_string(),
        _ => panic!("unsupported target: os={}, arch={}", target_os, target_arch),
    }
}

// /etc/os-release から Ubuntu バージョンを検出する
fn detect_linux_distro() -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(version) = line.strip_prefix("VERSION_ID=") {
                let version = version.trim_matches('"');
                match version {
                    "22.04" | "24.04" | "26.04" => return format!("ubuntu-{}", version),
                    _ => {}
                }
            }
        }
    }
    panic!(
        "unsupported Linux distribution. \
         set LIBVPX_TARGET environment variable to specify the target explicitly"
    );
}

// 外部ライブラリのリポジトリを git clone する
fn git_clone_external_lib(build_dir: &Path) {
    let (url, version) = get_url_and_version();
    let success = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--branch")
        .arg(version)
        .arg(url)
        .current_dir(build_dir)
        .status()
        .is_ok_and(|status| status.success());
    if !success {
        panic!("failed to clone {LIB_NAME} repository");
    }
}

// Cargo.toml から依存ライブラリの URL とバージョンタグを取得する
fn get_url_and_version() -> (String, String) {
    let cargo_toml = shiguredo_toml::Value::Table(
        shiguredo_toml::from_str(include_str!("Cargo.toml")).expect("failed to parse Cargo.toml"),
    );
    if let Some((Some(url), Some(version))) = cargo_toml
        .get("package")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("external-dependencies"))
        .and_then(|v| v.get(LIB_NAME))
        .map(|v| {
            (
                v.get("url").and_then(|s| s.as_str()),
                v.get("version").and_then(|s| s.as_str()),
            )
        })
    {
        (url.to_string(), version.to_string())
    } else {
        panic!(
            "Cargo.toml does not contain a valid [package.metadata.external-dependencies.{LIB_NAME}] table"
        );
    }
}
