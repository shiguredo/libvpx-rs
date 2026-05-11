//! [Hisui] 用の [libvpx] エンコーダーとデコーダー
//!
//! [Hisui]: https://github.com/shiguredo/hisui
//! [libvpx]: https://github.com/webmproject/libvpx
#![warn(missing_docs)]

use std::{
    ffi::{CStr, c_int, c_uint},
    mem::MaybeUninit,
    num::NonZeroUsize,
};

mod codec_info;
mod sys;

pub use codec_info::*;

/// ビルド時に参照したリポジトリ URL
pub const BUILD_REPOSITORY: &str = sys::BUILD_METADATA_REPOSITORY;

/// ビルド時に参照したリポジトリのバージョン（タグ）
pub const BUILD_VERSION: &str = sys::BUILD_METADATA_VERSION;

/// エラー
#[derive(Debug)]
pub struct Error {
    code: sys::vpx_codec_err_t,
    function: &'static str,
    reason: Option<&'static str>,
    detail: Option<String>,
}

impl Error {
    fn check(
        code: sys::vpx_codec_err_t,
        function: &'static str,
        ctx: Option<&sys::vpx_codec_ctx>,
    ) -> Result<(), Self> {
        if code == sys::vpx_codec_err_t_VPX_CODEC_OK {
            Ok(())
        } else {
            let detail = unsafe {
                if let Some(ctx) = ctx {
                    let detail_ptr = sys::vpx_codec_error_detail(ctx);
                    if detail_ptr.is_null() {
                        None
                    } else {
                        CStr::from_ptr(detail_ptr)
                            .to_str()
                            .ok()
                            .map(|s| s.to_owned())
                    }
                } else {
                    None
                }
            };
            Err(Self {
                code,
                function,
                reason: None,
                detail,
            })
        }
    }

    fn with_reason(
        code: sys::vpx_codec_err_t,
        function: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            code,
            function,
            reason: Some(reason),
            detail: None,
        }
    }

    fn reason(&self) -> Option<&str> {
        if self.reason.is_some() {
            return self.reason;
        }

        let reason = unsafe { sys::vpx_codec_err_to_string(self.code) };
        if reason.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(reason) }.to_str().ok()
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}() failed: code={}", self.function, self.code)?;
        if let Some(reason) = self.reason() {
            write!(f, ", reason={reason}")?;
        }
        if let Some(detail) = &self.detail {
            write!(f, ", detail={detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// デコーダー用コーデック種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderCodec {
    /// VP8
    Vp8,
    /// VP9
    Vp9,
}

/// デコーダーに指定する設定
#[derive(Debug, Clone)]
pub struct DecoderConfig {
    /// デコードするコーデック
    pub codec: DecoderCodec,
}

impl DecoderConfig {
    /// 指定したコーデックでデコーダー設定を生成する
    pub fn new(codec: DecoderCodec) -> Self {
        Self { codec }
    }
}

/// VP8 / VP9 デコーダー
pub struct Decoder {
    ctx: sys::vpx_codec_ctx,
    iter: sys::vpx_codec_iter_t,
}

impl Decoder {
    /// デコーダーインスタンスを生成する
    pub fn new(config: DecoderConfig) -> Result<Self, Error> {
        unsafe {
            let iface = match config.codec {
                DecoderCodec::Vp8 => sys::vpx_codec_vp8_dx(),
                DecoderCodec::Vp9 => sys::vpx_codec_vp9_dx(),
            };
            Self::init(iface)
        }
    }

    fn init(iface: *const sys::vpx_codec_iface) -> Result<Self, Error> {
        let mut ctx = MaybeUninit::<sys::vpx_codec_ctx>::zeroed();
        unsafe {
            let code = sys::vpx_codec_dec_init_ver(
                ctx.as_mut_ptr(),
                iface,
                std::ptr::null(), // cfg
                0,                // flags
                sys::VPX_DECODER_ABI_VERSION as i32,
            );
            let ctx = ctx.assume_init();
            Error::check(code, "vpx_codec_dec_init_ver", Some(&ctx))?;

            Ok(Self {
                ctx,
                iter: std::ptr::null(),
            })
        }
    }

    /// 圧縮された映像フレームをデコードする
    ///
    /// デコード結果は [`Decoder::next_frame()`] で取得できる
    pub fn decode(&mut self, data: &[u8]) -> Result<(), Error> {
        if !self.iter.is_null() {
            return Err(Error::with_reason(
                sys::vpx_codec_err_t_VPX_CODEC_ERROR,
                "shiguredo_libvpx::Decoder::decode",
                "still need to call shiguredo_libvpx::Decoder::next_frame()",
            ));
        }

        let code = unsafe {
            sys::vpx_codec_decode(
                &mut self.ctx,
                data.as_ptr(),
                data.len() as c_uint,
                std::ptr::null_mut(), // user_priv
                0, // deadline (ドキュメントによると、値は無視されるので常に 0 を指定しろとのこと）
            )
        };
        Error::check(code, "vpx_codec_decode", Some(&self.ctx))?;
        Ok(())
    }

    /// これ以上データが来ないことをデコーダーに伝える
    ///
    /// 残りのデコード結果は [`Decoder::next_frame()`] で取得できる
    pub fn finish(&mut self) -> Result<(), Error> {
        if !self.iter.is_null() {
            return Err(Error::with_reason(
                sys::vpx_codec_err_t_VPX_CODEC_ERROR,
                "shiguredo_libvpx::Decoder::finish",
                "still need to call shiguredo_libvpx::Decoder::next_frame()",
            ));
        }

        let code = unsafe {
            sys::vpx_codec_decode(
                &mut self.ctx,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
            )
        };
        Error::check(code, "vpx_codec_decode", Some(&self.ctx))?;
        Ok(())
    }

    /// デコード済みのフレームを取り出す
    ///
    /// [`Decoder::decode()`] や [`Decoder::finish()`] の後には、
    /// このメソッドを、結果が `None` になるまで呼び出し続ける必要がある
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame<'_>>, Error> {
        unsafe {
            let image = sys::vpx_codec_get_frame(&mut self.ctx, &mut self.iter);
            if image.is_null() {
                self.iter = std::ptr::null();
                return Ok(None);
            }
            let image = &*image;

            // デコーダーは I420 または 16-bit I420 のみ対応
            if !matches!(
                image.fmt,
                sys::vpx_img_fmt_VPX_IMG_FMT_I420 | sys::vpx_img_fmt_VPX_IMG_FMT_I42016
            ) {
                self.iter = std::ptr::null();
                return Err(Error::with_reason(
                    sys::vpx_codec_err_t_VPX_CODEC_UNSUP_FEATURE,
                    "vpx_codec_get_frame",
                    "unsupported image format",
                ));
            }

            Ok(Some(DecodedFrame(image)))
        }
    }
}

unsafe impl Send for Decoder {}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            sys::vpx_codec_destroy(&mut self.ctx);
        }
    }
}

impl std::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decoder").finish_non_exhaustive()
    }
}

/// デコードされた映像フレーム (I420 形式)
pub struct DecodedFrame<'a>(&'a sys::vpx_image);

impl DecodedFrame<'_> {
    /// フレームが高ビット深度（16ビット）かどうかを返す
    //
    // libvpx での高ビット深度フォーマットについてのメモ：
    // - libvpx は VP9 の 10-bit プロファイル（Profile 2 など）をサポート
    // - 高ビット深度データは 16-bit リトルエンディアン形式で格納される
    // - 実際の値範囲は 10-bit (0-1023) だが、上位6ビットは未使用
    // - YUV420 サブサンプリングは通常の 8-bit と同様に適用される
    // - ストライドは 16-bit 単位（バイト数は width * 2）で計算される
    pub fn is_high_depth(&self) -> bool {
        self.0.fmt == sys::vpx_img_fmt_VPX_IMG_FMT_I42016
    }

    /// フレームの Y 成分のデータを返す
    pub fn y_plane(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.0.planes[0], self.0.d_h as usize * self.y_stride())
        }
    }

    /// フレームの U 成分のデータを返す
    pub fn u_plane(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.0.planes[1],
                self.0.d_h.div_ceil(2) as usize * self.u_stride(),
            )
        }
    }

    /// フレームの V 成分のデータを返す
    pub fn v_plane(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.0.planes[2],
                self.0.d_h.div_ceil(2) as usize * self.v_stride(),
            )
        }
    }

    /// フレームの Y 成分のストライドを返す
    pub fn y_stride(&self) -> usize {
        self.0.stride[0] as usize
    }

    /// フレームの U 成分のストライドを返す
    pub fn u_stride(&self) -> usize {
        self.0.stride[1] as usize
    }

    /// フレームの V 成分のストライドを返す
    pub fn v_stride(&self) -> usize {
        self.0.stride[2] as usize
    }

    /// フレームの幅を返す
    pub fn width(&self) -> usize {
        self.0.d_w as usize
    }

    /// フレームの高さを返す
    pub fn height(&self) -> usize {
        self.0.d_h as usize
    }
}

/// エンコーダーの入力画像フォーマット
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// YUV 4:2:0 planar (3 プレーン: Y, U, V)
    I420,
    /// YUV 4:2:0 planar (3 プレーン: Y, V, U)
    Yv12,
    /// YUV 4:2:0 semi-planar (2 プレーン: Y, UV interleaved)
    Nv12,
    /// YUV 4:2:2 planar (3 プレーン: Y, U, V)
    I422,
    /// YUV 4:4:4 planar (3 プレーン: Y, U, V)
    I444,
    /// YUV 4:4:0 planar (3 プレーン: Y, U, V)
    I440,
    /// YUV 4:2:0 planar 16-bit (3 プレーン: Y, U, V)
    I42016,
    /// YUV 4:2:2 planar 16-bit (3 プレーン: Y, U, V)
    I42216,
    /// YUV 4:4:4 planar 16-bit (3 プレーン: Y, U, V)
    I44416,
    /// YUV 4:4:0 planar 16-bit (3 プレーン: Y, U, V)
    I44016,
}

/// エンコーダーに渡す画像データ
pub enum ImageData<'a> {
    /// I420 (3 プレーン: Y, U, V)
    I420 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// YV12 (3 プレーン: Y, V, U)
    Yv12 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// NV12 (2 プレーン: Y, UV interleaved)
    Nv12 {
        /// Y プレーン
        y: &'a [u8],
        /// UV interleaved プレーン
        uv: &'a [u8],
    },
    /// I422 (3 プレーン: Y, U, V)
    I422 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// I444 (3 プレーン: Y, U, V)
    I444 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// I440 (3 プレーン: Y, U, V)
    I440 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// I42016 (3 プレーン: Y, U, V / 16-bit)
    I42016 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// I42216 (3 プレーン: Y, U, V / 16-bit)
    I42216 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// I44416 (3 プレーン: Y, U, V / 16-bit)
    I44416 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
    /// I44016 (3 プレーン: Y, U, V / 16-bit)
    I44016 {
        /// Y プレーン
        y: &'a [u8],
        /// U プレーン
        u: &'a [u8],
        /// V プレーン
        v: &'a [u8],
    },
}

impl ImageData<'_> {
    /// この画像データに対応するフォーマットを返す
    fn format(&self) -> ImageFormat {
        match self {
            ImageData::I420 { .. } => ImageFormat::I420,
            ImageData::Yv12 { .. } => ImageFormat::Yv12,
            ImageData::Nv12 { .. } => ImageFormat::Nv12,
            ImageData::I422 { .. } => ImageFormat::I422,
            ImageData::I444 { .. } => ImageFormat::I444,
            ImageData::I440 { .. } => ImageFormat::I440,
            ImageData::I42016 { .. } => ImageFormat::I42016,
            ImageData::I42216 { .. } => ImageFormat::I42216,
            ImageData::I44416 { .. } => ImageFormat::I44416,
            ImageData::I44016 { .. } => ImageFormat::I44016,
        }
    }
}

/// 各プレーンの期待サイズ
enum PlaneSizes {
    /// 3 プレーン (I420, YV12, I422, I444, I440, I42016, I42216, I44416, I44016)
    ThreePlanes {
        y_size: usize,
        u_size: usize,
        v_size: usize,
    },
    /// 2 プレーン (NV12)
    TwoPlanes { y_size: usize, uv_size: usize },
}

/// エンコーダーに指定する設定
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    /// 入出力画像の幅
    pub width: usize,

    /// 入出力画像の高さ
    pub height: usize,

    /// 入力画像フォーマット
    pub image_format: ImageFormat,

    /// FPS の分子
    pub fps_numerator: usize,

    /// FPS の分母
    pub fps_denominator: usize,

    /// エンコードビットレート (bps 単位)
    pub target_bitrate: usize,

    /// libvpx に指定する品質調整用パラメーター
    pub min_quantizer: usize,

    /// libvpx に指定する品質調整用パラメーター
    pub max_quantizer: usize,

    /// libvpx に指定する品質調整用パラメーター
    pub cq_level: usize,

    /// エンコード速度設定 (VP8: 0-16, VP9: 0-9, 大きいほど高速)
    pub cpu_used: Option<usize>,

    /// エンコード期限設定
    pub deadline: EncodingDeadline,

    /// レート制御モード
    pub rate_control: RateControlMode,

    /// 先読みフレーム数 (None で無効、品質 vs 速度のトレードオフ)
    pub lag_in_frames: Option<NonZeroUsize>,

    /// スレッド数 (None で自動設定)
    pub threads: Option<NonZeroUsize>,

    /// エラー耐性モード (リアルタイム用途で有効)
    pub error_resilient: bool,

    /// キーフレーム間隔 (フレーム数)
    pub keyframe_interval: Option<NonZeroUsize>,

    // TODO(sile): 今は encode() がタイムスタンプの情報を受け取らないので、フレームドロップとは相性が悪い
    /// フレームドロップ閾値 (0-100, リアルタイム用途)
    pub frame_drop_threshold: Option<usize>,

    /// コーデック固有設定
    pub codec: CodecConfig,
}

impl EncoderConfig {
    /// 必須パラメータを指定してエンコーダー設定を生成する
    ///
    /// オプションパラメータは以下の値で初期化される:
    /// - `fps_numerator`: 30
    /// - `fps_denominator`: 1
    /// - `target_bitrate`: 2_000_000
    /// - `min_quantizer`: 0
    /// - `max_quantizer`: 63
    /// - `cq_level`: 10
    /// - `cpu_used`: None
    /// - `deadline`: Good
    /// - `rate_control`: Vbr
    /// - `lag_in_frames`: None
    /// - `threads`: None
    /// - `error_resilient`: false
    /// - `keyframe_interval`: None
    /// - `frame_drop_threshold`: None
    pub fn new(width: usize, height: usize, image_format: ImageFormat, codec: CodecConfig) -> Self {
        Self {
            width,
            height,
            image_format,
            fps_numerator: 30,
            fps_denominator: 1,
            target_bitrate: 2_000_000,
            min_quantizer: 0,
            max_quantizer: 63,
            cq_level: 10,
            cpu_used: None,
            deadline: EncodingDeadline::Good,
            rate_control: RateControlMode::Vbr,
            lag_in_frames: None,
            threads: None,
            error_resilient: false,
            keyframe_interval: None,
            frame_drop_threshold: None,
            codec,
        }
    }
}

/// エンコード期限設定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingDeadline {
    /// 最高品質 (最も時間がかかる)
    Best,
    /// 良い品質 (品質と速度のバランス)
    Good,
    /// リアルタイム (最も高速)
    Realtime,
}

/// レート制御モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControlMode {
    /// Variable Bitrate (可変ビットレート)
    Vbr,
    /// Constant Bitrate (固定ビットレート)
    Cbr,
    /// Constant Quality (固定品質)
    Cq,
}

/// VP9 プロファイル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vp9Profile {
    /// Profile 0 (8-bit 4:2:0)
    #[default]
    Profile0,
    /// Profile 2 (10/12-bit 4:2:0)
    Profile2,
}

/// VP9 固有の設定
#[derive(Debug, Clone, Default)]
pub struct Vp9Config {
    /// プロファイル
    pub profile: Vp9Profile,

    /// 適応的量子化モード (0-3)
    pub aq_mode: Option<i32>,

    /// デノイザー設定 (0-3)
    pub noise_sensitivity: Option<i32>,

    /// タイル列数 (並列処理用)
    pub tile_columns: Option<i32>,

    /// タイル行数 (並列処理用)
    pub tile_rows: Option<i32>,

    /// 行マルチスレッド有効
    pub row_mt: bool,

    /// フレーム並列デコード有効
    pub frame_parallel_decoding: bool,

    /// コンテンツタイプ最適化
    pub tune_content: Option<ContentType>,
}

/// VP8 固有の設定
#[derive(Debug, Clone, Default)]
pub struct Vp8Config {
    /// デノイザー設定 (0-3)
    pub noise_sensitivity: Option<i32>,

    /// 静的閾値
    pub static_threshold: Option<i32>,

    /// トークンパーティション数
    pub token_partitions: Option<i32>,

    /// 最大イントラビットレート率
    pub max_intra_bitrate_pct: Option<i32>,

    /// ARNRフィルタ設定
    pub arnr_config: Option<ArnrConfig>,
}

/// コンテンツタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// 通常の映像
    Default,
    /// スクリーン録画
    Screen,
}

/// エンコーダー用コーデック設定
#[derive(Debug, Clone)]
pub enum CodecConfig {
    /// VP8 コーデック設定
    Vp8(Vp8Config),
    /// VP9 コーデック設定
    Vp9(Vp9Config),
}

/// ARNR フィルタ設定
#[derive(Debug, Clone)]
pub struct ArnrConfig {
    /// 最大フレーム数
    pub max_frames: i32,
    /// 強度
    pub strength: i32,
    /// タイプ
    pub filter_type: i32,
}

/// エンコード時のオプション
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// キーフレームを強制する
    pub force_keyframe: bool,
}

/// エンコーダー再設定パラメータ
///
/// [`Encoder::reconfigure`] で動的に変更可能なパラメータ。本構造体に含まれる
/// フィールドのみが変更対象で、それ以外 (`width` / `height` / `image_format` /
/// `vpx_codec_control_` 経由のすべての設定) は再設定不可。`None` の項目は直前の値を
/// 維持する。
///
/// `fps_numerator` と `fps_denominator` は両方同時に指定するか、両方とも `None` に
/// する必要がある。
///
/// FPS を変更すると [`Encoder::encode`] が libvpx に渡すタイムベース (`g_timebase`)
/// が変わる。一方で本ラッパは PTS にフレーム番号 (`Encoder::frame_count`) を渡して
/// おり、FPS 変更直後の最初の `encode` で libvpx が PTS の単調性違反を検出して
/// エラーを返す可能性がある。現状の API では FPS 変更後の PTS 単調性を担保できない
/// ため、FPS の動的変更は推奨しない。どうしても変更が必要な場合は、`force_keyframe`
/// を立てて境界を明示する運用にする。
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ReconfigureParams {
    /// エンコードビットレート (bps)
    ///
    /// 1000 bps 以上 1_000_000_000 bps 以下を要求する。1000 未満は libvpx の kbps
    /// 解像度で表現できないため拒否、上限は libvpx の silent clip を回避するため
    /// 事前拒否する。
    pub target_bitrate: Option<usize>,

    /// FPS の分子
    pub fps_numerator: Option<usize>,

    /// FPS の分母
    pub fps_denominator: Option<usize>,

    /// 最小量子化値
    pub min_quantizer: Option<usize>,

    /// 最大量子化値
    pub max_quantizer: Option<usize>,

    /// キーフレーム最大間隔 (フレーム数、libvpx の `kf_max_dist` に対応)
    pub keyframe_interval: Option<NonZeroUsize>,
}

impl ReconfigureParams {
    /// `EncoderConfig` から `ReconfigureParams` を構築する内部コンバータ
    ///
    /// `Encoder::new` と `Encoder::reconfigure` の検査ロジックを
    /// `merge_reconfigure_params_into_cfg` に集約するためにのみ使う
    fn from_encoder_config(config: &EncoderConfig) -> Self {
        Self {
            target_bitrate: Some(config.target_bitrate),
            fps_numerator: Some(config.fps_numerator),
            fps_denominator: Some(config.fps_denominator),
            min_quantizer: Some(config.min_quantizer),
            max_quantizer: Some(config.max_quantizer),
            keyframe_interval: config.keyframe_interval,
        }
    }

    /// 全フィールドが `None` か（= 何も変更しない指示か）を返す
    fn is_empty(&self) -> bool {
        self.target_bitrate.is_none()
            && self.fps_numerator.is_none()
            && self.fps_denominator.is_none()
            && self.min_quantizer.is_none()
            && self.max_quantizer.is_none()
            && self.keyframe_interval.is_none()
    }
}

/// VP8 / VP9 エンコーダー
pub struct Encoder {
    ctx: sys::vpx_codec_ctx,
    // 1-pass 専用前提でシャローコピー可能とみなしている (2-pass バッファは常に NULL)
    // 2-pass 対応を追加する際はシャローコピー前提が崩れるため、ここを再設計する必要がある
    cfg: sys::vpx_codec_enc_cfg,
    img: sys::vpx_image,
    iter: sys::vpx_codec_iter_t,
    frame_count: usize,
    deadline: EncodingDeadline,
    image_format: ImageFormat,
    plane_sizes: PlaneSizes,
}

/// `INVALID_PARAM` のエラーを生成する内部ヘルパー
fn invalid_param(function: &'static str, reason: &'static str) -> Error {
    Error::with_reason(
        sys::vpx_codec_err_t_VPX_CODEC_INVALID_PARAM,
        function,
        reason,
    )
}

/// libvpx の `g_timebase.num` / `g_timebase.den` の上限値。
///
/// 出典: libvpx `vp9/vp9_cx_iface.c` および `vp8/vp8_cx_iface.c` の
/// `validate_config()`。`RANGE_CHECK(cfg, g_timebase.{num,den}, 1, 1000000000)`。
/// libvpx の更新で変わる可能性がある。
const VPX_MAX_TIMEBASE: usize = 1_000_000_000;

/// libvpx が許容する `rc_max_quantizer` の上限値 (= 63)。
///
/// 出典: libvpx `vp9/vp9_cx_iface.c` の `validate_config()`。
/// `RANGE_CHECK_HI(cfg, rc_max_quantizer, 63)`。VP8 は明示的な上限を持たないが、
/// 量子化ステップの定義上 0-63 を要求する。libvpx の更新で変わる可能性がある。
const VPX_MAX_QUANTIZER: c_uint = 63;

/// libvpx が許容する `rc_target_bitrate` の上限値 (kbps)。
///
/// 出典: libvpx `vp9/vp9_cx_iface.c` の `set_encoder_config()` と
/// `vp8/vp8_cx_iface.c` の `set_vp8_encoder_config()`。
/// `VPXMIN(..., 1000000)` で silent clip される。libvpx の更新で変わる可能性がある。
const VPX_MAX_TARGET_BITRATE_KBPS: c_uint = 1_000_000;

/// `VPX_MAX_TARGET_BITRATE_KBPS` を bps 単位で表したもの (= 1_000_000_000)。
///
/// 本ラッパは `target_bitrate` を bps で受け取り `/ 1000` で kbps に変換するが、
/// 単純な整数除算では `1_000_000_999` bps のような値が `1_000_000` kbps に
/// silent truncate されてしまう。それを防ぐため bps 段階で上限を弾く。
const VPX_MAX_TARGET_BITRATE_BPS: usize = VPX_MAX_TARGET_BITRATE_KBPS as usize * 1000;

/// `ReconfigureParams` の各フィールドを検査して `vpx_codec_enc_cfg` にマージする
///
/// `Encoder::new` と `Encoder::reconfigure` 双方から呼ばれる共通ロジック。
/// `function` はエラー時に `Error::function` として埋め込まれる呼び出し元名。
///
/// 検査は libvpx 内部の silent clip を完全に潰す方向に倒している。許容範囲外は
/// すべて `VPX_CODEC_INVALID_PARAM` で拒否する。
///
/// 注意: `min_quantizer` と `max_quantizer` の関係検査は、`params` の値だけでなく
/// 現在の `cfg` に既に格納された値も含めて行われる。例えば `min_quantizer` のみを
/// `Some` で渡した場合、検査対象は「新しい min」と「現在の `cfg.rc_max_quantizer`」
/// の組になる。
fn merge_reconfigure_params_into_cfg(
    cfg: &mut sys::vpx_codec_enc_cfg,
    params: &ReconfigureParams,
    function: &'static str,
) -> Result<(), Error> {
    if let Some(target_bitrate) = params.target_bitrate {
        // libvpx の `rc_target_bitrate` は kbps 単位。bps からの変換時に整数除算で
        // silent truncation されるのを防ぐため、kbps 上限値 * 1000 を超える bps を
        // 最初に弾く
        if target_bitrate > VPX_MAX_TARGET_BITRATE_BPS {
            return Err(invalid_param(
                function,
                "target_bitrate exceeds libvpx maximum (1_000_000_000 bps)",
            ));
        }
        let kbps = target_bitrate / 1000;
        if kbps == 0 {
            return Err(invalid_param(
                function,
                "target_bitrate must be at least 1000 bps (libvpx kbps resolution)",
            ));
        }
        // 上の上限検査で kbps <= VPX_MAX_TARGET_BITRATE_KBPS (1_000_000) が保証されるため c_uint に必ず収まる
        cfg.rc_target_bitrate = kbps as c_uint;
    }

    match (params.fps_numerator, params.fps_denominator) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(invalid_param(
                function,
                "fps_numerator and fps_denominator must be set together",
            ));
        }
        (Some(num), Some(den)) => {
            if num == 0 {
                return Err(invalid_param(function, "fps_numerator must be non-zero"));
            }
            if den == 0 {
                return Err(invalid_param(function, "fps_denominator must be non-zero"));
            }
            if num > VPX_MAX_TIMEBASE {
                return Err(invalid_param(
                    function,
                    "fps_numerator exceeds libvpx maximum (1_000_000_000)",
                ));
            }
            if den > VPX_MAX_TIMEBASE {
                return Err(invalid_param(
                    function,
                    "fps_denominator exceeds libvpx maximum (1_000_000_000)",
                ));
            }
            // libvpx の g_timebase は 1 フレームの提示時間 (秒) の分数表現
            // (libvpx `vpx/vpx_encoder.h` の `vpx_codec_enc_cfg::g_timebase` の項参照)
            // 上の上限検査で num, den <= VPX_MAX_TIMEBASE (1_000_000_000) が保証されるため c_int に必ず収まる
            cfg.g_timebase.num = den as c_int;
            cfg.g_timebase.den = num as c_int;
        }
        (None, None) => {}
    }

    if let Some(min_quantizer) = params.min_quantizer {
        cfg.rc_min_quantizer = c_uint::try_from(min_quantizer)
            .map_err(|_| invalid_param(function, "min_quantizer is out of range"))?;
    }

    if let Some(max_quantizer) = params.max_quantizer {
        cfg.rc_max_quantizer = c_uint::try_from(max_quantizer)
            .map_err(|_| invalid_param(function, "max_quantizer is out of range"))?;
    }

    if cfg.rc_max_quantizer > VPX_MAX_QUANTIZER {
        return Err(invalid_param(function, "max_quantizer must not exceed 63"));
    }

    if cfg.rc_min_quantizer > cfg.rc_max_quantizer {
        return Err(invalid_param(
            function,
            "min_quantizer must not exceed max_quantizer",
        ));
    }

    if let Some(kf_interval) = params.keyframe_interval {
        cfg.kf_max_dist = c_uint::try_from(kf_interval.get())
            .map_err(|_| invalid_param(function, "keyframe_interval is out of range"))?;
    }

    Ok(())
}

impl Encoder {
    /// エンコーダーインスタンスを生成する
    pub fn new(config: EncoderConfig) -> Result<Self, Error> {
        let mut cfg = MaybeUninit::<sys::vpx_codec_enc_cfg>::zeroed();
        unsafe {
            let iface = match &config.codec {
                CodecConfig::Vp8(_) => sys::vpx_codec_vp8_cx(),
                CodecConfig::Vp9(_) => sys::vpx_codec_vp9_cx(),
            };
            let usage = 0; // ドキュメントでは、常に 0 を指定しろ、とのこと
            let code = sys::vpx_codec_enc_config_default(iface, cfg.as_mut_ptr(), usage);
            Error::check(code, "vpx_codec_enc_config_default", None)?;

            let cfg = cfg.assume_init();
            Self::init(&config, cfg, iface)
        }
    }

    fn init(
        encoder_config: &EncoderConfig,
        mut vpx_config: sys::vpx_codec_enc_cfg,
        iface: *const sys::vpx_codec_iface,
    ) -> Result<Self, Error> {
        const FUNCTION: &str = "shiguredo_libvpx::Encoder::new";

        vpx_config.g_w = c_uint::try_from(encoder_config.width)
            .map_err(|_| invalid_param(FUNCTION, "width is out of range"))?;
        vpx_config.g_h = c_uint::try_from(encoder_config.height)
            .map_err(|_| invalid_param(FUNCTION, "height is out of range"))?;

        // プロファイル設定
        if let CodecConfig::Vp9(vp9_config) = &encoder_config.codec {
            vpx_config.g_profile = match vp9_config.profile {
                Vp9Profile::Profile0 => 0,
                Vp9Profile::Profile2 => 2,
            };
        }

        // ビットレート / FPS / 量子化レンジ / キーフレーム間隔は reconfigure と同じ検査で適用する
        merge_reconfigure_params_into_cfg(
            &mut vpx_config,
            &ReconfigureParams::from_encoder_config(encoder_config),
            FUNCTION,
        )?;

        // cq_level は VP9E_SET_CQ_LEVEL / VP8E_SET_CQEP に渡される。
        // 出典: libvpx `vp9/vp9_cx_iface.c` の `ctrl_set_cq_level()`。有効レンジは 0-63。
        if encoder_config.cq_level > VPX_MAX_QUANTIZER as usize {
            return Err(invalid_param(FUNCTION, "cq_level must not exceed 63"));
        }
        // 上の上限検査で cq_level <= 63 が保証されるため c_uint に必ず収まる
        let cq_level = encoder_config.cq_level as c_uint;

        if let Some(lag) = encoder_config.lag_in_frames {
            vpx_config.g_lag_in_frames = c_uint::try_from(lag.get())
                .map_err(|_| invalid_param(FUNCTION, "lag_in_frames is out of range"))?;
        }

        if let Some(threads) = encoder_config.threads {
            vpx_config.g_threads = c_uint::try_from(threads.get())
                .map_err(|_| invalid_param(FUNCTION, "threads is out of range"))?;
        }

        if encoder_config.error_resilient {
            vpx_config.g_error_resilient = 1;
        }

        if let Some(threshold) = encoder_config.frame_drop_threshold {
            vpx_config.rc_dropframe_thresh = c_uint::try_from(threshold)
                .map_err(|_| invalid_param(FUNCTION, "frame_drop_threshold is out of range"))?;
        }

        // レート制御モード設定
        vpx_config.rc_end_usage = match encoder_config.rate_control {
            RateControlMode::Vbr => sys::vpx_rc_mode_VPX_VBR,
            RateControlMode::Cbr => sys::vpx_rc_mode_VPX_CBR,
            RateControlMode::Cq => sys::vpx_rc_mode_VPX_CQ,
        };

        let mut ctx = MaybeUninit::<sys::vpx_codec_ctx>::zeroed();
        unsafe {
            let code = sys::vpx_codec_enc_init_ver(
                ctx.as_mut_ptr(),
                iface,
                &vpx_config,
                0, // flags
                sys::VPX_ENCODER_ABI_VERSION as i32,
            );
            Error::check(code, "vpx_codec_enc_init_ver", None)?;

            let img_fmt = match encoder_config.image_format {
                ImageFormat::I420 => sys::vpx_img_fmt_VPX_IMG_FMT_I420,
                ImageFormat::Yv12 => sys::vpx_img_fmt_VPX_IMG_FMT_YV12,
                ImageFormat::Nv12 => sys::vpx_img_fmt_VPX_IMG_FMT_NV12,
                ImageFormat::I422 => sys::vpx_img_fmt_VPX_IMG_FMT_I422,
                ImageFormat::I444 => sys::vpx_img_fmt_VPX_IMG_FMT_I444,
                ImageFormat::I440 => sys::vpx_img_fmt_VPX_IMG_FMT_I440,
                ImageFormat::I42016 => sys::vpx_img_fmt_VPX_IMG_FMT_I42016,
                ImageFormat::I42216 => sys::vpx_img_fmt_VPX_IMG_FMT_I42216,
                ImageFormat::I44416 => sys::vpx_img_fmt_VPX_IMG_FMT_I44416,
                ImageFormat::I44016 => sys::vpx_img_fmt_VPX_IMG_FMT_I44016,
            };

            let mut img = MaybeUninit::zeroed();
            let result = sys::vpx_img_alloc(
                img.as_mut_ptr(),
                img_fmt,
                vpx_config.g_w,
                vpx_config.g_h,
                1, // align に 1 を指定することで width == y_stride となることが保証される
            );
            if result.is_null() {
                // vpx_img_alloc が失敗した場合、初期化済みの codec context を手動で解放する
                sys::vpx_codec_destroy(ctx.as_mut_ptr());
                return Err(Error::with_reason(
                    sys::vpx_codec_err_t_VPX_CODEC_MEM_ERROR,
                    "vpx_img_alloc",
                    "image allocation failed",
                ));
            }

            let img = img.assume_init();
            let height = encoder_config.height;
            let plane_sizes = match encoder_config.image_format {
                ImageFormat::Nv12 => PlaneSizes::TwoPlanes {
                    y_size: height * img.stride[0] as usize,
                    uv_size: height.div_ceil(2) * img.stride[1] as usize,
                },
                // 4:2:0 系 (U/V は幅・高さともに半分)
                ImageFormat::I420 | ImageFormat::Yv12 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height.div_ceil(2) * img.stride[1] as usize,
                    v_size: height.div_ceil(2) * img.stride[2] as usize,
                },
                // 4:2:2 系 (U/V は幅が半分、高さは同じ)
                ImageFormat::I422 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height * img.stride[1] as usize,
                    v_size: height * img.stride[2] as usize,
                },
                // 4:4:4 系 (U/V は Y と同サイズ)
                ImageFormat::I444 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height * img.stride[1] as usize,
                    v_size: height * img.stride[2] as usize,
                },
                // 4:4:0 系 (U/V は幅は同じ、高さが半分)
                ImageFormat::I440 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height.div_ceil(2) * img.stride[1] as usize,
                    v_size: height.div_ceil(2) * img.stride[2] as usize,
                },
                // 16-bit 4:2:0 系
                ImageFormat::I42016 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height.div_ceil(2) * img.stride[1] as usize,
                    v_size: height.div_ceil(2) * img.stride[2] as usize,
                },
                // 16-bit 4:2:2 系
                ImageFormat::I42216 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height * img.stride[1] as usize,
                    v_size: height * img.stride[2] as usize,
                },
                // 16-bit 4:4:4 系
                ImageFormat::I44416 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height * img.stride[1] as usize,
                    v_size: height * img.stride[2] as usize,
                },
                // 16-bit 4:4:0 系
                ImageFormat::I44016 => PlaneSizes::ThreePlanes {
                    y_size: height * img.stride[0] as usize,
                    u_size: height.div_ceil(2) * img.stride[1] as usize,
                    v_size: height.div_ceil(2) * img.stride[2] as usize,
                },
            };

            let mut this = Self {
                ctx: ctx.assume_init(),
                cfg: vpx_config,
                img,
                iter: std::ptr::null(),
                frame_count: 0,
                deadline: encoder_config.deadline,
                image_format: encoder_config.image_format,
                plane_sizes,
            };
            // NOTE: これ以降の操作に失敗しても ctx は Drop によって確実に解放される

            // CQ Level設定 (有効レンジ 0-63 は前段で検査済み)
            let code = sys::vpx_codec_control_(
                &mut this.ctx,
                sys::vp8e_enc_control_id_VP8E_SET_CQ_LEVEL as c_int,
                cq_level,
            );
            Error::check(code, "vpx_codec_control_", Some(&this.ctx))?;

            // CPU使用率設定
            if let Some(cpu_used) = encoder_config.cpu_used {
                let code = sys::vpx_codec_control_(
                    &mut this.ctx,
                    sys::vp8e_enc_control_id_VP8E_SET_CPUUSED as c_int,
                    cpu_used,
                );
                Error::check(code, "vpx_codec_control_", Some(&this.ctx))?;
            }

            // コーデック固有設定
            match &encoder_config.codec {
                CodecConfig::Vp8(vp8_config) => this.configure_vp8(vp8_config)?,
                CodecConfig::Vp9(vp9_config) => this.configure_vp9(vp9_config)?,
            }

            Ok(this)
        }
    }

    fn configure_vp9(&mut self, vp9_config: &Vp9Config) -> Result<(), Error> {
        // 適応的量子化モード
        if let Some(aq_mode) = vp9_config.aq_mode {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_AQ_MODE as c_int,
                    aq_mode,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // デノイザー設定
        if let Some(noise_sensitivity) = vp9_config.noise_sensitivity {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_NOISE_SENSITIVITY as c_int,
                    noise_sensitivity,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // タイル列数
        if let Some(tile_columns) = vp9_config.tile_columns {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_TILE_COLUMNS as c_int,
                    tile_columns,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // タイル行数
        if let Some(tile_rows) = vp9_config.tile_rows {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_TILE_ROWS as c_int,
                    tile_rows,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // 行マルチスレッド
        if vp9_config.row_mt {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_ROW_MT as c_int,
                    1,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // フレーム並列デコード
        if vp9_config.frame_parallel_decoding {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_FRAME_PARALLEL_DECODING as c_int,
                    1,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // コンテンツタイプ最適化
        if let Some(tune_content) = vp9_config.tune_content {
            let content_type = match tune_content {
                ContentType::Default => sys::vp9e_tune_content_VP9E_CONTENT_DEFAULT,
                ContentType::Screen => sys::vp9e_tune_content_VP9E_CONTENT_SCREEN,
            };
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP9E_SET_TUNE_CONTENT as c_int,
                    content_type as c_int,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        Ok(())
    }

    fn configure_vp8(&mut self, vp8_config: &Vp8Config) -> Result<(), Error> {
        // デノイザー設定
        if let Some(noise_sensitivity) = vp8_config.noise_sensitivity {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP8E_SET_NOISE_SENSITIVITY as c_int,
                    noise_sensitivity,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // 静的閾値
        if let Some(static_threshold) = vp8_config.static_threshold {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP8E_SET_STATIC_THRESHOLD as c_int,
                    static_threshold,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // トークンパーティション数
        if let Some(token_partitions) = vp8_config.token_partitions {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP8E_SET_TOKEN_PARTITIONS as c_int,
                    token_partitions,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // 最大イントラビットレート率
        if let Some(max_intra_bitrate_pct) = vp8_config.max_intra_bitrate_pct {
            let code = unsafe {
                sys::vpx_codec_control_(
                    &mut self.ctx,
                    sys::vp8e_enc_control_id_VP8E_SET_MAX_INTRA_BITRATE_PCT as c_int,
                    max_intra_bitrate_pct,
                )
            };
            Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;
        }

        // ARNRフィルタ設定
        if let Some(arnr_config) = &vp8_config.arnr_config {
            self.configure_vp8_arnr(arnr_config)?;
        }

        Ok(())
    }

    fn configure_vp8_arnr(&mut self, arnr_config: &ArnrConfig) -> Result<(), Error> {
        // ARNRを有効化
        let code = unsafe {
            sys::vpx_codec_control_(
                &mut self.ctx,
                sys::vp8e_enc_control_id_VP8E_SET_ENABLEAUTOALTREF as c_int,
                1,
            )
        };
        Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;

        // ARNR最大フレーム数
        let code = unsafe {
            sys::vpx_codec_control_(
                &mut self.ctx,
                sys::vp8e_enc_control_id_VP8E_SET_ARNR_MAXFRAMES as c_int,
                arnr_config.max_frames,
            )
        };
        Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;

        // ARNR強度
        let code = unsafe {
            sys::vpx_codec_control_(
                &mut self.ctx,
                sys::vp8e_enc_control_id_VP8E_SET_ARNR_STRENGTH as c_int,
                arnr_config.strength,
            )
        };
        Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;

        // ARNRタイプ
        let code = unsafe {
            sys::vpx_codec_control_(
                &mut self.ctx,
                sys::vp8e_enc_control_id_VP8E_SET_ARNR_TYPE as c_int,
                arnr_config.filter_type,
            )
        };
        Error::check(code, "vpx_codec_control_", Some(&self.ctx))?;

        Ok(())
    }

    /// `next_frame()` の戻り値が `None` になっていない状態（= まだ取り出していない
    /// エンコード済みパケットが残っている可能性）でなければ `Ok(())` を返す内部ヘルパー
    fn ensure_iter_drained(&self, function: &'static str) -> Result<(), Error> {
        if !self.iter.is_null() {
            return Err(Error::with_reason(
                sys::vpx_codec_err_t_VPX_CODEC_ERROR,
                function,
                "still need to call shiguredo_libvpx::Encoder::next_frame()",
            ));
        }
        Ok(())
    }

    /// 画像データをエンコードする
    ///
    /// エンコード結果は [`Encoder::next_frame()`] で取得できる
    ///
    /// `image` のフォーマットはエンコーダー初期化時に指定した `ImageFormat` と一致する必要がある
    pub fn encode(&mut self, image: &ImageData<'_>, options: &EncodeOptions) -> Result<(), Error> {
        self.ensure_iter_drained("shiguredo_libvpx::Encoder::encode")?;

        // フォーマット整合性チェック
        if image.format() != self.image_format {
            return Err(invalid_param(
                "shiguredo_libvpx::Encoder::encode",
                "image format mismatch",
            ));
        }

        // プレーンサイズ検証
        match (image, &self.plane_sizes) {
            (
                ImageData::I420 { y, u, v }
                | ImageData::Yv12 { y, u, v }
                | ImageData::I422 { y, u, v }
                | ImageData::I444 { y, u, v }
                | ImageData::I440 { y, u, v }
                | ImageData::I42016 { y, u, v }
                | ImageData::I42216 { y, u, v }
                | ImageData::I44416 { y, u, v }
                | ImageData::I44016 { y, u, v },
                PlaneSizes::ThreePlanes {
                    y_size,
                    u_size,
                    v_size,
                },
            ) => {
                if y.len() != *y_size || u.len() != *u_size || v.len() != *v_size {
                    return Err(invalid_param(
                        "shiguredo_libvpx::Encoder::encode",
                        "invalid plane sizes",
                    ));
                }
            }
            (ImageData::Nv12 { y, uv }, PlaneSizes::TwoPlanes { y_size, uv_size }) => {
                if y.len() != *y_size || uv.len() != *uv_size {
                    return Err(invalid_param(
                        "shiguredo_libvpx::Encoder::encode",
                        "invalid plane sizes",
                    ));
                }
            }
            _ => unreachable!(),
        }

        // deadline 設定を適用
        let deadline = match self.deadline {
            EncodingDeadline::Best => sys::VPX_DL_BEST_QUALITY,
            EncodingDeadline::Good => sys::VPX_DL_GOOD_QUALITY,
            EncodingDeadline::Realtime => sys::VPX_DL_REALTIME,
        };

        // フラグ設定
        let mut flags: sys::vpx_enc_frame_flags_t = 0;
        if options.force_keyframe {
            flags |= sys::VPX_EFLAG_FORCE_KF as sys::vpx_enc_frame_flags_t;
        }

        let code = unsafe {
            // 画像データをバッファにコピー
            match image {
                ImageData::I420 { y, u, v }
                | ImageData::Yv12 { y, u, v }
                | ImageData::I422 { y, u, v }
                | ImageData::I444 { y, u, v }
                | ImageData::I440 { y, u, v }
                | ImageData::I42016 { y, u, v }
                | ImageData::I42216 { y, u, v }
                | ImageData::I44416 { y, u, v }
                | ImageData::I44016 { y, u, v } => {
                    std::slice::from_raw_parts_mut(self.img.planes[0], y.len()).copy_from_slice(y);
                    std::slice::from_raw_parts_mut(self.img.planes[1], u.len()).copy_from_slice(u);
                    std::slice::from_raw_parts_mut(self.img.planes[2], v.len()).copy_from_slice(v);
                }
                ImageData::Nv12 { y, uv } => {
                    std::slice::from_raw_parts_mut(self.img.planes[0], y.len()).copy_from_slice(y);
                    std::slice::from_raw_parts_mut(self.img.planes[1], uv.len())
                        .copy_from_slice(uv);
                }
            }

            // エンコード実行
            sys::vpx_codec_encode(
                &mut self.ctx,
                &self.img,
                self.frame_count as sys::vpx_codec_pts_t,
                1, // duration: 1 は「1 フレーム分」を意味する
                flags,
                deadline as sys::vpx_enc_deadline_t,
            )
        };
        Error::check(code, "vpx_codec_encode", Some(&self.ctx))?;
        self.frame_count += 1;
        Ok(())
    }

    /// エンコーダーのパラメータを動的に変更する
    ///
    /// ビットレート・FPS・量子化レンジ・キーフレーム間隔をエンコード中に変更する。
    /// `params` の `None` のフィールドは直前の値を維持する。すべてのフィールドが
    /// `None` の場合は何もせず `Ok(())` を返す (libvpx の `vpx_codec_enc_config_set`
    /// を呼ばない)。変更可能な項目と制約は [`ReconfigureParams`] を参照。
    ///
    /// 本ラッパは libvpx の silent clip を起こさない範囲のみを受け付ける。許容範囲外
    /// の値はすべて `VPX_CODEC_INVALID_PARAM` で拒否する。
    ///
    /// [`Encoder::next_frame`] のイテレーション途中（= libvpx 内部の cx data を
    /// 一部だけ消費した状態）で呼び出すとエラーを返す。`encode` 直後でまだ
    /// `next_frame` を呼んでいない状態は許容するが、`EncoderConfig::lag_in_frames`
    /// を指定したエンコーダーでは libvpx 内部に未取り出しのパケットが蓄積している
    /// 可能性があるため、`reconfigure` の前に `next_frame()` が `None` を返すまで
    /// ドレインすることを推奨する。
    ///
    /// `min_quantizer` と `max_quantizer` の関係検査は、本呼び出しで渡した値だけ
    /// でなく現在の値と組み合わせて行われる。例えば現在 `max_quantizer = 10` の
    /// エンコーダーに `min_quantizer = Some(20)` のみを渡すと `min > max` で
    /// 拒否される。両者を同時に変更したい場合は両方を `Some` で渡すこと。
    ///
    /// 失敗した場合は内部設定 (`self.cfg`) を変更しない。ラッパー側の事前検査で
    /// 失敗した場合は libvpx の `vpx_codec_enc_config_set` 自体を呼ばない。
    pub fn reconfigure(&mut self, params: &ReconfigureParams) -> Result<(), Error> {
        const FUNCTION: &str = "shiguredo_libvpx::Encoder::reconfigure";

        // 2-pass バッファが NULL であることを保証する。`Encoder::cfg` のシャロー
        // コピー前提が崩れていないことを release ビルドでも検査する
        assert!(
            self.cfg.rc_twopass_stats_in.buf.is_null(),
            "rc_twopass_stats_in must be null in 1-pass mode"
        );
        assert!(
            self.cfg.rc_firstpass_mb_stats_in.buf.is_null(),
            "rc_firstpass_mb_stats_in must be null in 1-pass mode"
        );

        self.ensure_iter_drained(FUNCTION)?;

        // 全フィールドが None の場合は libvpx を呼ばずに早期成功させる
        if params.is_empty() {
            return Ok(());
        }

        let mut new_cfg = self.cfg;
        merge_reconfigure_params_into_cfg(&mut new_cfg, params, FUNCTION)?;

        let code = unsafe { sys::vpx_codec_enc_config_set(&mut self.ctx, &new_cfg) };
        Error::check(code, "vpx_codec_enc_config_set", Some(&self.ctx))?;
        self.cfg = new_cfg;
        Ok(())
    }

    /// これ以上データが来ないことをエンコーダーに伝える
    ///
    /// 残りのエンコード結果は [`Encoder::next_frame()`] で取得できる
    pub fn finish(&mut self) -> Result<(), Error> {
        self.ensure_iter_drained("shiguredo_libvpx::Encoder::finish")?;

        let code = unsafe {
            sys::vpx_codec_encode(
                &mut self.ctx,
                std::ptr::null(),
                -1, // pts
                0,  // duration
                0,  // flags
                sys::VPX_DL_REALTIME as sys::vpx_enc_deadline_t,
            )
        };
        Error::check(code, "vpx_codec_encode", Some(&self.ctx))?;
        Ok(())
    }

    /// エンコード済みのフレームを取り出す
    ///
    /// [`Encoder::encode()`] や [`Encoder::finish()`] の後には、
    /// このメソッドを、結果が `None` になるまで呼び出し続ける必要がある
    pub fn next_frame(&mut self) -> Option<EncodedFrame<'_>> {
        unsafe {
            loop {
                let pkt = sys::vpx_codec_get_cx_data(&mut self.ctx, &mut self.iter);
                if pkt.is_null() {
                    self.iter = std::ptr::null();
                    break;
                }

                let pkt = &*pkt;
                if pkt.kind != sys::vpx_codec_cx_pkt_kind_VPX_CODEC_CX_FRAME_PKT {
                    continue;
                }

                return Some(EncodedFrame(&pkt.data.frame));
            }
        }
        None
    }
}

#[cfg(test)]
impl Encoder {
    /// テスト専用: 現在の `cfg.rc_target_bitrate` (kbps) を返す
    fn cfg_target_bitrate_kbps(&self) -> c_uint {
        self.cfg.rc_target_bitrate
    }

    /// テスト専用: 現在の `cfg.rc_min_quantizer` を返す
    fn cfg_min_quantizer(&self) -> c_uint {
        self.cfg.rc_min_quantizer
    }

    /// テスト専用: 現在の `cfg.rc_max_quantizer` を返す
    fn cfg_max_quantizer(&self) -> c_uint {
        self.cfg.rc_max_quantizer
    }

    /// テスト専用: 現在の `cfg.kf_max_dist` を返す
    fn cfg_kf_max_dist(&self) -> c_uint {
        self.cfg.kf_max_dist
    }

    /// テスト専用: 現在の `cfg.g_timebase.(num, den)` を返す
    fn cfg_timebase(&self) -> (c_int, c_int) {
        (self.cfg.g_timebase.num, self.cfg.g_timebase.den)
    }
}

unsafe impl Send for Encoder {}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            sys::vpx_img_free(&mut self.img);
            sys::vpx_codec_destroy(&mut self.ctx);
        }
    }
}

impl std::fmt::Debug for Encoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Encoder").finish_non_exhaustive()
    }
}

/// エンコードされた映像フレーム
pub struct EncodedFrame<'a>(&'a sys::vpx_codec_cx_pkt__bindgen_ty_1__bindgen_ty_1);

impl EncodedFrame<'_> {
    /// 圧縮データ
    pub fn data(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.0.buf as *mut u8, self.0.sz) }
    }

    /// フレームの幅
    pub fn width(&self) -> u16 {
        self.0.width[0] as u16
    }

    /// フレームの高さ
    pub fn height(&self) -> u16 {
        self.0.height[0] as u16
    }

    /// キーフレームかどうか
    pub fn is_keyframe(&self) -> bool {
        (self.0.flags & sys::VPX_FRAME_IS_KEY) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_vp8_decoder() {
        let config = DecoderConfig {
            codec: DecoderCodec::Vp8,
        };
        assert!(Decoder::new(config).is_ok());
    }

    #[test]
    fn init_vp9_decoder() {
        let config = DecoderConfig {
            codec: DecoderCodec::Vp9,
        };
        assert!(Decoder::new(config).is_ok());
    }

    #[test]
    fn decode_vp8_black() {
        let data = [
            80, 66, 0, 157, 1, 42, 128, 2, 224, 1, 2, 199, 8, 133, 133, 136, 153, 132, 136, 15, 2,
            0, 6, 22, 4, 247, 6, 129, 100, 159, 107, 219, 155, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39,
            56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123,
            39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56,
            123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 56, 123, 39, 55, 128, 254,
            250, 215, 128,
        ];
        let config = DecoderConfig {
            codec: DecoderCodec::Vp8,
        };
        let mut decoder = Decoder::new(config).expect("failed to create decoder");
        let mut decoded_count = 0;

        decoder.decode(&data).expect("failed to decode");
        while decoder
            .next_frame()
            .expect("failed to get next frame")
            .is_some()
        {
            decoded_count += 1;
        }

        decoder.finish().expect("failed to finish");
        while decoder
            .next_frame()
            .expect("failed to get next frame")
            .is_some()
        {
            decoded_count += 1;
        }

        assert_eq!(decoded_count, 1);
    }

    #[test]
    fn decode_vp9_black() {
        let data = [
            130, 73, 131, 66, 0, 39, 240, 29, 246, 0, 56, 36, 28, 24, 74, 16, 0, 80, 97, 246, 58,
            246, 128, 92, 209, 238, 0, 0, 0, 0, 0, 20, 103, 26, 154, 224, 98, 35, 126, 68, 120,
            240, 227, 199, 143, 30, 28, 238, 113, 218, 24, 0, 103, 26, 154, 224, 98, 35, 126, 68,
            120, 240, 227, 199, 143, 30, 28, 238, 113, 218, 24, 0,
        ];
        let config = DecoderConfig {
            codec: DecoderCodec::Vp9,
        };
        let mut decoder = Decoder::new(config).expect("failed to create decoder");
        let mut decoded_count = 0;

        decoder.decode(&data).expect("failed to decode");
        while decoder
            .next_frame()
            .expect("failed to get next frame")
            .is_some()
        {
            decoded_count += 1;
        }

        decoder.finish().expect("failed to finish");
        while decoder
            .next_frame()
            .expect("failed to get next frame")
            .is_some()
        {
            decoded_count += 1;
        }

        assert_eq!(decoded_count, 1);
    }

    #[test]
    fn init_vp8_encoder() {
        // OK
        let config = vp8_encoder_config(ImageFormat::I420);
        assert!(Encoder::new(config).is_ok());

        // NG
        let mut config = vp8_encoder_config(ImageFormat::I420);
        config.fps_denominator = 0;
        assert!(Encoder::new(config).is_err());
    }

    #[test]
    fn init_vp9_encoder() {
        // OK
        let config = vp9_encoder_config(ImageFormat::I420);
        assert!(Encoder::new(config).is_ok());

        // NG
        let mut config = vp9_encoder_config(ImageFormat::I420);
        config.fps_denominator = 0;
        assert!(Encoder::new(config).is_err());
    }

    #[test]
    fn encode_vp8_i420_black() {
        let config = vp8_encoder_config(ImageFormat::I420);
        let size = config.width * config.height;
        let mut encoder = Encoder::new(config).expect("failed to create");
        let mut encoded_count = 0;

        let y = vec![0; size];
        let u = vec![0; size / 4];
        let v = vec![0; size / 4];

        encoder
            .encode(
                &ImageData::I420 {
                    y: &y,
                    u: &u,
                    v: &v,
                },
                &EncodeOptions {
                    force_keyframe: false,
                },
            )
            .expect("failed to encode");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        encoder.finish().expect("failed to finish");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        assert_eq!(encoded_count, 1);
    }

    #[test]
    fn encode_vp9_i420_black() {
        let config = vp9_encoder_config(ImageFormat::I420);
        let size = config.width * config.height;
        let mut encoder = Encoder::new(config).expect("failed to create");
        let mut encoded_count = 0;

        let y = vec![0; size];
        let u = vec![0; size / 4];
        let v = vec![0; size / 4];

        encoder
            .encode(
                &ImageData::I420 {
                    y: &y,
                    u: &u,
                    v: &v,
                },
                &EncodeOptions {
                    force_keyframe: false,
                },
            )
            .expect("failed to encode");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        encoder.finish().expect("failed to finish");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        assert_eq!(encoded_count, 1);
    }

    #[test]
    fn encode_vp8_nv12_black() {
        let config = vp8_encoder_config(ImageFormat::Nv12);
        let size = config.width * config.height;
        let mut encoder = Encoder::new(config).expect("failed to create");
        let mut encoded_count = 0;

        let y = vec![0; size];
        let uv = vec![0; size / 2];

        encoder
            .encode(
                &ImageData::Nv12 { y: &y, uv: &uv },
                &EncodeOptions {
                    force_keyframe: false,
                },
            )
            .expect("failed to encode");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        encoder.finish().expect("failed to finish");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        assert_eq!(encoded_count, 1);
    }

    #[test]
    fn encode_vp9_nv12_black() {
        let config = vp9_encoder_config(ImageFormat::Nv12);
        let size = config.width * config.height;
        let mut encoder = Encoder::new(config).expect("failed to create");
        let mut encoded_count = 0;

        let y = vec![0; size];
        let uv = vec![0; size / 2];

        encoder
            .encode(
                &ImageData::Nv12 { y: &y, uv: &uv },
                &EncodeOptions {
                    force_keyframe: false,
                },
            )
            .expect("failed to encode");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        encoder.finish().expect("failed to finish");
        while encoder.next_frame().is_some() {
            encoded_count += 1;
        }

        assert_eq!(encoded_count, 1);
    }

    #[test]
    fn encode_format_mismatch() {
        // I420 エンコーダーに NV12 データを渡す
        let config = vp9_encoder_config(ImageFormat::I420);
        let size = config.width * config.height;
        let mut encoder = Encoder::new(config).expect("failed to create");

        let y = vec![0; size];
        let uv = vec![0; size / 2];

        let result = encoder.encode(
            &ImageData::Nv12 { y: &y, uv: &uv },
            &EncodeOptions {
                force_keyframe: false,
            },
        );
        assert!(result.is_err());

        // NV12 エンコーダーに I420 データを渡す
        let config = vp9_encoder_config(ImageFormat::Nv12);
        let size = config.width * config.height;
        let mut encoder = Encoder::new(config).expect("failed to create");

        let y = vec![0; size];
        let u = vec![0; size / 4];
        let v = vec![0; size / 4];

        let result = encoder.encode(
            &ImageData::I420 {
                y: &y,
                u: &u,
                v: &v,
            },
            &EncodeOptions {
                force_keyframe: false,
            },
        );
        assert!(result.is_err());
    }

    // テスト用ヘルパ群。reconfigure 関連と Encoder::new 関連で共用する

    /// 黒一色の I420 プレーンを生成する
    fn black_i420(width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let size = width * height;
        (vec![0; size], vec![0; size / 4], vec![0; size / 4])
    }

    /// `width`/`height` で横方向に Y 勾配を、縦方向に U/V 勾配を持つ I420 を生成する
    /// (4:2:0 のクロマサブサンプリング前提のため偶数かつ 4 以上の解像度のみ)
    fn gradient_i420(width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        assert!(width >= 4 && height >= 4 && width.is_multiple_of(2) && height.is_multiple_of(2));
        let mut y = vec![0u8; width * height];
        let uv_w = width / 2;
        let uv_h = height / 2;
        let mut u = vec![128u8; uv_w * uv_h];
        let mut v = vec![128u8; uv_w * uv_h];
        for row in 0..height {
            for col in 0..width {
                y[row * width + col] = ((col * 255) / (width - 1)) as u8;
            }
        }
        for row in 0..uv_h {
            for col in 0..uv_w {
                u[row * uv_w + col] = ((row * 255) / (uv_h - 1)) as u8;
                v[row * uv_w + col] = (255 - (row * 255) / (uv_h - 1)) as u8;
            }
        }
        (y, u, v)
    }

    fn encode_i420(encoder: &mut Encoder, y: &[u8], u: &[u8], v: &[u8]) {
        encoder
            .encode(
                &ImageData::I420 { y, u, v },
                &EncodeOptions {
                    force_keyframe: false,
                },
            )
            .expect("failed to encode");
    }

    fn drain_frames(encoder: &mut Encoder) -> usize {
        let mut total = 0usize;
        while let Some(frame) = encoder.next_frame() {
            total += frame.data().len();
        }
        total
    }

    /// `n` 枚エンコードして全フレームをドレインし、出力バイト数の合計を返す
    fn encode_n_frames_i420(
        encoder: &mut Encoder,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        n: usize,
    ) -> usize {
        let mut total = 0usize;
        for _ in 0..n {
            encode_i420(encoder, y, u, v);
            total += drain_frames(encoder);
        }
        total
    }

    /// テスト用に決定論的な VP9 EncoderConfig を返す
    fn deterministic_vp9_config(
        width: usize,
        height: usize,
        target_bitrate: usize,
    ) -> EncoderConfig {
        let mut config = EncoderConfig::new(
            width,
            height,
            ImageFormat::I420,
            CodecConfig::Vp9(Vp9Config::default()),
        );
        config.target_bitrate = target_bitrate;
        config.threads = NonZeroUsize::new(1);
        config.deadline = EncodingDeadline::Realtime;
        config.error_resilient = true;
        config
    }

    fn run_reconfigure_changes_basic_fields(config: EncoderConfig) {
        let (y, u, v) = black_i420(config.width, config.height);
        let mut encoder = Encoder::new(config).expect("failed to create");

        encode_i420(&mut encoder, &y, &u, &v);
        drain_frames(&mut encoder);

        encoder
            .reconfigure(&ReconfigureParams {
                target_bitrate: Some(500_000),
                fps_numerator: Some(60),
                fps_denominator: Some(1),
                keyframe_interval: NonZeroUsize::new(60),
                ..ReconfigureParams::default()
            })
            .expect("failed to reconfigure");

        // cfg getter で reconfigure が反映されたことを直接観測する
        assert_eq!(encoder.cfg_target_bitrate_kbps(), 500);
        assert_eq!(encoder.cfg_timebase(), (1, 60));
        assert_eq!(encoder.cfg_kf_max_dist(), 60);

        encode_i420(&mut encoder, &y, &u, &v);
        drain_frames(&mut encoder);

        encoder.finish().expect("failed to finish");
        drain_frames(&mut encoder);
    }

    #[test]
    fn reconfigure_vp9_changes_bitrate_fps_keyframe_interval() {
        run_reconfigure_changes_basic_fields(vp9_encoder_config(ImageFormat::I420));
    }

    #[test]
    fn reconfigure_vp8_changes_bitrate_fps_keyframe_interval() {
        run_reconfigure_changes_basic_fields(vp8_encoder_config(ImageFormat::I420));
    }

    /// `min_quantizer` 単独設定時、現在の `max_quantizer` と比較されることを検証する
    #[test]
    fn reconfigure_partial_min_quantizer_uses_current_max() {
        // 現在の状態を max_quantizer = 10 にする
        let mut config = vp9_encoder_config(ImageFormat::I420);
        config.min_quantizer = 0;
        config.max_quantizer = 10;
        let mut encoder = Encoder::new(config).expect("failed to create");
        assert_eq!(encoder.cfg_max_quantizer(), 10);

        // min_quantizer = 20 だけ渡すと、現在の max = 10 と比較されて拒否される
        let err = encoder
            .reconfigure(&ReconfigureParams {
                min_quantizer: Some(20),
                ..ReconfigureParams::default()
            })
            .expect_err("partial reconfigure must be rejected when min > current max");
        assert_eq!(
            err.reason(),
            Some("min_quantizer must not exceed max_quantizer")
        );

        // ラッパー側事前検査で弾かれており、内部状態は最初から変更されていない
        assert_eq!(encoder.cfg_min_quantizer(), 0);
        assert_eq!(encoder.cfg_max_quantizer(), 10);

        // 両方同時に渡せば成功する
        encoder
            .reconfigure(&ReconfigureParams {
                min_quantizer: Some(20),
                max_quantizer: Some(40),
                ..ReconfigureParams::default()
            })
            .expect("failed to reconfigure");
        assert_eq!(encoder.cfg_min_quantizer(), 20);
        assert_eq!(encoder.cfg_max_quantizer(), 40);
    }

    /// 量子化レンジを 0 に固定したエンコードと 63 に固定したエンコードで出力サイズを比較する。
    /// `reconfigure` が no-op に退化していたら両者は同等になるので比率で検出する。
    /// 決定論性のため `threads = 1` / `deadline = Realtime` / `error_resilient = true` で固定。
    #[test]
    fn reconfigure_low_quantizer_yields_far_more_bytes_than_high_quantizer() {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 128;
        const FRAMES: usize = 20;

        fn total_bytes_with_quantizer(quantizer: usize) -> usize {
            // 量子化レンジを 0 固定する側で CBR cap に頭打ちされないように上限を十分大きく取る
            let mut config = deterministic_vp9_config(WIDTH, HEIGHT, 50_000_000);
            config.min_quantizer = 0;
            config.max_quantizer = 63;
            let mut encoder = Encoder::new(config).expect("failed to create");

            encoder
                .reconfigure(&ReconfigureParams {
                    min_quantizer: Some(quantizer),
                    max_quantizer: Some(quantizer),
                    ..ReconfigureParams::default()
                })
                .expect("failed to reconfigure");

            let (y, u, v) = gradient_i420(WIDTH, HEIGHT);
            let mut total = encode_n_frames_i420(&mut encoder, &y, &u, &v, FRAMES);
            encoder.finish().expect("failed to finish");
            total += drain_frames(&mut encoder);
            total
        }

        let q0_bytes = total_bytes_with_quantizer(0);
        let q63_bytes = total_bytes_with_quantizer(63);
        // VP9 Realtime / error_resilient での 128x128 / 20 frames では実測比率が
        // 2 倍前後なので、単調性 (`>`) より強い 2 倍以上を要求する。
        // reconfigure が no-op に退化していたら同等になるので 2 倍で十分検出できる
        assert!(
            q0_bytes >= q63_bytes.saturating_mul(2),
            "expected q=0 ({q0_bytes}) to be >= 2x q=63 ({q63_bytes})",
        );
    }

    /// `iter` が non-null の状態で `reconfigure` が「順序違反」として弾かれることを検証する
    /// (VP8 は 1 フレーム encode 直後に `next_frame()` がパケットを返すため確実に iter を進められる)
    #[test]
    fn reconfigure_while_iter_active_is_rejected() {
        let config = vp8_encoder_config(ImageFormat::I420);
        let (y, u, v) = black_i420(config.width, config.height);
        let mut encoder = Encoder::new(config).expect("failed to create");

        encode_i420(&mut encoder, &y, &u, &v);
        assert!(
            encoder.next_frame().is_some(),
            "VP8 should produce a frame after encode"
        );

        let err = encoder
            .reconfigure(&ReconfigureParams {
                target_bitrate: Some(1_000_000),
                ..ReconfigureParams::default()
            })
            .expect_err("reconfigure must fail when iter is active");
        assert_eq!(
            err.reason(),
            Some("still need to call shiguredo_libvpx::Encoder::next_frame()")
        );

        drain_frames(&mut encoder);
    }

    #[test]
    fn reconfigure_rejects_invalid_params() {
        let cases: Vec<(&'static str, ReconfigureParams, &'static str)> = vec![
            (
                "target_bitrate=0",
                ReconfigureParams {
                    target_bitrate: Some(0),
                    ..ReconfigureParams::default()
                },
                "target_bitrate must be at least 1000 bps (libvpx kbps resolution)",
            ),
            (
                "target_bitrate=999",
                ReconfigureParams {
                    target_bitrate: Some(999),
                    ..ReconfigureParams::default()
                },
                "target_bitrate must be at least 1000 bps (libvpx kbps resolution)",
            ),
            (
                "target_bitrate=1_000_000_001 (bps→kbps silent truncation 境界)",
                ReconfigureParams {
                    target_bitrate: Some(1_000_000_001),
                    ..ReconfigureParams::default()
                },
                "target_bitrate exceeds libvpx maximum (1_000_000_000 bps)",
            ),
            (
                "target_bitrate=2_000_000_000",
                ReconfigureParams {
                    target_bitrate: Some(2_000_000_000),
                    ..ReconfigureParams::default()
                },
                "target_bitrate exceeds libvpx maximum (1_000_000_000 bps)",
            ),
            (
                "target_bitrate=usize::MAX",
                ReconfigureParams {
                    target_bitrate: Some(usize::MAX),
                    ..ReconfigureParams::default()
                },
                "target_bitrate exceeds libvpx maximum (1_000_000_000 bps)",
            ),
            (
                "fps_numerator のみ",
                ReconfigureParams {
                    fps_numerator: Some(60),
                    ..ReconfigureParams::default()
                },
                "fps_numerator and fps_denominator must be set together",
            ),
            (
                "fps_denominator のみ",
                ReconfigureParams {
                    fps_denominator: Some(1),
                    ..ReconfigureParams::default()
                },
                "fps_numerator and fps_denominator must be set together",
            ),
            (
                "fps_numerator=0",
                ReconfigureParams {
                    fps_numerator: Some(0),
                    fps_denominator: Some(1),
                    ..ReconfigureParams::default()
                },
                "fps_numerator must be non-zero",
            ),
            (
                "fps_denominator=0",
                ReconfigureParams {
                    fps_numerator: Some(30),
                    fps_denominator: Some(0),
                    ..ReconfigureParams::default()
                },
                "fps_denominator must be non-zero",
            ),
            (
                "fps_numerator=1_000_000_001",
                ReconfigureParams {
                    fps_numerator: Some(1_000_000_001),
                    fps_denominator: Some(1),
                    ..ReconfigureParams::default()
                },
                "fps_numerator exceeds libvpx maximum (1_000_000_000)",
            ),
            (
                "fps_denominator=1_000_000_001",
                ReconfigureParams {
                    fps_numerator: Some(30),
                    fps_denominator: Some(1_000_000_001),
                    ..ReconfigureParams::default()
                },
                "fps_denominator exceeds libvpx maximum (1_000_000_000)",
            ),
            (
                "fps_numerator=usize::MAX",
                ReconfigureParams {
                    fps_numerator: Some(usize::MAX),
                    fps_denominator: Some(1),
                    ..ReconfigureParams::default()
                },
                "fps_numerator exceeds libvpx maximum (1_000_000_000)",
            ),
            (
                "min_quantizer=usize::MAX",
                ReconfigureParams {
                    min_quantizer: Some(usize::MAX),
                    ..ReconfigureParams::default()
                },
                "min_quantizer is out of range",
            ),
            (
                "min > max",
                ReconfigureParams {
                    min_quantizer: Some(50),
                    max_quantizer: Some(10),
                    ..ReconfigureParams::default()
                },
                "min_quantizer must not exceed max_quantizer",
            ),
            (
                "max_quantizer=64",
                ReconfigureParams {
                    max_quantizer: Some(64),
                    ..ReconfigureParams::default()
                },
                "max_quantizer must not exceed 63",
            ),
        ];

        for (case_id, params, expected_reason) in cases {
            let mut encoder =
                Encoder::new(vp9_encoder_config(ImageFormat::I420)).expect("failed to create");
            let err = encoder
                .reconfigure(&params)
                .err()
                .unwrap_or_else(|| panic!("case [{case_id}]: reconfigure must fail"));
            assert_eq!(
                err.reason(),
                Some(expected_reason),
                "case [{case_id}]: expected reason mismatch",
            );
        }
    }

    /// 上限・下限の境界値が **成功** する側で通ることを検証する
    #[test]
    fn reconfigure_accepts_boundary_values() {
        // target_bitrate = 1000 (kbps=1, 最小成功)
        {
            let mut encoder =
                Encoder::new(vp9_encoder_config(ImageFormat::I420)).expect("failed to create");
            encoder
                .reconfigure(&ReconfigureParams {
                    target_bitrate: Some(1000),
                    ..ReconfigureParams::default()
                })
                .expect("target_bitrate=1000 must succeed");
            assert_eq!(encoder.cfg_target_bitrate_kbps(), 1);
        }

        // target_bitrate = 1_000_000_000 (= VPX_MAX_TARGET_BITRATE_BPS, 最大成功)
        {
            let mut encoder =
                Encoder::new(vp9_encoder_config(ImageFormat::I420)).expect("failed to create");
            encoder
                .reconfigure(&ReconfigureParams {
                    target_bitrate: Some(1_000_000_000),
                    ..ReconfigureParams::default()
                })
                .expect("target_bitrate=1_000_000_000 must succeed");
            assert_eq!(encoder.cfg_target_bitrate_kbps(), 1_000_000);
        }

        // fps = 1_000_000_000 / 1 (上限境界、成功側)
        {
            let mut encoder =
                Encoder::new(vp9_encoder_config(ImageFormat::I420)).expect("failed to create");
            encoder
                .reconfigure(&ReconfigureParams {
                    fps_numerator: Some(1_000_000_000),
                    fps_denominator: Some(1),
                    ..ReconfigureParams::default()
                })
                .expect("fps_numerator=1_000_000_000 must succeed");
            assert_eq!(encoder.cfg_timebase(), (1, 1_000_000_000));
        }

        // min == max (同値) と max=63, min=0 の境界
        {
            let mut config = vp9_encoder_config(ImageFormat::I420);
            config.min_quantizer = 0;
            config.max_quantizer = 63;
            let mut encoder = Encoder::new(config).expect("failed to create");
            encoder
                .reconfigure(&ReconfigureParams {
                    min_quantizer: Some(0),
                    max_quantizer: Some(63),
                    ..ReconfigureParams::default()
                })
                .expect("min=0,max=63 must succeed");
            assert_eq!(encoder.cfg_min_quantizer(), 0);
            assert_eq!(encoder.cfg_max_quantizer(), 63);

            encoder
                .reconfigure(&ReconfigureParams {
                    min_quantizer: Some(30),
                    max_quantizer: Some(30),
                    ..ReconfigureParams::default()
                })
                .expect("min==max must succeed");
            assert_eq!(encoder.cfg_min_quantizer(), 30);
            assert_eq!(encoder.cfg_max_quantizer(), 30);
        }
    }

    /// `reconfigure` 失敗時の `self.cfg` がロールバックされていることを直接観測する
    #[test]
    fn reconfigure_failure_rolls_back_internal_state() {
        let mut config = vp9_encoder_config(ImageFormat::I420);
        config.target_bitrate = 5_000_000;
        config.min_quantizer = 0;
        config.max_quantizer = 63;
        let mut encoder = Encoder::new(config).expect("failed to create");

        let before_bitrate = encoder.cfg_target_bitrate_kbps();
        let before_min = encoder.cfg_min_quantizer();
        let before_max = encoder.cfg_max_quantizer();

        // 失敗ケース: 成功する `target_bitrate` と矛盾する `min > max` を一緒に渡す。
        // ロールバックが効いていなければ `target_bitrate` だけ書き換わる
        let err = encoder
            .reconfigure(&ReconfigureParams {
                target_bitrate: Some(1_000_000),
                min_quantizer: Some(50),
                max_quantizer: Some(10),
                ..ReconfigureParams::default()
            })
            .expect_err("must fail by min > max");
        assert_eq!(
            err.reason(),
            Some("min_quantizer must not exceed max_quantizer")
        );

        // self.cfg のすべてのフィールドが失敗前のまま
        assert_eq!(encoder.cfg_target_bitrate_kbps(), before_bitrate);
        assert_eq!(encoder.cfg_min_quantizer(), before_min);
        assert_eq!(encoder.cfg_max_quantizer(), before_max);
    }

    /// 全フィールドが `None` の場合、`vpx_codec_enc_config_set` を呼ばずに早期成功する。
    /// その後の encode が問題なく走り、内部状態が変わっていないことを検証する。
    #[test]
    fn reconfigure_all_none_is_noop_and_does_not_invoke_libvpx() {
        let config = vp9_encoder_config(ImageFormat::I420);
        let (y, u, v) = black_i420(config.width, config.height);
        let mut encoder = Encoder::new(config).expect("failed to create");

        let before_bitrate = encoder.cfg_target_bitrate_kbps();
        let before_timebase = encoder.cfg_timebase();
        let before_min = encoder.cfg_min_quantizer();
        let before_max = encoder.cfg_max_quantizer();
        let before_kf = encoder.cfg_kf_max_dist();

        encoder
            .reconfigure(&ReconfigureParams::default())
            .expect("all-None reconfigure must succeed");

        assert_eq!(encoder.cfg_target_bitrate_kbps(), before_bitrate);
        assert_eq!(encoder.cfg_timebase(), before_timebase);
        assert_eq!(encoder.cfg_min_quantizer(), before_min);
        assert_eq!(encoder.cfg_max_quantizer(), before_max);
        assert_eq!(encoder.cfg_kf_max_dist(), before_kf);

        encode_i420(&mut encoder, &y, &u, &v);
        drain_frames(&mut encoder);
    }

    /// `Encoder::new` の入力検査厳格化を網羅する
    #[test]
    fn encoder_new_rejects_invalid_params() {
        type Mutate = fn(&mut EncoderConfig);
        let cases: Vec<(&'static str, Mutate, &'static str)> = vec![
            (
                "target_bitrate=999",
                |c| c.target_bitrate = 999,
                "target_bitrate must be at least 1000 bps (libvpx kbps resolution)",
            ),
            (
                "target_bitrate=1_000_000_001",
                |c| c.target_bitrate = 1_000_000_001,
                "target_bitrate exceeds libvpx maximum (1_000_000_000 bps)",
            ),
            (
                "width=usize::MAX",
                |c| c.width = usize::MAX,
                "width is out of range",
            ),
            (
                "height=usize::MAX",
                |c| c.height = usize::MAX,
                "height is out of range",
            ),
            (
                "fps_denominator=0",
                |c| c.fps_denominator = 0,
                "fps_denominator must be non-zero",
            ),
            (
                "fps_numerator=0",
                |c| c.fps_numerator = 0,
                "fps_numerator must be non-zero",
            ),
            (
                "min > max",
                |c| {
                    c.min_quantizer = 50;
                    c.max_quantizer = 10;
                },
                "min_quantizer must not exceed max_quantizer",
            ),
            (
                "max_quantizer=64",
                |c| c.max_quantizer = 64,
                "max_quantizer must not exceed 63",
            ),
            (
                "cq_level=64",
                |c| c.cq_level = 64,
                "cq_level must not exceed 63",
            ),
        ];

        for (case_id, mutate, expected_reason) in cases {
            let mut config = vp9_encoder_config(ImageFormat::I420);
            mutate(&mut config);
            let err = Encoder::new(config)
                .err()
                .unwrap_or_else(|| panic!("case [{case_id}]: Encoder::new must fail"));
            assert_eq!(
                err.reason(),
                Some(expected_reason),
                "case [{case_id}]: expected reason mismatch",
            );
        }
    }

    fn vp8_encoder_config(image_format: ImageFormat) -> EncoderConfig {
        let mut config = EncoderConfig::new(
            128,
            128,
            image_format,
            CodecConfig::Vp8(Vp8Config::default()),
        );
        config.target_bitrate = 1_000_000;
        config.min_quantizer = 1;
        config.max_quantizer = 1;
        config.cq_level = 1;
        config
    }

    fn vp9_encoder_config(image_format: ImageFormat) -> EncoderConfig {
        let mut config = EncoderConfig::new(
            128,
            128,
            image_format,
            CodecConfig::Vp9(Vp9Config::default()),
        );
        config.target_bitrate = 1_000_000;
        config.min_quantizer = 1;
        config.max_quantizer = 1;
        config.cq_level = 1;
        config
    }

    #[test]
    fn error_reason_falls_back_to_libvpx_string() {
        let e = Error::check(sys::vpx_codec_err_t_VPX_CODEC_MEM_ERROR, "test", None)
            .expect_err("not an error");
        // `Error::reason()` は明示 `reason` がない場合 libvpx の vpx_codec_err_to_string()
        // を経由して英文メッセージを返す
        let reason = e.reason().expect("reason must be available");
        assert!(
            !reason.is_empty(),
            "libvpx must return a non-empty error string for MEM_ERROR"
        );
    }
}
