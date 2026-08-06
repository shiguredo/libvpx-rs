# EncoderConfig::lag_in_frames の「None で無効」というドキュメントと実挙動が矛盾する

- Created: 2026-08-06
- Completed: {YYYY-MM-DD}
- Branch: feature/update-lag-in-frames-doc
- Polished: {YYYY-MM-DD}

## 目的

`EncoderConfig::lag_in_frames` のドキュメントが実際の挙動と一致するようにする。

## 現状

`EncoderConfig::lag_in_frames` (`src/lib.rs`) の doc は「先読みフレーム数 (None で無効)」と記述するが、`Encoder::init` は `None` のとき libvpx のデフォルト cfg をそのまま使う。

- VP9 のデフォルトは `g_lag_in_frames = 25` (vp9_cx_iface.c の cfg map) のため、`None` でも 25 フレームの先読み遅延が発生する
- VP8 のデフォルトは 0 のため、コーデックによって挙動が異なる

リアルタイム用途で何も指定していないのに 25 フレーム分のレイテンシが発生するのは意図しない挙動。

## 設計方針

以下のいずれか。

1. ドキュメントを実挙動に合わせる (「None の場合、VP9 では libvpx デフォルトの 25 が有効」と明記する)
2. `None` を明示的に 0 に上書きする

## 完了条件

- `lag_in_frames` のドキュメントが実挙動と一致している
- 必要に応じて挙動を変える場合は、コーデックごとのテストが追加されている
