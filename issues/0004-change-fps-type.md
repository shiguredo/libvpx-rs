# FPS を型レベルで「両方同時指定」に制約する

Created: 2026-05-11

Model: Opus 4.7

## 背景

`ReconfigureParams` および `EncoderConfig` は FPS を次のように表現する。

```rust
pub fps_numerator: Option<usize>,
pub fps_denominator: Option<usize>,
```

`ReconfigureParams` は「両方 `Some` か両方 `None`」「両方 `Some` なら両方非ゼロ
かつ 1_000_000_000 以下」という制約を持ち、`apply_dynamic_cfg` で runtime
検査している。

## 課題

- 「両方同時指定」「両方非ゼロ」が型では表現できておらず、runtime 検査と doc
  コメントに二重化されている。
- 利用者は片方だけ `Some` にしても compile error にならず、`reconfigure` を呼ぶ
  まで間違いに気付けない。
- `EncoderConfig` 側も `usize` を 2 つ並べているため、`fps_denominator = 0` で
  `Encoder::new` が runtime error になる。

## 根拠

`/review-diff-code` のレビュー (Sug-11) での指摘。型と検査の二重化を解消する設計
案として記録した。

## 対応案

`ReconfigureParams` と `EncoderConfig` の FPS を次のような型に置き換える。

```rust
#[derive(Debug, Clone, Copy)]
pub struct Fps {
    pub numerator: NonZeroU32,
    pub denominator: NonZeroU32,
}
```

- `NonZeroU32` を採用すれば libvpx の `g_timebase.num/.den` の `c_int` (=i32) と
  整合しやすい。
- `Option<Fps>` で「未指定」を表現する。`ReconfigureParams::fps`、
  `EncoderConfig::fps` といったフィールドにまとめる。
- 上限 (1_000_000_000) の検査は引き続き runtime で行う必要があるが、ゼロ検査と
  「両方同時」検査は型レベルで消える。

## 影響

公開 API の破壊的変更。`EncoderConfig::new` の初期値生成にも影響する。
CHANGES.md は `[CHANGE]`。優先度は低 (現状でも動くが、利用者のミスを早期検出する
ための設計改善)。
