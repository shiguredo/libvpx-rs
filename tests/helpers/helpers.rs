//! 統合テスト間で共有するヘルパー
//!
//! フレーム生成を提供する。

/// `width` / `height` の I420 グラデーションフレームを生成する
///
/// Y プレーンは水平方向のグラデーション、U / V プレーンは垂直方向の
/// グラデーションにする。4:2:0 のクロマサブサンプリング前提のため、
/// 偶数かつ 4 以上の解像度のみを対象とする。
pub(crate) fn generate_gradient_i420(width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
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
