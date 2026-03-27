use shiguredo_libvpx::{
    CodecConfig, Decoder, DecoderCodec, DecoderConfig, EncodeOptions, Encoder, EncoderConfig,
    ImageData, ImageFormat, Vp8Config, Vp9Config,
};

const WIDTH: usize = 64;
const HEIGHT: usize = 64;
// ラウンドトリップ PSNR の最低閾値 (dB)
const MIN_PSNR_DB: f64 = 25.0;

/// デコード結果を保持する構造体
struct DecodedI420 {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    width: usize,
    height: usize,
}

/// ストライドを考慮して 1 プレーンの MSE を計算する
fn plane_mse(
    original: &[u8],
    decoded: &[u8],
    width: usize,
    height: usize,
    decoded_stride: usize,
) -> f64 {
    let mut sum = 0.0_f64;
    for row in 0..height {
        let orig_start = row * width;
        let dec_start = row * decoded_stride;
        for col in 0..width {
            let diff = original[orig_start + col] as f64 - decoded[dec_start + col] as f64;
            sum += diff * diff;
        }
    }
    sum / (width * height) as f64
}

/// I420 フレーム全体の PSNR (dB) を計算する
///
/// 加重 MSE: (4 * MSE_Y + MSE_U + MSE_V) / 6
fn psnr_i420(orig_y: &[u8], orig_u: &[u8], orig_v: &[u8], decoded: &DecodedI420) -> f64 {
    let mse_y = plane_mse(
        orig_y,
        &decoded.y,
        decoded.width,
        decoded.height,
        decoded.y_stride,
    );
    let mse_u = plane_mse(
        orig_u,
        &decoded.u,
        decoded.width / 2,
        decoded.height / 2,
        decoded.u_stride,
    );
    let mse_v = plane_mse(
        orig_v,
        &decoded.v,
        decoded.width / 2,
        decoded.height / 2,
        decoded.v_stride,
    );

    let weighted_mse = (4.0 * mse_y + mse_u + mse_v) / 6.0;

    if weighted_mse == 0.0 {
        return f64::INFINITY;
    }

    10.0 * (255.0_f64 * 255.0 / weighted_mse).log10()
}

/// NV12 の UV インターリーブプレーンを U と V に分離する
fn deinterleave_nv12_uv(uv: &[u8], uv_width: usize, uv_height: usize) -> (Vec<u8>, Vec<u8>) {
    let size = uv_width * uv_height;
    let mut u = Vec::with_capacity(size);
    let mut v = Vec::with_capacity(size);
    for row in 0..uv_height {
        for col in 0..uv_width {
            let idx = row * uv_width * 2 + col * 2;
            u.push(uv[idx]);
            v.push(uv[idx + 1]);
        }
    }
    (u, v)
}

/// グラデーションパターンの I420 フレームを生成する
fn generate_gradient_i420(width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut y = vec![0u8; width * height];
    let mut u = vec![128u8; (width / 2) * (height / 2)];
    let mut v = vec![128u8; (width / 2) * (height / 2)];

    // Y: 水平グラデーション
    for row in 0..height {
        for col in 0..width {
            y[row * width + col] = ((col * 255) / width.saturating_sub(1)) as u8;
        }
    }

    // U/V: 垂直グラデーション
    let uv_w = width / 2;
    let uv_h = height / 2;
    for row in 0..uv_h {
        for col in 0..uv_w {
            u[row * uv_w + col] = ((row * 255) / uv_h.saturating_sub(1)) as u8;
            v[row * uv_w + col] = (255 - (row * 255) / uv_h.saturating_sub(1)) as u8;
        }
    }

    (y, u, v)
}

/// I420 フレームを NV12 形式に変換する
fn i420_to_nv12(u: &[u8], v: &[u8], uv_width: usize, uv_height: usize) -> Vec<u8> {
    let mut uv = Vec::with_capacity(uv_width * uv_height * 2);
    for row in 0..uv_height {
        for col in 0..uv_width {
            let idx = row * uv_width + col;
            uv.push(u[idx]);
            uv.push(v[idx]);
        }
    }
    uv
}

fn vp8_encoder_config(width: usize, height: usize, format: ImageFormat) -> EncoderConfig {
    let mut config = EncoderConfig::new(
        width,
        height,
        format,
        CodecConfig::Vp8(Vp8Config::default()),
    );
    config.target_bitrate = 1_000_000;
    config.min_quantizer = 1;
    config.max_quantizer = 1;
    config.cq_level = 1;
    config
}

fn vp9_encoder_config(width: usize, height: usize, format: ImageFormat) -> EncoderConfig {
    let mut config = EncoderConfig::new(
        width,
        height,
        format,
        CodecConfig::Vp9(Vp9Config::default()),
    );
    config.target_bitrate = 1_000_000;
    config.min_quantizer = 1;
    config.max_quantizer = 1;
    config.cq_level = 1;
    config
}

/// エンコード→デコードのラウンドトリップを実行し、エンコード済みデータを返す
fn encode_frame(config: EncoderConfig, image: &ImageData) -> Vec<Vec<u8>> {
    let mut encoder = Encoder::new(config).expect("failed to create encoder");
    let mut packets = Vec::new();

    encoder
        .encode(
            image,
            &EncodeOptions {
                force_keyframe: true,
            },
        )
        .expect("failed to encode");
    while let Some(frame) = encoder.next_frame() {
        packets.push(frame.data().to_vec());
    }

    encoder.finish().expect("failed to finish encoding");
    while let Some(frame) = encoder.next_frame() {
        packets.push(frame.data().to_vec());
    }

    packets
}

/// エンコード済みデータをデコードし、デコード結果を返す
fn decode_frames(codec: DecoderCodec, packets: &[Vec<u8>]) -> DecodedI420 {
    let config = DecoderConfig { codec };
    let mut decoder = Decoder::new(config).expect("failed to create decoder");

    let mut result = None;

    for packet in packets {
        decoder.decode(packet).expect("failed to decode");
        while let Some(frame) = decoder.next_frame() {
            assert!(result.is_none(), "expected exactly one decoded frame");
            result = Some(DecodedI420 {
                y: frame.y_plane().to_vec(),
                u: frame.u_plane().to_vec(),
                v: frame.v_plane().to_vec(),
                y_stride: frame.y_stride(),
                u_stride: frame.u_stride(),
                v_stride: frame.v_stride(),
                width: frame.width(),
                height: frame.height(),
            });
        }
    }

    decoder.finish().expect("failed to finish decoding");
    while let Some(frame) = decoder.next_frame() {
        assert!(result.is_none(), "expected exactly one decoded frame");
        result = Some(DecodedI420 {
            y: frame.y_plane().to_vec(),
            u: frame.u_plane().to_vec(),
            v: frame.v_plane().to_vec(),
            y_stride: frame.y_stride(),
            u_stride: frame.u_stride(),
            v_stride: frame.v_stride(),
            width: frame.width(),
            height: frame.height(),
        });
    }

    result.expect("no frame decoded")
}

#[test]
fn roundtrip_vp8_i420_psnr() {
    let (y, u, v) = generate_gradient_i420(WIDTH, HEIGHT);
    let image = ImageData::I420 {
        y: &y,
        u: &u,
        v: &v,
    };

    let config = vp8_encoder_config(WIDTH, HEIGHT, ImageFormat::I420);
    let packets = encode_frame(config, &image);
    let decoded = decode_frames(DecoderCodec::Vp8, &packets);

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);

    let psnr = psnr_i420(&y, &u, &v, &decoded);
    assert!(
        psnr >= MIN_PSNR_DB,
        "VP8 I420 PSNR {psnr:.2} dB is below threshold {MIN_PSNR_DB} dB"
    );
}

#[test]
fn roundtrip_vp9_i420_psnr() {
    let (y, u, v) = generate_gradient_i420(WIDTH, HEIGHT);
    let image = ImageData::I420 {
        y: &y,
        u: &u,
        v: &v,
    };

    let config = vp9_encoder_config(WIDTH, HEIGHT, ImageFormat::I420);
    let packets = encode_frame(config, &image);
    let decoded = decode_frames(DecoderCodec::Vp9, &packets);

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);

    let psnr = psnr_i420(&y, &u, &v, &decoded);
    assert!(
        psnr >= MIN_PSNR_DB,
        "VP9 I420 PSNR {psnr:.2} dB is below threshold {MIN_PSNR_DB} dB"
    );
}

#[test]
fn roundtrip_vp8_nv12_psnr() {
    let (y, u, v) = generate_gradient_i420(WIDTH, HEIGHT);
    let uv = i420_to_nv12(&u, &v, WIDTH / 2, HEIGHT / 2);
    let image = ImageData::Nv12 { y: &y, uv: &uv };

    let config = vp8_encoder_config(WIDTH, HEIGHT, ImageFormat::Nv12);
    let packets = encode_frame(config, &image);
    let decoded = decode_frames(DecoderCodec::Vp8, &packets);

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);

    // NV12 の UV をデインターリーブして I420 の U/V と比較する
    let (orig_u, orig_v) = deinterleave_nv12_uv(&uv, WIDTH / 2, HEIGHT / 2);
    let psnr = psnr_i420(&y, &orig_u, &orig_v, &decoded);
    assert!(
        psnr >= MIN_PSNR_DB,
        "VP8 NV12 PSNR {psnr:.2} dB is below threshold {MIN_PSNR_DB} dB"
    );
}

#[test]
fn roundtrip_vp9_nv12_psnr() {
    let (y, u, v) = generate_gradient_i420(WIDTH, HEIGHT);
    let uv = i420_to_nv12(&u, &v, WIDTH / 2, HEIGHT / 2);
    let image = ImageData::Nv12 { y: &y, uv: &uv };

    let config = vp9_encoder_config(WIDTH, HEIGHT, ImageFormat::Nv12);
    let packets = encode_frame(config, &image);
    let decoded = decode_frames(DecoderCodec::Vp9, &packets);

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);

    let (orig_u, orig_v) = deinterleave_nv12_uv(&uv, WIDTH / 2, HEIGHT / 2);
    let psnr = psnr_i420(&y, &orig_u, &orig_v, &decoded);
    assert!(
        psnr >= MIN_PSNR_DB,
        "VP9 NV12 PSNR {psnr:.2} dB is below threshold {MIN_PSNR_DB} dB"
    );
}
