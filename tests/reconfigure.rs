// 統合テスト (tests/*.rs) はファイルごとに独立したバイナリとしてコンパイルされる。
// 本テストは外部クレート視点で ReconfigureParams を構造体リテラルで構築できることと、
// reconfigure による量子化レンジの変更が実際の出力バイト量に反映されることを検証する。

use std::num::NonZeroUsize;

use shiguredo_libvpx::{
    CodecConfig, EncodeOptions, Encoder, EncoderConfig, EncodingDeadline, ImageData, ImageFormat,
    ReconfigureParams, Vp9Config,
};

#[path = "helpers/helpers.rs"]
mod helpers;
use helpers::generate_gradient_i420;

const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const FRAMES: usize = 20;

/// 1 フレームエンコードして全パケットをドレインし、出力バイト数の合計を返す
fn encode_and_drain(encoder: &mut Encoder, y: &[u8], u: &[u8], v: &[u8]) -> usize {
    encoder
        .encode(
            &ImageData::I420 { y, u, v },
            &EncodeOptions {
                force_keyframe: false,
            },
        )
        .expect("エンコードに失敗した");

    let mut total = 0usize;
    while let Some(frame) = encoder.next_frame() {
        total += frame.data().len();
    }
    total
}

/// 量子化レンジを `quantizer` に固定したエンコーダーで `FRAMES` 枚をエンコードし、
/// 出力バイト数の合計を返す
///
/// `ReconfigureParams` は外部クレート視点の構造体リテラル (`..Default::default()`
/// 付き) で構築する。`#[non_exhaustive]` が付いているとこの構築は E0639 で
/// コンパイルエラーになるため、このテスト自体が外部クレートからの構築可能性を検証する。
fn total_bytes_with_quantizer(quantizer: usize) -> usize {
    // 量子化レンジを 0 固定する側で CBR cap に頭打ちされないように上限を十分大きく取る
    let mut config = EncoderConfig::new(
        WIDTH,
        HEIGHT,
        ImageFormat::I420,
        CodecConfig::Vp9(Vp9Config::default()),
    );
    config.target_bitrate = 50_000_000;
    config.threads = NonZeroUsize::new(1);
    config.deadline = EncodingDeadline::Realtime;
    config.error_resilient = true;
    config.min_quantizer = 0;
    config.max_quantizer = 63;

    let mut encoder = Encoder::new(config).expect("エンコーダーの生成に失敗した");

    encoder
        .reconfigure(&ReconfigureParams {
            min_quantizer: Some(quantizer),
            max_quantizer: Some(quantizer),
            ..ReconfigureParams::default()
        })
        .expect("再設定に失敗した");

    let (y, u, v) = generate_gradient_i420(WIDTH, HEIGHT);
    let mut total = 0usize;
    for _ in 0..FRAMES {
        total += encode_and_drain(&mut encoder, &y, &u, &v);
    }

    encoder.finish().expect("終了処理に失敗した");
    while let Some(frame) = encoder.next_frame() {
        total += frame.data().len();
    }
    total
}

#[test]
fn reconfigure_with_struct_literal_changes_output_size() {
    let q0_bytes = total_bytes_with_quantizer(0);
    let q63_bytes = total_bytes_with_quantizer(63);

    // VP9 Realtime / error_resilient での 128x128 / 20 frames では実測比率が
    // 2 倍前後なので、単調性 (`>`) より強い 2 倍以上を要求する。
    // reconfigure が no-op に退化していたら同等になるので 2 倍で十分検出できる
    assert!(
        q0_bytes >= q63_bytes.saturating_mul(2),
        "q=0 ({q0_bytes}) が q=63 ({q63_bytes}) の 2 倍以上にならない",
    );
}
